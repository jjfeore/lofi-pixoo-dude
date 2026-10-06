"""Read-only PNG/GIF export analysis; writes only the JSON evidence record."""
import hashlib
import json
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
names = ["idle", "working-enter", "working", "finished"]
clips = {
    name: [Image.open(path).convert("RGB") for path in sorted((review / name).glob("*.png"))]
    for name in names
}
fingers = {tuple(pixel) for pixel in spec["finger_pixels"]}
regions = [spec["main_area"], spec["rear_area"], [21, 17, 9, 13], [25, 7, 35, 9], [34, 16, 26, 1]]
fixed = [(x, y) for y in range(64) for x in range(64) if (x, y) not in fingers and not any(inside(region, x, y) for region in regions)]
report = {"fixed_pixel_count": len(fixed), "clips": {}}
canonical_idle = [Image.open(root / spec["idle"] / f"{i:03}.png").convert("RGB") for i in range(32)]
assert all(digest(a) == digest(b) for a, b in zip(clips["idle"], canonical_idle))
joins = {
    "idle_to_enter": (clips["idle"][0], clips["working-enter"][0]),
    "enter_to_work": (clips["working-enter"][-1], clips["working"][0]),
    "work_to_finish": (clips["working"][0], clips["finished"][0]),
    "finish_to_idle": (clips["finished"][-1], clips["idle"][0]),
}
assert all(digest(a) == digest(b) for a, b in joins.values())
report["approved_idle_rgb_preserved"] = True
report["canonical_transition_endpoints_match"] = True
report["endpoint_note"] = "Endpoints match canonical frame zero. Actual hook changes can occur at any working/idle phase; hardware upload timing is approximate."
palms = [(x, y) for rect in [[36, 55, 3, 2], [36, 59, 4, 3]] for y in range(rect[1], rect[1] + rect[3]) for x in range(rect[0], rect[0] + rect[2])]
screen_bezels = [(x, y) for rect, aperture in [([52, 26, 12, 27], spec["main_area"]), ([33, 38, 16, 11], spec["rear_area"])] for y in range(rect[1], rect[1] + rect[3]) for x in range(rect[0], rect[0] + rect[2]) if not inside(aperture, x, y)]
window = [(x, y) for y in range(7, 17) for x in range(25, 60) if y < 16 or x >= 34]
all_gif_fixed = None
for name, frames in clips.items():
    assert len(frames) == 32 and all(frame.size == (64, 64) for frame in frames)
    assert all(frame.getpixel(pixel) == base.getpixel(pixel) for frame in frames for pixel in fixed)
    assert all(frame.getpixel(pixel) == base.getpixel(pixel) for frame in frames for pixel in palms + screen_bezels)
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
        "fixed_pixels_palms_and_bezels_preserved": True,
        "sheet_matches_numbered_frames": True,
    }
cycle = Image.open(review / "work-cycle-preview.gif")
assert cycle.n_frames == 160 and cycle.size == (512, 512)
assert sum(frame.info["duration"] for frame in ImageSequence.Iterator(cycle)) == 13280
manifest = json.loads((review / "pet.json").read_text())
assert manifest["animations"]["working"]["entry"] == "working-enter"
assert not manifest["animations"]["working-enter"]["loop"]
assert not manifest["animations"]["finished"]["loop"]
report["shared_gif_palette_fixed_colors"] = True
report["cycle_frames"] = 160
report["cycle_duration_ms"] = 13280
report["visual_review"] = "Native full-scene and head contacts inspected; owner review pending."
report["device_activation"] = "No upload or active-pack change for this revision."
(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
