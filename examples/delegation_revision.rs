//! Offline delegation: a courier drone, transfer arrows and green falling glyphs.
//! Reuses approved working frames; no device or network access.
use anyhow::{Context, Result, ensure};
use image::{Rgb, RgbImage, RgbaImage, imageops::FilterType};
use serde::Deserialize;
use serde_json::json;
use std::{collections::{BTreeMap, HashMap}, fs, path::{Path, PathBuf}};

type Rect = [u32; 4];

#[derive(Deserialize)]
struct Spec {
    id: String,
    base: PathBuf,
    approved_pack: PathBuf,
    frame_ms: u32,
    rear_area: Rect,
    window_mask: Vec<Rect>,
    traffic_mask: Vec<Rect>,
    drone: Drone,
    #[serde(default)]
    palette_from_gif: Option<PathBuf>,
    #[serde(default)]
    gif_exact_colors: Vec<[u8; 3]>,
    cars: HashMap<String, PathBuf>,
    traffic: Vec<Traffic>,
}

#[derive(Deserialize)]
struct Drone {
    source: PathBuf,
    crop: Rect,
    size: [u32; 2],
    lamp: [u32; 2],
    hover: [i32; 2],
    below_y: i32,
    outside_x: i32,
    arrival: [usize; 2],
    departure: [usize; 2],
    upload_ack_frame: usize,
    download_off_frame: usize,
    #[serde(default)]
    finish_exit: DroneExit,
    #[serde(default = "received_lamp")]
    received_lamp: [u8; 3],
    #[serde(default = "ack_lamp")]
    ack_lamp: [u8; 3],
}

fn received_lamp() -> [u8; 3] { [69, 255, 132] }
fn ack_lamp() -> [u8; 3] { [196, 255, 223] }

#[derive(Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum DroneExit {
    #[default]
    Right,
    Bottom,
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

fn contains([x, y, w, h]: Rect, px: u32, py: u32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn progress(i: usize, [start, end]: [usize; 2]) -> f32 {
    let t = ((i as f32 - start as f32) / (end - start) as f32).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: Rgb<u8>, b: Rgb<u8>, t: f32) -> Rgb<u8> {
    Rgb(std::array::from_fn(|c| (f32::from(a[c]) * (1.0 - t) + f32::from(b[c]) * t).round() as u8))
}

fn clear(frame: &mut RgbImage, base: &RgbImage, masks: &[Rect]) {
    for [x, y, w, h] in masks {
        for py in *y..y + h { for px in *x..x + w { frame.put_pixel(px, py, *base.get_pixel(px, py)); }}
    }
}

fn rain(i: usize) -> RgbImage {
    let mut image = RgbImage::new(11, 7);
    let tick = i / 2;
    for (column, phase) in [0, 3, 6, 2, 5].into_iter().enumerate() {
        let x = 1 + column as u32 * 2;
        let head = (tick + phase) % 8;
        for y in 0..7 {
            let age = (head as i32 - y as i32).rem_euclid(8);
            let color = match age { 0 => [168, 255, 188], 1 => [30, 213, 91], 2 => [8, 117, 48], 3 => [3, 47, 23], _ => [0, 0, 0] };
            image.put_pixel(x, y, Rgb(color));
            if age < 3 && (tick + column + y as usize).is_multiple_of(2) {
                image.put_pixel(x + 1, y, Rgb(color.map(|c| c / 2)));
            }
        }
    }
    image
}

fn arrow(i: usize, upload: bool) -> RgbImage {
    let mut image = RgbImage::new(11, 7);
    let glyph = if upload {
        vec![(5,0),(4,1),(5,1),(6,1),(3,2),(4,2),(5,2),(6,2),(7,2),(5,3),(5,4)]
    } else {
        vec![(5,0),(5,1),(5,2),(3,3),(4,3),(5,3),(6,3),(7,3),(4,4),(5,4),(6,4),(5,5)]
    };
    let pulse = 0.76 + 0.24 * (std::f32::consts::TAU * i as f32 / 8.0).cos().abs();
    let accent = if upload { [64, 242, 166] } else { [63, 222, 255] };
    for (x, y) in glyph {
        let brightness = if y == (i as u32 / 2) % 7 { 1.0 } else { pulse };
        image.put_pixel(x, y, Rgb(accent.map(|c| (c as f32 * brightness).round() as u8)));
    }
    for x in 2..=8 { image.put_pixel(x, 6, Rgb([19, 105, 84])); }
    for x in [2, 8] { image.put_pixel(x, 5, Rgb([19, 105, 84])); }
    image
}

fn draw_rear(frame: &mut RgbImage, source: &RgbImage, spec: &Spec, name: &str, i: usize) -> serde_json::Value {
    let [x, y, w, h] = spec.rear_area;
    let waveform = image::imageops::crop_imm(source, x, y, w, h).to_image();
    let matrix = rain(if name == "delegating-start" { i.saturating_sub(8) } else { i });
    let (from, to, amount, mode) = if name == "delegating" {
        (&matrix, &matrix, 1.0, "matrix")
    } else {
        let upload = name == "delegating-start";
        let symbol = arrow(i, upload);
        let before = if upload { &waveform } else { &matrix };
        let after = if upload { &matrix } else { &waveform };
        let (a, b, t, label) = if i <= 26 {
            (before, &symbol, progress(i, [0, 4]), if upload { "upload" } else { "download" })
        } else {
            (&symbol, after, progress(i, [27, 36]), if upload { "upload-to-matrix" } else { "download-to-waveform" })
        };
        for py in 0..h { for px in 0..w { frame.put_pixel(x + px, y + py, mix(*a.get_pixel(px, py), *b.get_pixel(px, py), t)); }}
        return json!({"mode":label,"amount":t,"matrix_frame":if upload {i.saturating_sub(8)} else {i}});
    };
    for py in 0..h { for px in 0..w { frame.put_pixel(x + px, y + py, mix(*from.get_pixel(px, py), *to.get_pixel(px, py), amount)); }}
    json!({"mode":mode,"amount":amount,"matrix_frame":i})
}

fn draw_drone(frame: &mut RgbImage, sprite: &RgbaImage, spec: &Spec, name: &str, i: usize) -> serde_json::Value {
    if name == "delegating" { return serde_json::Value::Null; }
    let drone = &spec.drone;
    let upload = name == "delegating-start";
    let [hx, hy] = drone.hover;
    let arrival = progress(i, drone.arrival);
    let departure = progress(i, drone.departure);
    let x = if upload { hx as f32 } else { drone.outside_x as f32 + (hx - drone.outside_x) as f32 * arrival };
    let y = if upload { drone.below_y as f32 + (hy - drone.below_y) as f32 * arrival } else { hy as f32 };
    let bottom_exit = !upload && drone.finish_exit == DroneExit::Bottom;
    let left = if bottom_exit { x.round() as i32 }
        else { (x + (drone.outside_x - hx) as f32 * departure).round() as i32 };
    let top = if bottom_exit { (y + (drone.below_y - hy) as f32 * departure).round() as i32 }
        else { y.round() as i32 };
    let received = if upload { i >= drone.upload_ack_frame } else { i < drone.download_off_frame };
    let ack = upload && (drone.upload_ack_frame..drone.upload_ack_frame + 3).contains(&i);
    let lamp = if ack { drone.ack_lamp } else if received { drone.received_lamp } else { [48, 48, 63] };
    let mut pixels = Vec::new();
    for (sx, sy, pixel) in sprite.enumerate_pixels() {
        let x = left + sx as i32;
        let y = top + sy as i32;
        if pixel[3] == 0 || !(0..64).contains(&x) || !(0..64).contains(&y)
            || !spec.window_mask.iter().any(|r| contains(*r, x as u32, y as u32)) { continue; }
        let color = if [sx, sy] == drone.lamp { lamp } else { [pixel[0], pixel[1], pixel[2]] };
        frame.put_pixel(x as u32, y as u32, Rgb(color));
        pixels.push([x, y]);
    }
    json!({"x":left,"y":top,"received":received,"ack":ack,"lamp":lamp,"visible_pixels":pixels})
}

fn draw_traffic(frame: &mut RgbImage, spec: &Spec, cars: &HashMap<String, RgbaImage>, i: usize) -> Result<Vec<serde_json::Value>> {
    let mut records = Vec::new();
    for vehicle in &spec.traffic {
        let sprite = cars.get(&vehicle.sprite).context("unknown traffic sprite")?;
        let t = (i as f32 * vehicle.passes as f32 / 32.0 + vehicle.phase).fract();
        let left = (vehicle.start_x as f32 + (vehicle.end_x - vehicle.start_x) as f32 * t).round() as i32;
        let mut pixels = Vec::new();
        for (sx, sy, _) in sprite.enumerate_pixels() {
            let x = left + sx as i32;
            let y = vehicle.lane + sy as i32;
            if !(0..64).contains(&x) || !(0..64).contains(&y)
                || !spec.traffic_mask.iter().any(|r| contains(*r, x as u32, y as u32)) { continue; }
            let sx = if vehicle.mirror { sprite.width() - 1 - sx } else { sx };
            let pixel = sprite.get_pixel(sx, sy);
            let old = frame.get_pixel(x as u32, y as u32);
            let alpha = u32::from(pixel[3]);
            frame.put_pixel(x as u32, y as u32, Rgb(std::array::from_fn(|c|
                ((u32::from(pixel[c]) * alpha + u32::from(old[c]) * (255 - alpha) + 127) / 255) as u8)));
            if alpha > 0 { pixels.push([x, y]); }
        }
        records.push(json!({"sprite":vehicle.sprite,"x":left,"lane":vehicle.lane,"pixels":pixels}));
    }
    Ok(records)
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.into_iter().zip(b).map(|(x, y)| (i32::from(x) - i32::from(y)).pow(2) as u32).sum()
}

fn gif_export(path: &Path, frames: &[RgbImage], palette: &[u8], scale: u32, ms: u32, exact_colors: &[[u8; 3]]) -> Result<()> {
    let size = (64 * scale) as u16;
    let mut encoder = gif::Encoder::new(fs::File::create(path)?, size, size, palette)?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let mut cache = HashMap::<[u8; 3], u8>::new();
    for (i, image) in frames.iter().enumerate() {
        let mut indices: Vec<u8> = image.pixels().map(|p| *cache.entry(p.0).or_insert_with(||
            palette.as_chunks::<3>().0.iter().enumerate().min_by_key(|(_, c)| distance(p.0, **c)).unwrap().0 as u8)).collect();
        // Reserve unused per-frame slots for small status lights. Keep every
        // other pixel's original index/color, including the approved room.
        let mut local_palette = palette.to_vec();
        let mut used = [false; 256];
        for (pixel, index) in image.pixels().zip(&indices) {
            if !exact_colors.contains(&pixel.0) { used[*index as usize] = true; }
        }
        let mut changed = false;
        for color in exact_colors {
            if !image.pixels().any(|pixel| pixel.0 == *color) { continue; }
            let slot = (0..palette.len() / 3).find(|slot| !used[*slot]).context("no free GIF color slot for status light")?;
            local_palette[slot * 3..slot * 3 + 3].copy_from_slice(color);
            used[slot] = true;
            for (pixel, index) in image.pixels().zip(&mut indices) {
                if pixel.0 == *color { *index = slot as u8; }
            }
            changed = true;
        }
        let mut enlarged = Vec::with_capacity(usize::from(size).pow(2));
        for y in 0..64 { for _ in 0..scale { for x in 0..64 { for _ in 0..scale { enlarged.push(indices[y * 64 + x]); }}}}
        let mut frame = gif::Frame::from_indexed_pixels(size, size, enlarged, None);
        if changed { frame.palette = Some(local_palette); }
        frame.delay = (((i as u32 + 1) * ms / 10) - (i as u32 * ms / 10)) as u16;
        frame.dispose = gif::DisposalMethod::Keep;
        encoder.write_frame(&frame)?;
    }
    Ok(())
}

fn copy_pack(source: &Path, output: &Path, manifest: &serde_json::Value) -> Result<()> {
    let mut clips = BTreeMap::new();
    for clip in manifest["animations"].as_object().context("animation map")?.values() {
        let name = clip["source"]["path"].as_str().context("sprite path")?.strip_suffix("-sprite.png").context("named sprites")?;
        let count = clip["source"]["frame_count"].as_u64().context("frame count")?;
        ensure!((1..=40).contains(&count), "invalid copied clip size");
        if let Some(old) = clips.insert(name, count) { ensure!(old == count, "inconsistent aliases"); }
    }
    for (name, count) in clips {
        fs::create_dir_all(output.join(name))?;
        for i in 0..count {
            let file = format!("{i:03}.png");
            fs::copy(source.join(name).join(&file), output.join(name).join(file))?;
        }
        for suffix in ["sprite.png", "poster.png", "contact.png", "native.gif", "preview.gif", "heads.png", "rear-contact.png", "gesture-poster.png"] {
            let file = format!("{name}-{suffix}");
            match fs::copy(source.join(&file), output.join(file)) {
                Ok(_) => {}, Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e.into()),
            }
        }
    }
    for file in ["motorized-motion.json", "compression-motion.json", "attention-motion.json", "interruption-motion.json",
        "work-cycle-preview.gif", "compaction-cycle-preview.gif", "interruption-cycle-preview.gif"] {
        match fs::copy(source.join(file), output.join(file)) {
            Ok(_) => {}, Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn export(output: &Path, name: &str, frames: &[RgbImage], palette: &[u8], spec: &Spec) -> Result<()> {
    fs::create_dir_all(output.join(name))?;
    let mut sheet = RgbImage::new(256, frames.len().div_ceil(4) as u32 * 64);
    let mut contact = RgbImage::new(512, frames.len().div_ceil(8) as u32 * 64);
    let mut rear = RgbImage::new(8 * 15, frames.len().div_ceil(8) as u32 * 11);
    let mut window = RgbImage::new(8 * 24, frames.len().div_ceil(8) as u32 * 23);
    for (i, frame) in frames.iter().enumerate() {
        frame.save(output.join(name).join(format!("{i:03}.png")))?;
        image::imageops::replace(&mut sheet, frame, (i as i64 % 4) * 64, (i as i64 / 4) * 64);
        image::imageops::replace(&mut contact, frame, (i as i64 % 8) * 64, (i as i64 / 8) * 64);
        let crop = image::imageops::crop_imm(frame, 33, 38, 15, 11).to_image();
        image::imageops::replace(&mut rear, &crop, (i as i64 % 8) * 15, (i as i64 / 8) * 11);
        let crop = image::imageops::crop_imm(frame, 30, 17, 24, 23).to_image();
        image::imageops::replace(&mut window, &crop, (i as i64 % 8) * 24, (i as i64 / 8) * 23);
    }
    sheet.save(output.join(format!("{name}-sprite.png")))?;
    image::imageops::resize(&frames[frames.len() / 2], 512, 512, FilterType::Nearest).save(output.join(format!("{name}-poster.png")))?;
    image::imageops::resize(&contact, 1024, contact.height() * 2, FilterType::Nearest).save(output.join(format!("{name}-contact.png")))?;
    image::imageops::resize(&rear, rear.width() * 8, rear.height() * 8, FilterType::Nearest).save(output.join(format!("{name}-rear-contact.png")))?;
    image::imageops::resize(&window, window.width() * 4, window.height() * 4, FilterType::Nearest).save(output.join(format!("{name}-window-contact.png")))?;
    gif_export(&output.join(format!("{name}-native.gif")), frames, palette, 1, spec.frame_ms, &spec.gif_exact_colors)?;
    gif_export(&output.join(format!("{name}-preview.gif")), frames, palette, 8, spec.frame_ms, &spec.gif_exact_colors)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let plan = PathBuf::from(args.next().context("provide authoring JSON")?);
    let output = PathBuf::from(args.next().context("provide fresh output directory")?);
    ensure!(args.next().is_none() && !output.exists(), "choose a new output directory");
    let root = plan.parent().unwrap_or(Path::new("."));
    let spec: Spec = serde_json::from_str(&fs::read_to_string(&plan)?)?;
    ensure!(spec.frame_ms == 83 && spec.rear_area == [35, 40, 11, 7], "accepted scene geometry and timing required");
    for [x,y,w,h] in spec.window_mask.iter().chain(&spec.traffic_mask) {
        ensure!(*w > 0 && *h > 0 && *x + *w <= 64 && *y + *h <= 64, "invalid scene mask");
    }
    ensure!(spec.drone.arrival[0] < spec.drone.arrival[1] && spec.drone.arrival[1] < spec.drone.upload_ack_frame
        && spec.drone.upload_ack_frame < spec.drone.download_off_frame && spec.drone.download_off_frame < spec.drone.departure[0]
        && spec.drone.departure[0] < spec.drone.departure[1] && spec.drone.departure[1] < 40, "invalid transfer phases");
    let base = image::open(root.join(&spec.base))?.to_rgb8();
    ensure!(base.dimensions() == (64, 64), "native base required");
    let approved = root.join(&spec.approved_pack);
    let working: Vec<RgbImage> = (0..32).map(|i| Ok(image::open(approved.join("working").join(format!("{i:03}.png")))?.to_rgb8())).collect::<Result<_>>()?;
    ensure!(working.iter().all(|im| im.dimensions() == (64, 64)), "native working frames required");
    let generated = image::open(root.join(&spec.drone.source))?.to_rgba8();
    let [x,y,w,h] = spec.drone.crop;
    ensure!(w > 0 && h > 0 && x + w <= generated.width() && y + h <= generated.height()
        && spec.drone.size.iter().all(|s| (1..=24).contains(s)), "invalid drone projection");
    let mut cropped = image::imageops::crop_imm(&generated, x, y, w, h).to_image();
    for pixel in cropped.pixels_mut() { pixel[3] = if pixel[3] >= 128 { 255 } else { 0 }; }
    let sprite = image::imageops::resize(&cropped, spec.drone.size[0], spec.drone.size[1], FilterType::Nearest);
    ensure!(spec.drone.lamp[0] < sprite.width() && spec.drone.lamp[1] < sprite.height()
        && sprite.get_pixel(spec.drone.lamp[0], spec.drone.lamp[1])[3] == 255, "lamp must lie on drone body");
    let cars: HashMap<_,_> = spec.cars.iter().map(|(name,path)| Ok((name.clone(),image::open(root.join(path))?.to_rgba8()))).collect::<Result<_>>()?;
    let mut manifest: serde_json::Value = serde_json::from_str(&fs::read_to_string(approved.join("pet.json"))?)?;
    fs::create_dir_all(&output)?;
    copy_pack(&approved, &output, &manifest)?;
    base.save(output.join("base.png"))?;
    sprite.save(output.join("drone-sprite.png"))?;
    image::imageops::resize(&sprite, sprite.width() * 24, sprite.height() * 24, FilterType::Nearest).save(output.join("drone-preview.png"))?;
    let mut clips = BTreeMap::new();
    let mut records = BTreeMap::new();
    for (name, count) in [("delegating-start",40),("delegating",32),("delegating-finished",40)] {
        let mut frames = Vec::new();
        let mut motion = Vec::new();
        for i in 0..count {
            let source_frame = i * 32 / count;
            let original = &working[source_frame];
            let mut frame = original.clone();
            clear(&mut frame, &base, &spec.traffic_mask);
            let rear = draw_rear(&mut frame, original, &spec, name, i);
            let traffic = if name == "delegating" { draw_traffic(&mut frame, &spec, &cars, i)? } else { Vec::new() };
            let drone = draw_drone(&mut frame, &sprite, &spec, name, i);
            for (x,y,pixel) in frame.enumerate_pixels() {
                let allowed = contains(spec.rear_area,x,y) || spec.traffic_mask.iter().any(|r| contains(*r,x,y))
                    || name != "delegating" && spec.window_mask.iter().any(|r| contains(*r,x,y));
                ensure!(allowed || pixel == original.get_pixel(x,y), "unapproved delegation pixel at {x},{y}");
            }
            motion.push(json!({"frame":i,"source_frame":source_frame,"rear":rear,"traffic":traffic,"drone":drone}));
            frames.push(frame);
        }
        clips.insert(name,frames);
        records.insert(name,motion);
        manifest["animations"][name] = json!({"source":{"type":"sprite_sheet","path":format!("{name}-sprite.png"),"columns":4,"frame_count":count},"frame_duration_ms":spec.frame_ms,"loop":name == "delegating"});
    }
    manifest["animations"]["delegating"]["entry"] = json!("delegating-start");
    manifest["animations"]["delegating"]["exit"] = json!("delegating-finished");
    let cycle: Vec<_> = clips["delegating-start"].iter().chain(&clips["delegating"]).chain(&clips["delegating-finished"]).cloned().collect();
    let mut fit = RgbImage::new(256, cycle.len().div_ceil(4) as u32 * 64);
    for (i,frame) in cycle.iter().enumerate() { image::imageops::replace(&mut fit,frame,(i as i64 % 4) * 64,(i as i64 / 4) * 64); }
    let palette = if let Some(path) = &spec.palette_from_gif {
        let decoder = gif::DecodeOptions::new().read_info(fs::File::open(root.join(path))?)?;
        decoder.global_palette().context("reference GIF needs a global palette")?.to_vec()
    } else {
        gif::Frame::from_rgb_speed(fit.width() as u16,fit.height() as u16,fit.as_raw(),1).palette.unwrap()
    };
    for (name,frames) in &clips { export(&output,name,frames,&palette,&spec)?; }
    gif_export(&output.join("delegation-cycle-preview.gif"),&cycle,&palette,8,spec.frame_ms,&spec.gif_exact_colors)?;
    manifest["id"] = json!(spec.id);
    fs::write(output.join("pet.json"),serde_json::to_vec_pretty(&manifest)?)?;
    fs::write(output.join("delegation-motion.json"),serde_json::to_vec_pretty(&records)?)?;
    println!("Prepared delegation start (40), loop (32), finish (40) and approved clips in {}",output.display());
    Ok(())
}
