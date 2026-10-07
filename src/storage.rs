use crate::{
    assets::{Clip, Pack},
    config::{Config, DeviceConfig},
    device::Device,
};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    net::{IpAddr, UdpSocket},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, lookup_host},
    sync::mpsc,
    task::{JoinHandle, JoinSet},
    time::{Duration, sleep, timeout},
};

pub struct StoredGif {
    pub filename: String,
    pub local_name: String,
    pub bytes: Arc<Vec<u8>>,
    pub duration_ms: u64,
}

pub struct Catalog {
    pub files: BTreeMap<String, StoredGif>,
    cache_dir: PathBuf,
}

/// Encode native 64x64 GIFs using one palette for the entire clip. GIF delays
/// have 10 ms precision; rounding cumulative times preserves the mean cadence.
fn encode_gif(frames: &[String], frame_ms: u32, looping: bool) -> Result<(Vec<u8>, u64)> {
    ensure!(!frames.is_empty() && frames.len() <= 40, "invalid GIF frame count");
    let mut rgb = Vec::with_capacity(frames.len() * 64 * 64 * 3);
    for encoded in frames {
        let frame = STANDARD.decode(encoded)?;
        ensure!(frame.len() == 64 * 64 * 3, "invalid prepared RGB frame");
        rgb.extend_from_slice(&frame);
    }
    // A tall temporary image lets the encoder choose and index a shared palette
    // across all frames, including exact colors when the clip uses <=256 colors.
    let indexed = gif::Frame::from_rgb_speed(64, (64 * frames.len()) as u16, &rgb, 10);
    let palette = indexed.palette.as_ref().context("missing GIF palette")?;
    let mut bytes = Vec::new();
    let mut elapsed_cs = 0;
    {
        let mut encoder = gif::Encoder::new(&mut bytes, 64, 64, palette)?;
        if looping {
            encoder.set_repeat(gif::Repeat::Infinite)?;
        }
        for (index, pixels) in indexed.buffer.chunks_exact(64 * 64).enumerate() {
            let next_cs = ((index as u64 + 1) * u64::from(frame_ms) + 5) / 10;
            let frame = gif::Frame {
                width: 64,
                height: 64,
                delay: (next_cs - elapsed_cs) as u16,
                dispose: gif::DisposalMethod::Keep,
                buffer: Cow::Borrowed(pixels),
                ..Default::default()
            };
            encoder.write_frame(&frame)?;
            elapsed_cs = next_cs;
        }
    }
    Ok((bytes, elapsed_cs * 10))
}

fn reachable(config: &Config, pack: &Pack) -> BTreeSet<String> {
    let mut pending = Vec::new();
    for state in ["idle", "working", "needs-input", "finished", "interrupted", "compacting", "delegating"] {
        if let Some(name) = config.mapping(state) {
            for working in [false, true] {
                if let Some(clip) = pack.resolve(name, working) {
                    pending.push(clip.name.clone());
                }
            }
        }
    }
    pending.extend(pack.manifest.default_animation.iter().cloned());
    let mut seen = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if seen.insert(name.clone()) && let Some(clip) = pack.clips.get(&name) {
            pending.extend(clip.entry.iter().chain(clip.exit.iter()).chain(clip.variants.values()).cloned());
        }
    }
    seen
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("pending-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()));
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

impl Catalog {
    pub fn prepare(config: &Config, pack: &Pack) -> Result<Self> {
        let storage = &config.device.storage;
        storage.validate()?;
        fs::create_dir_all(&storage.cache_dir)?;
        let mut catalog = Self { files: BTreeMap::new(), cache_dir: storage.cache_dir.clone() };
        for name in reachable(config, pack) {
            let clip = pack.clips.get(&name).context("configured clip missing")?;
            ensure!(name.len() <= 64 && name.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_')),
                "stored GIF clip names must be 1..64 letters, numbers, hyphens or underscores: {name}");
            let (bytes, duration_ms) = encode_gif(&clip.frames, clip.frame_ms, clip.looping)?;
            catalog.add(&name, format!("clip--{name}.gif"), bytes, duration_ms, &storage.directory)?;
            // Host-timed one-shots can hold their last frame when no base exists.
            // Preload that hold too so it never needs an event-time frame upload.
            let reaction = ["finished", "interrupted"].iter().any(|state| {
                config.mapping(state).is_some_and(|mapped| [false, true].iter().any(|working|
                    pack.resolve(mapped, *working).is_some_and(|c| c.name == name)))
            });
            if !clip.looping || reaction {
                let (bytes, duration_ms) = encode_gif(&clip.frames[clip.frames.len()-1..], clip.frame_ms, true)?;
                catalog.add(&format!("{name}:final"), format!("hold--{name}.gif"), bytes, duration_ms, &storage.directory)?;
            }
        }
        Ok(catalog)
    }

    fn add(&mut self, name: &str, filename: String, bytes: Vec<u8>, duration_ms: u64, directory: &str) -> Result<()> {
        ensure!(!self.files.values().any(|file| file.filename.eq_ignore_ascii_case(&filename)),
            "stored GIF filenames must be distinct ignoring case: {filename}");
        let path = self.cache_dir.join(&filename);
        if fs::read(&path).ok().as_deref() != Some(&bytes) {
            write_atomic(&path, &bytes)?;
        }
        self.files.insert(name.into(), StoredGif {
            local_name: format!("{directory}/{filename}"), filename,
            bytes: Arc::new(bytes), duration_ms,
        });
        Ok(())
    }

    pub fn get(&self, clip: &Clip) -> Result<&StoredGif> {
        self.files.get(&clip.name).with_context(|| format!("clip {} was not preloaded", clip.name))
    }

    pub fn report(&self) -> Value {
        json!({"cache_dir":self.cache_dir, "files":self.files.iter().map(|(name, file)| json!({
            "clip":name, "file":file.filename, "device_path":file.local_name,
            "bytes":file.bytes.len(), "duration_ms":file.duration_ms,
        })).collect::<Vec<_>>()})
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipts {
    version: u32,
    endpoint: String,
    directory: String,
    // Exact bytes, rather than a weak checksum, decide whether to upsert again.
    files: BTreeMap<String, String>,
}

#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub upserted: usize,
    pub unchanged: usize,
    pub file_commit_verified: bool,
    pub playback_verified: bool,
}

struct GifServer {
    base_url: String,
    generation: String,
    transfers: mpsc::Receiver<String>,
    task: JoinHandle<()>,
}

impl Drop for GifServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl GifServer {
    async fn start(config: &DeviceConfig, files: &BTreeMap<String, StoredGif>) -> Result<Self> {
        let listener = TcpListener::bind(&config.storage.bind).await
            .context("bind GIF download server; check storage.bind and whether another bridge is syncing")?;
        let listening = listener.local_addr()?;
        let url = reqwest::Url::parse(&config.url()?)?;
        let addresses: Vec<_> = lookup_host((url.host_str().context("missing device host")?, url.port_or_known_default().unwrap_or(80)))
            .await?.collect();
        ensure!(!addresses.is_empty(), "device hostname has no addresses");
        let allowed: BTreeSet<IpAddr> = addresses.iter().map(|addr| addr.ip()).collect();
        let base_url = if let Some(base) = &config.storage.public_base_url {
            base.trim_end_matches('/').to_owned()
        } else {
            let address = addresses.iter().find(|addr| addr.is_ipv4() == listening.is_ipv4())
                .context("device and GIF server address families differ")?;
            let socket = UdpSocket::bind(if address.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
            socket.connect(address)?;
            let host = socket.local_addr()?.ip();
            if host.is_ipv4() { format!("http://{host}:{}", listening.port()) }
            else { format!("http://[{host}]:{}", listening.port()) }
        };
        let generation = format!("{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos());
        let routes: BTreeMap<_, _> = files.values().map(|file|
            (format!("/{generation}/{}", file.filename), file.bytes.clone())).collect();
        let routes = Arc::new(routes);
        let (sender, transfers) = mpsc::channel(128);
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((stream, peer)) = accepted else { break };
                        if !allowed.contains(&peer.ip()) || connections.len() >= 8 { continue; }
                        let routes = routes.clone();
                        let sender = sender.clone();
                        connections.spawn(async move {
                            let _ = timeout(Duration::from_secs(10), serve_file(stream, &routes, sender)).await;
                        });
                    }
                    _ = connections.join_next(), if !connections.is_empty() => {}
                }
            }
        });
        Ok(Self { base_url, generation, transfers, task })
    }

    fn path(&self, file: &StoredGif) -> String {
        format!("/{}/{}", self.generation, file.filename)
    }

    async fn wait_for(&mut self, path: &str, timeout_ms: u64) -> Result<()> {
        timeout(Duration::from_millis(timeout_ms), async {
            while let Some(received) = self.transfers.recv().await {
                if received == path { return Ok(()); }
            }
            bail!("GIF download server stopped");
        }).await.with_context(|| format!(
            "Pixoo acknowledged SaveTFGif but no complete GIF fetch arrived at {}{path}; run outside the Codex sandbox and check LAN reachability/firewall or storage.public_base_url",
            self.base_url))?
    }

    async fn stop(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}

async fn serve_file(mut stream: TcpStream, routes: &BTreeMap<String, Arc<Vec<u8>>>, transfers: mpsc::Sender<String>) -> Result<()> {
    let mut request = Vec::new();
    let mut chunk = [0; 1024];
    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).await?;
        ensure!(read > 0 && request.len() + read <= 8192, "invalid GIF HTTP request");
        request.extend_from_slice(&chunk[..read]);
    }
    let line = std::str::from_utf8(&request)?.lines().next().context("empty request")?;
    let mut fields = line.split_whitespace();
    let method = fields.next().unwrap_or("");
    let path = fields.next().unwrap_or("");
    let Some(bytes) = routes.get(path).filter(|_| matches!(method, "GET" | "HEAD")) else {
        stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
        return Ok(());
    };
    let header = format!("HTTP/1.1 200 OK\r\nContent-Type: image/gif\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", bytes.len());
    stream.write_all(header.as_bytes()).await?;
    if method == "GET" {
        stream.write_all(bytes).await?;
        stream.shutdown().await?;
        let _ = transfers.send(path.into()).await;
    }
    Ok(())
}

pub async fn sync(config: &DeviceConfig, catalog: &Catalog, force: bool, status: Option<&Arc<Mutex<Value>>>) -> Result<SyncReport> {
    // OS file locking is released even after a crash, unlike a lockfile's mere
    // existence. Avoid concurrent bridge/CLI preloads sharing one receipt cache.
    let lock = fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(catalog.cache_dir.join("sync.lock"))?;
    lock.try_lock().map_err(|error| anyhow::anyhow!("another GIF sync is using this cache: {error}"))?;
    let endpoint = config.url()?;
    let receipt_path = catalog.cache_dir.join("device-transfers.json");
    let mut receipts: Receipts = if force { Receipts::default() } else {
        match fs::read(&receipt_path) {
            Ok(bytes) => {
                ensure!(bytes.len() <= 24 * 1024 * 1024, "storage receipts exceed 24 MiB");
                serde_json::from_slice(&bytes).context("invalid storage receipts; use sync-gifs --force to rebuild")?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Receipts::default(),
            Err(error) => return Err(error.into()),
        }
    };
    if receipts.version != 1 || receipts.endpoint != endpoint || receipts.directory != config.storage.directory {
        receipts = Receipts { version: 1, endpoint, directory: config.storage.directory.clone(), files: BTreeMap::new() };
    }
    if force && receipt_path.exists() {
        write_atomic(&receipt_path, &serde_json::to_vec(&receipts)?)?;
    }
    let mut report = SyncReport::default();
    let pending: Vec<_> = catalog.files.values().filter(|file| {
        if receipts.files.get(&file.local_name) == Some(&STANDARD.encode(file.bytes.as_ref())) {
            report.unchanged += 1;
            false
        } else { true }
    }).collect();
    if pending.is_empty() {
        if let Some(status) = status { status.lock().unwrap()["storage"] = json!({"phase":"ready", "summary":report}); }
        return Ok(report);
    }
    let device = Device::new(config.clone())?;
    let mut server = GifServer::start(config, &catalog.files).await?;
    for file in pending {
        if let Some(status) = status { status.lock().unwrap()["storage"] = json!({
            "phase":"upserting", "file":file.local_name, "upserted":report.upserted, "unchanged":report.unchanged,
        }); }
        let path = server.path(file);
        // A failed overwrite can invalidate the old file too. Do not retain an
        // optimistic receipt for its former contents while replacing it.
        if receipts.files.remove(&file.local_name).is_some() {
            write_atomic(&receipt_path, &serde_json::to_vec(&receipts)?)?;
        }
        device.command(json!({"Command":"Device/SaveTFGif", "LocalName":file.local_name,
            "NetName":format!("{}{path}", server.base_url)})).await?;
        server.wait_for(&path, config.storage.download_timeout_ms).await?;
        sleep(Duration::from_millis(config.storage.settle_ms)).await;
        receipts.files.insert(file.local_name.clone(), STANDARD.encode(file.bytes.as_ref()));
        write_atomic(&receipt_path, &serde_json::to_vec(&receipts)?)?;
        report.upserted += 1;
        eprintln!("GIF transferred for local storage: {}", file.local_name);
    }
    server.stop().await;
    if let Some(status) = status { status.lock().unwrap()["storage"] = json!({"phase":"ready", "summary":report}); }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::Manifest;
    use tokio::time::Instant;

    fn clip(name: &str, looping: bool) -> Arc<Clip> {
        Arc::new(Clip {
            name: name.into(), frames: vec![STANDARD.encode([12, 34, 56].repeat(64 * 64))],
            frame_ms: 83, looping, entry: None, exit: None, variants: BTreeMap::new(),
        })
    }

    #[test]
    fn native_gifs_preserve_pixels_timing_repeat_and_deterministic_bytes() {
        let frames: Vec<_> = (0..32).map(|i| STANDARD.encode([i, 22, 44].repeat(64 * 64))).collect();
        for looping in [false, true] {
            let (bytes, duration) = encode_gif(&frames, 83, looping).unwrap();
            assert_eq!(duration, 2660);
            assert_eq!(bytes, encode_gif(&frames, 83, looping).unwrap().0);
            let mut options = gif::DecodeOptions::new();
            options.set_color_output(gif::ColorOutput::RGBA);
            let mut reader = options.read_info(bytes.as_slice()).unwrap();
            assert_eq!((reader.width(), reader.height()), (64, 64));
            let mut total = 0;
            for index in 0..32 {
                let frame = reader.read_next_frame().unwrap().unwrap();
                assert_eq!(frame.buffer.as_ref(), [index, 22, 44, 255].repeat(64 * 64));
                assert!(frame.transparent.is_none());
                total += u64::from(frame.delay) * 10;
            }
            assert!(reader.read_next_frame().unwrap().is_none());
            assert_eq!(total, duration);
            assert_eq!(reader.repeat(), if looping { gif::Repeat::Infinite } else { gif::Repeat::Finite(0) });
        }
    }

    #[test]
    fn configured_catalog_includes_transitions_and_holds_but_skips_disabled_and_unused_clips() {
        let root = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.device.storage.cache_dir = root.path().into();
        config.animations.insert("needs-input".into(), "".into());
        let mut working = clip("working", true);
        Arc::get_mut(&mut working).unwrap().entry = Some("work-entry".into());
        let mut delegation = clip("delegating", true);
        Arc::get_mut(&mut delegation).unwrap().entry = Some("delegate-start".into());
        Arc::get_mut(&mut delegation).unwrap().exit = Some("delegate-end".into());
        let clips = [clip("idle", true), working, delegation, clip("work-entry", false),
            clip("delegate-start", false), clip("delegate-end", false), clip("needs-input", true), clip("unused", true)]
            .into_iter().map(|clip| (clip.name.clone(), clip)).collect();
        let pack = Pack {
            manifest: Manifest { schema_version: 1, id: "test".into(), canvas_size: 64,
                background: "#000000".into(), default_animation: Some("idle".into()), animations: BTreeMap::new() },
            clips, encoded_bytes: 0,
        };
        let catalog = Catalog::prepare(&config, &pack).unwrap();
        assert_eq!(catalog.files.len(), 9);
        assert!(!catalog.files.contains_key("unused"));
        assert!(!catalog.files.contains_key("needs-input"));
        for name in ["work-entry", "delegate-start", "delegate-end"] {
            let hold = &catalog.files[&format!("{name}:final")];
            let mut reader = gif::DecodeOptions::new().read_info(hold.bytes.as_slice()).unwrap();
            assert!(reader.read_next_frame().unwrap().is_some());
            assert!(reader.read_next_frame().unwrap().is_none());
            assert_eq!(reader.repeat(), gif::Repeat::Infinite);
        }
    }

    #[test]
    fn reviewed_delegation_pack_encodes_all_selected_clips_and_holds() {
        // Read the trusted checked-in fixture directly: directory canonicalization
        // is blocked in the managed Windows sandbox, even for existing validators.
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/pets/decker/revisions/delegation-v3/review"));
        let manifest: Manifest = serde_json::from_slice(&fs::read(path.join("pet.json")).unwrap()).unwrap();
        let background = crate::assets::parse_color(&manifest.background).unwrap();
        let mut clips = BTreeMap::new();
        for (name, spec) in &manifest.animations {
            let crate::assets::Source::SpriteSheet { path: source, columns, frame_count } = &spec.source else {
                panic!("review fixture must use native sprite sheets");
            };
            let sheet = image::open(path.join(source)).unwrap().to_rgba8();
            assert_eq!(sheet.dimensions(), (columns * 64, (*frame_count as u32).div_ceil(*columns) * 64));
            let frames = (0..*frame_count as u32).map(|index| {
                let cell = image::imageops::crop_imm(&sheet, index % columns * 64, index / columns * 64, 64, 64).to_image();
                STANDARD.encode(crate::assets::composite(&cell, background).as_raw())
            }).collect();
            clips.insert(name.clone(), Arc::new(Clip { name: name.clone(), frames,
                frame_ms: spec.frame_duration_ms.unwrap(), looping: spec.r#loop,
                entry: spec.entry.clone(), exit: spec.exit.clone(), variants: spec.variants.clone() }));
        }
        let pack = Pack { manifest, clips, encoded_bytes: 0 };
        let root = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.device.storage.cache_dir = root.path().into();
        let catalog = Catalog::prepare(&config, &pack).unwrap();
        assert_eq!(catalog.files.len(), 16, "11 physical clips and five final-frame holds");
        assert!(!catalog.files.contains_key("compacting"), "variant alias is not a physical playback selection");
        for (name, file) in &catalog.files {
            let held = name.strip_suffix(":final");
            let clip = &pack.clips[held.unwrap_or(name)];
            let mut reader = gif::DecodeOptions::new().read_info(file.bytes.as_slice()).unwrap();
            assert_eq!((reader.width(), reader.height()), (64, 64));
            let mut count = 0;
            let mut duration = 0;
            while let Some(frame) = reader.read_next_frame().unwrap() {
                assert_eq!((frame.width, frame.height), (64, 64));
                assert!(frame.transparent.is_none() && frame.palette.is_none());
                count += 1;
                duration += u64::from(frame.delay) * 10;
            }
            assert_eq!(count, if held.is_some() { 1 } else { clip.frames.len() });
            assert_eq!(duration, file.duration_ms);
            assert_eq!(reader.repeat(), if held.is_some() || clip.looping { gif::Repeat::Infinite } else { gif::Repeat::Finite(0) });
        }
    }

    #[test]
    fn storage_configuration_rejects_escaping_paths_and_credential_urls() {
        let mut storage = crate::config::StorageConfig::default();
        for directory in ["../other", "/root", "a//b", "a\\b", "", "a.b"] {
            storage.directory = directory.into();
            assert!(storage.validate().is_err());
        }
        storage.directory = "codex-pet/decker".into();
        for url in ["https://host/", "http://user:secret@host/", "http://host/?q=1", "http://host/prefix"] {
            storage.public_base_url = Some(url.into());
            assert!(storage.validate().is_err());
        }
        storage.public_base_url = Some("http://192.0.2.1:8787".into());
        assert!(storage.validate().is_ok());
    }

    struct CommandTiming {
        command: String,
        requested_at: Instant,
        replied_at: Instant,
    }

    struct MockDevice {
        address: String,
        calls: Arc<Mutex<Vec<Value>>>,
        files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
        fetch_mode: Arc<std::sync::atomic::AtomicU8>,
        response_delays: Arc<Mutex<BTreeMap<String, u64>>>,
        timings: Arc<Mutex<Vec<CommandTiming>>>,
        task: JoinHandle<()>,
    }

    impl Drop for MockDevice {
        fn drop(&mut self) { self.task.abort(); }
    }

    impl MockDevice {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let files = Arc::new(Mutex::new(BTreeMap::new()));
            let fetch_mode = Arc::new(std::sync::atomic::AtomicU8::new(1));
            let response_delays = Arc::new(Mutex::new(BTreeMap::<String, u64>::new()));
            let timings = Arc::new(Mutex::new(Vec::new()));
            let captured = calls.clone();
            let stored = files.clone();
            let mode = fetch_mode.clone();
            let delays = response_delays.clone();
            let recorded_timings = timings.clone();
            let task = tokio::spawn(async move {
                while let Ok((mut stream, _)) = listener.accept().await {
                    let mut request = Vec::new();
                    let mut chunk = [0; 2048];
                    let header_end = loop {
                        let count = stream.read(&mut chunk).await.unwrap();
                        assert!(count > 0);
                        request.extend_from_slice(&chunk[..count]);
                        if let Some(index) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") { break index + 4; }
                    };
                    let header = std::str::from_utf8(&request[..header_end]).unwrap();
                    let length: usize = header.lines().find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().unwrap())
                    }).unwrap();
                    while request.len() < header_end + length {
                        let count = stream.read(&mut chunk).await.unwrap();
                        assert!(count > 0);
                        request.extend_from_slice(&chunk[..count]);
                    }
                    let value: Value = serde_json::from_slice(&request[header_end..header_end+length]).unwrap();
                    let requested_at = Instant::now();
                    let command = value["Command"].as_str().unwrap().to_owned();
                    captured.lock().unwrap().push(value.clone());
                    let mut error_code = 0;
                    if value["Command"] == "Device/SaveTFGif" {
                        let mode = mode.load(std::sync::atomic::Ordering::Relaxed);
                        if mode != 0 {
                            let client = reqwest::Client::builder().no_proxy().build().unwrap();
                            let url = value["NetName"].as_str().unwrap();
                            if mode == 2 { client.head(url).send().await.unwrap(); }
                            else {
                                let bytes = client.get(url).send().await.unwrap().bytes().await.unwrap();
                                stored.lock().unwrap().insert(value["LocalName"].as_str().unwrap().into(), bytes.to_vec());
                            }
                        }
                    } else if value["Command"] == "Device/PlayTFGif"
                        && !stored.lock().unwrap().contains_key(value["FileName"].as_str().unwrap()) {
                        error_code = 2;
                    }
                    let delay_ms = delays.lock().unwrap().get(&command).copied().unwrap_or(0);
                    if delay_ms > 0 { sleep(Duration::from_millis(delay_ms)).await; }
                    let body = format!("{{\"error_code\":{error_code}}}");
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    recorded_timings.lock().unwrap().push(CommandTiming {
                        command, requested_at, replied_at: Instant::now(),
                    });
                    stream.write_all(response.as_bytes()).await.unwrap();
                    stream.shutdown().await.unwrap();
                }
            });
            Self { address, calls, files, fetch_mode, response_delays, timings, task }
        }

        fn config(&self, root: &Path) -> Config {
            let mut config = Config::default();
            config.device.address = self.address.clone();
            config.device.local_token = Some(123456);
            config.device.storage.cache_dir = root.into();
            config.device.storage.bind = "127.0.0.1:0".into();
            config.device.storage.download_timeout_ms = 100;
            config.device.storage.settle_ms = 0;
            config
        }
    }

    fn test_pack() -> Pack {
        let mut working = clip("working", true);
        Arc::get_mut(&mut working).unwrap().entry = Some("work-entry".into());
        let mut delegation = clip("delegating", true);
        Arc::get_mut(&mut delegation).unwrap().entry = Some("delegate-start".into());
        Arc::get_mut(&mut delegation).unwrap().exit = Some("delegate-end".into());
        Pack {
            manifest: Manifest { schema_version: 1, id: "mock".into(), canvas_size: 64,
                background: "#000000".into(), default_animation: Some("idle".into()), animations: BTreeMap::new() },
            clips: [clip("idle", true), working, delegation, clip("work-entry", false),
                clip("delegate-start", false), clip("delegate-end", false), clip("finished", false)]
                .into_iter().map(|clip| (clip.name.clone(), clip)).collect(),
            encoded_bytes: 0,
        }
    }

    #[tokio::test]
    async fn upsert_requires_get_then_skips_unchanged_bytes_and_replaces_only_changed_files() {
        let root = tempfile::tempdir().unwrap();
        let mock = MockDevice::start().await;
        let config = mock.config(root.path());
        let mut pack = test_pack();
        let catalog = Catalog::prepare(&config, &pack).unwrap();
        let first = sync(&config.device, &catalog, false, None).await.unwrap();
        assert_eq!(first.upserted, catalog.files.len());
        assert_eq!(mock.files.lock().unwrap().len(), first.upserted);
        let before = mock.calls.lock().unwrap().len();
        let second = sync(&config.device, &catalog, false, None).await.unwrap();
        assert_eq!((second.upserted, second.unchanged), (0, first.upserted));
        assert_eq!(mock.calls.lock().unwrap().len(), before);
        Arc::get_mut(pack.clips.get_mut("idle").unwrap()).unwrap().frame_ms = 100;
        let updated = Catalog::prepare(&config, &pack).unwrap();
        let changed = sync(&config.device, &updated, false, None).await.unwrap();
        assert_eq!((changed.upserted, changed.unchanged), (1, first.upserted - 1));
        let forced = sync(&config.device, &updated, true, None).await.unwrap();
        assert_eq!(forced.upserted, first.upserted);
        assert!(!forced.file_commit_verified && !forced.playback_verified);
    }

    #[tokio::test]
    async fn save_ack_or_head_without_get_never_records_success() {
        let root = tempfile::tempdir().unwrap();
        let mock = MockDevice::start().await;
        let config = mock.config(root.path());
        let catalog = Catalog::prepare(&config, &test_pack()).unwrap();
        for mode in [0, 2] {
            mock.fetch_mode.store(mode, std::sync::atomic::Ordering::Relaxed);
            let error = sync(&config.device, &catalog, false, None).await.unwrap_err();
            assert!(format!("{error:#}").contains("no complete GIF fetch"));
            assert!(!root.path().join("device-transfers.json").exists());
        }
        mock.fetch_mode.store(1, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(sync(&config.device, &catalog, false, None).await.unwrap().upserted, catalog.files.len());
        mock.fetch_mode.store(0, std::sync::atomic::Ordering::Relaxed);
        assert!(sync(&config.device, &catalog, true, None).await.is_err());
        mock.fetch_mode.store(1, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(sync(&config.device, &catalog, false, None).await.unwrap().upserted, catalog.files.len(),
            "a failed forced resync must not reuse the old receipts");
    }

    #[tokio::test]
    async fn stored_one_shot_deadline_excludes_brightness_and_response_time_and_bypasses_pacing() {
        let root = tempfile::tempdir().unwrap();
        let mock = MockDevice::start().await;
        let mut config = mock.config(root.path());
        config.device.brightness = Some(25);
        config.device.playback_start_delay_ms = 50;
        config.device.switch_interval_ms = 2000;
        mock.response_delays.lock().unwrap().extend([
            ("Channel/SetBrightness".into(), 100), ("Device/PlayTFGif".into(), 300),
        ]);
        let pack = test_pack();
        let catalog = Arc::new(Catalog::prepare(&config, &pack).unwrap());
        let hold = Duration::from_millis(catalog.files["work-entry"].duration_ms
            + config.device.playback_start_delay_ms);
        let initial = crate::device::Target { serial: 1, clip: Some(pack.clips["work-entry"].clone()),
            single_play: true, created: Instant::now() };
        let (targets, receiver) = tokio::sync::watch::channel(initial);
        let (acknowledgements, mut acks) = mpsc::channel(16);
        let writer = tokio::spawn(crate::device::writer(config.device, false, Some(catalog), receiver,
            acknowledgements, Arc::new(Mutex::new(json!({})))));
        let ack = timeout(Duration::from_secs(3), acks.recv()).await.unwrap().unwrap();
        let playback_started_at = ack.playback_ends_at - hold;
        {
            let timings = mock.timings.lock().unwrap();
            let brightness = timings.iter().find(|timing| timing.command == "Channel/SetBrightness").unwrap();
            let play = timings.iter().find(|timing| timing.command == "Device/PlayTFGif").unwrap();
            assert!(playback_started_at >= brightness.replied_at,
                "brightness setup must finish before the playback budget begins");
            assert!(playback_started_at <= play.requested_at,
                "the deadline must use the play request, not its response");
            assert!(ack.playback_ends_at <= play.replied_at,
                "a response slower than the clip must leave its deadline expired");
        }
        mock.response_delays.lock().unwrap().clear();
        targets.send_replace(crate::device::Target { serial: 2, clip: Some(pack.clips["working"].clone()),
            single_play: false, created: Instant::now() });
        assert_eq!(timeout(Duration::from_secs(1), acks.recv()).await.unwrap().unwrap().serial, 2,
            "an expired one-shot must return without the two-second switch pacing");
        drop(targets);
        timeout(Duration::from_secs(1), writer).await.unwrap().unwrap().unwrap();
    }

    #[tokio::test]
    async fn frame_upload_deadline_starts_after_the_last_frame_response() {
        let root = tempfile::tempdir().unwrap();
        let mock = MockDevice::start().await;
        let mut config = mock.config(root.path());
        config.device.transport = crate::config::Transport::Frames;
        config.device.frame_upload_interval_ms = 20;
        config.device.playback_start_delay_ms = 50;
        mock.response_delays.lock().unwrap().insert("Draw/SendHttpGif".into(), 100);
        let mut entry = clip("work-entry", false);
        let frame = entry.frames[0].clone();
        Arc::get_mut(&mut entry).unwrap().frames.push(frame);
        let hold = Duration::from_millis(entry.duration_ms() + config.device.playback_start_delay_ms);
        let initial = crate::device::Target { serial: 1, clip: Some(entry), single_play: true,
            created: Instant::now() };
        let (targets, receiver) = tokio::sync::watch::channel(initial);
        let (acknowledgements, mut acks) = mpsc::channel(16);
        let writer = tokio::spawn(crate::device::writer(config.device, false, None, receiver,
            acknowledgements, Arc::new(Mutex::new(json!({})))));
        let ack = timeout(Duration::from_secs(2), acks.recv()).await.unwrap().unwrap();
        let acknowledged_at = Instant::now();
        let playback_started_at = ack.playback_ends_at - hold;
        {
            let timings = mock.timings.lock().unwrap();
            let frames: Vec<_> = timings.iter().filter(|timing| timing.command == "Draw/SendHttpGif").collect();
            assert_eq!(frames.len(), 2);
            assert!(playback_started_at >= frames[1].replied_at,
                "a frame upload must not spend its playback budget during transmission");
            assert!(playback_started_at <= acknowledged_at);
        }
        drop(targets);
        timeout(Duration::from_secs(1), writer).await.unwrap().unwrap().unwrap();
    }

    #[tokio::test]
    async fn default_writer_selects_transitions_and_holds_with_no_event_time_uploads() {
        let root = tempfile::tempdir().unwrap();
        let mock = MockDevice::start().await;
        let config = mock.config(root.path());
        let pack = test_pack();
        let catalog = Arc::new(Catalog::prepare(&config, &pack).unwrap());
        let initial = crate::device::Target { serial: 1, clip: Some(pack.clips["idle"].clone()),
            single_play: false, created: tokio::time::Instant::now() };
        let (targets, receiver) = tokio::sync::watch::channel(initial);
        let (acknowledgements, mut acks) = mpsc::channel(16);
        let status = Arc::new(Mutex::new(json!({})));
        let writer = tokio::spawn(crate::device::writer(config.device, false, Some(catalog.clone()), receiver, acknowledgements, status.clone()));
        timeout(Duration::from_secs(3), acks.recv()).await.unwrap().unwrap();
        let preloaded = mock.calls.lock().unwrap().iter().filter(|value| value["Command"] == "Device/SaveTFGif").count();
        for (index, name) in ["work-entry", "working", "delegate-start", "delegating", "delegate-end", "working", "finished"].iter().enumerate() {
            let clip = pack.clips[*name].clone();
            targets.send_replace(crate::device::Target { serial: index as u64 + 2,
                single_play: !clip.looping, clip: Some(clip), created: tokio::time::Instant::now() });
            let ack = timeout(Duration::from_secs(1), acks.recv()).await.unwrap().unwrap();
            let timings = mock.timings.lock().unwrap();
            let play = timings.iter().rev().find(|timing| timing.command == "Device/PlayTFGif").unwrap();
            assert!(ack.playback_ends_at <= play.requested_at + Duration::from_millis(catalog.files[*name].duration_ms));
        }
        let mut held = clip("finished:final", true);
        Arc::get_mut(&mut held).unwrap().frames = pack.clips["finished"].frames.clone();
        targets.send_replace(crate::device::Target { serial: 20, clip: Some(held), single_play: false,
            created: tokio::time::Instant::now() });
        timeout(Duration::from_secs(1), acks.recv()).await.unwrap().unwrap();
        let before = mock.calls.lock().unwrap().len();
        sleep(Duration::from_millis(25)).await;
        assert_eq!(mock.calls.lock().unwrap().len(), before);
        {
            let calls = mock.calls.lock().unwrap();
            assert_eq!(calls.iter().filter(|value| value["Command"] == "Device/SaveTFGif").count(), preloaded);
            assert!(!calls.iter().any(|value| value["Command"].as_str().unwrap().starts_with("Draw/")));
            assert!(calls.iter().filter(|value| value["Command"] == "Device/PlayTFGif")
                .all(|value| value["FileType"] == 0 && !value["FileName"].as_str().unwrap().starts_with("http")));
            assert!(calls.iter().all(|value| value["LocalToken"] == 123456));
        }
        assert_eq!(status.lock().unwrap()["device"]["last_played_clip"], "finished:final");
        drop(targets);
        timeout(Duration::from_secs(1), writer).await.unwrap().unwrap().unwrap();
    }
}
