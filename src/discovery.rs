use crate::{
    config::{Config, DeviceConfig, DiscoveryConfig},
    device::Device,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    future::Future,
    io::Write,
    net::Ipv4Addr,
    sync::{Arc, Mutex},
};
use tokio::time::{Duration, Instant};

pub const DISCOVERY_URL: &str = "https://app.divoom-gz.com/Device/ReturnSameLANDevice";
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(30);
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize, Serialize)]
pub struct FoundDevice {
    #[serde(rename = "DeviceId")]
    pub device_id: u64,
    #[serde(rename = "DeviceMac")]
    pub mac: String,
    #[serde(rename = "DevicePrivateIP")]
    pub address: String,
    #[serde(rename = "Hardware")]
    pub hardware: u16,
}

#[derive(Deserialize)]
struct DiscoveryReply {
    #[serde(rename = "ReturnCode")]
    code: i64,
    #[serde(rename = "DeviceList", default)]
    devices: Vec<FoundDevice>,
}

pub async fn discover(client: &reqwest::Client, url: &str) -> Result<Vec<FoundDevice>> {
    // This service gets no local token, paths, artwork or conversation data.
    let mut response = client
        .post(url)
        .json(&json!({}))
        .send()
        .await
        .context("Divoom discovery request failed")?
        .error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 64 * 1024,
            "discovery response exceeds 64 KiB"
        );
        bytes.extend_from_slice(&chunk);
    }
    let reply: DiscoveryReply =
        serde_json::from_slice(&bytes).context("invalid discovery response")?;
    ensure!(
        reply.code == 0,
        "Divoom discovery failed (ReturnCode={})",
        reply.code
    );
    ensure!(reply.devices.len() <= 128, "too many discovered devices");
    Ok(reply.devices)
}

fn select(devices: Vec<FoundDevice>, identity: &DiscoveryConfig) -> Result<FoundDevice> {
    identity.validate()?;
    ensure!(identity.enabled, "discovery disabled");
    let expected_mac = identity
        .mac_address
        .as_deref()
        .and_then(DiscoveryConfig::normalize_mac);
    let mut matches = devices.into_iter().filter(|device| {
        identity.device_id.is_none_or(|id| id == device.device_id)
            && expected_mac
                .as_ref()
                .is_none_or(|mac| DiscoveryConfig::normalize_mac(&device.mac).as_ref() == Some(mac))
    });
    let device = matches
        .next()
        .context("the configured Pixoo was not listed by Divoom discovery")?;
    ensure!(
        matches.next().is_none(),
        "discovery returned multiple matches for this Pixoo"
    );
    ensure!(
        matches!(device.hardware, 90 | 92),
        "discovered device does not support this bridge's HTTP API"
    );
    let address: Ipv4Addr = device
        .address
        .parse()
        .context("discovery returned an invalid IPv4 address")?;
    ensure!(
        address.is_private(),
        "discovery returned an address outside private IPv4 networks"
    );
    Ok(device)
}

fn save_address(config: &DeviceConfig, new_address: &str) -> Result<()> {
    let path = config
        .config_path
        .as_ref()
        .context("no source config path; new address is in memory only")?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("discovery.lock"))?;
    lock.try_lock()
        .context("another process is updating device discovery settings")?;
    let original = fs::read_to_string(path).context("read config for recovered address")?;
    let latest: Config = toml::from_str(original.trim_start_matches('\u{feff}'))?;
    ensure!(
        latest.device.discovery == config.discovery,
        "discovery identity/settings changed; config not overwritten"
    );
    ensure!(
        latest.device.address == config.address || latest.device.address == new_address,
        "device address was edited; config not overwritten"
    );
    let mut document: toml_edit::DocumentMut = original.trim_start_matches('\u{feff}').parse()?;
    let decor = document["device"]["address"]
        .as_value()
        .context("device address is not a value")?
        .decor()
        .clone();
    document["device"]["address"] = toml_edit::value(new_address);
    *document["device"]["address"]
        .as_value_mut()
        .unwrap()
        .decor_mut() = decor;
    let pending = path.with_extension(format!(
        "discovery-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let write = || -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        file.write_all(document.to_string().as_bytes())?;
        file.sync_all()?;
        ensure!(
            fs::read_to_string(path)? == original,
            "config changed during recovery; config not overwritten"
        );
        fs::rename(&pending, path).context("save recovered Pixoo address")?;
        Ok(())
    };
    let result = write();
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result
}

pub struct Recovery {
    client: reqwest::Client,
    last_attempt: Option<Instant>,
    failures: u32,
}

impl Recovery {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(8))
                .build()?,
            last_attempt: None,
            failures: 0,
        })
    }

    pub async fn recover(&mut self, config: &mut DeviceConfig, status: &Arc<Mutex<Value>>) -> bool {
        self.failures = self.failures.saturating_add(1);
        if self.failures < 2 {
            return false;
        }
        let previous_attempt = self.last_attempt;
        let result = self
            .recover_from(config, status, DISCOVERY_URL, |candidate| async move {
                Device::new(candidate)?.probe().await.map(|_| ())
            })
            .await;
        if matches!(result, Ok(true)) {
            status.lock().unwrap()["connection"]["source"] = json!("divoom-service");
            return true;
        }
        if previous_attempt == self.last_attempt {
            return false;
        }
        let cloud_error = result
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_else(|| "Divoom still lists the old address".into());
        eprintln!("Divoom discovery: {cloud_error}; trying local MAC discovery");
        status.lock().unwrap()["connection"]["phase"] = json!("discovering-lan");
        let local = async {
            let address = crate::lan::find(config).await?;
            if address == config.address {
                return Ok(false);
            }
            Self::accept(config, status, address, |candidate| async move {
                Device::new(candidate)?.probe().await.map(|_| ())
            })
            .await
        }
        .await;
        match local {
            Ok(changed) => {
                if changed {
                    status.lock().unwrap()["connection"]["source"] = json!("local-mac");
                }
                changed
            }
            Err(error) => {
                status.lock().unwrap()["connection"]["phase"] = json!("retrying");
                status.lock().unwrap()["connection"]["discovery_error"] =
                    json!(format!("Divoom: {cloud_error}; LAN: {error:#}"));
                eprintln!("Pixoo local discovery: {error:#}; retaining the current address");
                false
            }
        }
    }

    pub fn connected(&mut self) {
        self.failures = 0;
    }

    pub async fn list(&self) -> Result<Vec<FoundDevice>> {
        discover(&self.client, DISCOVERY_URL).await
    }

    async fn recover_from<F, Fut>(
        &mut self,
        config: &mut DeviceConfig,
        status: &Arc<Mutex<Value>>,
        service_url: &str,
        probe: F,
    ) -> Result<bool>
    where
        F: FnOnce(DeviceConfig) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        if !config.discovery.enabled
            || self
                .last_attempt
                .is_some_and(|last| last.elapsed() < DISCOVERY_INTERVAL)
        {
            return Ok(false);
        }
        self.last_attempt = Some(Instant::now());
        status.lock().unwrap()["connection"] =
            json!({"address":config.address,"phase":"discovering"});
        let found = select(
            discover(&self.client, service_url).await?,
            &config.discovery,
        )?;
        let address = match config.url()?.parse::<reqwest::Url>()?.port() {
            Some(port) => format!("{}:{port}", found.address),
            None => found.address,
        };
        if address == config.address {
            status.lock().unwrap()["connection"]["phase"] = json!("retrying");
            return Ok(false);
        }
        Self::accept(config, status, address, probe).await
    }

    async fn accept<F, Fut>(
        config: &mut DeviceConfig,
        status: &Arc<Mutex<Value>>,
        address: String,
        probe: F,
    ) -> Result<bool>
    where
        F: FnOnce(DeviceConfig) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        let mut candidate = config.clone();
        candidate.address = address.clone();
        probe(candidate.clone())
            .await
            .context("discovered Pixoo did not answer its local status check")?;
        let saved = save_address(config, &address);
        let save_error = saved.as_ref().err().map(|error| format!("{error:#}"));
        eprintln!(
            "Pixoo recovered at {address}; previous address {}; config_saved={}",
            config.address,
            saved.is_ok()
        );
        if let Some(error) = &save_error {
            eprintln!("Recovered address is in memory only: {error}");
        }
        *config = candidate;
        status.lock().unwrap()["connection"] = json!({"address":address,"phase":"recovered",
            "config_saved":saved.is_ok(),"save_error":save_error});
        Ok(true)
    }
}

pub fn connection_failure(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<crate::device::InvalidResponse>()
        .is_some()
        || error.downcast_ref::<reqwest::Error>().is_some_and(|error| {
            error.is_connect() || error.is_timeout() || error.status().is_some()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    fn identity() -> DiscoveryConfig {
        DiscoveryConfig {
            enabled: true,
            device_id: Some(42),
            mac_address: Some("02:11:22:33:44:55".into()),
        }
    }

    fn found(id: u64, mac: &str, ip: &str) -> FoundDevice {
        FoundDevice {
            device_id: id,
            mac: mac.into(),
            address: ip.into(),
            hardware: 92,
        }
    }

    fn config() -> DeviceConfig {
        DeviceConfig {
            address: "192.168.1.30".into(),
            discovery: identity(),
            ..Default::default()
        }
    }

    fn source(address: &str) -> String {
        format!(
            "# keep this header\npack = 'pets/decker'\n[device]\naddress = '{address}' # my Pixoo\nlocal_token = 123\n[device.discovery]\nenabled = true\ndevice_id = 42\nmac_address = '02:11:22:33:44:55'\n[device.storage]\ndirectory = 'test'\n[idle_alternates]\nanimations = []\n"
        )
    }

    #[test]
    fn selects_exact_identity_and_rejects_ambiguous_or_non_lan_results() {
        let mac = "021122334455";
        let selected = select(
            vec![
                found(43, mac, "192.168.1.19"),
                found(42, mac, "192.168.1.18"),
            ],
            &identity(),
        )
        .unwrap();
        assert_eq!(selected.address, "192.168.1.18");
        for devices in [
            vec![],
            vec![found(42, "001122334455", "192.168.1.18")],
            vec![
                found(42, mac, "192.168.1.18"),
                found(42, mac, "192.168.1.19"),
            ],
            vec![found(42, mac, "https://example.com")],
            vec![found(42, mac, "8.8.8.8")],
            vec![found(42, mac, "127.0.0.1")],
            vec![found(42, mac, "0.0.0.0")],
        ] {
            assert!(select(devices, &identity()).is_err());
        }
        let mut unsupported = found(42, mac, "192.168.1.18");
        unsupported.hardware = 96;
        assert!(select(vec![unsupported], &identity()).is_err());
    }

    #[test]
    fn identity_validation_and_mac_normalization() {
        assert_eq!(
            DiscoveryConfig::normalize_mac("02-11-22-33-44-55").unwrap(),
            "021122334455"
        );
        for mac in [
            "02112233445",
            "0211223344550",
            "zzzzzzzzzzzz",
            "02/11/22/33/44/55",
        ] {
            assert!(DiscoveryConfig::normalize_mac(mac).is_none());
        }
        assert!(
            DiscoveryConfig {
                enabled: true,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            DiscoveryConfig {
                device_id: Some(0),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(DiscoveryConfig::default().validate().is_ok());
        for pins in [
            DiscoveryConfig {
                mac_address: None,
                ..identity()
            },
            DiscoveryConfig {
                device_id: None,
                ..identity()
            },
        ] {
            assert!(select(vec![found(42, "021122334455", "192.168.1.18")], &pins).is_ok());
        }
    }

    #[test]
    fn saves_only_address_preserving_comments_and_unrelated_settings() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("bridge.toml");
        fs::write(&path, source("192.168.1.30")).unwrap();
        let mut config = config();
        config.config_path = Some(path.clone());
        save_address(&config, "192.168.1.18").unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# keep this header"));
        assert!(saved.contains("# my Pixoo"));
        let actual: Value =
            serde_json::to_value(toml::from_str::<Config>(&saved).unwrap()).unwrap();
        let mut expected: Value =
            serde_json::to_value(toml::from_str::<Config>(&source("192.168.1.30")).unwrap())
                .unwrap();
        expected["device"]["address"] = json!("192.168.1.18");
        assert_eq!(actual, expected);
    }

    #[test]
    fn does_not_overwrite_new_address_or_identity_edits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("bridge.toml");
        let mut config = config();
        config.config_path = Some(path.clone());
        for text in [
            source("192.168.1.99"),
            source("192.168.1.30").replace("device_id = 42", "device_id = 43"),
        ] {
            fs::write(&path, &text).unwrap();
            assert!(save_address(&config, "192.168.1.18").is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
    }

    struct Service {
        url: String,
        requests: Arc<AtomicUsize>,
        task: JoinHandle<()>,
    }

    impl Drop for Service {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    impl Service {
        async fn start(body: String) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/discover", listener.local_addr().unwrap());
            let requests = Arc::new(AtomicUsize::new(0));
            let count = requests.clone();
            let task = tokio::spawn(async move {
                while let Ok((mut stream, _)) = listener.accept().await {
                    let mut bytes = Vec::new();
                    let mut chunk = [0; 2048];
                    let headers = loop {
                        let n = stream.read(&mut chunk).await.unwrap();
                        assert!(n > 0);
                        bytes.extend_from_slice(&chunk[..n]);
                        if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                            break end + 4;
                        }
                    };
                    while bytes.len() < headers + 2 {
                        let n = stream.read(&mut chunk).await.unwrap();
                        assert!(n > 0);
                        bytes.extend_from_slice(&chunk[..n]);
                    }
                    assert_eq!(&bytes[headers..], b"{}");
                    assert!(!String::from_utf8_lossy(&bytes).contains("LocalToken"));
                    count.fetch_add(1, Ordering::Relaxed);
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(reply.as_bytes()).await.unwrap();
                    stream.shutdown().await.unwrap();
                }
            });
            Self {
                url,
                requests,
                task,
            }
        }

        async fn matching() -> Self {
            Self::start(
                json!({"ReturnCode":0,"DeviceList":[{"DeviceId":42,"DeviceMac":"021122334455",
                "DevicePrivateIP":"192.168.1.18","Hardware":92}]})
                .to_string(),
            )
            .await
        }
    }

    #[tokio::test]
    async fn recovers_probes_then_saves_and_limits_discovery_requests() {
        let service = Service::matching().await;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("bridge.toml");
        fs::write(&path, source("192.168.1.30")).unwrap();
        let mut config = config();
        config.config_path = Some(path.clone());
        let status = Arc::new(Mutex::new(json!({})));
        let mut recovery = Recovery::new().unwrap();
        assert!(
            recovery
                .recover_from(&mut config, &status, &service.url, |candidate| async move {
                    assert_eq!(candidate.address, "192.168.1.18");
                    assert_eq!(candidate.local_token, None);
                    Ok(())
                })
                .await
                .unwrap()
        );
        assert_eq!(config.address, "192.168.1.18");
        assert_eq!(
            toml::from_str::<Config>(&fs::read_to_string(path).unwrap())
                .unwrap()
                .device
                .address,
            config.address
        );
        assert_eq!(status.lock().unwrap()["connection"]["config_saved"], true);
        assert!(
            !recovery
                .recover_from(&mut config, &status, &service.url, |_| async {
                    panic!("cooldown skipped")
                })
                .await
                .unwrap()
        );
        assert_eq!(service.requests.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn failed_probe_does_not_change_address_or_config() {
        let service = Service::matching().await;
        let mut config = config();
        let status = Arc::new(Mutex::new(json!({})));
        let result = Recovery::new()
            .unwrap()
            .recover_from(&mut config, &status, &service.url, |_| async {
                anyhow::bail!("offline")
            })
            .await;
        assert!(result.is_err());
        assert_eq!(config.address, "192.168.1.30");
    }

    #[tokio::test]
    async fn disabled_or_empty_discovery_keeps_current_address() {
        let service = Service::start(json!({"ReturnCode":0,"DeviceList":[]}).to_string()).await;
        let status = Arc::new(Mutex::new(json!({})));
        let mut config = config();
        config.discovery.enabled = false;
        let mut recovery = Recovery::new().unwrap();
        assert!(
            !recovery
                .recover_from(&mut config, &status, &service.url, |_| async {
                    panic!("disabled")
                })
                .await
                .unwrap()
        );
        assert_eq!(service.requests.load(Ordering::Relaxed), 0);
        config.discovery.enabled = true;
        assert!(
            recovery
                .recover_from(&mut config, &status, &service.url, |_| async {
                    panic!("empty")
                })
                .await
                .is_err()
        );
        assert_eq!(config.address, "192.168.1.30");
    }

    #[tokio::test]
    async fn reconnects_in_memory_when_config_cannot_be_saved() {
        let service = Service::matching().await;
        let status = Arc::new(Mutex::new(json!({})));
        let mut config = config();
        config.address.push_str(":8080");
        let mut recovery = Recovery::new().unwrap();
        assert!(
            recovery
                .recover_from(&mut config, &status, &service.url, |candidate| async move {
                    assert_eq!(candidate.address, "192.168.1.18:8080");
                    Ok(())
                })
                .await
                .unwrap()
        );
        assert_eq!(config.address, "192.168.1.18:8080");
        assert_eq!(status.lock().unwrap()["connection"]["config_saved"], false);
        assert!(status.lock().unwrap()["connection"]["save_error"].is_string());
    }

    #[tokio::test]
    async fn malformed_error_and_oversized_service_responses_are_rejected() {
        for body in [
            "not-json".to_string(),
            "{\"ReturnCode\":1}".to_string(),
            " ".repeat(65_537),
        ] {
            let service = Service::start(body).await;
            let recovery = Recovery::new().unwrap();
            assert!(discover(&recovery.client, &service.url).await.is_err());
        }
        assert!(!connection_failure(&anyhow::anyhow!(
            "Pixoo rejected command (error_code=2)"
        )));
        assert!(connection_failure(&anyhow::Error::new(
            crate::device::InvalidResponse
        )));
    }
}
