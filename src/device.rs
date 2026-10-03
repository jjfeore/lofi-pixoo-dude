use crate::{assets::Clip, config::DeviceConfig};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::{
    sync::{mpsc, watch},
    time::{Duration, Instant, sleep},
};

pub struct Device {
    client: reqwest::Client,
    config: DeviceConfig,
    url: String,
}

impl Device {
    pub fn new(config: DeviceConfig) -> Result<Self> {
        Ok(Self {
            url: config.url()?,
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_millis(config.timeout_ms))
                .build()?,
            config,
        })
    }
    pub async fn command(&self, mut body: Value) -> Result<Value> {
        if let Some(token) = self.config.local_token {
            body["LocalToken"] = json!(token);
        }
        let mut response = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .context("Pixoo request failed")?
            .error_for_status()?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= 64 * 1024,
                "Pixoo response exceeds 64 KiB"
            );
            bytes.extend_from_slice(&chunk);
        }
        let response: Value =
            serde_json::from_slice(&bytes).context("invalid Pixoo JSON response")?;
        ensure!(
            response.get("error_code").and_then(Value::as_i64) == Some(0),
            "Pixoo rejected command (error_code={})",
            response.get("error_code").unwrap_or(&Value::Null)
        );
        Ok(response)
    }
    pub async fn probe(&self) -> Result<Value> {
        let settings = self
            .command(json!({"Command":"Channel/GetAllConf"}))
            .await?;
        let channel = self.command(json!({"Command":"Channel/GetIndex"})).await?;
        Ok(json!({
            "reachable":true,"brightness":settings.get("Brightness"),
            "channel":channel.get("SelectIndex"),
            "firmware":settings.get("FirmwareVersion").or(settings.get("Version")),
            "note":"Status responses do not provide firmware on every model."
        }))
    }
}

#[derive(Clone)]
pub struct Target {
    pub serial: u64,
    pub clip: Option<Arc<Clip>>,
    pub single_play: bool,
    pub created: Instant,
}
impl Target {
    pub fn key(&self) -> Option<&str> {
        self.clip.as_ref().map(|c| c.name.as_str())
    }
}

pub struct Ack {
    pub serial: u64,
    pub hold_ms: u64,
}

pub async fn writer(
    config: DeviceConfig,
    dry_run: bool,
    mut rx: watch::Receiver<Target>,
    ack: mpsc::Sender<Ack>,
    status: Arc<std::sync::Mutex<Value>>,
) -> Result<()> {
    let device = if dry_run {
        None
    } else {
        Some(Device::new(config.clone())?)
    };
    let mut brightness_set = false;
    let mut last_key: Option<String> = None;
    let mut last_switch = Instant::now() - Duration::from_millis(config.switch_interval_ms);
    let mut retry_ms = 500;
    let mut one_shot_end: Option<Instant> = None;
    loop {
        let target = rx.borrow_and_update().clone();
        let Some(clip) = &target.clip else {
            if rx.changed().await.is_err() {
                return Ok(());
            }
            continue;
        };
        if !target.single_play && last_key.as_deref() == Some(&clip.name) {
            if rx.changed().await.is_err() {
                return Ok(());
            }
            continue;
        }
        if target.single_play && target.created.elapsed() > Duration::from_secs(10) {
            let _ = ack
                .send(Ack {
                    serial: target.serial,
                    hold_ms: 0,
                })
                .await;
            if rx.changed().await.is_err() {
                return Ok(());
            }
            continue;
        }
        // A scheduled return after a one-shot must not be held behind ordinary
        // switch pacing, which could otherwise replay a short gesture.
        let earliest = if one_shot_end.is_some_and(|end| Instant::now() >= end) {
            Instant::now()
        } else {
            last_switch + Duration::from_millis(config.switch_interval_ms)
        };
        if earliest > Instant::now() {
            tokio::select! {
                _ = tokio::time::sleep_until(earliest) => {},
                change = rx.changed() => { if change.is_err() { return Ok(()); } continue; }
            }
        }
        // A partial/cancelled upload invalidates the previous residency assumption.
        last_key = None;
        one_shot_end = None;
        last_switch = Instant::now();
        let upload = async {
            if let Some(device) = &device {
                if !brightness_set && let Some(brightness) = config.brightness {
                    device
                        .command(json!({"Command":"Channel/SetBrightness","Brightness":brightness}))
                        .await?;
                }
                device
                    .command(json!({"Command":"Draw/ResetHttpGifId"}))
                    .await?;
                for (index, data) in clip.frames.iter().enumerate() {
                    device
                        .command(json!({
                            "Command":"Draw/SendHttpGif", "PicNum":clip.frames.len(),
                            "PicWidth":64, "PicOffset":index, "PicID":1,
                            "PicSpeed":clip.frame_ms, "PicData":data
                        }))
                        .await?;
                    if index + 1 < clip.frames.len() {
                        sleep(Duration::from_millis(config.frame_upload_interval_ms)).await;
                    }
                }
            }
            Ok::<(), anyhow::Error>(())
        };
        let result = tokio::select! {
            result = upload => result,
            change = rx.changed() => { if change.is_err() { return Ok(()); } continue; }
        };
        match result {
            Ok(()) => {
                let upload_ms = last_switch.elapsed().as_millis();
                last_key = Some(clip.name.clone());
                last_switch = Instant::now();
                retry_ms = 500;
                brightness_set = true;
                let hold_ms = clip.duration_ms() + config.playback_start_delay_ms;
                if target.single_play || (!clip.looping && clip.frames.len() > 1) {
                    one_shot_end = Some(Instant::now() + Duration::from_millis(hold_ms));
                }
                {
                    let mut state = status.lock().unwrap();
                    state["device"] = json!({"dry_run":dry_run,"last_uploaded_clip":clip.name,"healthy":true,
                            "upload_ms":upload_ms});
                }
                let _ = ack
                    .send(Ack {
                        serial: target.serial,
                        hold_ms,
                    })
                    .await;
                eprintln!(
                    "{}: {}",
                    if dry_run { "simulated" } else { "uploaded" },
                    clip.name
                );
            }
            Err(error) => {
                status.lock().unwrap()["device"] =
                    json!({"dry_run":dry_run,"healthy":false,"error":error.to_string()});
                eprintln!("device unavailable; retrying with bounded backoff");
                tokio::select! {
                    _ = sleep(Duration::from_millis(retry_ms)) => {},
                    change = rx.changed() => { if change.is_err() { return Ok(()); } }
                }
                retry_ms = (retry_ms * 2).min(30_000);
                continue;
            }
        }
        if rx.changed().await.is_err() {
            return Ok(());
        }
    }
}
