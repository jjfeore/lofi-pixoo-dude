"""Read-only PNG/GIF export analysis; writes only the JSON evidence record."""
import hashlib
import json
import math
import sys
from pathlib import Path

from PIL import Image, ImageSequence


def inside(rect, x, y):
    rx, ry, width, height = rect
    return rx <= x < rx + width and ry <= y < ry + height


def digest(image):
    return hashlib.sha256(image.tobytes()).hexdigest()


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "review"
spec = json.loads((root / "authoring.json").read_text())
base = Image.open(root / spec["base"]).convert("RGB")
names = ["idle", "working-enter", "working", "finished", "interrupted"]
clips = {
    name: [Image.open(path).convert("RGB") for path in sorted((review / name).glob("*.png"))]
    for name in names
}
fingers = {tuple(pixel) for pixel in spec["finger_pixels"]}
gesture = spec["gesture"]
regions = [spec["main_area"], spec["rear_area"], [20, 17, 12, 16], [25, 7, 35, 9], [34, 16, 26, 1], gesture["arm_area"], *gesture["removal_regions"]]
removed_pixels = {tuple(pixel) for pixel in gesture["removal_pixels"]}
fixed = [(x, y) for y in range(64) for x in range(64) if (x, y) not in fingers | removed_pixels and not any(inside(region, x, y) for region in regions)]
report = {"fixed_pixel_count": len(fixed), "clips": {}}
canonical_idle = [Image.open(root / spec["idle"] / f"{i:03}.png").convert("RGB") for i in range(32)]
assert all(digest(a) == digest(b) for a, b in zip(clips["idle"], canonical_idle))
joins = {
    "idle_to_enter": (clips["idle"][0], clips["working-enter"][0]),
    "enter_to_work": (clips["working-enter"][-1], clips["working"][0]),
    "work_to_finish": (clips["working"][0], clips["finished"][0]),
    "finish_to_idle": (clips["finished"][-1], clips["idle"][0]),
    "work_to_interrupt": (clips["working"][0], clips["interrupted"][0]),
    "interrupt_to_idle": (clips["interrupted"][-1], clips["idle"][0]),
}
assert all(digest(a) == digest(b) for a, b in joins.values())
report["approved_idle_rgb_preserved"] = True
previous = root.parent / "work-v4" / "review"
accepted_working = [Image.open(previous / "working" / f"{i:03}.png").convert("RGB") for i in range(32)]
assert all(digest(a) == digest(b) for a, b in zip(clips["working"], accepted_working))
for filename in ["working-native.gif", "working-preview.gif"]:
    assert (review / filename).read_bytes() == (previous / filename).read_bytes(), filename
report["approved_working_rgb_and_gif_bytes_preserved"] = True
report["canonical_transition_endpoints_match"] = True
report["endpoint_note"] = "Endpoints match canonical frame zero. Actual hook changes can occur at any working/idle phase; hardware upload timing is approximate."
palms = [(x, y) for y in range(59, 62) for x in range(36, 40)]
near_hand = [(x, y) for y in range(58, 63) for x in range(35, 47) if (x, y) not in fingers]
retained = [(x, y) for y in range(64) for x in range(64) if any(inside(region, x, y) for region in gesture["retained_regions"])]
aperture = [(x, y) for x, top, bottom in spec["main_aperture"] for y in range(top, bottom + 1)]
main_bezel = [(x, y) for y in range(26, 53) for x in range(52, 64) if (x, y) not in aperture]
rear_bezel = [(x, y) for y in range(38, 49) for x in range(33, 49) if not inside(spec["rear_area"], x, y)]
screen_bezels = main_bezel
window = [(x, y) for y in range(7, 17) for x in range(25, 60) if y < 16 or x >= 34]
all_gif_fixed = None
for name, frames in clips.items():
    assert len(frames) == 32 and all(frame.size == (64, 64) for frame in frames)
    assert all(frame.getpixel(pixel) == base.getpixel(pixel) for frame in frames for pixel in fixed)
    assert all(frame.getpixel(pixel) == base.getpixel(pixel) for frame in frames for pixel in screen_bezels)
    if name in ["idle", "working"]:
        assert all(frame.getpixel(pixel) == base.getpixel(pixel) for frame in frames for pixel in rear_bezel + palms + near_hand)
    sheet = Image.open(review / f"{name}-sprite.png").convert("RGB")
    assert sheet.size == (256, 512)
    assert all(digest(frame) == digest(sheet.crop(((i % 4) * 64, (i // 4) * 64, (i % 4 + 1) * 64, (i // 4 + 1) * 64))) for i, frame in enumerate(frames))
    native = Image.open(review / f"{name}-native.gif")
    preview = Image.open(review / f"{name}-preview.gif")
    assert native.n_frames == 32 and preview.n_frames == 32
    assert native.size == (64, 64) and preview.size == (512, 512)
    delays = []
    for i, gif_frame in enumerate(ImageSequence.Iterator(native)):
        delays.append(gif_frame.info["duration"])
        fixed_rgb = [gif_frame.convert("RGB").getpixel(pixel) for pixel in fixed]
        if all_gif_fixed is None:
            all_gif_fixed = fixed_rgb
        assert fixed_rgb == all_gif_fixed
        preview.seek(i)
        rgb_preview = preview.convert("RGB")
        assert all(rgb_preview.getpixel((x * 8 + 4, y * 8 + 4)) == gif_frame.convert("RGB").getpixel((x, y)) for x, y in [(25, 28), (38, 56), (58, 35), (40, 43)])
    assert sum(delays) == 2650 and set(delays) == {80, 90}
    x, y, w, h = spec["rear_area"]
    rear_patterns = {digest(frame.crop((x, y, x + w, y + h))) for frame in frames}
    traffic_patterns = {tuple(frame.getpixel(pixel) for pixel in window) for frame in frames}
    typing = {tuple(frame.getpixel(pixel) for pixel in sorted(fingers)) for frame in frames}
    assert len(rear_patterns) > 1 and len(traffic_patterns) > 1
    report["clips"][name] = {
        "native_frames": len(frames),
        "unique_frames": len({digest(frame) for frame in frames}),
        "frame_duration_ms": 83,
        "native_duration_ms": 2656,
        "gif_duration_ms": sum(delays),
        "rear_monitor_patterns": len(rear_patterns),
        "window_traffic_patterns": len(traffic_patterns),
        "fingertip_poses": len(typing),
        "fixed_pixels_and_main_bezel_preserved": True,
        "sheet_matches_numbered_frames": True,
    }
cycle = Image.open(review / "work-cycle-preview.gif")
assert cycle.n_frames == 160 and cycle.size == (512, 512)
assert sum(frame.info["duration"] for frame in ImageSequence.Iterator(cycle)) == 13280
manifest = json.loads((review / "pet.json").read_text())
assert manifest["animations"]["working"]["entry"] == "working-enter"
assert not manifest["animations"]["working-enter"]["loop"]
assert not manifest["animations"]["finished"]["loop"]
assert not manifest["animations"]["interrupted"]["loop"]
skin = lambda p: p[0] > 85 and p[0] > p[1] + 15 and p[0] > p[2] + 15
cleared_hand = [(x, y) for y in range(57, 64) for x in range(35, 48) if (x, y) not in retained and skin(base.getpixel((x, y)))]
hand_evidence = {}
for name in ["working-enter", "finished", "interrupted"]:
    targets = gesture["enter_hands"] if name == "working-enter" else gesture["exit_hands"]
    checked = []
    for i in range(gesture["times"][0], gesture["times"][-1] + 1):
        key = max(k for k, time in enumerate(gesture["times"]) if time <= i)
        nxt = min(key + 1, 15)
        fraction = 0 if key == nxt else (i - gesture["times"][key]) / (gesture["times"][nxt] - gesture["times"][key])
        hand_y = targets[key][1] * (1 - fraction) + targets[nxt][1] * fraction
        if hand_y <= 45:
            assert not any(skin(clips[name][i].getpixel(pixel)) for pixel in cleared_hand), (name, i, "old foreground keyboard hand remains")
            checked.append(i)
    assert len(checked) >= 10
    hand_evidence[name] = {"old_foreground_hand_cleared_while_lifted": True, "checked_frames": checked}
moving_screen = [pixel for pixel in aperture if len({frame.getpixel(pixel) for frame in clips["working"]}) > 1]
assert len(moving_screen) == len(aperture)
report["full_main_screen"] = {"columns": len(spec["main_aperture"]), "visible_glass_pixels": len(aperture), "animated_glass_pixels": len(moving_screen), "bezel_preserved": True, "projection": "Per-column angled top and bottom edges; entire visible glass receives the scrolling source."}
report["gesture_hand_checks"] = hand_evidence
rig = gesture["rig"]
motion = json.loads((review / "rig-motion.json").read_text())
assert Image.open(review / "upper-arm-sprite.png").size == (rig["upper_length"] + 1, rig["sleeve_width"])
assert Image.open(review / "forearm-sprite.png").size == (rig["fore_length"] + 1, rig["fore_width"])
assert Image.open(review / "grip-hand-sprite.png").size == tuple(rig["hand_size"])
fore_texture = Image.open(review / "forearm-sprite.png").convert("RGBA")
fore_profile = [sum(fore_texture.getpixel((x, y))[3] >= 128 for y in range(fore_texture.height)) for x in range(fore_texture.width)]
assert min(fore_profile[2:-3]) >= 8
assert min(fore_profile) >= 6
report["foreground_proportions"] = {
    "upper_texture_px": [rig["upper_length"] + 1, rig["sleeve_width"]],
    "fore_texture_px": list(fore_texture.size),
    "hand_texture_px": rig["hand_size"],
    "fore_opaque_pixels_per_column": fore_profile,
    "original_foreground_hand_skin_pixels": len(cleared_hand),
    "reference": "Foreground hand and sleeve in the approved base; alpha coverage checked as well as texture bounding dimensions.",
}
rig_evidence = {}
static_regions = [spec["main_area"], spec["rear_area"], [20, 17, 12, 16], [25, 7, 35, 9], [34, 16, 26, 1], *gesture["removal_regions"]]
dynamic_fixed_counts = []
for name, poses in motion.items():
    assert [pose["frame"] for pose in poses] == list(range(2, 31))
    hand_areas, skin_areas = [], []
    for pose in poses:
        assert pose["shoulder"] == gesture["shoulder"] and pose["scale"] == 1.0
        assert math.isclose(pose["upper_length"], rig["upper_length"], abs_tol=0.0001)
        assert math.isclose(pose["fore_length"], rig["fore_length"], abs_tol=0.0001)
        hand = Image.open(review / "rig-frames" / name / f'{pose["frame"]:03}-hand.png').convert("RGBA")
        arm = Image.open(review / "rig-frames" / name / f'{pose["frame"]:03}-arm.png').convert("RGBA")
        assert all(clips[name][pose["frame"]].getpixel(pixel) == base.getpixel(pixel) for pixel in retained if arm.getpixel(pixel)[3] == 0), (name, pose["frame"], "stationary background limb moved")
        unaffected = [(x, y) for y in range(64) for x in range(64) if (x, y) not in fingers and arm.getpixel((x, y))[3] == 0 and not any(inside(region, x, y) for region in static_regions)]
        assert all(clips[name][pose["frame"]].getpixel(pixel) == base.getpixel(pixel) for pixel in unaffected)
        dynamic_fixed_counts.append(len(unaffected))
        pixels = [hand.getpixel((x, y)) for y in range(64) for x in range(64) if hand.getpixel((x, y))[3] >= 128]
        hand_areas.append(len(pixels))
        skin_areas.append(sum(skin(pixel) for pixel in pixels))
    wrist_steps = [math.dist(a["wrist"], b["wrist"]) for a, b in zip(poses, poses[1:])]
    elbow_steps = [math.dist(a["elbow"], b["elbow"]) for a, b in zip(poses, poses[1:])]
    assert max(wrist_steps) <= 5.1 and max(elbow_steps) <= 5.1
    assert max(hand_areas) / min(hand_areas) <= 1.55
    assert min(skin_areas) >= 25
    rig_evidence[name] = {
        "checked_gesture_frames": len(poses),
        "shoulder_fixed": True,
        "upper_arm_length_px": rig["upper_length"],
        "forearm_length_px": rig["fore_length"],
        "scale_in_all_frames": 1.0,
        "max_wrist_step_px": max(wrist_steps),
        "max_elbow_step_px": max(elbow_steps),
        "unoccluded_hand_opaque_pixels_min_max": [min(hand_areas), max(hand_areas)],
        "unoccluded_hand_skin_pixels_min_max": [min(skin_areas), max(skin_areas)],
        "background_limb_preserved_where_not_occluded": True,
    }
report["rig_checks"] = rig_evidence
report["unoccluded_scene_pixels_preserved_per_gesture_frame_min_max"] = [min(dynamic_fixed_counts), max(dynamic_fixed_counts)]
report["rig_note"] = "Generated foreground sleeve pieces and 11x9 hand are resized once, then only rotated and translated. Upper sleeve is 11 pixels thick, fore sleeve 9. Pixel coverage varies with raster rotation and viewport clipping at the bottom; limb length and transform scale remain constant. Wrist targets stay within fixed reach."
report["hand_count_note"] = "One composite foreground limb replaces the original near arm/hand. The background keyboard limb is pinned and may be naturally occluded by the moving foreground layer. Pixel checks establish original-hand removal; visual contacts still require owner review."
report["shared_gif_palette_fixed_colors"] = True
report["cycle_frames"] = 160
report["cycle_duration_ms"] = 13280
report["visual_review"] = "Native full-scene/head contacts, raised-hand poster and enlarged working poster inspected; owner review pending."
report["device_activation"] = "No upload or active-pack change for this revision."
(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
