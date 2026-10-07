//! Offline idle vignettes for owner review. No device or bridge access.
//! Generated raster sources are projected once onto the approved scene.
use anyhow::{Context, Result, ensure};
use image::{ImageEncoder, Rgb, RgbImage, RgbaImage, imageops::FilterType};
use serde::Deserialize;
use serde_json::json;
use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}};

type Rect = [u32; 4];

#[derive(Deserialize)]
struct Spec {
    id: String,
    approved_pack: PathBuf,
    base: PathBuf,
    car: PathBuf,
    expressions: PathBuf,
    mouth_poses: PathBuf,
    frames: usize,
    frame_ms: u32,
    car_size: [u32; 2],
    car_clip: Vec<Rect>,
    rear_area: Rect,
    eye_area: Rect,
    mouth_area: Rect,
    lamp_area: Rect,
    building_areas: Vec<Rect>,
    main_aperture: Vec<[u32; 3]>,
    clips: Vec<Clip>,
    refinement: Option<Refinement>,
}

#[derive(Deserialize)]
struct Refinement {
    #[serde(default)]
    envelope_offset: [i32; 2],
    preview_palettes: Option<PathBuf>,
    flyby_rows: Vec<[u32; 3]>,
    traffic_clear: Vec<Rect>,
    traffic_rows: Vec<[u32; 3]>,
    cars: HashMap<String, PathBuf>,
    traffic: HashMap<String, Vec<Traffic>>,
    floor_pixels: Vec<Vec<[u32; 2]>>,
    elevator_path: Vec<[u32; 2]>,
    elevator_visible: Vec<[u32; 2]>,
}

#[derive(Deserialize)]
struct Traffic {
    sprite: String,
    lane: i32,
    start_x: i32,
    end_x: i32,
    start_frame: usize,
    end_frame: usize,
    mirror: bool,
}

#[derive(Deserialize)]
struct Clip {
    id: String,
    name: String,
    differences: Vec<String>,
}

fn contains([x, y, w, h]: Rect, px: u32, py: u32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn difference(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.into_iter().zip(b).map(|(a, b)| (i32::from(a) - i32::from(b)).pow(2) as u32).sum()
}

fn blend(a: Rgb<u8>, b: Rgb<u8>, amount: f32) -> Rgb<u8> {
    Rgb(std::array::from_fn(|c| {
        (f32::from(a[c]) * (1.0 - amount) + f32::from(b[c]) * amount).round().clamp(0.0, 255.0) as u8
    }))
}

fn envelope(i: usize, start: usize, full: usize, fade: usize, end: usize) -> f32 {
    if i < start || i >= end { 0.0 }
    else if i < full { (i - start) as f32 / (full - start) as f32 }
    else if i <= fade { 1.0 }
    else { (end - i) as f32 / (end - fade) as f32 }
}

fn sprite(path: &Path, [w, h]: [u32; 2]) -> Result<RgbaImage> {
    let source = image::open(path)?.to_rgba8();
    ensure!(source.pixels().any(|p| p[3] == 0), "car must have true transparency");
    let mut bounds = [source.width(), source.height(), 0, 0];
    for (x, y, p) in source.enumerate_pixels() {
        if p[3] >= 200 {
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + 1);
            bounds[3] = bounds[3].max(y + 1);
        }
    }
    ensure!(bounds[2] > bounds[0] && bounds[3] > bounds[1], "empty car sprite");
    let cropped = image::imageops::crop_imm(&source, bounds[0], bounds[1], bounds[2] - bounds[0], bounds[3] - bounds[1]).to_image();
    Ok(image::imageops::resize(&cropped, w, h, FilterType::Nearest))
}

fn draw_car(frame: &mut RgbImage, car: &RgbaImage, clips: &[Rect], x: i32, y: i32) {
    for (sx, sy, p) in car.enumerate_pixels() {
        let dx = x + sx as i32;
        let dy = y + sy as i32;
        if dx < 0 || dy < 0 || !clips.iter().any(|r| contains(*r, dx as u32, dy as u32)) { continue; }
        let old = *frame.get_pixel(dx as u32, dy as u32);
        frame.put_pixel(dx as u32, dy as u32, blend(old, Rgb([p[0], p[1], p[2]]), f32::from(p[3]) / 255.0));
    }
}

fn in_rows(rows: &[[u32; 3]], x: u32, y: u32) -> bool {
    rows.iter().any(|[py, left, right]| y == *py && x >= *left && x <= *right)
}

fn draw_masked_car(frame: &mut RgbImage, car: &RgbaImage, rows: &[[u32; 3]], x: i32, y: i32, mirror: bool) -> Vec<[u32; 2]> {
    let mut visible = Vec::new();
    for (sx, sy, _) in car.enumerate_pixels() {
        let dx = x + sx as i32;
        let dy = y + sy as i32;
        if !(0..64).contains(&dx) || !(0..64).contains(&dy)
            || !in_rows(rows, dx as u32, dy as u32) { continue; }
        let pixel = car.get_pixel(if mirror { car.width() - 1 - sx } else { sx }, sy);
        if pixel[3] == 0 { continue; }
        let old = *frame.get_pixel(dx as u32, dy as u32);
        frame.put_pixel(dx as u32, dy as u32, blend(old, Rgb([pixel[0], pixel[1], pixel[2]]), f32::from(pixel[3]) / 255.0));
        visible.push([dx as u32, dy as u32]);
    }
    visible
}

fn alternate_traffic(frame: &mut RgbImage, base: &RgbImage, refinement: &Refinement,
    cars: &HashMap<String, RgbaImage>, name: &str, i: usize) -> Result<Vec<serde_json::Value>> {
    // Erase only the accepted traffic layers, using the original clean scene.
    // Vehicles follow independent one-pass paths; none wrap across the glass.
    for [x, y, w, h] in &refinement.traffic_clear {
        for py in *y..y + h { for px in *x..x + w {
            frame.put_pixel(px, py, *base.get_pixel(px, py));
        }}
    }
    let mut records = Vec::new();
    for vehicle in refinement.traffic.get(name).context("missing alternate traffic")? {
        let t = (i.saturating_sub(vehicle.start_frame) as f32
            / (vehicle.end_frame - vehicle.start_frame) as f32).clamp(0.0, 1.0);
        let x = (vehicle.start_x as f32 + (vehicle.end_x - vehicle.start_x) as f32 * t).round() as i32;
        let car = cars.get(&vehicle.sprite).context("unknown traffic sprite")?;
        let pixels = draw_masked_car(frame, car, &refinement.traffic_rows, x, vehicle.lane, vehicle.mirror);
        records.push(json!({"sprite":vehicle.sprite,"x":x,"lane":vehicle.lane,"pixels":pixels}));
    }
    Ok(records)
}

fn expression_cells(path: &Path, base: &RgbImage) -> Result<(Vec<RgbImage>, [i32; 2])> {
    let source = image::open(path)?.to_rgb8();
    ensure!(source.width() == source.height() && source.width() % 2 == 0, "four equal expression cells required");
    let edge = source.width() / 2;
    let cells: Vec<_> = (0..4).map(|i| {
        let cell = image::imageops::crop_imm(&source, (i % 2) * edge, (i / 2) * edge, edge, edge).to_image();
        image::imageops::resize(&cell, 64, 64, FilterType::Nearest)
    }).collect();
    // Register once using the neutral cheek, ear and forehead. The same offset
    // applies to all poses, so expression motion cannot translate the head.
    let mut best = (u64::MAX, [0, 0]);
    for dy in -2..=2 {
        for dx in -2..=2 {
            let mut score = 0u64;
            for y in 24..33 {
                for x in 15..24 {
                    let p = *base.get_pixel(x, y);
                    let q = *cells[0].get_pixel((x as i32 + dx) as u32, (y as i32 + dy) as u32);
                    score += u64::from(difference(p.0, q.0));
                }
            }
            if score < best.0 { best = (score, [dx, dy]); }
        }
    }
    Ok((cells, best.1))
}

fn draw_expression(frame: &mut RgbImage, cells: &[RgbImage], mouths: &[RgbImage], offset: [i32; 2], stage: usize, spec: &Spec) {
    if stage == 0 { return; }
    let [x, y, w, h] = spec.eye_area;
    for py in y..y + h {
        for px in x..x + w {
            // The generated eyelid sits one row below the neutral iris.
            // Register that feature locally without moving any head pixels.
            let sx = (px as i32 + offset[0]) as u32;
            let sy = (py as i32 + offset[1] + 1) as u32;
            let closed = *cells[if stage == 1 { 1 } else { 2 }].get_pixel(sx, sy);
            let old = *frame.get_pixel(px, py);
            frame.put_pixel(px, py, blend(old, closed, if stage == 1 { 0.65 } else { 1.0 }));
        }
    }
    // Reuse the accepted lower-face raster poses. Their neutral patch is
    // byte-identical to this idle face; they contain the reviewed one/two-pixel
    // chin drop, with no horizontal jaw movement or changed cheek silhouette.
    let [x, y, w, h] = spec.mouth_area;
    for py in y..y + h {
        for px in x..x + w {
            if mouths[stage].get_pixel(px, py) != mouths[0].get_pixel(px, py) {
                frame.put_pixel(px, py, *mouths[stage].get_pixel(px, py));
            }
        }
    }
}

fn tint_lamp(frame: &mut RgbImage, base: &RgbImage, area: Rect, color: [u8; 3], amount: f32, pulse: f32) {
    let [x, y, w, h] = area;
    for py in y..y + h {
        for px in x..x + w {
            let old = *base.get_pixel(px, py);
            let [r, g, b] = old.0.map(f32::from);
            // Recolor only the actual violet luminous pixels. Dark lamp
            // housing, nearby hair and foreground silhouettes remain fixed.
            if b > 50.0 && b > g * 1.7 && r > g * 1.15 {
                let level = r.max(b) / 255.0 * pulse;
                let target = Rgb(color.map(|c| (f32::from(c) * level).round().min(255.0) as u8));
                frame.put_pixel(px, py, blend(old, target, amount));
            }
        }
    }
}

fn rainbow(t: f32) -> [u8; 3] {
    let colors = [[214, 74, 255], [92, 155, 255], [58, 242, 255], [87, 245, 122], [255, 212, 65], [255, 104, 88], [214, 74, 255]];
    let p = t.clamp(0.0, 1.0) * 6.0;
    let i = (p.floor() as usize).min(5);
    blend(Rgb(colors[i]), Rgb(colors[i + 1]), p - i as f32).0
}

fn sweep(frame: &mut RgbImage, area: Rect, i: usize) {
    let amount = envelope(i, 3, 6, 30, 36);
    if amount == 0.0 { return; }
    let [x, y, w, h] = area;
    let head = ((i.saturating_sub(5)) * (w as usize + 3) / 28) as i32;
    for py in y..y + h {
        for px in x..x + w {
            let trail = head - (px - x) as i32;
            if (0..=2).contains(&trail) {
                let old = *frame.get_pixel(px, py);
                frame.put_pixel(px, py, blend(old, Rgb([90, 244, 250]), amount * (0.7 - trail as f32 * 0.2)));
            }
        }
    }
}

fn notification(frame: &mut RgbImage, spec: &Spec, i: usize) {
    let strength = envelope(i, 3, 8, 29, 36);
    if strength == 0.0 { return; }
    let [x, y, w, h] = spec.rear_area;
    let slide = if i < 9 { (9 - i) as i32 } else if i > 29 { -((i - 29) as i32) } else { 0 };
    for py in y..y + h {
        for px in x..x + w {
            let revised = spec.refinement.is_some();
            let margin = if revised { 0 } else { 1 };
            let offset = spec.refinement.as_ref().map_or([0, 0], |r| r.envelope_offset);
            let xx = px as i32 - x as i32 - margin - slide - offset[0];
            let yy = py as i32 - y as i32 - margin - offset[1];
            let outline = if revised {
                // Ten by six: the freed top/right space makes the flap legible.
                (0..10).contains(&xx) && (0..6).contains(&yy)
                    && (yy == 0 || yy == 5 || xx == 0 || xx == 9
                        || (yy <= 4 && (xx == yy || xx == 9 - yy)))
            } else {
                (0..9).contains(&xx) && (0..5).contains(&yy)
                    && (yy == 0 || yy == 4 || xx == 0 || xx == 8 || (yy <= 2 && (xx == yy * 2 || xx == 8 - yy * 2)))
            };
            let badge = !revised && px == x + w - 1 && py == y;
            let color = if badge { [255, 206, 98] } else if outline { [116, 250, 202] } else { [5, 18, 28] };
            let old = *frame.get_pixel(px, py);
            frame.put_pixel(px, py, blend(old, Rgb(color), strength));
        }
    }
    let strength = envelope(i, 7, 11, 27, 35);
    if strength == 0.0 { return; }
    // Content follows every column of the full angled glass, never the bezel.
    for (column, [x, top, bottom]) in spec.main_aperture.iter().enumerate() {
        for y in *top..=*bottom {
            let row = (y - top + i.saturating_sub(10) as u32 / 2) % (bottom - top + 1);
            let color = if row < 3 { [111, 238, 190] }
                else if row % 5 == 0 && column < 7 { [50, 170, 207] }
                else if row % 5 == 1 && column < 4 { [50, 129, 182] }
                else { [5, 24, 49] };
            let old = *frame.get_pixel(*x, y);
            frame.put_pixel(*x, y, blend(old, Rgb(color), strength));
        }
    }
}

fn city(frame: &mut RgbImage, spec: &Spec, i: usize) {
    if let Some(refinement) = &spec.refinement {
        if (6..=34).contains(&i) {
            // The path continues behind the nearer building. Only the exposed
            // tower shaft can show the single lit elevator pixel.
            let step = (i - 6) * (refinement.elevator_path.len() - 1) / 28;
            let [x, y] = refinement.elevator_path[step];
            if refinement.elevator_visible.contains(&[x, y]) {
                let amount = envelope(i, 4, 6, 24, 28);
                let old = *frame.get_pixel(x, y);
                frame.put_pixel(x, y, blend(old, Rgb([255, 207, 105]), amount));
            }
        }
        for (j, floor) in refinement.floor_pixels.iter().enumerate() {
            let amount = envelope(i, 7 + j * 3, 10 + j * 3, 21 + j * 2, 31 + j * 2);
            for (column, [x, y]) in floor.iter().enumerate() {
                let old = *frame.get_pixel(*x, *y);
                let color = if column == 0 { [255, 188, 99] } else { [181, 115, 64] };
                frame.put_pixel(*x, *y, blend(old, Rgb(color), amount * 0.9));
            }
        }
        return;
    }
    let amount = envelope(i, 3, 7, 29, 36);
    if amount == 0.0 { return; }
    let [x, top, w, height] = spec.building_areas[0];
    let elevator_y = top + (i.saturating_sub(6) as u32 * (height + 2) / 26);
    for py in top..top + height {
        for px in x..x + w {
            if py >= elevator_y && py < elevator_y + 2 {
                let old = *frame.get_pixel(px, py);
                frame.put_pixel(px, py, blend(old, Rgb([255, 207, 105]), amount));
            }
        }
    }
    for (j, [x, y, w, h]) in spec.building_areas.iter().copied().enumerate().skip(1) {
        let on = envelope(i, 5 + j * 3, 7 + j * 3, 20 + j * 2, 29 + j * 2);
        for py in y..y + h {
            for px in x..x + w {
                let old = *frame.get_pixel(px, py);
                let color = if px % 2 == 0 { [255, 188, 99] } else { [160, 97, 55] };
                frame.put_pixel(px, py, blend(old, Rgb(color), on * 0.9));
            }
        }
    }
}

fn allowed(spec: &Spec, name: &str, x: u32, y: u32) -> bool {
    if let Some(refinement) = &spec.refinement {
        if refinement.traffic_clear.iter().any(|r| contains(*r, x, y)) { return true; }
        return match name {
            "idle-flyby" => in_rows(&refinement.flyby_rows, x, y) || contains(spec.rear_area, x, y),
            "idle-yawn" => [spec.eye_area, spec.mouth_area, spec.lamp_area].iter().any(|r| contains(*r, x, y)),
            "idle-message" => contains(spec.rear_area, x, y) || spec.main_aperture.iter().any(|[px, top, bottom]| x == *px && y >= *top && y <= *bottom),
            "idle-city" => contains(spec.lamp_area, x, y)
                || refinement.floor_pixels.iter().flatten().any(|p| *p == [x, y])
                || refinement.elevator_visible.contains(&[x, y]),
            _ => false,
        };
    }
    match name {
        "idle-flyby" => spec.car_clip.iter().any(|r| contains(*r, x, y)) || contains(spec.rear_area, x, y),
        "idle-yawn" => [spec.eye_area, spec.mouth_area, spec.lamp_area].iter().any(|r| contains(*r, x, y)),
        "idle-message" => contains(spec.rear_area, x, y) || spec.main_aperture.iter().any(|[px, top, bottom]| x == *px && y >= *top && y <= *bottom),
        "idle-city" => contains(spec.lamp_area, x, y) || spec.building_areas.iter().any(|r| contains(*r, x, y)),
        _ => false,
    }
}

fn palette(frames: &[RgbImage]) -> Vec<u8> {
    let bytes: Vec<u8> = frames.iter().flat_map(|f| f.as_raw().iter().copied()).collect();
    gif::Frame::from_rgb_speed(64, (frames.len() * 64) as u16, &bytes, 1).palette.unwrap()
}

fn write_gif(path: &Path, frames: &[RgbImage], colors: &[u8], scale: u32, frame_ms: u32) -> Result<()> {
    let (w, h) = frames[0].dimensions();
    let mut encoder = gif::Encoder::new(fs::File::create(path)?, (w * scale) as u16, (h * scale) as u16, colors)?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let mut cache = HashMap::<[u8; 3], u8>::new();
    for (i, image) in frames.iter().enumerate() {
        let indices: Vec<u8> = image.pixels().map(|p| {
            *cache.entry(p.0).or_insert_with(|| colors.as_chunks::<3>().0.iter().enumerate()
                .min_by_key(|(_, c)| difference(p.0, [c[0], c[1], c[2]])).unwrap().0 as u8)
        }).collect();
        let mut frame = gif::Frame {
            width: (w * scale) as u16,
            height: (h * scale) as u16,
            // GIF uses centiseconds. Quantize cumulative timestamps so an
            // 83-ms clip alternates 80/90-ms delays without accumulating drift.
            delay: (((i as u32 + 1) * frame_ms / 10) - (i as u32 * frame_ms / 10)) as u16,
            dispose: gif::DisposalMethod::Keep,
            ..Default::default()
        };
        frame.buffer = (0..h * scale).flat_map(|y| {
            let indices = &indices;
            (0..w * scale).map(move |x| indices[((y / scale) * w + x / scale) as usize])
        }).collect::<Vec<_>>().into();
        encoder.write_frame(&frame)?;
    }
    Ok(())
}

fn export_clip(output: &Path, name: &str, frames: &[RgbImage], frame_ms: u32, palette_override: Option<&Vec<u8>>) -> Result<()> {
    fs::create_dir_all(output.join(name))?;
    let mut sheet = RgbImage::new(256, (frames.len() as u32).div_ceil(4) * 64);
    let mut contact = RgbImage::new(8 * 64, (frames.len() as u32).div_ceil(8) * 64);
    for (i, frame) in frames.iter().enumerate() {
        frame.save(output.join(name).join(format!("{i:03}.png")))?;
        image::imageops::replace(&mut sheet, frame, (i as i64 % 4) * 64, (i as i64 / 4) * 64);
        image::imageops::replace(&mut contact, frame, (i as i64 % 8) * 64, (i as i64 / 8) * 64);
    }
    image::codecs::png::PngEncoder::new_with_quality(
        fs::File::create(output.join(format!("{name}-sprite.png")))?,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    ).write_image(sheet.as_raw(), sheet.width(), sheet.height(), image::ExtendedColorType::Rgb8)?;
    image::imageops::resize(&contact, contact.width() * 2, contact.height() * 2, FilterType::Nearest)
        .save(output.join(format!("{name}-contact.png")))?;
    image::imageops::resize(&frames[20], 512, 512, FilterType::Nearest)
        .save(output.join(format!("{name}-poster.png")))?;
    let colors = palette_override.cloned().unwrap_or_else(|| palette(frames));
    write_gif(&output.join(format!("{name}-native.gif")), frames, &colors, 1, frame_ms)?;
    write_gif(&output.join(format!("{name}-preview.gif")), frames, &colors, 8, frame_ms)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let plan = PathBuf::from(args.next().context("provide authoring JSON")?);
    let output = PathBuf::from(args.next().context("provide new output directory")?);
    ensure!(args.next().is_none() && !output.exists(), "choose a new output directory");
    let spec: Spec = serde_json::from_str(&fs::read_to_string(&plan)?)?;
    ensure!(spec.frames == 40 && [83, 100].contains(&spec.frame_ms) && spec.clips.len() == 4, "expected four forty-frame vignettes");
    let root = plan.parent().unwrap_or(Path::new("."));
    // A placement-only revision retains the accepted preview palette so
    // quantization cannot recolor unrelated background or character pixels.
    let preview_palettes: HashMap<String, Vec<u8>> = if let Some(path) = spec.refinement.as_ref().and_then(|r| r.preview_palettes.as_ref()) {
        serde_json::from_str(&fs::read_to_string(root.join(path))?)?
    } else { HashMap::new() };
    ensure!(preview_palettes.values().all(|p| p.len() == 768), "256-color preview palettes required");
    let approved = root.join(&spec.approved_pack);
    let base = image::open(root.join(&spec.base))?.to_rgb8();
    let idle: Vec<RgbImage> = (0..32).map(|i| image::open(approved.join("idle").join(format!("{i:03}.png"))).map(|im| im.to_rgb8())).collect::<std::result::Result<_, _>>()?;
    ensure!(base.dimensions() == (64, 64) && idle.iter().all(|f| f.dimensions() == (64, 64)), "native source required");
    let car = image::imageops::flip_horizontal(&sprite(&root.join(&spec.car), spec.car_size)?);
    let traffic_cars: HashMap<String, RgbaImage> = if let Some(refinement) = &spec.refinement {
        ensure!(spec.frame_ms == 83, "revised alternates must match the accepted frame clock");
        ensure!(spec.lamp_area == [4, 15, 2, 10], "lamp must stay on the two-pixel bar");
        for vehicle in refinement.traffic.values().flatten() {
            ensure!(vehicle.start_frame < vehicle.end_frame && vehicle.end_frame < spec.frames,
                "invalid one-pass traffic timing");
        }
        ensure!(!refinement.elevator_path.is_empty() && refinement.floor_pixels.len() == 3,
            "tower geometry required");
        refinement.cars.iter().map(|(name, path)| -> Result<_> {
            let sprite = image::open(root.join(path))?.to_rgba8();
            ensure!(sprite.width() <= 4 && sprite.height() <= 2, "approved small traffic sprite required");
            Ok((name.clone(), sprite))
        }).collect::<Result<_>>()?
    } else { HashMap::new() };
    let (cells, offset) = expression_cells(&root.join(&spec.expressions), &idle[0])?;
    let mouths: Vec<RgbImage> = (0..4).map(|i| image::open(root.join(&spec.mouth_poses).join(format!("{i:03}.png"))).map(|im| im.to_rgb8())).collect::<std::result::Result<_, _>>()?;
    let [mx, my, mw, mh] = spec.mouth_area;
    ensure!(mouths.iter().all(|im| im.dimensions() == (64, 64))
        && (my..my + mh).all(|y| (mx..mx + mw).all(|x| mouths[0].get_pixel(x, y) == idle[0].get_pixel(x, y))),
        "approved mouth neutral must match idle exactly");
    fs::create_dir_all(&output)?;
    car.save(output.join("flyby-car-sprite.png"))?;
    let mut source_sheet = RgbImage::new(256, 64);
    for (i, cell) in cells.iter().enumerate() { image::imageops::replace(&mut source_sheet, cell, i as i64 * 64, 0); }
    source_sheet.save(output.join("expression-source-native.png"))?;
    let mut manifest: serde_json::Value = serde_json::from_str(&fs::read_to_string(approved.join("pet.json"))?)?;
    let idle_selection = manifest["animations"]["idle"].clone();
    manifest["id"] = json!(spec.id);
    manifest["animations"] = json!({"idle": idle_selection});
    // Keep the accepted normal idle exports byte-for-byte for review context.
    fs::create_dir_all(output.join("idle"))?;
    for i in 0..32 { fs::copy(approved.join("idle").join(format!("{i:03}.png")), output.join("idle").join(format!("{i:03}.png")))?; }
    for suffix in ["sprite.png", "native.gif", "preview.gif", "poster.png"] {
        let file = format!("idle-{suffix}");
        fs::copy(approved.join(&file), output.join(file))?;
    }
    let mut all = Vec::new();
    let mut qa = Vec::new();
    for clip in &spec.clips {
        ensure!(clip.differences.len() >= 2, "two differences required");
        let mut frames = Vec::new();
        let mut changed_counts = Vec::new();
        let mut motion = Vec::new();
        for i in 0..spec.frames {
            // Traverse one complete accepted ambient cycle and return to its
            // canonical phase; no generated body or hand poses enter the clip.
            let source_index = i * 32 / (spec.frames - 1) % 32;
            let source = &idle[source_index];
            let mut frame = source.clone();
            let mut record = json!({"frame":i,"idle_source":source_index});
            if let Some(refinement) = &spec.refinement {
                record["traffic"] = json!(alternate_traffic(&mut frame, &base, refinement, &traffic_cars, &clip.id, i)?);
            }
            match clip.id.as_str() {
                "idle-flyby" => {
                    let x = 68 - i.saturating_sub(3) as i32 * 58 / 32;
                    if (3..=36).contains(&i) {
                        if let Some(refinement) = &spec.refinement {
                            record["car_pixels"] = json!(draw_masked_car(&mut frame, &car, &refinement.flyby_rows, x, 17, false));
                        } else { draw_car(&mut frame, &car, &spec.car_clip, x, 17); }
                    }
                    sweep(&mut frame, spec.rear_area, i);
                    record["car_x"] = json!(x);
                }
                "idle-yawn" => {
                    let stage = match i { 0..=6 | 34..=39 => 0, 7..=11 | 29..=33 => 1, 12..=16 | 24..=28 => 2, _ => 3 };
                    draw_expression(&mut frame, &cells, &mouths, offset, stage, &spec);
                    tint_lamp(&mut frame, source, spec.lamp_area, [255, 175, 74], envelope(i, 3, 10, 26, 36), 1.8);
                    record["expression_stage"] = json!(stage);
                }
                "idle-message" => notification(&mut frame, &spec, i),
                "idle-city" => {
                    city(&mut frame, &spec, i);
                    let t = (i.saturating_sub(3) as f32 / 33.0).clamp(0.0, 1.0);
                    tint_lamp(&mut frame, source, spec.lamp_area, rainbow(t), envelope(i, 3, 7, 30, 36), 1.2 + 0.5 * (t * std::f32::consts::TAU).sin().abs());
                    record["rainbow_phase"] = json!(t);
                }
                _ => anyhow::bail!("unknown vignette"),
            }
            let mut changes = 0;
            for (x, y, p) in frame.enumerate_pixels() {
                if p != source.get_pixel(x, y) {
                    ensure!(allowed(&spec, &clip.id, x, y), "outside layer changed: {} {x},{y}", clip.id);
                    changes += 1;
                }
            }
            changed_counts.push(changes);
            frames.push(frame);
            motion.push(record);
        }
        ensure!(frames[0] == idle[0] && frames[spec.frames - 1] == idle[0], "canonical idle endpoints required");
        ensure!(changed_counts.iter().any(|n| *n > 0), "missing animation");
        let unique = frames.iter().map(|f| f.as_raw()).collect::<HashSet<_>>().len();
        export_clip(&output, &clip.id, &frames, spec.frame_ms, preview_palettes.get(&clip.id))?;
        manifest["animations"][&clip.id] = json!({"source":{"type":"sprite_sheet","path":format!("{}-sprite.png",clip.id),"columns":4,"frame_count":spec.frames},"frame_duration_ms":spec.frame_ms,"loop":false});
        qa.push(json!({"id":clip.id,"name":clip.name,"differences":clip.differences,"frames":spec.frames,"unique_frames":unique,"duration_ms":spec.frames as u32 * spec.frame_ms,"first_and_last_match_idle":true,"all_pixels_outside_layers_preserved":true,"changed_pixels":changed_counts,"motion":motion}));
        all.push(frames);
    }
    // Four synchronized square previews, ordered flyby / yawn / message / city.
    let mut overview = Vec::new();
    let mut key_poses = RgbImage::new(4 * 64, 4 * 64);
    for i in 0..spec.frames {
        let mut frame = RgbImage::new(128, 128);
        for (j, clip) in all.iter().enumerate() { image::imageops::replace(&mut frame, &clip[i], (j as i64 % 2) * 64, (j as i64 / 2) * 64); }
        overview.push(frame);
    }
    for (row, clip) in all.iter().enumerate() {
        for (col, i) in [0, 10, 20, 30].into_iter().enumerate() { image::imageops::replace(&mut key_poses, &clip[i], col as i64 * 64, row as i64 * 64); }
    }
    image::imageops::resize(&key_poses, 1024, 1024, FilterType::Nearest).save(output.join("key-poses.png"))?;
    let bytes: Vec<u8> = overview.iter().flat_map(|f| f.as_raw().iter().copied()).collect();
    let colors = preview_palettes.get("overview").cloned().unwrap_or_else(||
        gif::Frame::from_rgb_speed(128, (spec.frames * 128) as u16, &bytes, 1).palette.unwrap());
    write_gif(&output.join("alternates-overview.gif"), &overview, &colors, 6, spec.frame_ms)?;
    fs::write(output.join("pet.json"), serde_json::to_vec_pretty(&manifest)?)?;
    fs::write(output.join("qa.json"), serde_json::to_vec_pretty(&json!({"approved_pack":spec.approved_pack,"expression_registration":offset,"frames":spec.frames,"frame_ms":spec.frame_ms,"clips":qa,"review_only":true,"bridge_modified":false}))?)?;
    println!("Exported four idle vignettes to {}", output.display());
    Ok(())
}
