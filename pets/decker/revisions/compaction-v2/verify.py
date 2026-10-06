"""Read-only comparisons against approved artwork; writes only verification JSON."""
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


def load_frames(directory):
    paths = sorted(directory.glob("*.png"))
    assert [p.name for p in paths] == [f"{i:03}.png" for i in range(32)]
    images = [Image.open(p).convert("RGB") for p in paths]
    assert all(im.size == (64, 64) for im in images)
    return images


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "review"
spec = json.loads((root / "authoring.json").read_text())
approved = root / spec["approved_pack"]
base = Image.open(root / spec["base"]).convert("RGB")
work_spec = json.loads((root.parent / "idle-v4" / "work-authoring.json").read_text())
glass = {(x, y) for x, top, bottom in work_spec["main_aperture"] for y in range(top, bottom + 1)}
rear = {(x, y) for y in range(40, 47) for x in range(35, 46)}
traffic = {(x, y) for y in range(64) for x in range(64)
           if any(inside(rect, x, y) for rect in spec["traffic_mask"])}
fixed = [(x, y) for y in range(64) for x in range(64) if (x, y) not in rear | traffic]
bezels = [(x, y) for y in range(26, 53) for x in range(52, 64) if (x, y) not in glass]
bezels += [(x, y) for y in range(38, 49) for x in range(33, 49) if (x, y) not in rear]
fingertips = [tuple(p) for p in work_spec["finger_pixels"]]
manifest = json.loads((review / "pet.json").read_text())
records = json.loads((review / "compression-motion.json").read_text())
assert len(records) == 32 and [r["frame"] for r in records] == list(range(32))
assert [r["phase"] for r in records] == ["read"] * 8 + ["compress"] * 8 + ["packed"] * 4 + ["deliver-and-refill"] * 12
assert all(r["row_width"] == 3 and r["row_positions"] == [2, 3, 4] for r in records[16:20])
assert all(a["row_width"] >= b["row_width"] for a, b in zip(records[8:15], records[9:16]))
assert records[-1]["incoming_offset"] == 0
assert len(spec["traffic"]["compacting-idle"]) == 2
assert len(spec["traffic"]["compacting-working"]) == 3
assert sum(t["passes"] for t in spec["traffic"]["compacting-working"]) > sum(t["passes"] for t in spec["traffic"]["compacting-idle"])

for name in ["idle", "working-enter", "working", "finished"]:
    for i in range(32):
        assert (review / name / f"{i:03}.png").read_bytes() == (approved / name / f"{i:03}.png").read_bytes()
    for suffix in ["sprite.png", "poster.png", "contact.png", "heads.png", "native.gif", "preview.gif"]:
        assert (review / f"{name}-{suffix}").read_bytes() == (approved / f"{name}-{suffix}").read_bytes()
    assert manifest["animations"][name] == json.loads((approved / "pet.json").read_text())["animations"][name]
assert (review / "motorized-motion.json").read_bytes() == (approved / "motorized-motion.json").read_bytes()
assert (review / "work-cycle-preview.gif").read_bytes() == (approved / "work-cycle-preview.gif").read_bytes()

report = {
    "method": "Read-only PNG/GIF decoding and exact accepted-frame/artifact comparisons.",
    "frames_per_variant": 32,
    "frame_ms": 83,
    "native_duration_ms": 2656,
    "gif_duration_ms": 2650,
    "approved_base_clips_and_previews_byte_identical": True,
    "motorized_metadata_and_cycle_byte_identical": True,
    "compression_phases": {"read": 8, "compress": 8, "packed": 4, "deliver_and_refill": 12},
    "clips": {},
}
traffic_totals = {}
for name, source in [("compacting-idle", "idle"), ("compacting-working", "working")]:
    frames = load_frames(review / name)
    reference = load_frames(approved / source)
    assert all(a.getpixel(p) == b.getpixel(p) for a, b in zip(frames, reference) for p in fixed)
    assert all(im.getpixel(p) == base.getpixel(p) for im in frames for p in bezels)
    assert all(a.getpixel(p) == b.getpixel(p) for a, b in zip(frames, reference) for p in glass | set(fingertips))
    assert sum(len({im.getpixel(p) for im in frames}) > 1 for p in glass) == 209
    rear_patterns = {tuple(im.getpixel(p) for p in sorted(rear)) for im in frames}
    assert len(rear_patterns) >= 16
    assert all(frames[-1].getpixel(p) == frames[0].getpixel(p) for p in rear)
    for im in frames[16:20]:
        lit = [p for p in rear if im.getpixel(p) in [tuple(c) for c in (spec["working_colors"] if source == "working" else spec["idle_colors"])]]
        assert len(lit) == 9
        assert max(x for x, y in lit) - min(x for x, y in lit) == 2
        assert max(y for x, y in lit) - min(y for x, y in lit) == 2
    traffic_totals[name] = sum(im.getpixel(p) != base.getpixel(p) for im in frames for p in traffic)
    lane_variation = {}
    for vehicle in spec["traffic"][name]:
        sprite = Image.open(root / spec["cars"][vehicle["sprite"]])
        points = sorted((x, y) for x, y in traffic if vehicle["lane"] <= y < vehicle["lane"] + sprite.height)
        patterns = {tuple(im.getpixel(p) for p in points) for im in frames}
        assert len(patterns) > 2
        assert any(im.getpixel(p) != base.getpixel(p) for im in frames for p in points)
        lane_variation[str(vehicle["lane"])] = len(patterns)
    sheet = Image.open(review / f"{name}-sprite.png").convert("RGB")
    assert sheet.size == (256, 512)
    for i, im in enumerate(frames):
        x, y = i % 4 * 64, i // 4 * 64
        assert digest(sheet.crop((x, y, x + 64, y + 64))) == digest(im)
    native = Image.open(review / f"{name}-native.gif")
    enlarged = Image.open(review / f"{name}-preview.gif")
    old_gif = Image.open(approved / f"{source}-native.gif")
    assert native.n_frames == enlarged.n_frames == 32
    assert native.size == (64, 64) and enlarged.size == (512, 512)
    assert native.getpalette() == old_gif.getpalette()
    delays = []
    for i in range(32):
        native.seek(i)
        enlarged.seek(i)
        old_gif.seek(i)
        a, b, old = native.convert("RGB"), enlarged.convert("RGB"), old_gif.convert("RGB")
        delays.append(native.info["duration"])
        assert all(a.getpixel(p) == old.getpixel(p) for p in fixed)
        assert all(a.getpixel((x, y)) == b.getpixel((x * 8 + 4, y * 8 + 4)) for y in range(64) for x in range(64))
    assert sum(delays) == 2650 and set(delays) == {80, 90}
    clip = manifest["animations"][name]
    assert clip["loop"] and clip["frame_duration_ms"] == 83 and clip["source"]["frame_count"] == 32
    report["clips"][name] = {
        "unique_complete_frames": len({digest(im) for im in frames}),
        "fixed_pixels_match_corresponding_approved_frame": len(fixed),
        "body_visor_fingertips_and_main_screen_exact": True,
        "animated_main_glass_pixels": 209,
        "fixed_bezel_pixels": len(bezels),
        "rear_screen_patterns": len(rear_patterns),
        "rear_screen_first_last_pixels_identical": True,
        "packet_dimensions": [3, 3],
        "vehicle_types": sorted({t["sprite"] for t in spec["traffic"][name]}),
        "passes_per_cycle": sum(t["passes"] for t in spec["traffic"][name]),
        "traffic_lane_patterns": lane_variation,
        "traffic_changed_pixel_total": traffic_totals[name],
        "decoded_gif_pixels_outside_overlays_match_approved": True,
    }
assert traffic_totals["compacting-working"] > traffic_totals["compacting-idle"]
logical = manifest["animations"]["compacting"]
assert logical["variants"] == {"idle": "compacting-idle", "working": "compacting-working"}
assert logical["source"] == manifest["animations"]["compacting-idle"]["source"]
assert all(clip["source"]["frame_count"] == 32 for clip in manifest["animations"].values())
cycle = Image.open(review / "compaction-cycle-preview.gif")
assert cycle.n_frames == 64 and cycle.size == (512, 512)
assert sum(f.info["duration"] for f in ImageSequence.Iterator(cycle)) == 5310
report["compaction_variant_manifest"] = "idle and working mapped to 32-frame full-scene sources"
report["cycle_preview_frames"] = 64
report["cycle_preview_duration_ms"] = 5310
report["review"] = "Native full-scene and rear-screen contacts inspected; owner review pending."
report["activation"] = "Separate review pack; no device upload or bridge/configuration changes."
(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
