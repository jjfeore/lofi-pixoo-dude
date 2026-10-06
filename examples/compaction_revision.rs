//! Offline compaction or attention overlays on approved decker frames.
//! Reuses accepted poses and masks; no network access or runtime changes.
use anyhow::{Context, Result, ensure};
use image::{Rgb, RgbImage, RgbaImage, imageops::FilterType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::{BTreeSet, HashMap}, fs, path::{Path, PathBuf}};

type Rect = [u32; 4];

#[derive(Deserialize)]
struct Spec {
    id: String,
    base: PathBuf,
    approved_pack: PathBuf,
    palette_from: PathBuf,
    frames: usize,
    frame_ms: u32,
    rear_area: Rect,
    traffic_mask: Vec<Rect>,
    cars: HashMap<String, PathBuf>,
    traffic: HashMap<String, Vec<Traffic>>,
    idle_colors: [[u8; 3]; 2],
    working_colors: [[u8; 3]; 2],
    attention: Option<Attention>,
}

#[derive(Deserialize)]
struct Attention {
    main_aperture: Vec<[u32; 3]>,
    visor_area: Rect,
    visor_pixels: Vec<[u32; 2]>,
    finger_pixels: Vec<[u32; 2]>,
    pause_typing: bool,
    amber: [u8; 3],
    #[serde(default)]
    symbol: AttentionSymbol,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum AttentionSymbol {
    #[default]
    Exclamation,
    WarningTriangle,
}

fn amber_pixel(pixel: Rgb<u8>, amber: [u8; 3], pulse: f32, floor: f32) -> Rgb<u8> {
    let light = (0.2126 * f32::from(pixel[0]) + 0.7152 * f32::from(pixel[1])
        + 0.0722 * f32::from(pixel[2])) / 255.0;
    let intensity = (floor + (1.0 - floor) * light) * pulse;
    Rgb(amber.map(|c| (f32::from(c) * intensity).round() as u8))
}

fn attention_allowed(spec: &Spec, attention: &Attention, x: u32, y: u32) -> bool {
    contains(spec.rear_area, x, y)
        || spec.traffic_mask.iter().any(|r| contains(*r, x, y))
        || attention.main_aperture.iter().any(|[px, top, bottom]| *px == x && y >= *top && y <= *bottom)
        || attention.visor_pixels.contains(&[x, y])
        || (attention.pause_typing && attention.finger_pixels.contains(&[x, y]))
}

fn draw_attention(frame: &mut RgbImage, base: &RgbImage, resting: &RgbImage,
    spec: &Spec, attention: &Attention, i: usize) -> f32 {
    let pulse = 0.82 + 0.18 * (std::f32::consts::TAU * i as f32 / spec.frames as f32).cos();
    for [x, top, bottom] in &attention.main_aperture {
        for y in *top..=*bottom {
            frame.put_pixel(*x, y, amber_pixel(*frame.get_pixel(*x, y), attention.amber, pulse, 0.06));
        }
    }
    for [x, y] in &attention.visor_pixels {
        frame.put_pixel(*x, *y, amber_pixel(*frame.get_pixel(*x, *y), attention.amber, pulse, 0.45));
    }
    if attention.pause_typing {
        for [x, y] in &attention.finger_pixels {
            frame.put_pixel(*x, *y, *resting.get_pixel(*x, *y));
        }
    }
    let [x, y, w, h] = spec.rear_area;
    for py in y..y + h { for px in x..x + w {
        frame.put_pixel(px, py, *base.get_pixel(px, py));
    }}
    let symbol = attention.amber.map(|c| (f32::from(c) * pulse).round() as u8);
    match attention.symbol {
        AttentionSymbol::Exclamation => {
            // A large steady exclamation mark, with a clear gap above its dot.
            for py in [1, 2, 3, 5] { for px in w / 2 - 1..=w / 2 + 1 {
                frame.put_pixel(x + px, y + py, Rgb(symbol));
            }}
        }
        AttentionSymbol::WarningTriangle => {
            // A filled nine-pixel warning triangle with dark punctuation.
            // A quiet highlight travels through its face without moving its outline.
            for (py, half_width) in [0, 1, 2, 2, 3, 4, 4].into_iter().enumerate() {
                for px in w / 2 - half_width..=w / 2 + half_width {
                    if px == w / 2 && [2, 3, 5].contains(&py) { continue; }
                    let wave = 0.5 + 0.5 * (std::f32::consts::TAU
                        * (px as f32 / (w - 1) as f32 - i as f32 / spec.frames as f32)).cos();
                    let shade = pulse * (0.85 + 0.15 * wave);
                    frame.put_pixel(x + px, y + py as u32,
                        Rgb(attention.amber.map(|c| (f32::from(c) * shade).round() as u8)));
                }
            }
        }
    }
    pulse
}

fn copy_optional(from: &Path, to: &Path) -> Result<()> {
    match fs::copy(from, to) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[derive(Deserialize)]
struct Traffic {
    sprite: String,
    lane: i32,
    start_x: i32,
    end_x: i32,
    passes: u32,
    phase: f32,
    mirror: bool,
}

#[derive(Serialize)]
struct Motion {
    frame: usize,
    phase: &'static str,
    row_width: u32,
    row_positions: [u32; 3],
    packet_x: i32,
    incoming_offset: i32,
}

fn contains([x, y, w, h]: Rect, px: u32, py: u32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn motion(i: usize, frames: usize, width: u32) -> Motion {
    let t = i as f32 / frames as f32;
    let compression = smoothstep((t - 0.25) / 0.25);
    let delivery = smoothstep((t - 0.625) / 0.1875);
    let incoming = smoothstep((t - 0.625) / 0.34375);
    let center = (width / 2) as i32;
    Motion {
        frame: i,
        phase: if t < 0.25 { "read" } else if t < 0.5 { "compress" }
            else if t < 0.625 { "packed" } else { "deliver-and-refill" },
        row_width: ((width - 2) as f32 * (1.0 - compression) + 3.0 * compression).round() as u32,
        row_positions: [1 + compression.round() as u32, 3, 5 - compression.round() as u32],
        packet_x: center - 1 + ((width + 2) as f32 * delivery).round() as i32,
        incoming_offset: (-((width + 1) as f32 * (1.0 - incoming))).round() as i32,
    }
}

fn put_clipped(frame: &mut RgbImage, area: Rect, x: i32, y: i32, color: [u8; 3]) {
    if (0..64).contains(&x) && (0..64).contains(&y) && contains(area, x as u32, y as u32) {
        frame.put_pixel(x as u32, y as u32, Rgb(color));
    }
}

fn draw_rows(frame: &mut RgbImage, area: Rect, positions: [u32; 3], width: u32,
    offset: i32, tick: usize, colors: [[u8; 3]; 2]) {
    let [x, y, w, _] = area;
    let left = (w - width) as i32 / 2 + offset;
    for (row, py) in positions.into_iter().enumerate() {
        for px in 0..width {
            // Break long rows into readable blocks; the packed rows form one tile.
            if width > 5 && (px as usize + tick + row) % 3 == 2 { continue; }
            let color = colors[usize::from(row == 1)];
            put_clipped(frame, area, x as i32 + left + px as i32, (y + py) as i32, color);
        }
    }
}

fn draw_compression(frame: &mut RgbImage, base: &RgbImage, spec: &Spec,
    record: &Motion, working: bool) {
    let [x, y, w, h] = spec.rear_area;
    for py in y..y + h { for px in x..x + w {
        frame.put_pixel(px, py, *base.get_pixel(px, py));
    }}
    let colors = if working { spec.working_colors } else { spec.idle_colors };
    let tick = record.frame / 2;
    if record.phase != "deliver-and-refill" {
        draw_rows(frame, spec.rear_area, record.row_positions, record.row_width, 0, tick, colors);
    } else {
        draw_rows(frame, spec.rear_area, [2, 3, 4], 3,
            record.packet_x - (w as i32 - 3) / 2, tick, colors);
        draw_rows(frame, spec.rear_area, [1, 3, 5], w - 2, record.incoming_offset, tick, colors);
    }
}

fn draw_traffic(frame: &mut RgbImage, base: &RgbImage, spec: &Spec,
    cars: &HashMap<String, RgbaImage>, name: &str, i: usize) -> Result<()> {
    for [x, y, w, h] in &spec.traffic_mask { for py in *y..y + h { for px in *x..x + w {
        frame.put_pixel(px, py, *base.get_pixel(px, py));
    }}}
    for t in spec.traffic.get(name).context("missing traffic profile")? {
        let sprite = cars.get(&t.sprite).context("unknown traffic sprite")?;
        let phase = (i as f32 * t.passes as f32 / spec.frames as f32 + t.phase).fract();
        let left = (t.start_x as f32 + (t.end_x - t.start_x) as f32 * phase).round() as i32;
        for (sx, sy, _) in sprite.enumerate_pixels() {
            let x = left + sx as i32;
            let y = t.lane + sy as i32;
            if !(0..64).contains(&x) || !(0..64).contains(&y)
                || !spec.traffic_mask.iter().any(|r| contains(*r, x as u32, y as u32)) { continue; }
            let sx = if t.mirror { sprite.width() - 1 - sx } else { sx };
            let pixel = sprite.get_pixel(sx, sy);
            let old = frame.get_pixel(x as u32, y as u32);
            let alpha = u32::from(pixel[3]);
            let mixed = Rgb(std::array::from_fn(|c| {
                ((u32::from(pixel[c]) * alpha + u32::from(old[c]) * (255 - alpha) + 127) / 255) as u8
            }));
            frame.put_pixel(x as u32, y as u32, mixed);
        }
    }
    Ok(())
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.into_iter().zip(b).map(|(x, y)| (i32::from(x) - i32::from(y)).pow(2) as u32).sum()
}

fn gif_export(path: &Path, frames: &[RgbImage], palette: &[u8], scale: u32, ms: u32) -> Result<()> {
    let size = (64 * scale) as u16;
    let mut encoder = gif::Encoder::new(fs::File::create(path)?, size, size, palette)?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let mut cache = HashMap::<[u8; 3], u8>::new();
    for (i, image) in frames.iter().enumerate() {
        let indices: Vec<u8> = image.pixels().map(|p| {
            *cache.entry(p.0).or_insert_with(|| {
                palette.as_chunks::<3>().0.iter().enumerate()
                    .min_by_key(|(_, c)| distance(p.0, **c)).unwrap().0 as u8
            })
        }).collect();
        let mut enlarged = Vec::with_capacity(usize::from(size).pow(2));
        for y in 0..64 { for _ in 0..scale { for x in 0..64 { for _ in 0..scale {
            enlarged.push(indices[y * 64 + x]);
        }}}}
        let mut frame = gif::Frame::from_indexed_pixels(size, size, enlarged, None);
        frame.delay = (((i as u32 + 1) * ms / 10) - (i as u32 * ms / 10)) as u16;
        frame.dispose = gif::DisposalMethod::Keep;
        encoder.write_frame(&frame)?;
    }
    Ok(())
}

fn export_clip(output: &Path, name: &str, frames: &[RgbImage], palette: &[u8], spec: &Spec) -> Result<()> {
    fs::create_dir_all(output.join(name))?;
    let mut sheet = RgbImage::new(256, (frames.len() as u32).div_ceil(4) * 64);
    let mut contact = RgbImage::new(512, (frames.len() as u32).div_ceil(8) * 64);
    let [x, y, w, h] = spec.rear_area;
    let mut rear = RgbImage::new(8 * (w + 4), (frames.len() as u32).div_ceil(8) * (h + 4));
    for (i, frame) in frames.iter().enumerate() {
        frame.save(output.join(name).join(format!("{i:03}.png")))?;
        image::imageops::replace(&mut sheet, frame, (i as i64 % 4) * 64, (i as i64 / 4) * 64);
        image::imageops::replace(&mut contact, frame, (i as i64 % 8) * 64, (i as i64 / 8) * 64);
        let crop = image::imageops::crop_imm(frame, x - 2, y - 2, w + 4, h + 4).to_image();
        image::imageops::replace(&mut rear, &crop,
            (i as i64 % 8) * i64::from(w + 4), (i as i64 / 8) * i64::from(h + 4));
    }
    sheet.save(output.join(format!("{name}-sprite.png")))?;
    image::imageops::resize(&frames[0], 512, 512, FilterType::Nearest)
        .save(output.join(format!("{name}-poster.png")))?;
    image::imageops::resize(&contact, 1024, contact.height() * 2, FilterType::Nearest)
        .save(output.join(format!("{name}-contact.png")))?;
    image::imageops::resize(&rear, rear.width() * 8, rear.height() * 8, FilterType::Nearest)
        .save(output.join(format!("{name}-rear-contact.png")))?;
    gif_export(&output.join(format!("{name}-native.gif")), frames, palette, 1, spec.frame_ms)?;
    gif_export(&output.join(format!("{name}-preview.gif")), frames, palette, 8, spec.frame_ms)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let plan = PathBuf::from(args.next().context("provide authoring JSON")?);
    let output = PathBuf::from(args.next().context("provide a new output directory")?);
    ensure!(args.next().is_none() && !output.exists(), "use a new output directory");
    let root = plan.parent().unwrap_or(Path::new("."));
    let spec: Spec = serde_json::from_str(&fs::read_to_string(&plan)?)?;
    ensure!(spec.frames == 32 && spec.frame_ms == 83, "approved loops require 32 frames at 83ms");
    ensure!(spec.rear_area[2..] == [11, 7] && spec.rear_area[0] >= 2 && spec.rear_area[1] >= 2
        && spec.rear_area[0] <= 51 && spec.rear_area[1] <= 55,
        "this compression layout requires an 11x7 rear screen with crop margin");
    for [x, y, w, h] in spec.traffic_mask.iter().chain(std::iter::once(&spec.rear_area)) {
        ensure!(*w > 0 && *h > 0 && x.checked_add(*w).is_some_and(|v| v <= 64)
            && y.checked_add(*h).is_some_and(|v| v <= 64), "overlay outside canvas");
    }
    for traffic in spec.traffic.values().flatten() {
        ensure!((1..=4).contains(&traffic.passes) && (0.0..1.0).contains(&traffic.phase),
            "invalid traffic timing");
    }
    let approved = root.join(&spec.approved_pack);
    let base = image::open(root.join(&spec.base))?.to_rgb8();
    ensure!(base.dimensions() == (64, 64), "base must be native 64x64");
    let cars: HashMap<_, _> = spec.cars.iter().map(|(name, path)| -> Result<_> {
        let car = image::open(root.join(path))?.to_rgba8();
        ensure!(car.width() <= 8 && car.height() <= 4 && car.pixels().any(|p| p[3] < 255),
            "use small transparent approved traffic sprites");
        Ok((name.clone(), car))
    }).collect::<Result<_>>()?;
    let decoder = gif::DecodeOptions::new().read_info(fs::File::open(root.join(&spec.palette_from))?)?;
    let palette = decoder.global_palette().context("reference GIF has no global palette")?.to_vec();
    fs::create_dir_all(&output)?;
    let mut manifest: serde_json::Value = serde_json::from_str(&fs::read_to_string(approved.join("pet.json"))?)?;
    manifest["id"] = json!(spec.id);
    // Logical variants may share a sheet. Copy each physical clip once.
    let names: BTreeSet<_> = manifest["animations"].as_object().context("missing animations")?
        .values().map(|clip| -> Result<String> {
            let path = clip["source"]["path"].as_str().context("missing sprite path")?;
            Ok(path.strip_suffix("-sprite.png").context("approved pack must use named sprite sheets")?.into())
        }).collect::<Result<_>>()?;
    for name in names {
        fs::create_dir_all(output.join(&name))?;
        for i in 0..spec.frames {
            let file = format!("{i:03}.png");
            fs::copy(approved.join(&name).join(&file), output.join(&name).join(file))?;
        }
        for suffix in ["sprite.png", "poster.png", "contact.png", "native.gif", "preview.gif"] {
            let file = format!("{name}-{suffix}");
            fs::copy(approved.join(&file), output.join(file))?;
        }
        for suffix in ["heads.png", "rear-contact.png", "gesture-poster.png"] {
            let file = format!("{name}-{suffix}");
            copy_optional(&approved.join(&file), &output.join(file))?;
        }
    }
    for file in ["motorized-motion.json", "compression-motion.json", "work-cycle-preview.gif", "compaction-cycle-preview.gif"] {
        copy_optional(&approved.join(file), &output.join(file))?;
    }
    base.save(output.join("base.png"))?;
    if let Some(attention) = &spec.attention {
        ensure!(attention.main_aperture.len() == 9
            && attention.main_aperture.iter().map(|c| c[0]).collect::<BTreeSet<_>>().len() == 9
            && attention.main_aperture.iter().all(|[x, top, bottom]| *x < 64 && *top <= *bottom && *bottom < 64),
            "invalid attention monitor aperture");
        let [vx, vy, vw, vh] = attention.visor_area;
        ensure!(vw > 0 && vh > 0 && vx.checked_add(vw).is_some_and(|v| v <= 64)
            && vy.checked_add(vh).is_some_and(|v| v <= 64)
            && !attention.visor_pixels.is_empty()
            && attention.visor_pixels.iter().all(|[x, y]| contains(attention.visor_area, *x, *y)),
            "invalid attention visor mask");
        ensure!(attention.finger_pixels.iter().all(|[x, y]| *x < 64 && *y < 64), "invalid fingertip mask");
        let resting = image::open(approved.join("working/000.png"))?.to_rgb8();
        let mut frames = Vec::new();
        let mut records = Vec::new();
        for i in 0..spec.frames {
            let original = image::open(approved.join("working").join(format!("{i:03}.png")))?.to_rgb8();
            ensure!(original.dimensions() == (64, 64), "approved frame must be native 64x64");
            let mut frame = original.clone();
            let pulse = draw_attention(&mut frame, &base, &resting, &spec, attention, i);
            draw_traffic(&mut frame, &base, &spec, &cars, "needs-input", i)?;
            for (x, y, p) in frame.enumerate_pixels() {
                ensure!(attention_allowed(&spec, attention, x, y) || p == original.get_pixel(x, y),
                    "approved attention pixel changed at {x},{y}");
            }
            let mut record = json!({"frame":i,"pulse":pulse,"visor":"down","typing_paused":attention.pause_typing});
            if matches!(attention.symbol, AttentionSymbol::WarningTriangle) {
                record["rear_symbol"] = json!(attention.symbol);
                record["rear_highlight_phase"] = json!(i as f32 / spec.frames as f32);
            }
            records.push(record);
            frames.push(frame);
        }
        // Fit the new amber shades once, so every preview frame shares its palette.
        let mut palette_input = RgbImage::new(256, 512);
        for (i, frame) in frames.iter().enumerate() {
            image::imageops::replace(&mut palette_input, frame, (i as i64 % 4) * 64, (i as i64 / 4) * 64);
        }
        let palette = gif::Frame::from_rgb_speed(256, 512, palette_input.as_raw(), 1).palette.unwrap();
        export_clip(&output, "needs-input", &frames, &palette, &spec)?;
        manifest["animations"]["needs-input"] = json!({"source":{"type":"sprite_sheet",
            "path":"needs-input-sprite.png","columns":4,"frame_count":spec.frames},
            "frame_duration_ms":spec.frame_ms,"loop":true});
        fs::write(output.join("pet.json"), serde_json::to_vec_pretty(&manifest)?)?;
        fs::write(output.join("attention-motion.json"), serde_json::to_vec_pretty(&records)?)?;
        println!("Prepared {} needs-input frames and copied all approved clips into {}", spec.frames, output.display());
        return Ok(());
    }
    let records: Vec<_> = (0..spec.frames).map(|i| motion(i, spec.frames, spec.rear_area[2])).collect();
    let mut clips = HashMap::new();
    for (name, source, working) in [("compacting-idle", "idle", false), ("compacting-working", "working", true)] {
        let mut frames = Vec::new();
        for record in &records {
            let original = image::open(approved.join(source).join(format!("{:03}.png", record.frame)))?.to_rgb8();
            ensure!(original.dimensions() == (64, 64), "approved frame must be 64x64");
            let mut frame = original.clone();
            draw_compression(&mut frame, &base, &spec, record, working);
            draw_traffic(&mut frame, &base, &spec, &cars, name, record.frame)?;
            for (x, y, p) in frame.enumerate_pixels() {
                ensure!(contains(spec.rear_area, x, y) || spec.traffic_mask.iter().any(|r| contains(*r, x, y))
                    || p == original.get_pixel(x, y), "approved pixel changed at {x},{y}");
            }
            frames.push(frame);
        }
        export_clip(&output, name, &frames, &palette, &spec)?;
        manifest["animations"][name] = json!({"source":{"type":"sprite_sheet",
            "path":format!("{name}-sprite.png"),"columns":4,"frame_count":spec.frames},
            "frame_duration_ms":spec.frame_ms,"loop":true});
        clips.insert(name, frames);
    }
    manifest["animations"]["compacting"] = manifest["animations"]["compacting-idle"].clone();
    manifest["animations"]["compacting"]["variants"] = json!({"idle":"compacting-idle","working":"compacting-working"});
    fs::write(output.join("pet.json"), serde_json::to_vec_pretty(&manifest)?)?;
    fs::write(output.join("compression-motion.json"), serde_json::to_vec_pretty(&records)?)?;
    let cycle: Vec<_> = clips["compacting-idle"].iter().chain(clips["compacting-working"].iter()).cloned().collect();
    gif_export(&output.join("compaction-cycle-preview.gif"), &cycle, &palette, 8, spec.frame_ms)?;
    println!("Prepared two {}-frame compaction variants plus four unchanged approved clips in {}", spec.frames, output.display());
    Ok(())
}
