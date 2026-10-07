mod assets;
mod config;
mod device;
mod engine;
mod event;
mod ipc;
mod idle;
mod runtime;
mod setup;
mod storage;
#[cfg(windows)]
mod winpipe;

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use config::{Config, DEFAULT_PIPE};
use event::Event;
use std::{
    fs,
    io::{BufRead, Read},
    path::{Path, PathBuf},
    process::Stdio,
};

#[derive(Parser)]
#[command(version, about = "Local Codex lifecycle pets for Divoom Pixoo64")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the persistent bridge; Ctrl+C stops it.
    Run {
        #[arg(short, long)]
        config: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Upsert configured native GIFs to device storage; run before or outside the bridge.
    SyncGifs {
        #[arg(short, long)]
        config: PathBuf,
        /// Resend all GIFs, including those with unchanged transfer receipts.
        #[arg(long)]
        force: bool,
        /// Export the native GIFs and plan without making any device requests.
        #[arg(long)]
        prepare_only: bool,
    },
    /// Forward official hook JSON from stdin; returns harmless success by default.
    Emit {
        #[arg(long, default_value=DEFAULT_PIPE)]
        pipe: String,
        #[arg(long)]
        json: Option<String>,
        #[arg(long)]
        strict: bool,
    },
    /// Forward the official completion notification, preserving configured integrations.
    Notify {
        #[arg(short, long)]
        config: PathBuf,
        notification: String,
    },
    /// Query the running bridge.
    Status {
        #[arg(long, default_value=DEFAULT_PIPE)]
        pipe: String,
    },
    /// Validate and predecode a custom pet pack without accessing a device.
    Validate {
        pack: PathBuf,
        #[arg(long, default_value_t = 40)]
        max_frames: usize,
    },
    /// Validate configuration and its animation references without accessing a device.
    CheckConfig {
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Export a prepared clip as an enlarged GIF and optional contact sheet.
    Preview {
        pack: PathBuf,
        clip: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        contact: Option<PathBuf>,
        #[arg(long)]
        sheet: Option<PathBuf>,
        #[arg(long, default_value="working", value_parser=["working", "idle"])]
        pose: String,
    },
    /// Import PNGs, sprite grids, GIFs, or numbered frame folders into pet.json.
    Import {
        directory: PathBuf,
        #[arg(long, default_value = "custom-pet")]
        id: String,
        #[arg(long, default_value_t = 83)]
        frame_ms: u32,
        #[arg(long)]
        force: bool,
    },
    /// Export a square source as exact 64x64 RGB and optional 512x512 preview.
    Prepare {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        preview: Option<PathBuf>,
        #[arg(long, default_value = "#000000")]
        background: String,
    },
    /// Export a generated frame grid; optional masks preserve a canonical background.
    Sheet {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        columns: u32,
        #[arg(long)]
        frame_count: usize,
        #[arg(long)]
        base: Option<PathBuf>,
        /// Repeat x,y,width,height for regions that may change from the base.
        #[arg(long)]
        mask: Vec<String>,
        #[arg(long)]
        first_base: Option<PathBuf>,
        #[arg(long)]
        last_base: Option<PathBuf>,
    },
    /// Replay official or normalized JSONL events offline and print state selections.
    Replay {
        #[arg(short, long)]
        config: PathBuf,
        events: PathBuf,
    },
    /// Generate a hook fragment and notify command for review; does not edit Codex settings.
    Hooks {
        #[arg(short, long)]
        config: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        executable: Option<PathBuf>,
    },
    /// Prepare a reviewed merge of Codex hooks/settings; writes only a new staging directory.
    Setup {
        #[arg(short, long)]
        config: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Defaults to CODEX_HOME or ~/.codex.
        #[arg(long)]
        codex_home: Option<PathBuf>,
    },
    /// Read device status; does not change display/channel/brightness.
    Probe {
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Create a small diagnostic pack for testing the bridge, not personal pet artwork.
    Example { output: PathBuf },
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = execute(Cli::parse()).await {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

async fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Run { config, dry_run } => {
            let mut config = Config::load(&config)?;
            config.dry_run |= dry_run;
            let pack = assets::Pack::load(&config.pack, config.device.max_frames)?;
            runtime::run(config, pack).await?;
        }
        Command::SyncGifs { config, force, prepare_only } => {
            let config = Config::load(&config)?;
            let pack = assets::Pack::load(&config.pack, config.device.max_frames)?;
            let catalog = storage::Catalog::prepare(&config, &pack)?;
            let mut report = catalog.report();
            if prepare_only || config.dry_run {
                report["preload"] = serde_json::json!("not requested; no device requests");
            } else {
                report["preload"] = serde_json::to_value(storage::sync(&config.device, &catalog, force, None).await?)?;
            }
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Emit { pipe, json, strict } => {
            let result = async {
                let input = match json {
                    Some(json) => json,
                    None => {
                        let mut bytes = Vec::new();
                        std::io::stdin()
                            .take(2 * 1024 * 1024 + 1)
                            .read_to_end(&mut bytes)?;
                        ensure!(bytes.len() <= 2 * 1024 * 1024, "hook input too large");
                        String::from_utf8(bytes)?
                    }
                };
                if let Some(event) = Event::normalize(&serde_json::from_str(&input)?)? {
                    ipc::send(&pipe, ipc::Request::Event(event), 50).await?;
                }
                Ok::<(), anyhow::Error>(())
            }
            .await;
            // Valid no-op JSON for Stop/SubagentStop and all other command hooks.
            println!("{{}}");
            if strict {
                result?;
            }
        }
        Command::Notify {
            config,
            notification,
        } => {
            let config = Config::load(&config)?;
            let forwarding = config.notification_forward;
            if let Some(program) = forwarding.first() {
                let mut command = std::process::Command::new(program);
                command
                    .args(&forwarding[1..])
                    .arg(&notification)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                if command.spawn().is_err() {
                    eprintln!("existing completion notifier could not be launched");
                }
            }
            let value = serde_json::from_str(&notification);
            if let Ok(value) = value
                && let Ok(Some(event)) = Event::normalize(&value)
            {
                let _ = ipc::send(&config.pipe, ipc::Request::Event(event), 50).await;
            }
        }
        Command::Status { pipe } => {
            let status = ipc::send(&pipe, ipc::Request::Status, 1000)
                .await?
                .context("no bridge status")?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Command::CheckConfig { config } => {
            let config = Config::load(&config)?;
            if !config.dry_run { config.device.url()?; }
            let pack = assets::Pack::load(&config.pack, config.device.max_frames)?;
            let alternates = config.idle_alternate_clips(&pack)?;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "valid":true,"pack":pack.manifest.id,"pipe":config.pipe,
                "animations":pack.clips.keys().collect::<Vec<_>>(),
                "idle_alternates":alternates.iter().map(|clip| clip.name.as_str()).collect::<Vec<_>>(),
            }))?);
        }
        Command::Validate { pack, max_frames } => {
            let pack = assets::Pack::load(&pack, max_frames)?;
            println!("{}", serde_json::to_string_pretty(&pack.report())?);
        }
        Command::Preview {
            pack,
            clip,
            output,
            contact,
            sheet,
            pose,
        } => {
            let pack = assets::Pack::load(&pack, 40)?;
            assets::preview(
                &pack,
                &clip,
                &output,
                contact.as_deref(),
                sheet.as_deref(),
                pose == "working",
            )?;
            println!("saved preview {}", output.display());
        }
        Command::Import {
            directory,
            id,
            frame_ms,
            force,
        } => {
            assets::import_directory(&directory, &id, frame_ms, force)?;
            println!("wrote {}", directory.join("pet.json").display());
        }
        Command::Prepare {
            input,
            output,
            preview,
            background,
        } => {
            assets::prepare(&input, &output, preview.as_deref(), &background)?;
            println!("saved exact 64x64 RGB image: {}", output.display());
        }
        Command::Sheet {
            input,
            output,
            columns,
            frame_count,
            base,
            mask,
            first_base,
            last_base,
        } => {
            assets::export_sheet(
                &input,
                &output,
                columns,
                frame_count,
                assets::SheetOptions {
                    base: base.as_deref(),
                    masks: &mask,
                    first: first_base.as_deref(),
                    last: last_base.as_deref(),
                },
            )?;
            println!("exported {frame_count} frames to {}", output.display());
        }
        Command::Replay { config, events } => {
            let config = Config::load(&config)?;
            let pack = assets::Pack::load(&config.pack, config.device.max_frames)?;
            ensure!(
                fs::metadata(&events)?.len() <= 16 * 1024 * 1024,
                "replay file exceeds 16 MiB"
            );
            let mut engine = engine::Engine::default();
            for line in std::io::BufReader::new(fs::File::open(events)?).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let value: serde_json::Value = serde_json::from_str(&line)?;
                let event = if value.get("kind").is_some() {
                    Some(serde_json::from_value::<Event>(value)?)
                } else {
                    Event::normalize(&value)?
                };
                if let Some(event) = event {
                    event.validate()?;
                    let outcome = engine.apply(&event);
                    println!(
                        "{}",
                        serde_json::json!({
                            "event":event.kind,"state":engine.snapshot(),
                            "clip":runtime::base(&engine,&config,&pack).map(|c|c.name.clone()),
                            "reaction":outcome.reaction.filter(|state|config.mapping(state)
                                .is_some_and(|name| pack.resolve(name,engine.busy()).is_some()))
                        })
                    );
                }
            }
        }
        Command::Hooks {
            config,
            output,
            executable,
        } => {
            let file = shell_path(config.canonicalize()?);
            let config = Config::load(&file)?;
            let pack = assets::Pack::load(&config.pack, config.device.max_frames)?;
            let exe = shell_path(
                executable
                    .unwrap_or(std::env::current_exe()?)
                    .canonicalize()?,
            );
            let hooks = setup::hook_spec(&config, &pack, &exe)?;
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            ensure!(
                !output.exists(),
                "hook output exists; choose a new filename for review"
            );
            fs::write(&output, serde_json::to_vec_pretty(&hooks)?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "hooks_file":output,
                    "notify":[exe.to_string_lossy(),"notify","--config",file.to_string_lossy()],
                    "instruction":"Merge hooks with existing handlers; preserve the existing notify array in notification_forward. Review/trust changed hooks in Codex."
                }))?
            );
        }
        Command::Setup {
            config,
            output,
            codex_home,
        } => {
            setup::prepare(&config, &output, codex_home.as_deref())?;
            println!(
                "Prepared {}. Review config.toml, hooks.json and bridge.toml before applying.",
                output.display()
            );
        }
        Command::Probe { config } => {
            let config = Config::load(&config)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&device::Device::new(config.device)?.probe().await?)?
            );
        }
        Command::Example { output } => generate_example(&output)?,
    }
    Ok(())
}

fn shell_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn generate_example(output: &Path) -> Result<()> {
    ensure!(!output.exists(), "example output already exists");
    fs::create_dir_all(output)?;
    for (name, color) in [
        ("idle", [60, 50, 120]),
        ("working", [0, 160, 220]),
        ("needs-input", [220, 130, 0]),
        ("finished", [0, 170, 100]),
        ("interrupted", [220, 20, 50]),
        ("compacting", [190, 20, 180]),
        ("delegating", [20, 180, 40]),
        ("working-enter", [0, 110, 180]),
    ] {
        let mut sheet = image::RgbaImage::new(64 * 4, 64);
        for frame in 0..4 {
            for y in 0..64 {
                for x in 0..64 {
                    let border = (8..56).contains(&x)
                        && (12..52).contains(&y)
                        && (!(11..53).contains(&x) || !(15..49).contains(&y));
                    let marker =
                        (18 + frame * 6..24 + frame * 6).contains(&x) && (28..36).contains(&y);
                    sheet.put_pixel(
                        x + frame * 64,
                        y,
                        image::Rgba(if border || marker {
                            [color[0], color[1], color[2], 255]
                        } else {
                            [0, 0, 0, 255]
                        }),
                    );
                }
            }
        }
        sheet.save(output.join(format!("{name}.png")))?;
    }
    assets::import_directory(output, "diagnostic", 100, false)?;
    println!("created diagnostic pack {}", output.display());
    Ok(())
}
