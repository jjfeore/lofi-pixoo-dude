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
            local_token: None,
            brightness: None,
            frame_upload_interval_ms: 150,
            switch_interval_ms: 1000,
            playback_start_delay_ms: 0,
            timeout_ms: 3000,
            max_frames: 40,
        }
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
