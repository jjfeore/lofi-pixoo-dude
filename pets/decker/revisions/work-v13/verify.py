"""Read-only animation analysis; writes only a JSON evidence record."""
import hashlib
import json
import sys
from pathlib import Path

from PIL import Image, ImageSequence


def inside(rect, x, y):
    rx, ry, w, h = rect
    return rx <= x < rx + w and ry <= y < ry + h


def digest(im):
    return hashlib.sha256(im.tobytes()).hexdigest()


def cyan(p):
    return p[0] < 100 and p[1] > 105 and p[2] > 120 and p[1] > p[0] + 70


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "review"
spec = json.loads((root / "authoring.json").read_text())
assert spec["gesture"] is None and spec["character"] is None
assert spec["motorized"] is not None
base = Image.open(root / spec["base"]).convert("RGB")
names = ["idle", "working-enter", "working", "finished"]
clips = {
    name: [Image.open(p).convert("RGB") for p in sorted((review / name).glob("*.png"))]
    for name in names
}
idle = [Image.open(root / spec["idle"] / f"{i:03}.png").convert("RGB") for i in range(32)]
for name, reference, directory in [
    ("idle", root.parent / "idle-v3" / "review", "frames"),
    ("working", root.parent / "work-v4" / "review", "working"),
]:
    old = [Image.open(reference / directory / f"{i:03}.png").convert("RGB") for i in range(32)]
    assert len(clips[name]) == len(old) and all(digest(a) == digest(b) for a, b in zip(clips[name], old))
    for kind in ["native", "preview"]:
        assert (review / f"{name}-{kind}.gif").read_bytes() == (root.parent / "work-v12" / "review" / f"{name}-{kind}.gif").read_bytes()
assert all(digest(a) == digest(b) for a, b in [
    (clips["working-enter"][0], clips["idle"][0]),
    (clips["working-enter"][-1], clips["working"][0]),
    (clips["finished"][0], clips["working"][0]),
    (clips["finished"][-1], clips["idle"][0]),
])

fingers = {tuple(p) for p in spec["finger_pixels"]}
visor_area = [20, 17, 12, 16]
regions = [spec["main_area"], spec["rear_area"], visor_area, [25, 7, 35, 9], [34, 16, 26, 1]]
fixed = [(x, y) for y in range(64) for x in range(64) if (x, y) not in fingers and not any(inside(r, x, y) for r in regions)]
aperture = {(x, y) for x, top, bottom in spec["main_aperture"] for y in range(top, bottom + 1)}
main_bezel = [(x, y) for y in range(26, 53) for x in range(52, 64) if (x, y) not in aperture]
rear_bezel = [(x, y) for y in range(38, 49) for x in range(33, 49) if not inside(spec["rear_area"], x, y)]
body = [(x, y) for y in range(14, 64) for x in range(0, 49) if (x, y) not in fingers and not any(inside(r, x, y) for r in regions)]
hands = [(x, y) for y in range(51, 64) for x in range(32, 49) if (x, y) not in fingers]
motion = json.loads((review / "motorized-motion.json").read_text())
entry = motion["frames"]["working-enter"]
finish = motion["frames"]["finished"]
assert len(entry) == len(finish) == 32
start, end = spec["motorized"]["flip_frames"]
assert all(abs(a["lowered"] + b["lowered"] - 1) < 1e-6 for a, b in zip(entry, finish))
assert all(a["lowered"] <= b["lowered"] for a, b in zip(entry, entry[1:]))
assert all(a["lowered"] >= b["lowered"] for a, b in zip(finish, finish[1:]))
assert all(abs(finish[i]["lowered"] - entry[start + end - i]["lowered"]) < 1e-6 for i in range(start, end + 1))
assert min(p["visor_bounds"][3] for p in entry) == spec["motorized"]["minimum_height"]
assert len({tuple(p["visor_bounds"]) for p in entry}) >= 10
for frame, record in zip(clips["working-enter"], entry):
    assert all(frame.getpixel(p) == idle[record["typing_source_frame"]].getpixel(p) for p in fingers)
for frame, record in zip(clips["finished"], finish):
    assert all(frame.getpixel(p) == idle[record["typing_source_frame"]].getpixel(p) for p in fingers)
for frames, records in [(clips["working-enter"], entry), (clips["finished"], finish)]:
    for i, (frame, record) in enumerate(zip(frames, records)):
        if record["lowered"] == 1:
            assert not any(cyan(frame.getpixel((x, y))) for y in range(17, 24) for x in range(21, 30)), ("parked glow", i)
        if record["lowered"] == 0:
            assert all(frame.getpixel((x, y)) == base.getpixel((x, y)) for y in range(25, 31) for x in range(20, 31)), ("eye visor remained", i)

report = {
    "method": "Existing approved base, fingertip animation and visor layers; motorized foreshortening only.",
    "accepted_idle_v3_and_working_v4_rgb_preserved": True,
    "unchanged_prior_loop_gif_bytes": True,
    "canonical_endpoints_match": True,
    "fixed_scene_pixels": len(fixed),
    "fixed_body_pixels_outside_visor_and_fingertips": len(body),
    "fixed_hand_wrist_palm_pixels": len(hands),
    "typing_matches_accepted_fingertip_frames": True,
    "minimum_visor_height": spec["motorized"]["minimum_height"],
    "different_visor_bounds": len({tuple(p["visor_bounds"]) for p in entry}),
    "lowered_visor_has_no_extra_forehead_glow": True,
    "raised_visor_restores_canonical_eye_region": True,
    "finish_inverts_only_visor_and_state_progress": True,
    "ambient_timeline": "Forward in both clips: render() passes increasing frame index to screens and traffic.",
    "clips": {},
}
window = [(x, y) for y in range(7, 17) for x in range(35, 60)]
gif_fixed = None
for name, frames in clips.items():
    assert len(frames) == 32 and all(f.size == (64, 64) for f in frames)
    assert all(f.getpixel(p) == base.getpixel(p) for f in frames for p in fixed + main_bezel + rear_bezel + body + hands)
    sheet = Image.open(review / f"{name}-sprite.png").convert("RGB")
    assert sheet.size == (256, 512)
    for i, f in enumerate(frames):
        assert digest(f) == digest(sheet.crop((i % 4 * 64, i // 4 * 64, (i % 4 + 1) * 64, (i // 4 + 1) * 64)))
    native = Image.open(review / f"{name}-native.gif")
    preview = Image.open(review / f"{name}-preview.gif")
    assert native.n_frames == preview.n_frames == 32
    assert native.size == (64, 64) and preview.size == (512, 512)
    delays = []
    for i, f in enumerate(ImageSequence.Iterator(native)):
        delays.append(f.info["duration"])
        pixels = [f.convert("RGB").getpixel(p) for p in fixed]
        if gif_fixed is None:
            gif_fixed = pixels
        assert pixels == gif_fixed
        preview.seek(i)
        assert all(preview.convert("RGB").getpixel((x * 8 + 4, y * 8 + 4)) == f.convert("RGB").getpixel((x, y)) for x, y in [(24, 28), (38, 56), (58, 35), (40, 43)])
    assert sum(delays) == 2650 and set(delays) == {80, 90}
    x, y, w, h = spec["rear_area"]
    rear = {digest(f.crop((x, y, x + w, y + h))) for f in frames}
    traffic = {tuple(f.getpixel(p) for p in window) for f in frames}
    typing = {tuple(f.getpixel(p) for p in sorted(fingers)) for f in frames}
    assert len(rear) > 1 and len(traffic) > 1 and len(typing) > 1
    report["clips"][name] = {
        "frames": 32,
        "unique_full_frames": len({digest(f) for f in frames}),
        "frame_ms": 83,
        "native_duration_ms": 2656,
        "gif_duration_ms": sum(delays),
        "typing_patterns": len(typing),
        "rear_monitor_patterns": len(rear),
        "traffic_patterns": len(traffic),
    }
moving_glass = sum(len({f.getpixel(p) for f in clips["working"]}) > 1 for p in aperture)
assert moving_glass == len(aperture) == 209
report["animated_working_glass_pixels"] = moving_glass
cycle = Image.open(review / "work-cycle-preview.gif")
assert cycle.n_frames == 160 and cycle.size == (512, 512)
assert sum(f.info["duration"] for f in ImageSequence.Iterator(cycle)) == 13280
report["cycle_frames"] = 160
report["cycle_duration_ms"] = 13280
manifest = json.loads((review / "pet.json").read_text())
assert manifest["animations"]["working"]["entry"] == "working-enter"
for name in names:
    clip = manifest["animations"][name]
    assert clip["source"]["frame_count"] == 32 and clip["frame_duration_ms"] == 83
    assert clip["loop"] == (name in ["idle", "working"])
report["review"] = "Native full-scene/head contacts and halfway-flip poster inspected; owner review pending."
report["activation"] = "Separate review pack; no hardware upload or active configuration change."
(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
