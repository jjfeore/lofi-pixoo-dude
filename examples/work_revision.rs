//! Offline compositing of generated work layers onto the approved decker base.
//! No network access or bridge-runtime changes. Output must be a new directory.
use anyhow::{Context, Result, ensure};
use image::{Rgb, RgbImage, Rgba, RgbaImage, imageops::FilterType};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

type Rect = [u32; 4];

#[derive(Deserialize)]
struct Spec {
    id: String,
    base: PathBuf,
    idle: PathBuf,
    forehead: PathBuf,
    visor: PathBuf,
    main_screen: PathBuf,
    processing: PathBuf,
    waveform: PathBuf,
    cyan_car: PathBuf,
    magenta_car: PathBuf,
    frames: usize,
    frame_ms: u32,
    main_area: Rect,
    main_aperture: Option<Vec<[u32; 3]>>,
    rear_area: Rect,
    visor_up: Rect,
    visor_down: Rect,
    visor_mask: Vec<[u32; 2]>,
    finger_pixels: Vec<[u32; 2]>,
    traffic: HashMap<String, Vec<Traffic>>,
    gesture: Option<Gesture>,
    palette_from: Option<PathBuf>,
    character: Option<Character>,
    motorized: Option<MotorizedVisor>,
}

#[derive(Deserialize, serde::Serialize)]
struct MotorizedVisor {
    flip_frames: [usize; 2],
    screen_frames: [usize; 2],
    minimum_height: u32,
}

#[derive(Deserialize)]
struct Character {
    source: PathBuf,
    clean: PathBuf,
    columns: u32,
    pose_count: usize,
    size: [u32; 2],
    origin: [u32; 2],
    area: Rect,
    clear_regions: Vec<Rect>,
    enter_poses: Vec<usize>,
    exit_poses: Vec<usize>,
    work_start: usize,
    work_end: usize,
    forehead_edit: Option<ForeheadEdit>,
    additional_atlases: Option<Vec<CharacterAtlas>>,
}

#[derive(Deserialize, serde::Serialize)]
struct CharacterAtlas {
    source: PathBuf,
    columns: u32,
    pose_count: usize,
    size: Option<[u32; 2]>,
    origin: Option<[u32; 2]>,
}

#[derive(Deserialize)]
struct ForeheadEdit {
    source: PathBuf,
    poses: Vec<usize>,
    area: Rect,
    source_poses: Option<Vec<usize>>,
}

struct CharacterLayers {
    room: RgbImage,
    poses: Vec<RgbaImage>,
}

#[derive(Deserialize)]
struct Gesture {
    source: PathBuf,
    clean: PathBuf,
    source_shoulder: [f32; 2],
    shoulder: [f32; 2],
    times: Vec<usize>,
    enter_hands: Vec<[f32; 2]>,
    exit_hands: Vec<[f32; 2]>,
    removal_regions: Vec<Rect>,
    removal_pixels: Vec<[u32; 2]>,
    arm_area: Rect,
    interrupted: bool,
    rig: Option<Rig>,
    #[serde(default)]
    foreground: bool,
    #[serde(default)]
    retained_regions: Vec<Rect>,
}

#[derive(Deserialize)]
struct Rig {
    upper: PathBuf,
    fore: PathBuf,
    hand: PathBuf,
    upper_length: u32,
    fore_length: u32,
    sleeve_width: u32,
    fore_width: Option<u32>,
    hand_size: [u32; 2],
    hand_pivot: [f32; 2],
    hand_angles: Vec<f32>,
}

struct RigLayers {
    upper: RgbaImage,
    fore: RgbaImage,
    hand: RgbaImage,
}

struct RigPose {
    arm: RgbaImage,
    hand: RgbaImage,
    elbow: [f32; 2],
    wrist: [f32; 2],
    hand_angle: f32,
}

struct ArmLayers {
    clean: RgbImage,
    poses: Vec<RgbaImage>,
    hands: Vec<[f32; 2]>,
    rig: Option<RigLayers>,
}

#[derive(Deserialize, serde::Serialize)]
struct Traffic {
    sprite: String,
    lane: i32,
    start_x: i32,
    end_x: i32,
    passes: u32,
    mirror: bool,
}

struct Layers {
    base: RgbImage,
    underlay: RgbImage,
    idle: Vec<RgbImage>,
    raised: RgbaImage,
    visor: RgbaImage,
    main: RgbImage,
    processing: RgbaImage,
    wave: RgbaImage,
    cars: HashMap<String, RgbaImage>,
    arm: Option<ArmLayers>,
    character: Option<CharacterLayers>,
}

fn load_character(root: &Path, base: &RgbImage, spec: &Character) -> Result<CharacterLayers> {
    ensure!(
        spec.columns > 0 && spec.pose_count > 0 && spec.pose_count <= 40,
        "invalid character atlas geometry"
    );
    let rows = (spec.pose_count as u32).div_ceil(spec.columns);
    let source = image::open(root.join(&spec.source))?.to_rgba8();
    ensure!(
        source.width() * rows == source.height() * spec.columns,
        "character atlas cells must be square"
    );
    ensure!(
        source.pixels().any(|p| p[3] == 0),
        "character atlas needs true alpha"
    );
    ensure!(
        spec.size.iter().all(|n| *n > 0)
            && spec.origin[0] + spec.size[0] <= 64
            && spec.origin[1] + spec.size[1] <= 64,
        "character projection outside canvas"
    );
    let [ax, ay, aw, ah] = spec.area;
    ensure!(
        aw > 0 && ah > 0 && ax + aw <= 64 && ay + ah <= 64,
        "character area outside canvas"
    );
    let total_poses = spec.pose_count
        + spec.additional_atlases.as_ref().map_or(0, |atlases| {
            atlases.iter().map(|atlas| atlas.pose_count).sum::<usize>()
        });
    ensure!(total_poses <= 128, "too many character source poses");
    ensure!(
        spec.enter_poses.len() == 32
            && spec.exit_poses.len() == 32
            && spec
                .enter_poses
                .iter()
                .chain(&spec.exit_poses)
                .all(|n| *n < total_poses)
            && spec.work_start < spec.work_end
            && spec.work_end < 32,
        "invalid character timeline"
    );
    let clean = image::open(root.join(&spec.clean))?.to_rgb8();
    ensure!(
        clean.dimensions() == (64, 64),
        "character clean plate must be 64x64"
    );
    let mut room = base.clone();
    for [x, y, w, h] in &spec.clear_regions {
        ensure!(
            *w > 0 && *h > 0 && *x + *w <= 64 && *y + *h <= 64,
            "character clear region outside canvas"
        );
        for py in *y..y + h {
            for px in *x..x + w {
                room.put_pixel(px, py, *clean.get_pixel(px, py));
            }
        }
    }
    let palette: Vec<_> = base.pixels().map(|p| p.0).collect();
    let forehead_source: Option<RgbaImage> = spec
        .forehead_edit
        .as_ref()
        .map(|edit| -> Result<RgbaImage> {
            let source = image::open(root.join(&edit.source))?.to_rgba8();
            ensure!(
                source.width() * rows == source.height() * spec.columns,
                "forehead edit atlas geometry differs"
            );
            ensure!(
                edit.poses.iter().all(|i| *i < spec.pose_count),
                "forehead edit index outside atlas"
            );
            ensure!(
                edit.source_poses
                    .as_ref()
                    .is_none_or(|indices| indices.len() == spec.pose_count
                        && indices.iter().all(|i| *i < spec.pose_count)),
                "forehead edit source mapping outside atlas"
            );
            let [x, y, w, h] = edit.area;
            ensure!(
                w > 0 && h > 0 && x + w <= 64 && y + h <= 64,
                "forehead edit region outside canvas"
            );
            Ok(source)
        })
        .transpose()?;
    let mut poses = Vec::new();
    for i in 0..spec.pose_count {
        let column = i as u32 % spec.columns;
        let row = i as u32 / spec.columns;
        let x0 = column * source.width() / spec.columns;
        let x1 = (column + 1) * source.width() / spec.columns;
        let y0 = row * source.height() / rows;
        let y1 = (row + 1) * source.height() / rows;
        let cell = image::imageops::crop_imm(&source, x0, y0, x1 - x0, y1 - y0).to_image();
        // One common scene projection, never pose-dependent limb fitting.
        let mut native =
            image::imageops::resize(&cell, spec.size[0], spec.size[1], FilterType::Nearest);
        if let (Some(edit), Some(edited)) = (&spec.forehead_edit, &forehead_source)
            && edit.poses.contains(&i)
        {
            let edited_index = edit.source_poses.as_ref().map_or(i, |indices| indices[i]) as u32;
            let edited_column = edited_index % spec.columns;
            let edited_row = edited_index / spec.columns;
            let ex0 = edited_column * edited.width() / spec.columns;
            let ex1 = (edited_column + 1) * edited.width() / spec.columns;
            let ey0 = edited_row * edited.height() / rows;
            let ey1 = (edited_row + 1) * edited.height() / rows;
            let edit_cell =
                image::imageops::crop_imm(edited, ex0, ey0, ex1 - ex0, ey1 - ey0).to_image();
            let edit_native = image::imageops::resize(
                &edit_cell,
                spec.size[0],
                spec.size[1],
                FilterType::Nearest,
            );
            for (x, y, pixel) in edit_native.enumerate_pixels() {
                if contains(edit.area, x + spec.origin[0], y + spec.origin[1]) {
                    native.put_pixel(x, y, *pixel);
                }
            }
        }
        let mut pose = RgbaImage::new(64, 64);
        for (x, y, pixel) in native.enumerate_pixels() {
            if pixel[3] < 128 {
                continue;
            }
            let px = x + spec.origin[0];
            let py = y + spec.origin[1];
            if contains(spec.area, px, py) {
                let rgb = [pixel[0], pixel[1], pixel[2]];
                let color = palette.iter().min_by_key(|p| distance(rgb, **p)).unwrap();
                pose.put_pixel(px, py, Rgba([color[0], color[1], color[2], 255]));
            }
        }
        ensure!(pose.pixels().any(|p| p[3] > 0), "empty character pose {i}");
        poses.push(pose);
    }
    // Each repair atlas has one complete-character projection. Registration is
    // fixed for the entire source, never fitted to individual limbs or frames.
    if let Some(atlases) = &spec.additional_atlases {
        for atlas in atlases {
            ensure!(
                atlas.columns > 0 && atlas.pose_count > 0 && atlas.pose_count <= 40,
                "invalid additional character atlas geometry"
            );
            let rows = (atlas.pose_count as u32).div_ceil(atlas.columns);
            let source = image::open(root.join(&atlas.source))?.to_rgba8();
            ensure!(
                source.width() * rows == source.height() * atlas.columns,
                "additional character atlas cells must be square"
            );
            ensure!(
                source.pixels().any(|p| p[3] == 0),
                "additional character atlas needs true alpha"
            );
            let size = atlas.size.unwrap_or(spec.size);
            let origin = atlas.origin.unwrap_or(spec.origin);
            ensure!(
                size.iter().all(|n| *n > 0)
                    && origin[0] + size[0] <= 64
                    && origin[1] + size[1] <= 64,
                "additional character projection outside canvas"
            );
            for i in 0..atlas.pose_count {
                let column = i as u32 % atlas.columns;
                let row = i as u32 / atlas.columns;
                let x0 = column * source.width() / atlas.columns;
                let x1 = (column + 1) * source.width() / atlas.columns;
                let y0 = row * source.height() / rows;
                let y1 = (row + 1) * source.height() / rows;
                let cell = image::imageops::crop_imm(&source, x0, y0, x1 - x0, y1 - y0).to_image();
                let native = image::imageops::resize(&cell, size[0], size[1], FilterType::Nearest);
                let mut pose = RgbaImage::new(64, 64);
                for (x, y, pixel) in native.enumerate_pixels() {
                    if pixel[3] < 128 {
                        continue;
                    }
                    let px = x + origin[0];
                    let py = y + origin[1];
                    if contains(spec.area, px, py) {
                        let rgb = [pixel[0], pixel[1], pixel[2]];
                        let color = palette.iter().min_by_key(|p| distance(rgb, **p)).unwrap();
                        pose.put_pixel(px, py, Rgba([color[0], color[1], color[2], 255]));
                    }
                }
                ensure!(pose.pixels().any(|p| p[3] > 0), "empty additional pose {i}");
                poses.push(pose);
            }
        }
    }
    Ok(CharacterLayers { room, poses })
}

fn character_pose_index(spec: &Character, name: &str, i: usize) -> usize {
    if name == "working-enter" {
        spec.enter_poses[i]
    } else {
        spec.exit_poses[i]
    }
}

fn skin(pixel: [u8; 3]) -> bool {
    pixel[0] > 85
        && pixel[0] > pixel[1].saturating_add(15)
        && pixel[0] > pixel[2].saturating_add(15)
}

fn load_arm(root: &Path, base: &RgbImage, gesture: &Gesture) -> Result<ArmLayers> {
    ensure!(
        gesture.times.len() == 16
            && gesture.enter_hands.len() == 16
            && gesture.exit_hands.len() == 16,
        "gesture needs sixteen key poses"
    );
    ensure!(
        gesture.times.windows(2).all(|p| p[0] < p[1]),
        "gesture times must increase"
    );
    let sheet = if gesture.rig.is_none() {
        let source = image::open(root.join(&gesture.source))?.to_rgba8();
        ensure!(
            source.pixels().any(|p| p[3] == 0),
            "arm atlas must be transparent"
        );
        ensure!(
            source.width() == source.height(),
            "arm atlas must be a square 4x4 grid"
        );
        image::imageops::resize(&source, 256, 256, FilterType::Nearest)
    } else {
        RgbaImage::new(0, 0)
    };
    let skin_palette: Vec<_> = base
        .enumerate_pixels()
        .filter(|(x, y, p)| contains([35, 54, 13, 9], *x, *y) && skin(p.0))
        .map(|(_, _, p)| p.0)
        .collect();
    let sleeve_palette: Vec<_> = base
        .enumerate_pixels()
        .filter(|(x, y, p)| contains([10, 40, 24, 16], *x, *y) && !skin(p.0))
        .map(|(_, _, p)| p.0)
        .collect();
    let mut poses = Vec::new();
    let mut hands = Vec::new();
    for i in 0..if gesture.rig.is_some() { 0 } else { 16 } {
        let mut pose =
            image::imageops::crop_imm(&sheet, (i % 4) * 64, (i / 4) * 64, 64, 64).to_image();
        let mut hand_bounds = [64, 64, 0, 0];
        for (x, y, pixel) in pose.enumerate_pixels_mut() {
            if pixel[3] < 128 {
                *pixel = Rgba([0, 0, 0, 0]);
                continue;
            }
            pixel[3] = 255;
            let is_skin = skin([pixel[0], pixel[1], pixel[2]]);
            if is_skin {
                hand_bounds[0] = hand_bounds[0].min(x);
                hand_bounds[1] = hand_bounds[1].min(y);
                hand_bounds[2] = hand_bounds[2].max(x + 1);
                hand_bounds[3] = hand_bounds[3].max(y + 1);
            }
            let palette = if is_skin {
                &skin_palette
            } else {
                &sleeve_palette
            };
            let color = *palette
                .iter()
                .min_by_key(|c| distance([pixel[0], pixel[1], pixel[2]], **c))
                .unwrap();
            *pixel = Rgba([color[0], color[1], color[2], 255]);
        }
        ensure!(hand_bounds[2] > hand_bounds[0], "pose {i} has no hand");
        hands.push([
            (hand_bounds[0] + hand_bounds[2] - 1) as f32 / 2.0,
            (hand_bounds[1] + hand_bounds[3] - 1) as f32 / 2.0,
        ]);
        poses.push(pose);
    }
    let mut clean = image::open(root.join(&gesture.clean))?.to_rgb8();
    ensure!(
        clean.dimensions() == (64, 64),
        "arm clean plate must be native 64x64"
    );
    for region in &gesture.removal_regions {
        for y in region[1]..region[1] + region[3] {
            for x in region[0]..region[0] + region[2] {
                // The old hand is erased before compositing a replacement.
                ensure!(
                    gesture.retained_regions.iter().any(|r| contains(*r, x, y))
                        || !skin(clean.get_pixel(x, y).0),
                    "clean plate retains hand skin at {x},{y}"
                );
            }
        }
    }
    for [x, y] in &gesture.removal_pixels {
        ensure!(
            !skin(clean.get_pixel(*x, *y).0),
            "clean plate retains old finger"
        );
    }
    // Color-register only the sampled clean region to the approved scene palette.
    let palette: Vec<_> = base
        .enumerate_pixels()
        .filter(|(x, y, p)| {
            (contains([28, 47, 22, 11], *x, *y)
                || (gesture.foreground && contains([0, 40, 35, 24], *x, *y)))
                && !skin(p.0)
        })
        .map(|(_, _, p)| p.0)
        .collect();
    for pixel in clean.pixels_mut() {
        pixel.0 = *palette
            .iter()
            .min_by_key(|c| distance(pixel.0, **c))
            .unwrap();
    }
    let rig = gesture
        .rig
        .as_ref()
        .map(|rig| -> Result<RigLayers> {
            ensure!(
                rig.hand_angles.len() == 16
                    && (1..=30).contains(&rig.upper_length)
                    && (1..=30).contains(&rig.fore_length)
                    && (1..=16).contains(&rig.sleeve_width)
                    && (1..=16).contains(&rig.fore_width.unwrap_or(rig.sleeve_width)),
                "invalid articulated rig"
            );
            let mut upper = cropped_sprite(
                &root.join(&rig.upper),
                rig.upper_length + 1,
                rig.sleeve_width,
            )?;
            let mut fore = cropped_sprite(
                &root.join(&rig.fore),
                rig.fore_length + 1,
                rig.fore_width.unwrap_or(rig.sleeve_width),
            )?;
            let mut hand =
                cropped_sprite(&root.join(&rig.hand), rig.hand_size[0], rig.hand_size[1])?;
            // Textures are normalized once. Frames only rotate and translate them.
            for (texture, is_hand) in [(&mut upper, false), (&mut fore, false), (&mut hand, true)] {
                for pixel in texture.pixels_mut() {
                    if pixel[3] < 128 {
                        *pixel = Rgba([0, 0, 0, 0]);
                        continue;
                    }
                    let palette = if is_hand && skin([pixel[0], pixel[1], pixel[2]]) {
                        &skin_palette
                    } else {
                        &sleeve_palette
                    };
                    let color = *palette
                        .iter()
                        .min_by_key(|c| distance([pixel[0], pixel[1], pixel[2]], **c))
                        .unwrap();
                    *pixel = Rgba([color[0], color[1], color[2], 255]);
                }
            }
            Ok(RigLayers { upper, fore, hand })
        })
        .transpose()?;
    Ok(ArmLayers {
        clean,
        poses,
        hands,
        rig,
    })
}

fn contains([x, y, w, h]: Rect, px: u32, py: u32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn main_screen_pixel(spec: &Spec, x: u32, y: u32) -> bool {
    spec.main_aperture.as_ref().map_or_else(
        || contains(spec.main_area, x, y),
        |columns| {
            columns
                .iter()
                .any(|[px, top, bottom]| x == *px && y >= *top && y <= *bottom)
        },
    )
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| (i32::from(a) - i32::from(b)).pow(2) as u32)
        .sum()
}

fn mix(a: Rgb<u8>, b: Rgb<u8>, amount: f32) -> Rgb<u8> {
    Rgb(std::array::from_fn(|c| {
        (f32::from(a[c]) * (1.0 - amount) + f32::from(b[c]) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    }))
}

fn alpha_over(frame: &mut RgbImage, pixel: Rgba<u8>, x: u32, y: u32) {
    frame.put_pixel(
        x,
        y,
        mix(
            *frame.get_pixel(x, y),
            Rgb([pixel[0], pixel[1], pixel[2]]),
            f32::from(pixel[3]) / 255.0,
        ),
    );
}

fn cropped_sprite(path: &Path, w: u32, h: u32) -> Result<RgbaImage> {
    let source = image::open(path)?.to_rgba8();
    ensure!(
        source.pixels().any(|p| p[3] == 0),
        "sprite needs genuine transparency: {}",
        path.display()
    );
    let mut bounds = [source.width(), source.height(), 0, 0];
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] >= 128 {
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + 1);
            bounds[3] = bounds[3].max(y + 1);
        }
    }
    ensure!(
        bounds[2] > bounds[0] && bounds[3] > bounds[1],
        "empty sprite"
    );
    let crop = image::imageops::crop_imm(
        &source,
        bounds[0],
        bounds[1],
        bounds[2] - bounds[0],
        bounds[3] - bounds[1],
    )
    .to_image();
    Ok(image::imageops::resize(&crop, w, h, FilterType::Nearest))
}

fn load_layers(root: &Path, spec: &Spec) -> Result<Layers> {
    let base = image::open(root.join(&spec.base))?.to_rgb8();
    let clean = image::open(root.join(&spec.forehead))?.to_rgb8();
    ensure!(
        base.dimensions() == (64, 64) && clean.dimensions() == (64, 64),
        "base and clean plate must be native 64x64"
    );
    let idle: Vec<_> = (0..spec.frames)
        .map(|i| {
            image::open(root.join(&spec.idle).join(format!("{i:03}.png"))).map(|p| p.to_rgb8())
        })
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        idle.iter().all(|i| i.dimensions() == (64, 64)),
        "idle frames must be native 64x64"
    );
    let [ux, uy, uw, uh] = spec.visor_up;
    let mut raised = RgbaImage::new(uw, uh);
    let mut underlay = base.clone();
    // Sample only the selected generated clean plate, with the canonical palette.
    // Every source pixel outside the visor footprint is discarded.
    let palette: Vec<_> = base
        .enumerate_pixels()
        .filter(|(x, y, _)| {
            (contains([20, 16, 11, 10], *x, *y) && !spec.visor_mask.contains(&[*x, *y]))
                || contains([19, 25, 9, 8], *x, *y)
        })
        .map(|(_, _, p)| p.0)
        .collect();
    for [x, y] in &spec.visor_mask {
        ensure!(
            contains(spec.visor_up, *x, *y),
            "visor mask outside source crop"
        );
        let old = base.get_pixel(*x, *y);
        raised.put_pixel(x - ux, y - uy, Rgba([old[0], old[1], old[2], 255]));
        let pixel = clean.get_pixel(*x, *y).0;
        let color = *palette.iter().min_by_key(|c| distance(pixel, **c)).unwrap();
        underlay.put_pixel(*x, *y, Rgb(color));
    }
    let visor = cropped_sprite(
        &root.join(&spec.visor),
        spec.visor_down[2],
        spec.visor_down[3],
    )?;
    let main = image::imageops::resize(
        &image::open(root.join(&spec.main_screen))?.to_rgb8(),
        spec.main_area[2],
        spec.main_area[3],
        FilterType::Nearest,
    );
    let processing = cropped_sprite(
        &root.join(&spec.processing),
        spec.rear_area[2] - 2,
        spec.rear_area[3] - 2,
    )?;
    let wave = image::open(root.join(&spec.waveform))?.to_rgba8();
    ensure!(
        wave.dimensions() == (spec.rear_area[2], spec.rear_area[3]),
        "waveform dimensions differ from screen"
    );
    let cyan = image::open(root.join(&spec.cyan_car))?.to_rgba8();
    let magenta = cropped_sprite(&root.join(&spec.magenta_car), 4, 2)?;
    let far = image::imageops::resize(&cyan, 3, 1, FilterType::Nearest);
    let cars = HashMap::from([
        ("cyan".into(), cyan),
        ("magenta".into(), magenta),
        ("cyan-far".into(), far),
    ]);
    let arm = spec
        .gesture
        .as_ref()
        .map(|g| load_arm(root, &base, g))
        .transpose()?;
    let character = spec
        .character
        .as_ref()
        .map(|c| load_character(root, &base, c))
        .transpose()?;
    Ok(Layers {
        base,
        underlay,
        idle,
        raised,
        visor,
        main,
        processing,
        wave,
        cars,
        arm,
        character,
    })
}

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn progress(name: &str, i: usize) -> (f32, f32) {
    match name {
        "working-enter" => (
            smoothstep((i as f32 - 4.0) / 17.0),
            smoothstep((i as f32 - 12.0) / 16.0),
        ),
        "finished" => (
            1.0 - smoothstep((i as f32 - 4.0) / 18.0),
            1.0 - smoothstep((i as f32 - 5.0) / 21.0),
        ),
        _ => (1.0, 1.0),
    }
}

fn gesture_progress(name: &str, i: usize) -> (f32, f32) {
    let lower = smoothstep((i as f32 - 12.0) / 10.0);
    match name {
        "working-enter" => (lower, smoothstep((i as f32 - 14.0) / 12.0)),
        "finished" | "interrupted" => (1.0 - lower, 1.0 - smoothstep((i as f32 - 14.0) / 12.0)),
        _ => (1.0, 1.0),
    }
}

fn motorized_progress(motor: &MotorizedVisor, name: &str, i: usize) -> (f32, f32) {
    let phase = |[start, end]: [usize; 2]| {
        smoothstep((i as f32 - start as f32) / (end - start) as f32)
    };
    let lowered = phase(motor.flip_frames);
    let work = phase(motor.screen_frames);
    if name == "finished" {
        (1.0 - lowered, 1.0 - work)
    } else {
        (lowered, work)
    }
}

fn arm_enabled(spec: &Spec, name: &str, i: usize) -> bool {
    spec.gesture
        .as_ref()
        .is_some_and(|g| name != "working" && i >= g.times[0] && i <= g.times[15])
}

fn rotated_piece(source: &RgbaImage, origin: [f32; 2], pivot: [f32; 2], angle: f32) -> RgbaImage {
    let mut canvas = RgbaImage::new(64, 64);
    let (sin, cos) = angle.sin_cos();
    // Inverse sampling through a pure rotation: determinant and scale stay one.
    for y in 0..64 {
        for x in 0..64 {
            let dx = x as f32 - origin[0];
            let dy = y as f32 - origin[1];
            let sx = (pivot[0] + cos * dx + sin * dy).round() as i32;
            let sy = (pivot[1] - sin * dx + cos * dy).round() as i32;
            if sx >= 0 && sy >= 0 && sx < source.width() as i32 && sy < source.height() as i32 {
                canvas.put_pixel(x, y, *source.get_pixel(sx as u32, sy as u32));
            }
        }
    }
    canvas
}

fn rig_pose(gesture: &Gesture, textures: &RigLayers, name: &str, i: usize) -> Result<RigPose> {
    let rig = gesture.rig.as_ref().context("rig absent")?;
    let key = gesture
        .times
        .iter()
        .rposition(|t| *t <= i)
        .context("before gesture")?;
    let next = (key + 1).min(15);
    let blend = if key == next {
        0.0
    } else {
        (i - gesture.times[key]) as f32 / (gesture.times[next] - gesture.times[key]) as f32
    };
    let targets = if name == "working-enter" {
        &gesture.enter_hands
    } else {
        &gesture.exit_hands
    };
    // In articulated mode these targets specify the wrist, not a source centroid.
    let wrist: [f32; 2] =
        std::array::from_fn(|c| targets[key][c] * (1.0 - blend) + targets[next][c] * blend);
    let delta: [f32; 2] = std::array::from_fn(|c| wrist[c] - gesture.shoulder[c]);
    let d = delta[0].hypot(delta[1]);
    let upper = rig.upper_length as f32;
    let fore = rig.fore_length as f32;
    ensure!(
        d > (upper - fore).abs() + 0.1 && d < upper + fore - 0.1,
        "wrist unreachable without stretching at {name} frame {i}"
    );
    let along = (upper * upper - fore * fore + d * d) / (2.0 * d);
    let bend = (upper * upper - along * along).max(0.0).sqrt();
    // The same elbow branch keeps the joint near the body as the wrist rises.
    let elbow = [
        gesture.shoulder[0] + (delta[0] * along - delta[1] * bend) / d,
        gesture.shoulder[1] + (delta[1] * along + delta[0] * bend) / d,
    ];
    let upper_angle = (elbow[1] - gesture.shoulder[1]).atan2(elbow[0] - gesture.shoulder[0]);
    let fore_angle = (wrist[1] - elbow[1]).atan2(wrist[0] - elbow[0]);
    let hand_angle = rig.hand_angles[key] * (1.0 - blend) + rig.hand_angles[next] * blend;
    let hand = rotated_piece(
        &textures.hand,
        wrist,
        rig.hand_pivot,
        hand_angle.to_radians(),
    );
    let mut arm = RgbaImage::new(64, 64);
    for piece in [
        rotated_piece(
            &textures.upper,
            gesture.shoulder,
            [0.0, (rig.sleeve_width / 2) as f32],
            upper_angle,
        ),
        rotated_piece(
            &textures.fore,
            elbow,
            [0.0, (rig.fore_width.unwrap_or(rig.sleeve_width) / 2) as f32],
            fore_angle,
        ),
        hand.clone(),
    ] {
        for (x, y, pixel) in piece.enumerate_pixels() {
            if pixel[3] > 0 && contains(gesture.arm_area, x, y) && (gesture.foreground || y < 57) {
                arm.put_pixel(x, y, *pixel);
            }
        }
    }
    Ok(RigPose {
        arm,
        hand,
        elbow,
        wrist,
        hand_angle,
    })
}

fn draw_arm(
    frame: &mut RgbImage,
    layers: &Layers,
    spec: &Spec,
    name: &str,
    i: usize,
) -> Result<()> {
    let gesture = spec.gesture.as_ref().context("gesture absent")?;
    let arm = layers.arm.as_ref().context("arm layers absent")?;
    for [x, y, w, h] in &gesture.removal_regions {
        for py in *y..y + h {
            for px in *x..x + w {
                if !gesture
                    .retained_regions
                    .iter()
                    .any(|r| contains(*r, px, py))
                {
                    frame.put_pixel(px, py, *arm.clean.get_pixel(px, py));
                }
            }
        }
    }
    for [x, y] in &gesture.removal_pixels {
        frame.put_pixel(*x, *y, *arm.clean.get_pixel(*x, *y));
    }
    if let Some(textures) = &arm.rig {
        let pose = rig_pose(gesture, textures, name, i)?;
        for (x, y, pixel) in pose.arm.enumerate_pixels() {
            if pixel[3] > 0 {
                alpha_over(frame, *pixel, x, y);
            }
        }
        return Ok(());
    }
    let key = gesture.times.iter().rposition(|t| *t <= i).unwrap();
    let next = (key + 1).min(15);
    let blend = if key == next {
        0.0
    } else {
        (i - gesture.times[key]) as f32 / (gesture.times[next] - gesture.times[key]) as f32
    };
    let targets = if name == "working-enter" {
        &gesture.enter_hands
    } else {
        &gesture.exit_hands
    };
    let target: [f32; 2] =
        std::array::from_fn(|c| targets[key][c] * (1.0 - blend) + targets[next][c] * blend);
    let source: [f32; 2] = std::array::from_fn(|c| arm.hands[key][c] - gesture.source_shoulder[c]);
    let dest: [f32; 2] = std::array::from_fn(|c| target[c] - gesture.shoulder[c]);
    let length = source[0] * source[0] + source[1] * source[1];
    let a = (source[0] * dest[0] + source[1] * dest[1]) / length;
    let b = (source[0] * dest[1] - source[1] * dest[0]) / length;
    let det = a * a + b * b;
    ensure!(det > 0.01, "invalid arm registration");
    let [x, y, w, h] = gesture.arm_area;
    for py in y..y + h {
        for px in x..x + w {
            // Moving far arm is behind the nearer keyboard hand/sleeve.
            if py >= 57 {
                continue;
            }
            let dx = px as f32 - gesture.shoulder[0];
            let dy = py as f32 - gesture.shoulder[1];
            let sx = (gesture.source_shoulder[0] + (a * dx + b * dy) / det).round() as i32;
            let sy = (gesture.source_shoulder[1] + (-b * dx + a * dy) / det).round() as i32;
            if !(0..64).contains(&sx) || !(0..64).contains(&sy) {
                continue;
            }
            alpha_over(
                frame,
                *arm.poses[key].get_pixel(sx as u32, sy as u32),
                px,
                py,
            );
        }
    }
    Ok(())
}

fn visor_bounds(spec: &Spec, lowered: f32, motorized: bool) -> Rect {
    let [ux, uy, uw, uh] = spec.visor_up;
    let [dx, dy, dw, dh] = spec.visor_down;
    let x = (ux as f32 * (1.0 - lowered) + dx as f32 * lowered).round() as u32;
    let w = (uw as f32 * (1.0 - lowered) + dw as f32 * lowered).round() as u32;
    let full_height = uh as f32 * (1.0 - lowered) + dh as f32 * lowered;
    let (y, h) = if motorized {
        let minimum = spec.motorized.as_ref().unwrap().minimum_height as f32;
        // Foreshortening makes the plate turn edge-on halfway through the flip.
        // The head/body remain the approved pixels; only the visor is projected.
        let h = (minimum + (full_height - minimum) * (std::f32::consts::PI * lowered).cos().abs())
            .round() as u32;
        let center = (uy as f32 + uh as f32 / 2.0) * (1.0 - lowered)
            + (dy as f32 + dh as f32 / 2.0) * lowered;
        ((center - h as f32 / 2.0).round() as u32, h)
    } else {
        (
            (uy as f32 * (1.0 - lowered) + dy as f32 * lowered).round() as u32,
            full_height.round() as u32,
        )
    };
    [x, y, w, h]
}

fn draw_visor(
    frame: &mut RgbImage,
    layers: &Layers,
    spec: &Spec,
    lowered: f32,
    i: usize,
    motorized: bool,
) {
    let [x, y, w, h] = visor_bounds(spec, lowered, motorized);
    let old = image::imageops::resize(&layers.raised, w, h, FilterType::Nearest);
    let new = image::imageops::resize(&layers.visor, w, h, FilterType::Nearest);
    let morph = smoothstep((lowered - 0.2) / 0.7);
    for sy in 0..h {
        for sx in 0..w {
            let a = old.get_pixel(sx, sy);
            let b = new.get_pixel(sx, sy);
            // Interpolate premultiplied color: no opaque rectangular copy of hair.
            let alpha = f32::from(a[3]) * (1.0 - morph) + f32::from(b[3]) * morph;
            if alpha < 1.0 {
                continue;
            }
            let mut pixel = Rgba(std::array::from_fn(|c| {
                if c == 3 {
                    alpha.round() as u8
                } else {
                    ((f32::from(a[c]) * f32::from(a[3]) * (1.0 - morph)
                        + f32::from(b[c]) * f32::from(b[3]) * morph)
                        / alpha)
                        .round() as u8
                }
            }));
            // A restrained power pulse confined to the generated cyan lens.
            if lowered > 0.95 && pixel[1] > pixel[0].saturating_add(60) && pixel[2] > 150 {
                let pulse =
                    0.94 + 0.06 * (std::f32::consts::TAU * i as f32 / spec.frames as f32).cos();
                for c in 0..3 {
                    pixel[c] = (f32::from(pixel[c]) * pulse).round() as u8;
                }
            }
            alpha_over(frame, pixel, x + sx, y + sy);
        }
    }
}

fn draw_screens(frame: &mut RgbImage, layers: &Layers, spec: &Spec, work: f32, i: usize) {
    let [x, y, w, h] = spec.main_area;
    if let Some(columns) = &spec.main_aperture {
        let shift = i as u32 * h / spec.frames as u32;
        for [px, top, bottom] in columns {
            for py in *top..=*bottom {
                let sy = (py - top) * h / (bottom - top + 1);
                let idle = *layers.idle[i].get_pixel(*px, py);
                let active = *layers.main.get_pixel(px - x, (sy + shift) % h);
                frame.put_pixel(*px, py, mix(idle, active, work));
            }
        }
    } else {
        for sy in 0..h {
            for sx in 0..w {
                let idle = *layers.base.get_pixel(x + sx, y + (sy + i as u32 / 2) % h);
                let active = *layers.main.get_pixel(sx, (sy + i as u32) % h);
                frame.put_pixel(x + sx, y + sy, mix(idle, active, work));
            }
        }
    }
    let [x, y, w, h] = spec.rear_area;
    let mut idle = layers.base.clone();
    let mut active = layers.base.clone();
    let wave_shift = i as u32 * w / spec.frames as u32;
    for sy in 0..h {
        for sx in 0..w {
            alpha_over(
                &mut idle,
                *layers.wave.get_pixel((sx + wave_shift) % w, sy),
                x + sx,
                y + sy,
            );
        }
    }
    let pw = layers.processing.width();
    let ph = layers.processing.height();
    let pattern_shift = i as u32 * pw * 2 / spec.frames as u32;
    for sy in 0..ph {
        for sx in 0..pw {
            let mut pixel = *layers.processing.get_pixel((sx + pattern_shift) % pw, sy);
            let pulse =
                0.82 + 0.18 * (std::f32::consts::TAU * i as f32 * 2.0 / spec.frames as f32).cos();
            for c in 0..3 {
                pixel[c] = (f32::from(pixel[c]) * pulse).round() as u8;
            }
            alpha_over(&mut active, pixel, x + sx + 1, y + sy + 1);
        }
    }
    for sy in 0..h {
        for sx in 0..w {
            frame.put_pixel(
                x + sx,
                y + sy,
                mix(
                    *idle.get_pixel(x + sx, y + sy),
                    *active.get_pixel(x + sx, y + sy),
                    work,
                ),
            );
        }
    }
}

fn draw_traffic(
    frame: &mut RgbImage,
    layers: &Layers,
    spec: &Spec,
    name: &str,
    i: usize,
) -> Result<()> {
    for traffic in spec.traffic.get(name).context("missing traffic profile")? {
        let car = layers
            .cars
            .get(&traffic.sprite)
            .context("unknown traffic sprite")?;
        let t = i as f32 / (spec.frames - 1) as f32 * traffic.passes as f32;
        let phase = if i == spec.frames - 1 { 1.0 } else { t.fract() };
        let car_x = (traffic.start_x as f32 + (traffic.end_x - traffic.start_x) as f32 * phase)
            .round() as i32;
        for (sx, sy, _) in car.enumerate_pixels() {
            let x = car_x + sx as i32;
            let y = traffic.lane + sy as i32;
            if !(0..64).contains(&x) || !(0..64).contains(&y) {
                continue;
            }
            let (x, y) = (x as u32, y as u32);
            // The bottom lane passes behind the foreground hair silhouette.
            if !contains([25, 7, 35, 9], x, y) && !contains([34, 16, 26, 1], x, y) {
                continue;
            }
            let source_x = if traffic.mirror {
                car.width() - 1 - sx
            } else {
                sx
            };
            alpha_over(frame, *car.get_pixel(source_x, sy), x, y);
        }
    }
    Ok(())
}

fn render(layers: &Layers, spec: &Spec, name: &str) -> Result<Vec<RgbImage>> {
    let mut frames = Vec::new();
    for i in 0..spec.frames {
        let character_active =
            spec.character.is_some() && matches!(name, "working-enter" | "finished");
        let motorized_active =
            spec.motorized.is_some() && matches!(name, "working-enter" | "finished");
        let (lowered, work) = if character_active {
            let c = spec.character.as_ref().unwrap();
            let t = if name == "working-enter" {
                i
            } else {
                spec.frames - 1 - i
            };
            let work =
                smoothstep((t as f32 - c.work_start as f32) / (c.work_end - c.work_start) as f32);
            (work, work)
        } else if motorized_active {
            motorized_progress(spec.motorized.as_ref().unwrap(), name, i)
        } else if spec.gesture.is_some() {
            gesture_progress(name, i)
        } else {
            progress(name, i)
        };
        let mut frame = if character_active {
            layers.character.as_ref().unwrap().room.clone()
        } else {
            layers.underlay.clone()
        };
        // Approved registered fingertip pixels, twice the idle cadence in work.
        let pose = if work > 0.5 { (i * 2) % spec.frames } else { i };
        for [x, y] in &spec.finger_pixels {
            if character_active {
                continue;
            }
            if arm_enabled(spec, name, i) {
                if spec.gesture.as_ref().is_some_and(|g| g.foreground) {
                    // The far hand holds its canonical keyboard pose while the
                    // near hand is removed and raised; neither types here.
                    continue;
                }
                if *y < 58 {
                    continue;
                }
            }
            frame.put_pixel(*x, *y, *layers.idle[pose].get_pixel(*x, *y));
        }
        draw_screens(&mut frame, layers, spec, work, i);
        draw_traffic(&mut frame, layers, spec, name, i)?;
        if character_active {
            let character = spec.character.as_ref().unwrap();
            let poses = &layers.character.as_ref().unwrap().poses;
            let pose = &poses[character_pose_index(character, name, i)];
            for (x, y, pixel) in pose.enumerate_pixels() {
                if pixel[3] > 0 {
                    alpha_over(&mut frame, *pixel, x, y);
                }
            }
        } else {
            draw_visor(&mut frame, layers, spec, lowered, i, motorized_active);
        }
        if !character_active && arm_enabled(spec, name, i) {
            draw_arm(&mut frame, layers, spec, name, i)?;
        }
        if name == "interrupted" && i < 8 {
            // A single restrained warning pulse, confined to the supplied inserts.
            let amount = 0.7 * (std::f32::consts::PI * i as f32 / 8.0).sin();
            for [x, y, w, h] in [spec.main_area, spec.rear_area, spec.visor_down] {
                for py in y..y + h {
                    for px in x..x + w {
                        if contains(spec.main_area, px, py) && !main_screen_pixel(spec, px, py) {
                            continue;
                        }
                        let pixel = *frame.get_pixel(px, py);
                        if pixel[1] > pixel[0].saturating_add(40)
                            || pixel[2] > pixel[0].saturating_add(40)
                        {
                            frame.put_pixel(px, py, mix(pixel, Rgb([229, 39, 92]), amount));
                        }
                    }
                }
            }
        }
        frames.push(frame);
    }
    Ok(frames)
}

fn allowed(spec: &Spec, x: u32, y: u32) -> bool {
    spec.finger_pixels.contains(&[x, y])
        || contains(spec.main_area, x, y)
        || contains(spec.rear_area, x, y)
        || contains(spec.visor_up, x, y)
        || contains(spec.visor_down, x, y)
        || contains(
            [
                spec.visor_down[0].min(spec.visor_up[0]),
                spec.visor_up[1],
                spec.visor_up[2].max(spec.visor_down[2]) + 1,
                16,
            ],
            x,
            y,
        )
        || spec.gesture.as_ref().is_some_and(|g| {
            contains(g.arm_area, x, y)
                || g.removal_regions.iter().any(|r| contains(*r, x, y))
                || g.removal_pixels.contains(&[x, y])
        })
        || spec.character.as_ref().is_some_and(|c| {
            contains(c.area, x, y) || c.clear_regions.iter().any(|r| contains(*r, x, y))
        })
        || contains([25, 7, 35, 9], x, y)
        || contains([34, 16, 26, 1], x, y)
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
    for (i, frame) in frames.iter().enumerate() {
        let indices: Vec<u8> = frame
            .pixels()
            .map(|p| {
                *cache.entry(p.0).or_insert_with(|| {
                    palette
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, c)| distance(p.0, **c))
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
        let mut gif = gif::Frame::from_indexed_pixels(size, size, enlarged, None);
        gif.delay = (((i as u32 + 1) * frame_ms / 10) - (i as u32 * frame_ms / 10)) as u16;
        gif.dispose = gif::DisposalMethod::Keep;
        encoder.write_frame(&gif)?;
    }
    Ok(())
}

fn export_clip(
    output: &Path,
    name: &str,
    frames: &[RgbImage],
    palette: &[u8],
    frame_ms: u32,
) -> Result<()> {
    let directory = output.join(name);
    fs::create_dir_all(&directory)?;
    image::imageops::resize(&frames[0], 512, 512, FilterType::Nearest)
        .save(output.join(format!("{name}-poster.png")))?;
    if name == "working-enter" || name == "finished" || name == "interrupted" {
        image::imageops::resize(&frames[12], 512, 512, FilterType::Nearest)
            .save(output.join(format!("{name}-gesture-poster.png")))?;
    }
    let mut sheet = RgbImage::new(256, (frames.len() as u32).div_ceil(4) * 64);
    let mut contact = RgbImage::new(512, (frames.len() as u32).div_ceil(8) * 64);
    let mut heads = RgbImage::new(8 * 18, (frames.len() as u32).div_ceil(8) * 22);
    for (i, frame) in frames.iter().enumerate() {
        frame.save(directory.join(format!("{i:03}.png")))?;
        image::imageops::replace(&mut sheet, frame, (i as i64 % 4) * 64, (i as i64 / 4) * 64);
        image::imageops::replace(
            &mut contact,
            frame,
            (i as i64 % 8) * 64,
            (i as i64 / 8) * 64,
        );
        image::imageops::replace(
            &mut heads,
            &image::imageops::crop_imm(frame, 14, 15, 18, 22).to_image(),
            (i as i64 % 8) * 18,
            (i as i64 / 8) * 22,
        );
    }
    sheet.save(output.join(format!("{name}-sprite.png")))?;
    image::imageops::resize(&contact, 1024, contact.height() * 2, FilterType::Nearest)
        .save(output.join(format!("{name}-contact.png")))?;
    image::imageops::resize(
        &heads,
        heads.width() * 8,
        heads.height() * 8,
        FilterType::Nearest,
    )
    .save(output.join(format!("{name}-heads.png")))?;
    gif_preview(
        &output.join(format!("{name}-native.gif")),
        frames,
        palette,
        1,
        frame_ms,
    )?;
    gif_preview(
        &output.join(format!("{name}-preview.gif")),
        frames,
        palette,
        8,
        frame_ms,
    )?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let plan = PathBuf::from(args.next().context("provide authoring JSON")?);
    let output = PathBuf::from(args.next().context("provide a new output directory")?);
    ensure!(
        args.next().is_none() && !output.exists(),
        "use exactly two arguments and a new output directory"
    );
    let spec: Spec = serde_json::from_str(&fs::read_to_string(&plan)?)?;
    ensure!(
        spec.character.is_none() || spec.gesture.is_none(),
        "choose full-character poses or the limb rig, not both"
    );
    if let Some(motor) = &spec.motorized {
        ensure!(
            spec.character.is_none() && spec.gesture.is_none(),
            "motorized visor keeps the approved typing character; no limb/character atlas"
        );
        for [start, end] in [motor.flip_frames, motor.screen_frames] {
            ensure!(start < end && end < spec.frames, "motorized timing outside clip");
        }
        ensure!(
            motor.minimum_height > 0
                && motor.minimum_height <= spec.visor_up[3].min(spec.visor_down[3]),
            "motorized minimum height outside visor endpoints"
        );
    }
    ensure!(
        spec.frames == 32 && spec.frame_ms == 83,
        "this revision requires 32 frames at 83ms"
    );
    for [x, y, w, h] in [
        spec.main_area,
        spec.rear_area,
        spec.visor_up,
        spec.visor_down,
    ] {
        ensure!(
            w > 0 && h > 0 && x + w <= 64 && y + h <= 64,
            "region outside canvas"
        );
    }
    if let Some(columns) = &spec.main_aperture {
        ensure!(
            columns.len() == spec.main_area[2] as usize,
            "aperture column count mismatch"
        );
        for [x, top, bottom] in columns {
            ensure!(
                *top <= *bottom
                    && contains(spec.main_area, *x, *top)
                    && contains(spec.main_area, *x, *bottom),
                "screen aperture outside bounds"
            );
        }
    }
    if let Some(gesture) = &spec.gesture {
        for [x, y, w, h] in gesture
            .removal_regions
            .iter()
            .chain(std::iter::once(&gesture.arm_area))
        {
            ensure!(
                *w > 0 && *h > 0 && *x + *w <= 64 && *y + *h <= 64,
                "arm region outside canvas"
            );
        }
    }
    let layers = load_layers(plan.parent().unwrap_or(Path::new(".")), &spec)?;
    let working = render(&layers, &spec, "working")?;
    let mut entering = render(&layers, &spec, "working-enter")?;
    let mut finished = render(&layers, &spec, "finished")?;
    entering[0] = layers.idle[0].clone();
    entering[spec.frames - 1] = working[0].clone();
    finished[0] = working[0].clone();
    finished[spec.frames - 1] = layers.idle[0].clone();
    let mut interrupted = if spec.gesture.as_ref().is_some_and(|g| g.interrupted) {
        Some(render(&layers, &spec, "interrupted")?)
    } else {
        None
    };
    if let Some(frames) = &mut interrupted {
        frames[0] = working[0].clone();
        frames[spec.frames - 1] = layers.idle[0].clone();
    }
    let mut clips = vec![
        ("idle", &layers.idle),
        ("working-enter", &entering),
        ("working", &working),
        ("finished", &finished),
    ];
    if let Some(frames) = &interrupted {
        clips.push(("interrupted", frames));
    }
    for (name, frames) in &clips {
        for frame in *frames {
            for (x, y, pixel) in frame.enumerate_pixels() {
                ensure!(
                    allowed(&spec, x, y) || pixel == layers.base.get_pixel(x, y),
                    "fixed pixel changed in {name} at {x},{y}"
                );
            }
        }
    }
    fs::create_dir_all(&output)?;
    layers.base.save(output.join("base.png"))?;
    layers.underlay.save(output.join("underlay.png"))?;
    layers.raised.save(output.join("visor-up-sprite.png"))?;
    layers.visor.save(output.join("visor-down-sprite.png"))?;
    if let Some(motor) = &spec.motorized {
        let mut motion = serde_json::Map::new();
        for name in ["working-enter", "finished"] {
            let records: Vec<_> = (0..spec.frames)
                .map(|i| {
                    let (lowered, work) = motorized_progress(motor, name, i);
                    let typing = if i == 0 || i == spec.frames - 1 {
                        0
                    } else if work > 0.5 {
                        (i * 2) % spec.frames
                    } else {
                        i
                    };
                    json!({"frame":i,"lowered":lowered,"work_mix":work,"visor_bounds":visor_bounds(&spec,lowered,true),"typing_source_frame":typing})
                })
                .collect();
            motion.insert(name.into(), json!(records));
        }
        fs::write(
            output.join("motorized-motion.json"),
            serde_json::to_vec_pretty(&json!({
                "method": "approved typing character; existing visor layers only; foreshortened motorized flip",
                "config": motor,
                "frames": motion,
                "canonical_endpoints_override": [0, spec.frames - 1],
                "background_timeline": "forward in both clips; only visor/state progress inverts"
            }))?,
        )?;
    }
    if let Some(character) = &layers.character {
        let character_spec = spec.character.as_ref().unwrap();
        character
            .room
            .save(output.join("character-room-base.png"))?;
        let pose_dir = output.join("character-poses");
        fs::create_dir_all(&pose_dir)?;
        for (i, pose) in character.poses.iter().enumerate() {
            pose.save(pose_dir.join(format!("{i:03}.png")))?;
        }
        fs::write(
            output.join("character-motion.json"),
            serde_json::to_vec_pretty(&json!({
                "method": "complete generated character poses; no articulated limb sprites",
                "projection_size": character_spec.size,
                "projection_origin": character_spec.origin,
                "additional_atlases": character_spec.additional_atlases,
                "source_pose_count": character.poses.len(),
                "working-enter": character_spec.enter_poses,
                "finished": character_spec.exit_poses,
                "canonical_endpoints_override": [0, spec.frames - 1],
                "background_timeline": "forward in both clips; only character action reverses"
            }))?,
        )?;
    }
    if let Some(arm) = &layers.arm {
        arm.clean.save(output.join("arm-clean-plate.png"))?;
        fs::create_dir_all(output.join("arm-sprites"))?;
        for (i, pose) in arm.poses.iter().enumerate() {
            pose.save(output.join("arm-sprites").join(format!("{i:03}.png")))?;
        }
        if let Some(textures) = &arm.rig {
            let gesture = spec.gesture.as_ref().unwrap();
            textures.upper.save(output.join("upper-arm-sprite.png"))?;
            textures.fore.save(output.join("forearm-sprite.png"))?;
            textures.hand.save(output.join("grip-hand-sprite.png"))?;
            let mut motion = serde_json::Map::new();
            for name in ["working-enter", "finished", "interrupted"] {
                if name == "interrupted" && !gesture.interrupted {
                    continue;
                }
                let dir = output.join("rig-frames").join(name);
                fs::create_dir_all(&dir)?;
                let mut records = Vec::new();
                for i in gesture.times[0]..=gesture.times[15] {
                    let pose = rig_pose(gesture, textures, name, i)?;
                    pose.arm.save(dir.join(format!("{i:03}-arm.png")))?;
                    pose.hand.save(dir.join(format!("{i:03}-hand.png")))?;
                    records.push(json!({"frame":i,"shoulder":gesture.shoulder,"elbow":pose.elbow,"wrist":pose.wrist,"hand_angle_degrees":pose.hand_angle,"upper_length":(pose.elbow[0]-gesture.shoulder[0]).hypot(pose.elbow[1]-gesture.shoulder[1]),"fore_length":(pose.wrist[0]-pose.elbow[0]).hypot(pose.wrist[1]-pose.elbow[1]),"scale":1.0}));
                }
                motion.insert(name.into(), json!(records));
            }
            fs::write(
                output.join("rig-motion.json"),
                serde_json::to_vec_pretty(&motion)?,
            )?;
        }
    }
    layers.main.save(output.join("main-screen-sprite.png"))?;
    layers
        .processing
        .save(output.join("processing-sprite.png"))?;
    for (name, sprite) in &layers.cars {
        sprite.save(output.join(format!("{name}-car-sprite.png")))?;
    }
    let mut palette_input = RgbImage::new(256, 512 * clips.len() as u32);
    for (j, (_, frames)) in clips.iter().enumerate() {
        for (i, frame) in frames.iter().enumerate() {
            image::imageops::replace(
                &mut palette_input,
                frame,
                (i as i64 % 4) * 64,
                (j as i64 * 8 + i as i64 / 4) * 64,
            );
        }
    }
    let palette = if let Some(path) = &spec.palette_from {
        let decoder = gif::DecodeOptions::new().read_info(fs::File::open(
            plan.parent().unwrap_or(Path::new(".")).join(path),
        )?)?;
        decoder
            .global_palette()
            .context("reference GIF has no global palette")?
            .to_vec()
    } else {
        gif::Frame::from_rgb_speed(
            palette_input.width() as u16,
            palette_input.height() as u16,
            palette_input.as_raw(),
            1,
        )
        .palette
        .unwrap()
    };
    let mut manifest = serde_json::Map::new();
    let mut qa = serde_json::Map::new();
    for (name, frames) in &clips {
        export_clip(&output, name, frames, &palette, spec.frame_ms)?;
        let mut clip = json!({"source":{"type":"sprite_sheet","path":format!("{name}-sprite.png"),"columns":4,"frame_count":spec.frames},"frame_duration_ms":spec.frame_ms,"loop":*name=="idle" || *name=="working"});
        if *name == "working" {
            clip["entry"] = json!("working-enter");
        }
        manifest.insert((*name).into(), clip);
        qa.insert((*name).into(),json!({"frames":frames.len(),"unique_frames":frames.iter().map(|f|f.as_raw()).collect::<std::collections::HashSet<_>>().len(),"native_duration_ms":spec.frame_ms*spec.frames as u32,"gif_duration_ms":spec.frame_ms*spec.frames as u32/10*10}));
    }
    let cycle: Vec<_> = entering
        .iter()
        .chain(working.iter())
        .chain(working.iter())
        .chain(finished.iter())
        .chain(layers.idle.iter())
        .cloned()
        .collect();
    gif_preview(
        &output.join("work-cycle-preview.gif"),
        &cycle,
        &palette,
        8,
        spec.frame_ms,
    )?;
    fs::write(
        output.join("pet.json"),
        serde_json::to_vec_pretty(
            &json!({"schema_version":1,"id":spec.id,"canvas_size":64,"background":"#000000","default_animation":"idle","animations":manifest}),
        )?,
    )?;
    fs::write(
        output.join("qa.json"),
        serde_json::to_vec_pretty(
            &json!({"clips":qa,"fixed_scene_matches_base":true,"global_gif_palette":true,"transition_endpoints_match":true,"cycle_frames":cycle.len(),"cycle_duration_ms":cycle.len() as u32*spec.frame_ms,"traffic_profiles":spec.traffic,"rear_monitor":"idle green waveform; working magenta-cyan circuit pattern"}),
        )?,
    )?;
    println!(
        "Prepared {} 32-frame clips in {}",
        clips.len(),
        output.display()
    );
    Ok(())
}
