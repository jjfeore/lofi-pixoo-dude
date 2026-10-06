//! Offline interruption: approved motorized finish, jaw expression and pursuit.
//! No device/network access or changes to the bridge runtime.
use anyhow::{Context, Result, ensure};
use image::{Rgb, RgbImage, RgbaImage, imageops::FilterType};
use serde::Deserialize;
use serde_json::json;
use std::{collections::{BTreeMap, BTreeSet, HashMap}, fs, path::{Path, PathBuf}};

type Rect = [u32; 4];

#[derive(Deserialize)]
struct Spec {
    id: String,
    base: PathBuf,
    approved_pack: PathBuf,
    frames: usize,
    frame_ms: u32,
    prelude_frames: usize,
    main_aperture: Vec<[u32; 3]>,
    rear_area: Rect,
    traffic_mask: Vec<Rect>,
    cars: HashMap<String, PathBuf>,
    chase: Vec<Vehicle>,
    expression_study: Option<PathBuf>,
    expression_area: Rect,
    expression_frames: Vec<Option<usize>>,
    mouth: Option<MouthAnimation>,
    shock_rays: Option<[ShockRay; 3]>,
    red: [u8; 3],
    red_recovery: [usize; 2],
    rear_recovery: [usize; 2],
}

#[derive(Deserialize)]
struct MouthAnimation {
    origin: [u32; 2],
    stages: Vec<u8>,
    jaw: Option<JawMotion>,
}

#[derive(Deserialize)]
struct JawMotion {
    columns: Vec<[u32; 4]>,
    drops: [u32; 4],
}

#[derive(Deserialize)]
struct ShockRay {
    core: Vec<[u32; 2]>,
    edge: Vec<[u32; 2]>,
}

fn mouth_pixels(mouth: &MouthAnimation, stage: u8) -> Vec<([u32; 2], Rgb<u8>)> {
    let [x, y] = mouth.origin;
    let mut pixels = Vec::new();
    if stage == 0 { return pixels; }
    for dx in 0..3 {
        let color = if stage > 1 && dx > 0 { [238, 210, 191] } else { [17, 8, 19] };
        pixels.push(([x + dx, y], Rgb(color)));
    }
    if stage > 1 {
        for dx in 0..3 { pixels.push(([x + dx, y + 1], Rgb([12, 5, 15]))); }
    }
    if stage > 2 {
        pixels.push(([x + 1, y + 2], Rgb([12, 5, 15])));
        pixels.push(([x + 2, y + 2], Rgb([150, 61, 80])));
    }
    pixels
}

fn jaw_pixels(mouth: &MouthAnimation, stage: u8, original: &RgbImage) -> Vec<([u32; 2], Rgb<u8>)> {
    let Some(jaw) = &mouth.jaw else { return Vec::new(); };
    let mut pixels = Vec::new();
    for [x, top, bottom, maximum] in &jaw.columns {
        let drop = jaw.drops[usize::from(stage)].min(*maximum);
        if drop == 0 { continue; }
        // Keep the rear hinge steady; move the chin and its outline down together.
        // Extend only the upper beard seam to cover the gap, then draw the mouth.
        for y in *top..=*bottom + drop {
            let source_y = y.saturating_sub(drop).max(*top);
            pixels.push(([*x, y], *original.get_pixel(*x, source_y)));
        }
    }
    pixels
}

#[derive(Deserialize)]
struct Vehicle {
    id: String,
    sprite: String,
    police: bool,
    lane: i32,
    start_x: i32,
    end_x: i32,
    beacon_phase: usize,
}

fn contains([x, y, w, h]: Rect, px: u32, py: u32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn phase(i: usize, [start, end]: [usize; 2]) -> f32 {
    smooth((i as f32 - start as f32) / (end - start) as f32)
}

fn mix(a: Rgb<u8>, b: Rgb<u8>, t: f32) -> Rgb<u8> {
    Rgb(std::array::from_fn(|c| (f32::from(a[c]) * (1.0 - t) + f32::from(b[c]) * t).round() as u8))
}

fn warning_color(p: Rgb<u8>, red: [u8; 3], floor: f32, pulse: f32) -> Rgb<u8> {
    let light = (0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])) / 255.0;
    Rgb(red.map(|c| (f32::from(c) * (floor + (1.0 - floor) * light) * pulse).round() as u8))
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.into_iter().zip(b).map(|(x, y)| (i32::from(x) - i32::from(y)).pow(2) as u32).sum()
}

fn load_clip(pack: &Path, name: &str, count: usize) -> Result<Vec<RgbImage>> {
    (0..count).map(|i| {
        let image = image::open(pack.join(name).join(format!("{i:03}.png")))?.to_rgb8();
        ensure!(image.dimensions() == (64, 64), "native frame required");
        Ok(image)
    }).collect()
}

fn copy_optional(source: &Path, output: &Path) -> Result<()> {
    match fs::copy(source, output) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn copy_pack(source: &Path, output: &Path, manifest: &serde_json::Value) -> Result<()> {
    let mut clips = BTreeMap::new();
    for clip in manifest["animations"].as_object().context("missing animations")?.values() {
        let name = clip["source"]["path"].as_str().context("sprite path")?
            .strip_suffix("-sprite.png").context("named sprites required")?;
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
        for suffix in ["sprite.png", "poster.png", "contact.png", "native.gif", "preview.gif"] {
            let file = format!("{name}-{suffix}");
            fs::copy(source.join(&file), output.join(file))?;
        }
        for suffix in ["heads.png", "rear-contact.png", "gesture-poster.png"] {
            let file = format!("{name}-{suffix}");
            copy_optional(&source.join(&file), &output.join(file))?;
        }
    }
    for file in ["motorized-motion.json", "compression-motion.json", "attention-motion.json", "work-cycle-preview.gif", "compaction-cycle-preview.gif"] {
        copy_optional(&source.join(file), &output.join(file))?;
    }
    Ok(())
}

fn study(root: &Path, spec: &Spec, output: &Path) -> Result<Vec<RgbImage>> {
    let atlas = image::open(root.join(spec.expression_study.as_ref().context("expression study required")?))?.to_rgb8();
    let mut poses = Vec::new();
    let mut contact = RgbImage::new(256, 64);
    fs::create_dir_all(output.join("expression-study"))?;
    for i in 0..4 {
        let (x, y) = (i % 2, i / 2);
        let (x0, y0) = (x * atlas.width() / 2, y * atlas.height() / 2);
        let (x1, y1) = ((x + 1) * atlas.width() / 2, (y + 1) * atlas.height() / 2);
        let crop = image::imageops::crop_imm(&atlas, x0, y0, x1 - x0, y1 - y0).to_image();
        let pose = image::imageops::resize(&crop, 64, 64, FilterType::Nearest);
        pose.save(output.join("expression-study").join(format!("{i:03}.png")))?;
        image::imageops::replace(&mut contact, &pose, i64::from(i) * 64, 0);
        poses.push(pose);
    }
    image::imageops::resize(&contact, 1024, 256, FilterType::Nearest).save(output.join("expression-study-contact.png"))?;
    Ok(poses)
}

fn draw_chase(frame: &mut RgbImage, base: &RgbImage, spec: &Spec, cars: &HashMap<String, RgbaImage>, i: usize) -> Result<Vec<serde_json::Value>> {
    for [x, y, w, h] in &spec.traffic_mask { for py in *y..y + h { for px in *x..x + w {
        frame.put_pixel(px, py, *base.get_pixel(px, py));
    }}}
    let mut records = Vec::new();
    for car in &spec.chase {
        let sprite = cars.get(&car.sprite).context("unknown chase sprite")?;
        let left = (car.start_x as f32 + (car.end_x - car.start_x) as f32 * i as f32 / (spec.frames - 1) as f32).round() as i32;
        let red_on = (i / 2 + car.beacon_phase).is_multiple_of(2);
        let mut visible = 0;
        for (sx, sy, original) in sprite.enumerate_pixels() {
            let (x, y) = (left + sx as i32, car.lane + sy as i32);
            if !(0..64).contains(&x) || !(0..64).contains(&y)
                || !spec.traffic_mask.iter().any(|r| contains(*r, x as u32, y as u32)) { continue; }
            let mut pixel = *original;
            if car.police {
                if pixel[3] > 0 {
                    let light = f32::from(pixel[0].max(pixel[1]).max(pixel[2])) / 255.0;
                    pixel[0] = (24.0 + 56.0 * light).round() as u8;
                    pixel[1] = (42.0 + 93.0 * light).round() as u8;
                    pixel[2] = (66.0 + 118.0 * light).round() as u8;
                }
                if sy == 0 && (sx == 1 || sx == 2) {
                    let color = if sx == 1 && red_on { [255, 38, 66] }
                        else if sx == 2 && !red_on { [45, 142, 255] }
                        else { [29, 33, 59] };
                    pixel = image::Rgba([color[0], color[1], color[2], 255]);
                }
            }
            let old = frame.get_pixel(x as u32, y as u32);
            let alpha = u32::from(pixel[3]);
            if alpha > 0 { visible += 1; }
            frame.put_pixel(x as u32, y as u32, Rgb(std::array::from_fn(|c|
                ((u32::from(pixel[c]) * alpha + u32::from(old[c]) * (255 - alpha) + 127) / 255) as u8)));
        }
        records.push(json!({"id":car.id,"police":car.police,"x":left,"lane":car.lane,"visible_pixels":visible,
            "beacon":if car.police { if red_on {"red"} else {"blue"} } else {"none"}}));
    }
    Ok(records)
}

fn stop_screen(frame: &mut RgbImage, base: &RgbImage, spec: &Spec, original: &RgbImage, i: usize, strength: f32) {
    let [x, y, w, h] = spec.rear_area;
    for py in y..y + h { for px in x..x + w {
        frame.put_pixel(px, py, *base.get_pixel(px, py));
    }}
    let pulse = 0.88 + 0.12 * (std::f32::consts::TAU * i as f32 / 12.0).cos();
    for (py, half) in [2, 3, 4, 4, 4, 3, 2].into_iter().enumerate() {
        for px in w / 2 - half..=w / 2 + half {
            let white_bar = py == 3 && (w / 2 - 2..=w / 2 + 2).contains(&px);
            let color = if white_bar { [255, 231, 233] } else { spec.red.map(|c| (f32::from(c) * pulse).round() as u8) };
            frame.put_pixel(x + px, y + py as u32, Rgb(color));
        }
    }
    for py in y..y + h { for px in x..x + w {
        frame.put_pixel(px, py, mix(*original.get_pixel(px, py), *frame.get_pixel(px, py), strength));
    }}
}

fn gif_export(path: &Path, frames: &[RgbImage], palette: &[u8], scale: u32, ms: u32) -> Result<()> {
    let size = (64 * scale) as u16;
    let mut encoder = gif::Encoder::new(fs::File::create(path)?, size, size, palette)?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let mut cache = HashMap::<[u8; 3], u8>::new();
    for (i, image) in frames.iter().enumerate() {
        let indices: Vec<u8> = image.pixels().map(|p| *cache.entry(p.0).or_insert_with(||
            palette.as_chunks::<3>().0.iter().enumerate().min_by_key(|(_, c)| distance(p.0, **c)).unwrap().0 as u8)).collect();
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

fn export(output: &Path, frames: &[RgbImage], palette: &[u8], spec: &Spec) -> Result<()> {
    fs::create_dir_all(output.join("interrupted"))?;
    let mut sheet = RgbImage::new(256, frames.len().div_ceil(4) as u32 * 64);
    let mut contact = RgbImage::new(512, frames.len().div_ceil(8) as u32 * 64);
    let mut heads = RgbImage::new(8 * 38, frames.len().div_ceil(8) as u32 * 34);
    let mut rear = RgbImage::new(8 * 15, frames.len().div_ceil(8) as u32 * 11);
    for (i, frame) in frames.iter().enumerate() {
        frame.save(output.join("interrupted").join(format!("{i:03}.png")))?;
        image::imageops::replace(&mut sheet, frame, (i as i64 % 4) * 64, (i as i64 / 4) * 64);
        image::imageops::replace(&mut contact, frame, (i as i64 % 8) * 64, (i as i64 / 8) * 64);
        let head = image::imageops::crop_imm(frame, 2, 12, 38, 34).to_image();
        image::imageops::replace(&mut heads, &head, (i as i64 % 8) * 38, (i as i64 / 8) * 34);
        let rear_crop = image::imageops::crop_imm(frame, 33, 38, 15, 11).to_image();
        image::imageops::replace(&mut rear, &rear_crop, (i as i64 % 8) * 15, (i as i64 / 8) * 11);
    }
    sheet.save(output.join("interrupted-sprite.png"))?;
    image::imageops::resize(&contact, 1024, contact.height() * 2, FilterType::Nearest).save(output.join("interrupted-contact.png"))?;
    image::imageops::resize(&heads, heads.width() * 4, heads.height() * 4, FilterType::Nearest).save(output.join("interrupted-heads.png"))?;
    image::imageops::resize(&rear, rear.width() * 8, rear.height() * 8, FilterType::Nearest).save(output.join("interrupted-rear-contact.png"))?;
    image::imageops::resize(&frames[4], 512, 512, FilterType::Nearest).save(output.join("interrupted-poster.png"))?;
    gif_export(&output.join("interrupted-native.gif"), frames, palette, 1, spec.frame_ms)?;
    gif_export(&output.join("interrupted-preview.gif"), frames, palette, 8, spec.frame_ms)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let plan = PathBuf::from(args.next().context("provide authoring JSON")?);
    let output = PathBuf::from(args.next().context("provide new output directory")?);
    ensure!(args.next().is_none() && !output.exists(), "choose a new output directory");
    let root = plan.parent().unwrap_or(Path::new("."));
    let spec: Spec = serde_json::from_str(&fs::read_to_string(&plan)?)?;
    ensure!(spec.frames == 40 && spec.frame_ms == 83 && spec.prelude_frames == 8, "40 frames at 83ms with 8-frame shock prelude required");
    ensure!(spec.rear_area[2..] == [11, 7], "11x7 rear display required");
    ensure!(spec.main_aperture.len() == 9 && spec.main_aperture.iter().map(|p| p[0]).collect::<BTreeSet<_>>().len() == 9
        && spec.main_aperture.iter().all(|[x, a, b]| *x < 64 && *a <= *b && *b < 64), "invalid glass aperture");
    for [x, y, w, h] in spec.traffic_mask.iter().chain([&spec.rear_area, &spec.expression_area]) {
        ensure!(*w > 0 && *h > 0 && x.checked_add(*w).is_some_and(|v| v <= 64)
            && y.checked_add(*h).is_some_and(|v| v <= 64), "invalid overlay region");
    }
    ensure!(spec.expression_frames.len() == spec.frames && spec.expression_frames.iter().flatten().all(|i| *i < 4), "invalid expression timing");
    if let Some(mouth) = &spec.mouth {
        ensure!(mouth.stages.len() == spec.frames && mouth.stages.iter().all(|s| *s <= 3)
            && mouth.stages[0] == 0 && mouth.stages[spec.frames - 1] == 0,
            "invalid mouth stages");
        ensure!(mouth.origin[0] <= 61 && mouth.origin[1] <= 61
            && spec.expression_frames.iter().all(Option::is_none), "mouth-only motion cannot use jaw patches");
        if let Some(jaw) = &mouth.jaw {
            ensure!(!jaw.columns.is_empty() && jaw.drops[0] == 0
                && jaw.drops.windows(2).all(|v| v[0] <= v[1])
                && jaw.drops.iter().all(|d| *d <= 2)
                && jaw.columns.iter().map(|c| c[0]).collect::<BTreeSet<_>>().len() == jaw.columns.len()
                && jaw.columns.iter().all(|[x, top, bottom, maximum]| *x < 64
                    && *top <= *bottom && *maximum <= 2 && *bottom + *maximum < 64),
                "invalid jaw columns or mouth-linked travel");
        }
    }
    if let Some(rays) = &spec.shock_rays {
        for ray in rays {
            ensure!(!ray.core.is_empty() && !ray.edge.is_empty()
                && ray.core.iter().chain(&ray.edge).all(|[x, y]| *x < 64 && *y < 64),
                "invalid shock ray");
        }
    }
    for [start, end] in [spec.red_recovery, spec.rear_recovery] {
        ensure!(start < end && end < spec.frames, "invalid recovery timing");
    }
    ensure!(spec.chase.iter().filter(|v| !v.police).count() == 1 && spec.chase.iter().filter(|v| v.police).count() >= 3
        && spec.chase.iter().all(|v| v.start_x < v.end_x), "one getaway and several forward pursuers required");
    let approved = root.join(&spec.approved_pack);
    let base = image::open(root.join(&spec.base))?.to_rgb8();
    ensure!(base.dimensions() == (64, 64), "native base required");
    let working = load_clip(&approved, "working", 32)?;
    let idle = load_clip(&approved, "idle", 32)?;
    let finished = load_clip(&approved, "finished", 32)?;
    let motor: serde_json::Value = serde_json::from_str(&fs::read_to_string(approved.join("motorized-motion.json"))?)?;
    let motor_frames = motor["frames"]["finished"].as_array().context("finish motion missing")?;
    ensure!(motor_frames.len() == 32, "finish pose records required");
    let cars = spec.cars.iter().map(|(name, path)| -> Result<_> {
        let image = image::open(root.join(path))?.to_rgba8();
        ensure!(image.dimensions() == (4, 2), "approved tiny vehicle required");
        Ok((name.clone(), image))
    }).collect::<Result<HashMap<_, _>>>()?;
    let mut manifest: serde_json::Value = serde_json::from_str(&fs::read_to_string(approved.join("pet.json"))?)?;
    fs::create_dir_all(&output)?;
    copy_pack(&approved, &output, &manifest)?;
    base.save(output.join("base.png"))?;
    let expressions = if spec.expression_study.is_some() { Some(study(root, &spec, &output)?) } else { None };
    let [ex, ey, ew, eh] = spec.expression_area;
    let colors: BTreeSet<_> = (ey..ey + eh).flat_map(|y| (ex..ex + ew).map(move |x| (x, y)))
        .map(|(x, y)| working[0].get_pixel(x, y).0).collect();
    let mut frames = Vec::new();
    let mut records = Vec::new();
    let old_sparks = [[31, 25], [32, 24], [32, 23], [33, 22], [31, 31], [32, 32]];
    let sparks: BTreeSet<_> = if let Some(rays) = &spec.shock_rays {
        rays.iter().flat_map(|r| r.core.iter().chain(&r.edge)).copied().collect()
    } else { old_sparks.into_iter().collect() };
    for i in 0..spec.frames {
        let j = i.saturating_sub(spec.prelude_frames);
        let original = if i < spec.prelude_frames { &working[i] } else { &finished[j] };
        let mut frame = original.clone();
        let red_amount = if i == 0 { 0.0 } else { 1.0 - phase(i, spec.red_recovery) };
        let pulse = if i < 8 { [1.0, 0.75, 1.0, 0.8, 1.0, 0.82, 0.96, 0.9][i] }
            else { 0.94 + 0.06 * (std::f32::consts::TAU * i as f32 / 16.0).cos() };
        let glitch: i32 = match i { 2 | 4 | 6 => 3, 3 | 5 => -2, _ => 0 };
        if let Some(pose) = spec.expression_frames[i] {
            for y in ey..ey + eh { for x in ex..ex + ew {
                let source = expressions.as_ref().context("expression source required")?[pose].get_pixel(x, y).0;
                let color = colors.iter().min_by_key(|c| distance(source, **c)).unwrap();
                frame.put_pixel(x, y, Rgb(*color));
            }}
        }
        let jaw = spec.mouth.as_ref().map(|m| jaw_pixels(m, m.stages[i], original)).unwrap_or_default();
        for ([x, y], color) in &jaw { frame.put_pixel(*x, *y, *color); }
        let mouth = spec.mouth.as_ref().map(|m| mouth_pixels(m, m.stages[i])).unwrap_or_default();
        for ([x, y], color) in &mouth { frame.put_pixel(*x, *y, *color); }
        for [x, top, bottom] in &spec.main_aperture {
            for y in *top..=*bottom {
                let row = (*top as i32 + (y as i32 - *top as i32 + glitch).rem_euclid((bottom - top + 1) as i32)) as u32;
                let pixel = *original.get_pixel(*x, row);
                let mut alert = warning_color(pixel, spec.red, 0.08, pulse);
                if glitch != 0 && y == *top + 5 + (i as u32 % 3) { alert = Rgb([255, 141, 166]); }
                frame.put_pixel(*x, y, mix(pixel, alert, red_amount));
            }
        }
        let bounds: Rect = serde_json::from_value(motor_frames[j]["visor_bounds"].clone())?;
        for y in bounds[1]..bounds[1] + bounds[3] { for x in bounds[0]..bounds[0] + bounds[2] {
            let p = *frame.get_pixel(x, y);
            if p[1] > p[0].saturating_add(35) && p[2] > p[0].saturating_add(35) {
                frame.put_pixel(x, y, mix(p, warning_color(p, spec.red, 0.45, pulse), red_amount));
            }
        }}
        let rear_strength = if i == 0 { 0.0 } else { 1.0 - phase(i, spec.rear_recovery) };
        stop_screen(&mut frame, &base, &spec, original, i, rear_strength);
        if (2..=6).contains(&i) && i.is_multiple_of(2) {
            if let Some(rays) = &spec.shock_rays {
                for ray in rays {
                    for [x, y] in &ray.edge { frame.put_pixel(*x, *y, Rgb([240, 27, 61])); }
                    for [x, y] in &ray.core { frame.put_pixel(*x, *y, Rgb([255, 79, 108])); }
                }
            } else {
                for [x, y] in &sparks { frame.put_pixel(*x, *y, Rgb([253, 70, 117])); }
            }
        }
        let chase = draw_chase(&mut frame, &base, &spec, &cars, i)?;
        for (x, y, p) in frame.enumerate_pixels() {
            let allowed = spec.main_aperture.iter().any(|[px, a, b]| x == *px && y >= *a && y <= *b)
                || contains(spec.rear_area, x, y) || contains(bounds, x, y)
                || spec.traffic_mask.iter().any(|r| contains(*r, x, y))
                || spec.expression_frames[i].is_some() && contains(spec.expression_area, x, y)
                || jaw.iter().any(|(p, _)| *p == [x, y])
                || mouth.iter().any(|(p, _)| *p == [x, y])
                || (2..=6).contains(&i) && i.is_multiple_of(2) && sparks.contains(&[x, y]);
            ensure!(allowed || p == original.get_pixel(x, y), "unapproved interruption pixel at {x},{y}");
        }
        let mut record = json!({"frame":i,"source":if i < 8 {"working"} else {"finished"},"source_frame":if i < 8 {i} else {j},
            "phase":if i == 0 {"working"} else if i < 8 {"dumpshock"} else if j < 4 {"warning-hold"} else if j < 20 {"motorized-jack-out"} else {"recover-to-idle"},
            "expression":spec.expression_frames[i],"red_amount":red_amount,"pulse":pulse,"glitch_shift":glitch,
            "visor_bounds":bounds,"lowered":motor_frames[j]["lowered"],"rear_stop_strength":rear_strength,"chase":chase});
        if let Some(m) = &spec.mouth {
            record["mouth_stage"] = json!(m.stages[i]);
            if let Some(jaw) = &m.jaw { record["jaw_drop"] = json!(jaw.drops[usize::from(m.stages[i])]); }
        }
        if spec.shock_rays.is_some() {
            record["shock_ray_count"] = json!(if (2..=6).contains(&i) && i.is_multiple_of(2) { 3 } else { 0 });
        }
        records.push(record);
        frames.push(frame);
    }
    ensure!(frames[0] == working[0] && frames[39] == idle[0], "canonical working and idle endpoints required");
    let cycle: Vec<_> = working.iter().chain(&frames).chain(&idle).cloned().collect();
    let mut palette_input = RgbImage::new(256, cycle.len().div_ceil(4) as u32 * 64);
    for (i, frame) in cycle.iter().enumerate() {
        image::imageops::replace(&mut palette_input, frame, (i as i64 % 4) * 64, (i as i64 / 4) * 64);
    }
    let palette = gif::Frame::from_rgb_speed(palette_input.width() as u16, palette_input.height() as u16, palette_input.as_raw(), 1).palette.unwrap();
    export(&output, &frames, &palette, &spec)?;
    gif_export(&output.join("interruption-cycle-preview.gif"), &cycle, &palette, 8, spec.frame_ms)?;
    manifest["id"] = json!(spec.id);
    manifest["animations"]["interrupted"] = json!({"source":{"type":"sprite_sheet","path":"interrupted-sprite.png","columns":4,"frame_count":spec.frames},
        "frame_duration_ms":spec.frame_ms,"loop":false});
    fs::write(output.join("pet.json"), serde_json::to_vec_pretty(&manifest)?)?;
    fs::write(output.join("interruption-motion.json"), serde_json::to_vec_pretty(&records)?)?;
    println!("Prepared {} interruption frames, approved clips and context preview in {}", spec.frames, output.display());
    Ok(())
}
