use crate::assets::{Clip, Pack};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
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
    pub idle_alternates: IdleAlternatesConfig,
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
            idle_alternates: IdleAlternatesConfig::default(),
            notification_forward: Vec::new(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct IdleAlternatesConfig {
    pub animations: Vec<String>,
    pub min_interval_ms: u64,
    pub max_interval_ms: u64,
    pub avoid_immediate_repeat: bool,
}

impl Default for IdleAlternatesConfig {
    fn default() -> Self {
        Self {
            animations: Vec::new(),
            min_interval_ms: 45_000,
            max_interval_ms: 60_000,
            avoid_immediate_repeat: true,
        }
    }
}

impl IdleAlternatesConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.min_interval_ms >= 100 && self.max_interval_ms <= 86_400_000
            && self.min_interval_ms <= self.max_interval_ms,
            "idle_alternates intervals must satisfy 100 <= min_interval_ms <= max_interval_ms <= 86400000");
        let mut names = BTreeSet::new();
        for name in &self.animations {
            ensure!(!name.trim().is_empty(), "idle_alternates animation names must not be empty");
            ensure!(names.insert(name), "duplicate idle_alternates animation: {name}");
        }
        Ok(())
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
        config.idle_alternates.validate()?;
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

    pub fn idle_alternate_clips(&self, pack: &Pack) -> Result<Vec<Arc<Clip>>> {
        self.idle_alternates.validate()?;
        if self.idle_alternates.animations.is_empty() { return Ok(Vec::new()); }
        let idle = self.mapping("idle").and_then(|name| pack.resolve(name, false))
            .context("idle_alternates requires an enabled idle animation in the pack")?;
        ensure!(idle.looping || idle.frames.len() == 1,
            "idle_alternates requires a looping or single-frame normal idle animation");
        let mut names = BTreeSet::new();
        self.idle_alternates.animations.iter().map(|name| {
            let clip = pack.resolve(name, false)
                .with_context(|| format!("idle_alternates references missing animation: {name}"))?;
            ensure!(clip.name != idle.name, "idle_alternates must differ from normal idle: {name}");
            ensure!(names.insert(clip.name.clone()), "idle_alternates entries resolve to the same animation: {name}");
            Ok(clip)
        }).collect()
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
    fn idle_alternate_defaults_and_invalid_settings() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.idle_alternates.animations.is_empty());
        assert_eq!(config.idle_alternates.min_interval_ms, 45_000);
        assert_eq!(config.idle_alternates.max_interval_ms, 60_000);
        for text in [
            "min_interval_ms = 0", "min_interval_ms = 60001", "max_interval_ms = 86400001",
            "animations = ['']", "animations = ['one', 'one']",
        ] {
            let config: Config = toml::from_str(&format!("[idle_alternates]\n{text}\n")).unwrap();
            assert!(config.idle_alternates.validate().is_err(), "{text}");
        }
        assert!(toml::from_str::<Config>("[idle_alternates]\ninterval_ms = 45000\n").is_err());
        let config: Config = toml::from_str("[idle_alternates]\nanimations = ['one']\nmin_interval_ms = 1000\nmax_interval_ms = 1000\navoid_immediate_repeat = false\n").unwrap();
        assert!(config.idle_alternates.validate().is_ok());
    }

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
