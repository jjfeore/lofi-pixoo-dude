use crate::{assets::Clip, config::{DeviceConfig, Transport}, discovery::{self, Recovery}, storage::{self, Catalog}};
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

#[derive(Debug)]
pub struct InvalidResponse;
impl std::fmt::Display for InvalidResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("invalid Pixoo API response") }
}
impl std::error::Error for InvalidResponse {}

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
            serde_json::from_slice(&bytes).context(InvalidResponse)?;
        let error_code = response.get("error_code").and_then(Value::as_i64).context(InvalidResponse)?;
        ensure!(
            error_code == 0,
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
    pub playback_ends_at: Instant,
}

async fn changed_or_disconnected(rx: &mut watch::Receiver<Target>, device: Option<&Device>, monitor: bool,
    interval: Duration) -> Result<bool> {
    loop {
        tokio::select! {
            change = rx.changed() => return Ok(change.is_ok()),
            _ = sleep(interval), if monitor && device.is_some() => {
                device.unwrap().command(json!({"Command":"Channel/GetIndex"})).await?;
            }
        }
    }
}

async fn preload(config: &mut DeviceConfig, catalog: &Catalog, device: &mut Option<Device>,
    recovery: &mut Recovery, status: &Arc<std::sync::Mutex<Value>>) -> Result<()> {
    let mut retry_ms = 500;
    loop {
        match storage::sync(config, catalog, false, Some(status)).await {
            Ok(_) => return Ok(()),
            Err(error) => {
                status.lock().unwrap()["storage"] = json!({"phase":"failed", "error":format!("{error:#}")});
                eprintln!("GIF preload failed: {error:#}; retrying with bounded backoff");
                if discovery::connection_failure(&error) && recovery.recover(config, status).await {
                    *device = Some(Device::new(config.clone())?);
                    retry_ms = 500;
                    continue;
                }
                sleep(Duration::from_millis(retry_ms)).await;
                retry_ms = (retry_ms * 2).min(30_000);
            }
        }
    }
}

async fn recover_connection(error: &anyhow::Error, config: &mut DeviceConfig, device: &mut Option<Device>,
    catalog: Option<&Catalog>, recovery: &mut Recovery, status: &Arc<std::sync::Mutex<Value>>) -> Result<bool> {
    if !discovery::connection_failure(error) || !recovery.recover(config, status).await { return Ok(false); }
    *device = Some(Device::new(config.clone())?);
    if let Some(catalog) = catalog { preload(config, catalog, device, recovery, status).await?; }
    Ok(true)
}

pub async fn writer(
    mut config: DeviceConfig,
    dry_run: bool,
    catalog: Option<Arc<Catalog>>,
    mut rx: watch::Receiver<Target>,
    ack: mpsc::Sender<Ack>,
    status: Arc<std::sync::Mutex<Value>>,
) -> Result<()> {
    ensure!(config.transport != Transport::StoredGif || catalog.is_some(), "stored GIF catalog missing");
    let mut recovery = Recovery::new()?;
    status.lock().unwrap()["connection"] = json!({"address":config.address,"automatic_discovery":config.discovery.enabled});
    let mut device = if dry_run {
        None
    } else {
        Some(Device::new(config.clone())?)
    };
    if let Some(catalog) = &catalog {
        if dry_run {
            status.lock().unwrap()["storage"] = json!({"phase":"dry-run", "files":catalog.files.len()});
        } else {
            preload(&mut config, catalog, &mut device, &mut recovery, &status).await?;
        }
    }
    let mut brightness_set = false;
    let mut last_key: Option<String> = None;
    let mut last_switch = Instant::now() - Duration::from_millis(config.switch_interval_ms);
    let mut retry_ms = 500;
    let mut one_shot_end: Option<Instant> = None;
    let mut wait_for_change = false;
    loop {
        if wait_for_change {
            match changed_or_disconnected(&mut rx, device.as_ref(), config.discovery.enabled, discovery::HEALTH_INTERVAL).await {
                Ok(false) => return Ok(()),
                Ok(true) => {},
                Err(error) => {
                    status.lock().unwrap()["device"] = json!({"dry_run":false,"healthy":false,"error":format!("{error:#}")});
                    eprintln!("Pixoo health check failed: {error:#}");
                    recover_connection(&error, &mut config, &mut device, catalog.as_deref(), &mut recovery, &status).await?;
                    last_key = None;
                    brightness_set = false;
                }
            }
        }
        wait_for_change = false;
        let target = rx.borrow_and_update().clone();
        let Some(clip) = &target.clip else {
            wait_for_change = true;
            continue;
        };
        if !target.single_play && last_key.as_deref() == Some(&clip.name) {
            // A cancelled one-shot can return to a loop that never stopped.
            // Confirm the new target serial without replaying that same loop;
            // the idle scheduler waits for this acknowledgement before arming.
            if clip.looping || clip.frames.len() == 1 {
                let _ = ack.send(Ack {
                    serial: target.serial,
                    playback_ends_at: Instant::now() + Duration::from_millis(clip.duration_ms()),
                }).await;
            }
            wait_for_change = true;
            continue;
        }
        if target.single_play && target.created.elapsed() > Duration::from_secs(10) {
            let _ = ack
                .send(Ack {
                    serial: target.serial,
                    playback_ends_at: Instant::now(),
                })
                .await;
            wait_for_change = true;
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
        // An interrupted request makes the currently selected clip uncertain.
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
                if let Some(catalog) = &catalog {
                    let command = json!({"Command":"Device/PlayTFGif", "FileType":0,
                        "FileName":catalog.get(clip)?.local_name});
                    // Playback may begin before the HTTP response arrives. Keep
                    // that response time out of the one-shot's playback budget.
                    let playback_started_at = Instant::now();
                    device.command(command).await?;
                    return Ok(playback_started_at);
                } else {
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
            }
            // Frame uploads become playable only after their last frame; dry
            // runs simulate an immediate selection at this same boundary.
            Ok::<Instant, anyhow::Error>(Instant::now())
        };
        let result = tokio::select! {
            result = upload => result,
            change = rx.changed() => { if change.is_err() { return Ok(()); } continue; }
        };
        match result {
            Ok(playback_started_at) => {
                recovery.connected();
                let upload_ms = last_switch.elapsed().as_millis();
                last_key = Some(clip.name.clone());
                last_switch = Instant::now();
                retry_ms = 500;
                brightness_set = true;
                let hold_ms = catalog.as_ref().map(|catalog| catalog.get(clip).map(|file| file.duration_ms))
                    .transpose()?.unwrap_or_else(|| clip.duration_ms()) + config.playback_start_delay_ms;
                let playback_ends_at = playback_started_at + Duration::from_millis(hold_ms);
                if target.single_play || (!clip.looping && clip.frames.len() > 1) {
                    one_shot_end = Some(playback_ends_at);
                }
                {
                    let mut state = status.lock().unwrap();
                    state["connection"]["address"] = json!(config.address);
                    state["connection"]["phase"] = json!("connected");
                    if let Some(connection) = state["connection"].as_object_mut() { connection.remove("discovery_error"); }
                    state["device"] = json!({"dry_run":dry_run,"last_clip":clip.name,"healthy":true,
                        "transport":config.transport.name(), "selection_ms":upload_ms});
                    if catalog.is_some() {
                        state["device"]["last_played_clip"] = json!(clip.name);
                    } else {
                        state["device"]["last_uploaded_clip"] = json!(clip.name);
                        state["device"]["upload_ms"] = json!(upload_ms);
                    }
                }
                let _ = ack
                    .send(Ack {
                        serial: target.serial,
                        playback_ends_at,
                    })
                    .await;
                eprintln!(
                    "{}: {}",
                    if dry_run { "simulated" } else if catalog.is_some() { "selected local GIF" } else { "uploaded" },
                    clip.name
                );
            }
            Err(error) => {
                status.lock().unwrap()["device"] =
                    json!({"dry_run":dry_run,"healthy":false,"error":format!("{error:#}")});
                eprintln!("device unavailable: {error:#}; retrying with bounded backoff");
                if recover_connection(&error, &mut config, &mut device, catalog.as_deref(), &mut recovery, &status).await? {
                    brightness_set = false;
                    retry_ms = 500;
                    continue;
                }
                tokio::select! {
                    _ = sleep(Duration::from_millis(retry_ms)) => {},
                    change = rx.changed() => { if change.is_err() { return Ok(()); } }
                }
                retry_ms = (retry_ms * 2).min(30_000);
                continue;
            }
        }
        wait_for_change = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{net::TcpListener, time::timeout};

    #[tokio::test]
    async fn unchanged_loop_detects_disconnect_without_an_event_or_display_write() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        drop(listener);
        let device = Device::new(DeviceConfig { address, timeout_ms: 100, ..Default::default() }).unwrap();
        let (_sender, mut receiver) = watch::channel(Target { serial: 1, clip: None, single_play: false, created: Instant::now() });
        let result = timeout(Duration::from_secs(2),
            changed_or_disconnected(&mut receiver, Some(&device), true, Duration::from_millis(5))).await.unwrap();
        assert!(discovery::connection_failure(&result.unwrap_err()));
        assert_eq!(receiver.borrow().serial, 1);
    }

    #[tokio::test]
    async fn disabled_monitor_waits_for_state_changes_and_closed_watch_exits() {
        let (sender, mut receiver) = watch::channel(Target { serial: 1, clip: None, single_play: false, created: Instant::now() });
        assert!(timeout(Duration::from_millis(20),
            changed_or_disconnected(&mut receiver, None, false, Duration::from_millis(1))).await.is_err());
        sender.send_replace(Target { serial: 2, clip: None, single_play: false, created: Instant::now() });
        assert!(changed_or_disconnected(&mut receiver, None, false, Duration::from_millis(1)).await.unwrap());
        drop(sender);
        assert!(!changed_or_disconnected(&mut receiver, None, false, Duration::from_millis(1)).await.unwrap());
    }
}
