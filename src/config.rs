use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const DEFAULT_PIPE: &str = r"\\.\pipe\pixoo-pet";

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub pack: PathBuf,
    pub pipe: String,
    pub dry_run: bool,
    pub device: DeviceConfig,
    pub animations: BTreeMap<String, String>,
    pub notification_forward: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            pack: PathBuf::from("pets/default"),
            pipe: DEFAULT_PIPE.into(),
            dry_run: false,
            device: DeviceConfig::default(),
            animations: BTreeMap::new(),
            notification_forward: Vec::new(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceConfig {
    pub address: String,
    pub transport: Transport,
    pub storage: StorageConfig,
    pub local_token: Option<u32>,
    pub brightness: Option<u8>,
    pub frame_upload_interval_ms: u64,
    pub switch_interval_ms: u64,
    pub playback_start_delay_ms: u64,
    pub timeout_ms: u64,
    pub max_frames: usize,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            address: String::new(),
            transport: Transport::default(),
            storage: StorageConfig::default(),
            local_token: None,
            brightness: None,
            frame_upload_interval_ms: 150,
            switch_interval_ms: 0,
            playback_start_delay_ms: 0,
            timeout_ms: 3000,
            max_frames: 40,
        }
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    Frames,
    #[default]
    StoredGif,
}

impl Transport {
    pub fn name(self) -> &'static str {
        match self {
            Self::Frames => "frames",
            Self::StoredGif => "stored-gif",
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageConfig {
    pub cache_dir: PathBuf,
    pub directory: String,
    pub bind: String,
    pub public_base_url: Option<String>,
    pub download_timeout_ms: u64,
    pub settle_ms: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from("gif-cache"),
            directory: "codex-pet".into(),
            bind: "0.0.0.0:8787".into(),
            public_base_url: None,
            download_timeout_ms: 15_000,
            settle_ms: 500,
        }
    }
}

impl StorageConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.directory.is_empty() && self.directory.len() <= 64
                && self.directory.split('/').all(|part| !part.is_empty()
                    && part.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))),
            "storage.directory must be a relative path of letters, numbers, hyphens or underscores"
        );
        self.bind.parse::<std::net::SocketAddr>().context("storage.bind must be IP:port")?;
        ensure!((100..=60_000).contains(&self.download_timeout_ms),
            "storage.download_timeout_ms must be 100..60000");
        ensure!(self.settle_ms <= 10_000, "storage.settle_ms must be 0..10000");
        if let Some(base) = &self.public_base_url {
            let url = reqwest::Url::parse(base).context("invalid storage.public_base_url")?;
            ensure!(url.scheme() == "http" && url.host_str().is_some()
                && url.username().is_empty() && url.password().is_none()
                && url.path() == "/" && url.query().is_none() && url.fragment().is_none(),
                "storage.public_base_url must be an HTTP origin without credentials, path, query or fragment");
        }
        Ok(())
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        let mut config: Self =
            toml::from_str(text.trim_start_matches('\u{feff}')).context("parse bridge TOML")?;
        ensure!(config.version == 1, "unsupported bridge config version");
        ensure!(
            config.pipe.starts_with(r"\\.\pipe\") && config.pipe.len() <= 200,
            "pipe must be a Windows named-pipe path"
        );
        ensure!(
            config.device.brightness.is_none_or(|b| b <= 100),
            "brightness must be 0..100"
        );
        ensure!(
            (1..=40).contains(&config.device.max_frames),
            "max_frames must be 1..40"
        );
        ensure!(
            (100..=30_000).contains(&config.device.timeout_ms),
            "timeout_ms must be 100..30000"
        );
        ensure!(
            config.device.frame_upload_interval_ms <= 10_000
                && config.device.switch_interval_ms <= 60_000
                && config.device.playback_start_delay_ms <= 60_000,
            "device delays exceed supported bounds"
        );
        if config.pack.is_relative() {
            config.pack = path.parent().unwrap_or(Path::new(".")).join(&config.pack);
        }
        config.device.storage.validate()?;
        if config.device.storage.cache_dir.is_relative() {
            config.device.storage.cache_dir = path.parent().unwrap_or(Path::new("."))
                .join(&config.device.storage.cache_dir);
        }
        Ok(config)
    }

    pub fn mapping<'a>(&'a self, state: &'a str) -> Option<&'a str> {
        match self.animations.get(state) {
            Some(name) if name.is_empty() => None,
            Some(name) => Some(name),
            None => Some(state),
        }
    }
}

impl DeviceConfig {
    pub fn url(&self) -> Result<String> {
        let address = self.address.trim();
        if address.is_empty() {
            bail!("set device.address to the Pixoo's IP or hostname");
        }
        ensure!(
            !address.contains(['/', '\\', '@', '?', '#']),
            "address must be a host[:port], not a URL"
        );
        let url = reqwest::Url::parse(&format!("http://{address}/post"))
            .context("invalid device address")?;
        ensure!(url.host_str().is_some(), "invalid device host");
        Ok(url.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_transport_uses_stored_gifs_and_frames_remain_explicit() {
        for text in ["", "[device]\naddress = '192.0.2.1'\n"] {
            let config: Config = toml::from_str(text).unwrap();
            assert!(config.device.transport == Transport::StoredGif);
            assert_eq!(config.device.switch_interval_ms, 0);
        }

        let config: Config =
            toml::from_str("[device]\ntransport = 'frames'\nswitch_interval_ms = 1000\n")
                .unwrap();
        assert!(config.device.transport == Transport::Frames);
        assert_eq!(config.device.switch_interval_ms, 1000);
    }
}
