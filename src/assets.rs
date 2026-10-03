use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{AnimationDecoder, ImageDecoder, ImageReader, RgbaImage, imageops::FilterType};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::BufReader,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

const MAX_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PACK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub canvas_size: u32,
    #[serde(default = "black")]
    pub background: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_animation: Option<String>,
    pub animations: BTreeMap<String, ClipSpec>,
}
fn black() -> String {
    "#000000".into()
}
fn yes() -> bool {
    true
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClipSpec {
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_duration_ms: Option<u32>,
    #[serde(default = "yes")]
    pub r#loop: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variants: BTreeMap<String, String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Image {
        path: String,
    },
    SpriteSheet {
        path: String,
        columns: u32,
        frame_count: usize,
    },
    Frames {
        paths: Vec<String>,
    },
    Gif {
        path: String,
    },
}

pub struct Clip {
    pub name: String,
    pub frames: Vec<String>,
    pub frame_ms: u32,
    pub looping: bool,
    pub entry: Option<String>,
    pub variants: BTreeMap<String, String>,
}
impl Clip {
    pub fn duration_ms(&self) -> u64 {
        self.frames.len() as u64 * self.frame_ms as u64
    }
}

pub fn preview(
    pack: &Pack,
    name: &str,
    output: &Path,
    contact: Option<&Path>,
    native_sheet: Option<&Path>,
    working: bool,
) -> Result<()> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let clip = pack.resolve(name, working).context("clip not found")?;
    for path in std::iter::once(output).chain(contact).chain(native_sheet) {
        ensure!(
            !path.exists(),
            "preview output exists; choose a new filename"
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut frames = Vec::new();
    let columns = clip.frames.len().min(4) as u32;
    let rows = (clip.frames.len() as u32).div_ceil(columns);
    let mut sheet = image::RgbImage::new(columns * 64, rows * 64);
    for (index, encoded) in clip.frames.iter().enumerate() {
        let raw = STANDARD.decode(encoded)?;
        let rgb = image::RgbImage::from_raw(64, 64, raw).context("invalid prepared frame")?;
        image::imageops::replace(
            &mut sheet,
            &rgb,
            ((index as u32 % columns) * 64) as i64,
            ((index as u32 / columns) * 64) as i64,
        );
        let rgba = image::DynamicImage::ImageRgb8(rgb).to_rgba8();
        let enlarged = image::imageops::resize(&rgba, 512, 512, FilterType::Nearest);
        frames.push(image::Frame::from_parts(
            enlarged,
            0,
            0,
            image::Delay::from_numer_denom_ms(clip.frame_ms, 1),
        ));
    }
    let mut encoder = GifEncoder::new(fs::File::create(output)?);
    encoder.set_repeat(if clip.looping {
        Repeat::Infinite
    } else {
        Repeat::Finite(0)
    })?;
    encoder.encode_frames(frames)?;
    if let Some(path) = native_sheet {
        sheet.save(path)?;
    }
    if let Some(path) = contact {
        image::imageops::resize(
            &sheet,
            sheet.width() * 4,
            sheet.height() * 4,
            FilterType::Nearest,
        )
        .save(path)?;
    }
    Ok(())
}
pub struct Pack {
    pub manifest: Manifest,
    pub clips: BTreeMap<String, Arc<Clip>>,
    pub encoded_bytes: usize,
}

pub fn parse_color(value: &str) -> Result<[u8; 3]> {
    ensure!(
        value.len() == 7 && value.starts_with('#') && value.is_ascii(),
        "background must be #RRGGBB"
    );
    Ok([
        u8::from_str_radix(&value[1..3], 16)?,
        u8::from_str_radix(&value[3..5], 16)?,
        u8::from_str_radix(&value[5..7], 16)?,
    ])
}

pub fn composite(image: &RgbaImage, background: [u8; 3]) -> image::RgbImage {
    image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let pixel = image.get_pixel(x, y).0;
        let alpha = pixel[3] as u32;
        image::Rgb(std::array::from_fn(|i| {
            ((pixel[i] as u32 * alpha + background[i] as u32 * (255 - alpha) + 127) / 255) as u8
        }))
    })
}

fn asset_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    ensure!(
        !path.is_absolute()
            && path
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
        "asset paths must stay inside the pack"
    );
    let resolved = root
        .join(path)
        .canonicalize()
        .with_context(|| format!("missing asset {relative}"))?;
    ensure!(resolved.starts_with(root), "asset symlink escapes the pack");
    ensure!(
        fs::metadata(&resolved)?.len() <= MAX_ASSET_BYTES,
        "asset exceeds 16 MiB"
    );
    Ok(resolved)
}

fn read_image(path: &Path) -> Result<RgbaImage> {
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?.to_rgba8())
}

pub struct SheetOptions<'a> {
    pub base: Option<&'a Path>,
    pub masks: &'a [String],
    pub first: Option<&'a Path>,
    pub last: Option<&'a Path>,
}

pub fn export_sheet(
    input: &Path,
    output: &Path,
    columns: u32,
    count: usize,
    options: SheetOptions<'_>,
) -> Result<()> {
    ensure!(
        (1..=40).contains(&count) && (1..=40).contains(&columns),
        "invalid frame grid"
    );
    ensure!(
        !output.exists(),
        "frame output directory exists; choose a new directory"
    );
    let rows = (count as u32).div_ceil(columns);
    let source = read_image(input)?;
    ensure!(
        (source.width() as i64 * rows as i64 - source.height() as i64 * columns as i64).abs()
            <= columns.max(rows) as i64,
        "source aspect ratio must match the declared frame grid"
    );
    let grid = image::imageops::resize(&source, columns * 64, rows * 64, FilterType::Nearest);
    let load_base = |path: &Path| -> Result<RgbaImage> {
        let image = read_image(path)?;
        ensure!(
            image.dimensions() == (64, 64),
            "canonical base must be exactly 64x64"
        );
        Ok(image)
    };
    let base = options.base.map(load_base).transpose()?;
    let first = options.first.map(load_base).transpose()?;
    let last = options.last.map(load_base).transpose()?;
    let masks: Vec<[u32; 4]> = options
        .masks
        .iter()
        .map(|value| -> Result<_> {
            let numbers = value
                .split(',')
                .map(str::parse::<u32>)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let rect: [u32; 4] = numbers
                .try_into()
                .map_err(|_| anyhow::anyhow!("mask must be x,y,width,height"))?;
            ensure!(
                rect[2] > 0
                    && rect[3] > 0
                    && rect[0].checked_add(rect[2]).is_some_and(|n| n <= 64)
                    && rect[1].checked_add(rect[3]).is_some_and(|n| n <= 64),
                "mask exceeds 64x64 frame"
            );
            Ok(rect)
        })
        .collect::<Result<_>>()?;
    ensure!(
        masks.is_empty() || base.is_some(),
        "masks require a canonical base"
    );
    fs::create_dir_all(output)?;
    for index in 0..count as u32 {
        let cell = image::imageops::crop_imm(
            &grid,
            (index % columns) * 64,
            (index / columns) * 64,
            64,
            64,
        )
        .to_image();
        let mut frame = if !masks.is_empty() {
            base.as_ref().unwrap().clone()
        } else {
            cell.clone()
        };
        for [x, y, width, height] in &masks {
            for py in *y..y + height {
                for px in *x..x + width {
                    frame.put_pixel(px, py, *cell.get_pixel(px, py));
                }
            }
        }
        if index == 0
            && let Some(first) = &first
        {
            frame = first.clone();
        }
        if index == count as u32 - 1
            && let Some(last) = &last
        {
            frame = last.clone();
        }
        composite(&frame, [0, 0, 0]).save(output.join(format!("{index:03}.png")))?;
    }
    Ok(())
}
fn encode(frame: &RgbaImage, background: [u8; 3]) -> Result<String> {
    ensure!(
        frame.dimensions() == (64, 64),
        "every frame must be exactly 64x64"
    );
    Ok(STANDARD.encode(composite(frame, background).as_raw()))
}

impl Pack {
    pub fn load(path: &Path, max_frames: usize) -> Result<Self> {
        ensure!((1..=40).contains(&max_frames), "max_frames must be 1..40");
        let root = path.canonicalize().context("pack directory not found")?;
        let manifest_path = root.join("pet.json");
        ensure!(
            fs::metadata(&manifest_path)?.len() <= 1024 * 1024,
            "manifest too large"
        );
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(manifest_path)?).context("parse pet.json")?;
        Self::from_manifest(&root, manifest, max_frames)
    }
    fn from_manifest(root: &Path, manifest: Manifest, max_frames: usize) -> Result<Self> {
        let root = root.canonicalize()?;
        ensure!(
            manifest.schema_version == 1 && manifest.canvas_size == 64,
            "require schema 1 and canvas_size 64"
        );
        ensure!(
            !manifest.id.is_empty() && manifest.id.len() <= 128,
            "invalid pet ID"
        );
        ensure!(
            manifest.animations.len() <= 64,
            "at most 64 clips are supported"
        );
        let background = parse_color(&manifest.background)?;
        let mut clips = BTreeMap::new();
        let mut encoded_bytes = 0;
        for (name, spec) in &manifest.animations {
            ensure!(
                !name.is_empty() && name.len() <= 128,
                "invalid animation name"
            );
            let mut frames = Vec::new();
            let mut inherited_ms = None;
            match &spec.source {
                Source::Image { path } => {
                    frames.push(encode(&read_image(&asset_path(&root, path)?)?, background)?)
                }
                Source::SpriteSheet {
                    path,
                    columns,
                    frame_count,
                } => {
                    ensure!(
                        *columns > 0
                            && *columns <= 40
                            && *frame_count > 0
                            && *frame_count <= max_frames,
                        "invalid sprite-sheet columns/frame_count for {name}"
                    );
                    let sheet = read_image(&asset_path(&root, path)?)?;
                    let rows = (*frame_count as u32).div_ceil(*columns);
                    ensure!(
                        sheet.dimensions() == (columns * 64, rows * 64),
                        "sprite-sheet dimensions don't match metadata for {name}"
                    );
                    for index in 0..*frame_count as u32 {
                        let frame = image::imageops::crop_imm(
                            &sheet,
                            (index % columns) * 64,
                            (index / columns) * 64,
                            64,
                            64,
                        )
                        .to_image();
                        frames.push(encode(&frame, background)?);
                    }
                }
                Source::Frames { paths } => {
                    ensure!(
                        !paths.is_empty() && paths.len() <= max_frames,
                        "invalid frame count for {name}"
                    );
                    for path in paths {
                        frames.push(encode(&read_image(&asset_path(&root, path)?)?, background)?);
                    }
                }
                Source::Gif { path } => {
                    let mut decoder = image::codecs::gif::GifDecoder::new(BufReader::new(
                        fs::File::open(asset_path(&root, path)?)?,
                    ))?;
                    ensure!(decoder.dimensions() == (64, 64), "GIF canvas must be 64x64");
                    let mut limits = image::Limits::default();
                    limits.max_alloc = Some(4 * 1024 * 1024);
                    decoder.set_limits(limits)?;
                    for frame in decoder.into_frames() {
                        ensure!(
                            frames.len() < max_frames,
                            "GIF exceeds configured frame limit"
                        );
                        let frame = frame?;
                        let (numerator, denominator) = frame.delay().numer_denom_ms();
                        let ms = (numerator as f64 / denominator as f64).round().max(1.) as u32;
                        if spec.frame_duration_ms.is_none() {
                            ensure!(
                                inherited_ms.is_none_or(|previous| previous == ms),
                                "variable GIF timing: explicitly set frame_duration_ms to normalize"
                            );
                            inherited_ms = Some(ms);
                        }
                        frames.push(encode(frame.buffer(), background)?);
                    }
                }
            }
            ensure!(!frames.is_empty(), "empty clip {name}");
            let frame_ms = spec.frame_duration_ms.or(inherited_ms).unwrap_or(83);
            ensure!(
                (10..=10_000).contains(&frame_ms),
                "frame duration must be 10..10000 ms"
            );
            encoded_bytes += frames.iter().map(String::len).sum::<usize>();
            ensure!(
                encoded_bytes <= MAX_PACK_BYTES,
                "prepared pack exceeds 16 MiB"
            );
            clips.insert(
                name.clone(),
                Arc::new(Clip {
                    name: name.clone(),
                    frames,
                    frame_ms,
                    looping: spec.r#loop,
                    entry: spec.entry.clone(),
                    variants: spec.variants.clone(),
                }),
            );
        }
        for clip in clips.values() {
            for target in clip.entry.iter().chain(clip.variants.values()) {
                ensure!(
                    clips.contains_key(target),
                    "clip {} references missing clip {target}",
                    clip.name
                );
            }
            if let Some(entry) = &clip.entry {
                ensure!(
                    !clips[entry].looping,
                    "entry clip {entry} must have loop=false"
                );
                ensure!(entry != &clip.name, "clip cannot be its own entry");
            }
        }
        if let Some(default) = &manifest.default_animation {
            ensure!(
                clips.contains_key(default),
                "default_animation references missing artwork"
            );
        }
        Ok(Self {
            manifest,
            clips,
            encoded_bytes,
        })
    }

    pub fn resolve(&self, name: &str, working: bool) -> Option<Arc<Clip>> {
        let clip = self.clips.get(name)?;
        let variant = if working { "working" } else { "idle" };
        Some(
            clip.variants
                .get(variant)
                .and_then(|name| self.clips.get(name))
                .unwrap_or(clip)
                .clone(),
        )
    }

    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({
            "id":self.manifest.id, "prepared_bytes":self.encoded_bytes,
            "clips":self.clips.values().map(|c| serde_json::json!({
                "name":c.name,"frames":c.frames.len(),"frame_ms":c.frame_ms,
                "loop":c.looping,"duration_ms":c.duration_ms()
            })).collect::<Vec<_>>()
        })
    }
}

pub fn prepare(
    input: &Path,
    output: &Path,
    preview: Option<&Path>,
    background: &str,
) -> Result<()> {
    ensure!(
        !output.exists(),
        "output exists; choose a new versioned filename"
    );
    if let Some(preview) = preview {
        ensure!(!preview.exists(), "preview exists");
    }
    let image = read_image(input)?;
    ensure!(
        image.width() == image.height(),
        "base image must be square; choose composition before export"
    );
    let small = image::imageops::resize(&image, 64, 64, FilterType::Nearest);
    let rgb = composite(&small, parse_color(background)?);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    rgb.save(output)?;
    if let Some(preview) = preview {
        if let Some(parent) = preview.parent() {
            fs::create_dir_all(parent)?;
        }
        image::imageops::resize(&rgb, 512, 512, FilterType::Nearest).save(preview)?;
    }
    Ok(())
}

pub fn import_directory(directory: &Path, id: &str, frame_ms: u32, force: bool) -> Result<()> {
    let output = directory.join("pet.json");
    ensure!(
        force || !output.exists(),
        "pet.json exists; use --force to replace it"
    );
    let mut animations = BTreeMap::new();
    let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() && matches!(entry.file_name().to_str(), Some("sources" | "previews")) {
            continue;
        }
        let name = if path.is_dir() {
            entry.file_name().to_string_lossy().into_owned()
        } else {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let source = if path.is_dir() {
            let mut paths = fs::read_dir(&path)?
                .collect::<std::io::Result<Vec<_>>>()?
                .into_iter()
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("png"))
                })
                .map(|e| format!("{name}/{}", e.file_name().to_string_lossy()))
                .collect::<Vec<_>>();
            paths.sort();
            if paths.is_empty() {
                continue;
            }
            Source::Frames { paths }
        } else {
            let file = entry.file_name().to_string_lossy().into_owned();
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            match extension.as_str() {
                "gif" => Source::Gif { path: file },
                "png" => {
                    let (width, height) = ImageReader::open(&path)?
                        .with_guessed_format()?
                        .into_dimensions()?;
                    if (width, height) == (64, 64) {
                        Source::Image { path: file }
                    } else {
                        ensure!(
                            width % 64 == 0 && height % 64 == 0,
                            "sprite grids must have dimensions divisible by 64"
                        );
                        Source::SpriteSheet {
                            path: file,
                            columns: width / 64,
                            frame_count: (width / 64 * height / 64) as usize,
                        }
                    }
                }
                _ => continue,
            }
        };
        let looping = !matches!(name.as_str(), "finished" | "interrupted" | "working-enter");
        let spec = ClipSpec {
            source,
            frame_duration_ms: Some(frame_ms),
            r#loop: looping,
            entry: None,
            variants: BTreeMap::new(),
        };
        ensure!(
            animations.insert(name.clone(), spec).is_none(),
            "duplicate clip {name}"
        );
    }
    if animations.contains_key("working-enter")
        && let Some(working) = animations.get_mut("working")
    {
        working.entry = Some("working-enter".into());
    }
    let manifest = Manifest {
        schema_version: 1,
        id: id.into(),
        canvas_size: 64,
        background: black(),
        default_animation: animations.contains_key("idle").then(|| "idle".into()),
        animations,
    };
    Pack::from_manifest(directory, manifest.clone(), 40)?;
    fs::write(output, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_is_composited_and_optional_clips_are_valid() {
        let root = tempfile::tempdir().unwrap();
        let frame = RgbaImage::from_pixel(64, 64, image::Rgba([200, 0, 0, 128]));
        frame.save(root.path().join("idle.png")).unwrap();
        fs::write(
            root.path().join("pet.json"),
            r##"{
            "schema_version":1,"id":"partial","canvas_size":64,"background":"#000000",
            "animations":{"idle":{"source":{"type":"image","path":"idle.png"}}}
        }"##,
        )
        .unwrap();
        let pack = Pack::load(root.path(), 40).unwrap();
        let raw = STANDARD.decode(&pack.clips["idle"].frames[0]).unwrap();
        assert_eq!(&raw[..3], &[100, 0, 0]);
        assert!(pack.resolve("working", true).is_none());
    }
    #[test]
    fn rejects_escape_and_wrong_dimensions() {
        let root = tempfile::tempdir().unwrap();
        let manifest = r#"{"schema_version":1,"id":"bad","canvas_size":64,
            "animations":{"idle":{"source":{"type":"image","path":"../secret.png"}}}}"#;
        fs::write(root.path().join("pet.json"), manifest).unwrap();
        assert!(Pack::load(root.path(), 40).is_err());
        RgbaImage::new(63, 64)
            .save(root.path().join("idle.png"))
            .unwrap();
        fs::write(
            root.path().join("pet.json"),
            manifest.replace("../secret.png", "idle.png"),
        )
        .unwrap();
        assert!(Pack::load(root.path(), 40).is_err());
    }
    #[test]
    fn directory_import_preserves_sources_and_refuses_accidental_overwrite() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("working")).unwrap();
        RgbaImage::new(64, 64)
            .save(root.path().join("idle.png"))
            .unwrap();
        RgbaImage::new(64, 64)
            .save(root.path().join("working/001.png"))
            .unwrap();
        import_directory(root.path(), "imported", 83, false).unwrap();
        assert_eq!(Pack::load(root.path(), 40).unwrap().clips.len(), 2);
        assert!(import_directory(root.path(), "other", 83, false).is_err());
        assert!(root.path().join("working/001.png").exists());
    }
    #[test]
    fn sheet_export_freezes_background_and_anchors_endpoints() {
        let root = tempfile::tempdir().unwrap();
        let base = RgbaImage::from_pixel(64, 64, image::Rgba([10, 20, 30, 255]));
        base.save(root.path().join("base.png")).unwrap();
        let source = RgbaImage::from_fn(128, 64, |x, _| {
            image::Rgba(if x < 64 {
                [200, 0, 0, 255]
            } else {
                [0, 200, 0, 255]
            })
        });
        source.save(root.path().join("sheet.png")).unwrap();
        let path = root.path().join("out");
        let base_path = root.path().join("base.png");
        export_sheet(
            &root.path().join("sheet.png"),
            &path,
            2,
            2,
            SheetOptions {
                base: Some(&base_path),
                masks: &["4,5,8,9".into()],
                first: None,
                last: Some(&base_path),
            },
        )
        .unwrap();
        let first = read_image(&path.join("000.png")).unwrap();
        assert_eq!(first.get_pixel(0, 0), base.get_pixel(0, 0));
        assert_eq!(first.get_pixel(4, 5).0, [200, 0, 0, 255]);
        assert_eq!(read_image(&path.join("001.png")).unwrap(), base);
        assert!(!path.join("002.png").exists());
    }
    #[test]
    fn gif_variable_timing_requires_deliberate_normalization() {
        use image::codecs::gif::GifEncoder;
        let root = tempfile::tempdir().unwrap();
        let mut encoder = GifEncoder::new(fs::File::create(root.path().join("idle.gif")).unwrap());
        for delay in [50, 100] {
            encoder
                .encode_frame(image::Frame::from_parts(
                    RgbaImage::from_pixel(64, 64, image::Rgba([10, 20, 30, 255])),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(delay, 1),
                ))
                .unwrap();
        }
        drop(encoder);
        let manifest = r#"{"schema_version":1,"id":"gif","canvas_size":64,
            "animations":{"idle":{"source":{"type":"gif","path":"idle.gif"}}}}"#;
        fs::write(root.path().join("pet.json"), manifest).unwrap();
        assert!(Pack::load(root.path(), 40).is_err());
        let mut manifest: serde_json::Value = serde_json::from_str(manifest).unwrap();
        manifest["animations"]["idle"]["frame_duration_ms"] = serde_json::json!(83);
        fs::write(
            root.path().join("pet.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let pack = Pack::load(root.path(), 40).unwrap();
        assert_eq!(pack.clips["idle"].frames.len(), 2);
        assert_eq!(pack.clips["idle"].frame_ms, 83);
    }
}
