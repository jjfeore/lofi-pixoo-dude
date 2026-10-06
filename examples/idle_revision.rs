//! Offline artwork preparation: registered fingertip poses and explicit layers.
//! Run with an authoring JSON and a NEW output directory. No device access.
use anyhow::{Context, Result, ensure};
use image::{Rgb, RgbImage, RgbaImage, imageops::FilterType};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

type Rect = [u32; 4];

#[derive(Deserialize)]
struct Spec {
    #[serde(default = "default_id")]
    id: String,
    base: PathBuf,
    poses: PathBuf,
    car: PathBuf,
    pose_schedule: Vec<usize>,
    neutral_poses: Vec<usize>,
    fingers: Vec<Fingers>,
    monitor: Rect,
    monitor_aperture: Option<Vec<[u32; 3]>>,
    palette_from: Option<PathBuf>,
    cursor: Option<Rect>,
    #[serde(default)]
    cursor_color: [u8; 3],
    #[serde(default)]
    cursor_on_frames: usize,
    waveform: Option<Waveform>,
    second_car: Option<Car>,
    car_clip: Rect,
    car_size: [u32; 2],
    car_start_x: i32,
    car_end_x: i32,
    car_y: i32,
    frame_ms: u32,
}

fn default_id() -> String {
    "decker-idle-v2".into()
}

#[derive(Deserialize)]
struct Waveform {
    source: PathBuf,
    area: Rect,
}

#[derive(Deserialize)]
struct Car {
    clip: Rect,
    size: [u32; 2],
    start_x: i32,
    end_x: i32,
    y: i32,
    #[serde(default)]
    mirror: bool,
}

#[derive(Deserialize)]
struct Fingers {
    anchor: Rect,
    area: Rect,
    pixels: Vec<[u32; 2]>,
    poses: Vec<usize>,
}

fn contains([x, y, w, h]: Rect, px: u32, py: u32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn skin(c: Rgb<u8>) -> bool {
    let [r, g, b] = c.0.map(i32::from);
    r > 85 && r > g + 15 && r > b + 15
}

fn distance(a: Rgb<u8>, b: Rgb<u8>) -> u32 {
    a.0.into_iter()
        .zip(b.0)
        .map(|(a, b)| (i32::from(a) - i32::from(b)).pow(2) as u32)
        .sum()
}

// Register to palm anchors before sampling fingertips. Generated camera/hand
// translations are corrected in source coordinates, never copied into the base.
fn registration(base: &RgbImage, pose: &RgbImage, [x, y, w, h]: Rect) -> (i32, i32) {
    let mut best = (u64::MAX, 0, 0);
    for dy in -6..=6 {
        for dx in -6..=6 {
            let mut score = 0u64;
            for py in y..y + h {
                for px in x..x + w {
                    let sx = px as i32 + dx;
                    let sy = py as i32 + dy;
                    if !(0..64).contains(&sx) || !(0..64).contains(&sy) {
                        score += 1_000_000;
                        continue;
                    }
                    let a = *base.get_pixel(px, py);
                    let b = *pose.get_pixel(sx as u32, sy as u32);
                    score += u64::from(distance(a, b));
                    if skin(a) != skin(b) {
                        score += 150_000;
                    }
                }
            }
            score += (dx.abs() + dy.abs()) as u64;
            if score < best.0 {
                best = (score, dx, dy);
            }
        }
    }
    (best.1, best.2)
}

fn finger_palette(base: &RgbImage, [x, y, w, h]: Rect) -> Vec<Rgb<u8>> {
    let mut colors = Vec::new();
    for py in y.saturating_sub(1)..(y + h + 1).min(64) {
        for px in x.saturating_sub(1)..(x + w + 1).min(64) {
            let color = *base.get_pixel(px, py);
            if !colors.contains(&color) {
                colors.push(color);
            }
        }
    }
    colors
}

fn cropped_car(path: &Path, size: [u32; 2]) -> Result<RgbaImage> {
    ensure!(
        size.iter().all(|v| (1..=64).contains(v)),
        "invalid car size"
    );
    let car = image::open(path)?.to_rgba8();
    ensure!(
        car.pixels().any(|p| p[3] == 0),
        "car source must have genuine transparency"
    );
    let mut bounds = [car.width(), car.height(), 0, 0];
    for (x, y, p) in car.enumerate_pixels() {
        if p[3] >= 128 {
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + 1);
            bounds[3] = bounds[3].max(y + 1);
        }
    }
    ensure!(
        bounds[2] > bounds[0] && bounds[3] > bounds[1],
        "empty car sprite"
    );
    let crop = image::imageops::crop_imm(
        &car,
        bounds[0],
        bounds[1],
        bounds[2] - bounds[0],
        bounds[3] - bounds[1],
    )
    .to_image();
    Ok(image::imageops::resize(
        &crop,
        size[0],
        size[1],
        FilterType::Nearest,
    ))
}

// Preserve a thin generated green trace when reducing it to a tiny screen.
// Maximum color sampling prevents nearest-neighbor sampling from losing strokes.
fn reduced_trace(path: &Path, [_, _, w, h]: Rect) -> Result<RgbaImage> {
    let source = image::open(path)?.to_rgb8();
    let mut trace = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let x0 = x * source.width() / w;
            let x1 = ((x + 1) * source.width()).div_ceil(w);
            let y0 = y * source.height() / h;
            let y1 = ((y + 1) * source.height()).div_ceil(h);
            let pixel = (y0..y1)
                .flat_map(|sy| (x0..x1).map(move |sx| (sx, sy)))
                .map(|(sx, sy)| *source.get_pixel(sx, sy))
                .max_by_key(|p| i32::from(p[1]) - i32::from(p[0].max(p[2])))
                .unwrap();
            if pixel[1] > 100
                && pixel[1] > pixel[0].saturating_add(40)
                && pixel[1] > pixel[2].saturating_add(40)
            {
                trace.put_pixel(x, y, image::Rgba([pixel[0], pixel[1], pixel[2], 255]));
            }
        }
    }
    ensure!(
        trace.pixels().any(|p| p[3] > 0),
        "no green trace in waveform source"
    );
    Ok(trace)
}

fn draw_car(frame: &mut RgbImage, car: &RgbaImage, clip: Rect, car_x: i32, car_y: i32) {
    for (x, y, pixel) in car.enumerate_pixels() {
        let dx = car_x + x as i32;
        let dy = car_y + y as i32;
        if dx < 0 || dy < 0 || dx >= 64 || dy >= 64 || !contains(clip, dx as u32, dy as u32) {
            continue;
        }
        let old = *frame.get_pixel(dx as u32, dy as u32);
        let alpha = u32::from(pixel[3]);
        frame.put_pixel(
            dx as u32,
            dy as u32,
            Rgb(std::array::from_fn(|c| {
                ((u32::from(pixel[c]) * alpha + u32::from(old[c]) * (255 - alpha) + 127) / 255)
                    as u8
            })),
        );
    }
}

fn gif_preview(
    path: &Path,
    frames: &[RgbImage],
    palette: &[u8],
    scale: u32,
    frame_ms: u32,
) -> Result<()> {
    let size = (64 * scale) as u16;
    let mut encoder = gif::Encoder::new(fs::File::create(path)?, size, size, palette)?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let mut cache = HashMap::<[u8; 3], u8>::new();
    for (i, image) in frames.iter().enumerate() {
        let indices: Vec<u8> = image
            .pixels()
            .map(|p| {
                *cache.entry(p.0).or_insert_with(|| {
                    palette
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, c)| distance(*p, Rgb([c[0], c[1], c[2]])))
                        .unwrap()
                        .0 as u8
                })
            })
            .collect();
        let mut enlarged = Vec::with_capacity(usize::from(size).pow(2));
        for y in 0..64 {
            for _ in 0..scale {
                for x in 0..64 {
                    for _ in 0..scale {
                        enlarged.push(indices[y * 64 + x]);
                    }
                }
            }
        }
        let mut frame = gif::Frame::from_indexed_pixels(size, size, enlarged, None);
        // Distribute 10ms GIF ticks across frames, preserving the 83ms average.
        frame.delay = (((i as u32 + 1) * frame_ms / 10) - (i as u32 * frame_ms / 10)) as u16;
        frame.dispose = gif::DisposalMethod::Keep;
        encoder.write_frame(&frame)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let plan = PathBuf::from(args.next().context("provide authoring JSON")?);
    let output = PathBuf::from(args.next().context("provide new output directory")?);
    ensure!(args.next().is_none(), "unexpected arguments");
    ensure!(
        !output.exists(),
        "output exists; use a new revision build directory"
    );
    let spec: Spec = serde_json::from_str(&fs::read_to_string(&plan)?)?;
    let root = plan.parent().unwrap_or(Path::new("."));
    let base = image::open(root.join(&spec.base))?.to_rgb8();
    ensure!(base.dimensions() == (64, 64), "base must be 64x64");
    ensure!(
        (24..=40).contains(&spec.pose_schedule.len()),
        "sustained loop requires 24-40 frames"
    );
    ensure!(
        (10..=60_000).contains(&spec.frame_ms),
        "invalid frame timing"
    );
    let mut dynamic = vec![spec.car_clip];
    if let Some(columns) = &spec.monitor_aperture {
        let [x, y, w, h] = spec.monitor;
        ensure!(
            w > 0
                && h > 0
                && x.checked_add(w).is_some_and(|v| v <= 64)
                && y.checked_add(h).is_some_and(|v| v <= 64),
            "monitor outside canvas"
        );
        ensure!(
            !columns.is_empty()
                && columns.iter().all(|[px, top, bottom]| {
                    *px >= x
                        && *px < x + w
                        && *top >= y
                        && *top <= *bottom
                        && *bottom < y + h
                })
                && columns
                    .iter()
                    .map(|c| c[0])
                    .collect::<HashSet<_>>()
                    .len()
                    == columns.len(),
            "invalid monitor aperture columns"
        );
        dynamic.extend(
            columns
                .iter()
                .map(|[px, top, bottom]| [*px, *top, 1, bottom - top + 1]),
        );
    } else {
        dynamic.push(spec.monitor);
    }
    if let Some(cursor) = spec.cursor {
        dynamic.push(cursor);
    }
    if let Some(waveform) = &spec.waveform {
        dynamic.push(waveform.area);
    }
    if let Some(car) = &spec.second_car {
        dynamic.push(car.clip);
    }
    dynamic.extend(spec.fingers.iter().map(|f| f.area));
    for [x, y, w, h] in &dynamic {
        ensure!(
            *w > 0
                && *h > 0
                && x.checked_add(*w).is_some_and(|v| v <= 64)
                && y.checked_add(*h).is_some_and(|v| v <= 64),
            "region outside canvas"
        );
    }
    for f in &spec.fingers {
        let [x, y, w, h] = f.anchor;
        ensure!(
            w > 0 && h > 0 && x + w <= 64 && y + h <= 64,
            "invalid palm anchor"
        );
        ensure!(
            (y..y + h).all(|py| (x..x + w).all(|px| !contains(f.area, px, py))),
            "palm anchors must not be animated"
        );
        ensure!(
            !f.pixels.is_empty() && f.pixels.iter().all(|[px, py]| contains(f.area, *px, *py)),
            "fingertip mask must be nonempty and inside its area"
        );
    }
    let poses: Vec<RgbImage> = (0..=spec.pose_schedule.iter().max().copied().unwrap())
        .map(|i| {
            image::open(root.join(&spec.poses).join(format!("{i:03}.png"))).map(|i| i.to_rgb8())
        })
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        poses.iter().all(|i| i.dimensions() == (64, 64)),
        "poses must be 64x64"
    );
    let palettes: Vec<_> = spec
        .fingers
        .iter()
        .map(|f| finger_palette(&base, f.area))
        .collect();
    let car = cropped_car(&root.join(&spec.car), spec.car_size)?;
    let second_car = spec
        .second_car
        .as_ref()
        .map(|c| -> Result<RgbaImage> {
            let sprite = cropped_car(&root.join(&spec.car), c.size)?;
            Ok(if c.mirror {
                image::imageops::flip_horizontal(&sprite)
            } else {
                sprite
            })
        })
        .transpose()?;
    let waveform = spec
        .waveform
        .as_ref()
        .map(|w| reduced_trace(&root.join(&w.source), w.area))
        .transpose()?;
    let mut frames = Vec::new();
    let mut registrations = Vec::new();
    for (i, pose_index) in spec.pose_schedule.iter().copied().enumerate() {
        let pose = &poses[pose_index];
        let mut frame = base.clone();
        let mut offsets = Vec::new();
        for (j, finger) in spec.fingers.iter().enumerate() {
            let (dx, dy) = registration(&base, pose, finger.anchor);
            offsets.push([dx, dy]);
            if spec.neutral_poses.contains(&pose_index) || !finger.poses.contains(&pose_index) {
                continue;
            }
            let [x, y, w, h] = finger.area;
            for py in y..y + h {
                for px in x..x + w {
                    if !finger.pixels.contains(&[px, py]) {
                        continue;
                    }
                    let sx = (px as i32 + dx).clamp(0, 63) as u32;
                    let sy = (py as i32 + dy).clamp(0, 63) as u32;
                    let source = *pose.get_pixel(sx, sy);
                    let pixel = *palettes[j]
                        .iter()
                        .min_by_key(|c| distance(source, **c))
                        .unwrap();
                    // A fingertip may flex one pixel beyond its original silhouette.
                    if skin(pixel) && !skin(*base.get_pixel(px, py)) {
                        let adjacent = (py.saturating_sub(1)..=(py + 1).min(63)).any(|ay| {
                            (px.saturating_sub(1)..=(px + 1).min(63))
                                .any(|ax| skin(*base.get_pixel(ax, ay)))
                        });
                        if !adjacent {
                            continue;
                        }
                    }
                    frame.put_pixel(px, py, pixel);
                }
            }
        }
        registrations.push(offsets);
        if let Some(columns) = &spec.monitor_aperture {
            // Scroll the whole angled glass, with one complete cycle per loop.
            // A separate column period preserves the drawn perspective and
            // never samples the stationary bezel above or below the screen.
            for [x, top, bottom] in columns {
                let height = bottom - top + 1;
                let shift = i as u32 * height / spec.pose_schedule.len() as u32;
                for y in *top..=*bottom {
                    let source_y = top + (y - top + shift) % height;
                    frame.put_pixel(*x, y, *base.get_pixel(*x, source_y));
                }
            }
        } else {
            let [x, y, w, h] = spec.monitor;
            let shift = (i as u32 / 2) % h;
            for py in 0..h {
                for px in 0..w {
                    frame.put_pixel(
                        x + px,
                        y + py,
                        *base.get_pixel(x + px, y + (py + shift) % h),
                    );
                }
            }
        }
        if let Some([x, y, w, h]) = spec.cursor
            && i < spec.cursor_on_frames
        {
            for py in y..y + h {
                for px in x..x + w {
                    frame.put_pixel(px, py, Rgb(spec.cursor_color));
                }
            }
        }
        if let (Some(wave), Some(trace)) = (&spec.waveform, &waveform) {
            let [x, y, w, h] = wave.area;
            let shift = i as u32 * w / spec.pose_schedule.len() as u32;
            for py in 0..h {
                for px in 0..w {
                    let pixel = trace.get_pixel((px + shift) % w, py);
                    if pixel[3] != 0 {
                        frame.put_pixel(x + px, y + py, Rgb([pixel[0], pixel[1], pixel[2]]));
                    }
                }
            }
        }
        let car_x = spec.car_start_x
            + (spec.car_end_x - spec.car_start_x) * i as i32
                / (spec.pose_schedule.len() - 1) as i32;
        draw_car(&mut frame, &car, spec.car_clip, car_x, spec.car_y);
        if let (Some(c), Some(sprite)) = (&spec.second_car, &second_car) {
            let car_x = c.start_x
                + (c.end_x - c.start_x) * i as i32 / (spec.pose_schedule.len() - 1) as i32;
            draw_car(&mut frame, sprite, c.clip, car_x, c.y);
        }
        for (x, y, p) in frame.enumerate_pixels() {
            ensure!(
                dynamic.iter().any(|r| contains(*r, x, y)) || p == base.get_pixel(x, y),
                "fixed pixel changed at {x},{y}"
            );
        }
        frames.push(frame);
    }
    fs::create_dir_all(output.join("frames"))?;
    base.save(output.join("base.png"))?;
    car.save(output.join("car-sprite.png"))?;
    if let Some(car) = second_car {
        car.save(output.join("second-car-sprite.png"))?;
    }
    if let Some(waveform) = waveform {
        waveform.save(output.join("waveform-sprite.png"))?;
    }
    let mut sheet = RgbImage::new(256, (frames.len() as u32).div_ceil(4) * 64);
    let mut contact = RgbImage::new(512, (frames.len() as u32).div_ceil(8) * 64);
    for (i, frame) in frames.iter().enumerate() {
        frame.save(output.join("frames").join(format!("{i:03}.png")))?;
        image::imageops::replace(&mut sheet, frame, (i as i64 % 4) * 64, (i as i64 / 4) * 64);
        image::imageops::replace(
            &mut contact,
            frame,
            (i as i64 % 8) * 64,
            (i as i64 / 8) * 64,
        );
    }
    sheet.save(output.join("idle-sprite.png"))?;
    image::imageops::resize(&contact, 1024, contact.height() * 2, FilterType::Nearest)
        .save(output.join("contact.png"))?;
    let mut hands = RgbImage::new(8 * 24, (frames.len() as u32).div_ceil(8) * 15);
    for (i, frame) in frames.iter().enumerate() {
        image::imageops::replace(
            &mut hands,
            &image::imageops::crop_imm(frame, 27, 48, 24, 15).to_image(),
            (i as i64 % 8) * 24,
            (i as i64 / 8) * 15,
        );
    }
    image::imageops::resize(
        &hands,
        hands.width() * 8,
        hands.height() * 8,
        FilterType::Nearest,
    )
    .save(output.join("hands-contact.png"))?;
    if spec.monitor_aperture.is_some() {
        let [x, y, w, h] = spec.monitor;
        let left = x.saturating_sub(3);
        let top = y.saturating_sub(1);
        let width = (x + w + 1).min(64) - left;
        let height = (y + h + 1).min(64) - top;
        let mut monitors =
            RgbImage::new(8 * width, (frames.len() as u32).div_ceil(8) * height);
        for (i, frame) in frames.iter().enumerate() {
            image::imageops::replace(
                &mut monitors,
                &image::imageops::crop_imm(frame, left, top, width, height).to_image(),
                (i as i64 % 8) * i64::from(width),
                (i as i64 / 8) * i64::from(height),
            );
        }
        image::imageops::resize(
            &monitors,
            monitors.width() * 8,
            monitors.height() * 8,
            FilterType::Nearest,
        )
        .save(output.join("monitor-contact.png"))?;
    }
    let palette = if let Some(path) = &spec.palette_from {
        let decoder = gif::DecodeOptions::new().read_info(fs::File::open(root.join(path))?)?;
        decoder
            .global_palette()
            .context("reference GIF has no global palette")?
            .to_vec()
    } else {
        gif::Frame::from_rgb_speed(
            sheet.width() as u16,
            sheet.height() as u16,
            sheet.as_raw(),
            1,
        )
        .palette
        .unwrap()
    };
    gif_preview(
        &output.join("idle-native.gif"),
        &frames,
        &palette,
        1,
        spec.frame_ms,
    )?;
    gif_preview(
        &output.join("idle-preview.gif"),
        &frames,
        &palette,
        8,
        spec.frame_ms,
    )?;
    let unique = frames
        .iter()
        .map(|f| f.as_raw())
        .collect::<HashSet<_>>()
        .len();
    fs::write(
        output.join("pet.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1,"id":spec.id,"canvas_size":64,"background":"#000000",
            "default_animation":"idle","animations":{"idle":{"source":{"type":"sprite_sheet",
                "path":"idle-sprite.png","columns":4,"frame_count":frames.len()},"frame_duration_ms":spec.frame_ms,"loop":true}}
        }))?,
    )?;
    fs::write(
        output.join("qa.json"),
        serde_json::to_vec_pretty(&json!({
            "frames":frames.len(),"unique_frames":unique,"frame_ms":spec.frame_ms,
            "duration_ms":frames.len() as u32*spec.frame_ms,"gif_duration_ms":frames.len() as u32*spec.frame_ms/10*10,
            "fixed_regions_match_base":true,"global_gif_palette":true,
            "dynamic_regions":dynamic,"registration_offsets":registrations,
            "monitor_aperture":spec.monitor_aperture
        }))?,
    )?;
    println!(
        "Prepared {} frames ({} unique) in {}",
        frames.len(),
        unique,
        output.display()
    );
    Ok(())
}
