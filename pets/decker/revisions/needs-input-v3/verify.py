"""Read-only native/preview analysis; writes only a JSON evidence record."""
import hashlib
import json
import sys
from pathlib import Path

from PIL import Image


def inside(rect, x, y):
    rx, ry, w, h = rect
    return rx <= x < rx + w and ry <= y < ry + h


def digest(im):
    return hashlib.sha256(im.tobytes()).hexdigest()


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "review"
spec = json.loads((root / "authoring.json").read_text())
attention = spec["attention"]
approved = root / spec["approved_pack"]
base = Image.open(root / spec["base"]).convert("RGB")
paths = sorted((review / "needs-input").glob("*.png"))
assert [p.name for p in paths] == [f"{i:03}.png" for i in range(32)]
frames = [Image.open(p).convert("RGB") for p in paths]
source = [Image.open(approved / "working" / f"{i:03}.png").convert("RGB") for i in range(32)]
assert all(im.size == (64, 64) for im in frames)
glass = {(x, y) for x, top, bottom in attention["main_aperture"] for y in range(top, bottom + 1)}
rear = {(x, y) for y in range(40, 47) for x in range(35, 46)}
traffic = {(x, y) for y in range(64) for x in range(64)
           if any(inside(r, x, y) for r in spec["traffic_mask"])}
visor = {tuple(p) for p in attention["visor_pixels"]}
fingers = {tuple(p) for p in attention["finger_pixels"]}
allowed = glass | rear | traffic | visor | fingers
fixed = [(x, y) for y in range(64) for x in range(64) if (x, y) not in allowed]
bezels = [(x, y) for y in range(26, 53) for x in range(52, 64) if (x, y) not in glass]
bezels += [(x, y) for y in range(38, 49) for x in range(33, 49) if (x, y) not in rear]
housing = [(x, y) for y in range(25, 31) for x in range(20, 31) if (x, y) not in visor]
body = [(x, y) for y in range(17, 64) for x in range(50) if (x, y) not in rear | visor]
body = [p for p in body if p not in fingers]
assert len(glass) == 209 and len(visor) == 16 and len(fingers) == 13
assert all(a.getpixel(p) == b.getpixel(p) for a, b in zip(frames, source) for p in fixed + housing)
assert all(im.getpixel(p) == base.getpixel(p) for im in frames for p in bezels)
assert all(im.getpixel(p) == source[0].getpixel(p) for im in frames for p in body)
assert all(im.getpixel(p) == old.getpixel(p) for im, old in zip(frames, source) for p in fingers)
typing_patterns = len({tuple(im.getpixel(p) for p in sorted(fingers)) for im in frames})
animated_fingers = sum(len({im.getpixel(p) for im in frames}) > 1 for p in fingers)
assert typing_patterns > 1 and animated_fingers > 0
# This refinement may change only the rear glyph and previously paused fingertips.
previous = root.parent / 'needs-input-v2' / 'review'
unchanged = [(x, y) for y in range(64) for x in range(64) if (x, y) not in rear | fingers]
for i, im in enumerate(frames):
    old = Image.open(previous / 'needs-input' / f'{i:03}.png').convert('RGB')
    assert all(im.getpixel(p) == old.getpixel(p) for p in unchanged)
assert all(len({im.getpixel(p) for im in frames}) > 1 for p in glass)
for im in frames:
    assert all((lambda c: c[0] > c[1] > 2 * c[2])(im.getpixel(p)) for p in glass | visor)
    assert not any(c[0] < 100 and c[1] > 105 and c[2] > 120 and c[1] > c[0] + 70
                   for c in [im.getpixel((x, y)) for y in range(17, 24) for x in range(21, 30)])

records = json.loads((review / "attention-motion.json").read_text())
assert len(records) == 32 and [r["frame"] for r in records] == list(range(32))
assert all(r["visor"] == "down" and not r["typing_paused"] for r in records)
assert all(r["rear_symbol"] == "warning_triangle" for r in records)
assert attention["symbol"] == "warning_triangle" and not attention["pause_typing"]
pulses = [r["pulse"] for r in records]
assert min(pulses) >= 0.639 and max(pulses) <= 1.001
assert abs(pulses[0] - pulses[-1]) < 0.005
# Every pixel has content changes after removing the common brightness pulse.
content_pixels = sum(max(im.getpixel(p)[0] / pulse for im, pulse in zip(frames, pulses))
                     - min(im.getpixel(p)[0] / pulse for im, pulse in zip(frames, pulses)) > 5 for p in glass)
assert content_pixels == 209
old_records = json.loads((previous / 'attention-motion.json').read_text())
assert pulses == [r['pulse'] for r in old_records]
warning_rows = [
    '.....#.....',
    '....###....',
    '...##.##...',
    '...##.##...',
    '..#######..',
    '.####.####.',
    '.#########.',
]
symbol = {(35 + x, 40 + y) for y, row in enumerate(warning_rows)
          for x, char in enumerate(row) if char == '#'}
assert len(symbol) == 36
assert all(im.getpixel(p)[0] >= 138 for im in frames for p in symbol)
assert all((lambda c: c[0] > c[1] > 2 * c[2])(im.getpixel(p)) for im in frames for p in symbol)
assert all(im.getpixel(p) == base.getpixel(p) for im in frames for p in rear - symbol)
assert all(im.getpixel(p) == base.getpixel(p) for im in frames for p in [(40, 42), (40, 43), (40, 45)])
assert all(im.getpixel((40, 44))[0] >= 138 for im in frames)
highlight_pixels = sum(max(im.getpixel(p)[0] / pulse for im, pulse in zip(frames, pulses))
                       - min(im.getpixel(p)[0] / pulse for im, pulse in zip(frames, pulses)) > 20 for p in symbol)
assert highlight_pixels == 36
traffic_patterns = {}
for vehicle in spec["traffic"]["needs-input"]:
    sprite = Image.open(root / spec["cars"][vehicle["sprite"]])
    points = sorted((x, y) for x, y in traffic if vehicle["lane"] <= y < vehicle["lane"] + sprite.height)
    patterns = {tuple(im.getpixel(p) for p in points) for im in frames}
    assert len(patterns) > 2
    assert any(im.getpixel(p) != base.getpixel(p) for im in frames for p in points)
    traffic_patterns[str(vehicle["lane"])] = len(patterns)

sheet = Image.open(review / "needs-input-sprite.png").convert("RGB")
assert sheet.size == (256, 512)
for i, im in enumerate(frames):
    x, y = i % 4 * 64, i // 4 * 64
    assert digest(sheet.crop((x, y, x + 64, y + 64))) == digest(im)
native = Image.open(review / "needs-input-native.gif")
enlarged = Image.open(review / "needs-input-preview.gif")
assert native.n_frames == enlarged.n_frames == 32
assert native.size == (64, 64) and enlarged.size == (512, 512)
assert native.getpalette() == enlarged.getpalette()
delays = []
gif_fixed = None
gif_typing = set()
for i in range(32):
    native.seek(i)
    enlarged.seek(i)
    a, b = native.convert("RGB"), enlarged.convert("RGB")
    delays.append(native.info["duration"])
    pixels = tuple(a.getpixel(p) for p in fixed + housing)
    gif_typing.add(tuple(a.getpixel(p) for p in sorted(fingers)))
    if gif_fixed is None:
        gif_fixed = pixels
    assert pixels == gif_fixed
    assert all(a.getpixel((x, y)) == b.getpixel((x * 8 + 4, y * 8 + 4)) for y in range(64) for x in range(64))
assert sum(delays) == 2650 and set(delays) == {80, 90}
assert len(gif_typing) > 1

old_manifest = json.loads((approved / "pet.json").read_text())
manifest = json.loads((review / "pet.json").read_text())
assert set(manifest["animations"]) == set(old_manifest["animations"]) | {"needs-input"}
for name, clip in old_manifest["animations"].items():
    assert manifest["animations"][name] == clip
    prefix = clip["source"]["path"].removesuffix("-sprite.png")
    for i in range(32):
        assert (review / prefix / f"{i:03}.png").read_bytes() == (approved / prefix / f"{i:03}.png").read_bytes()
    for suffix in ["sprite.png", "poster.png", "contact.png", "native.gif", "preview.gif"]:
        assert (review / f"{prefix}-{suffix}").read_bytes() == (approved / f"{prefix}-{suffix}").read_bytes()
for file in ["motorized-motion.json", "compression-motion.json", "work-cycle-preview.gif", "compaction-cycle-preview.gif"]:
    assert (review / file).read_bytes() == (approved / file).read_bytes()
clip = manifest["animations"]["needs-input"]
assert clip["loop"] and clip["frame_duration_ms"] == 83
assert clip["source"]["columns"] == 4 and clip["source"]["frame_count"] == 32

report = {
    "method": "Read-only native PNG/GIF comparisons against the accepted working pose and combined pack.",
    "frames": 32,
    "unique_complete_frames": len({digest(im) for im in frames}),
    "frame_ms": 83,
    "native_duration_ms": 2656,
    "gif_duration_ms": 2650,
    "fixed_pixels_match_corresponding_working_frame": len(fixed),
    "fixed_body_and_hand_regions_outside_typing_pixels": len(body),
    "typing_pixels_match_approved_working_frame": len(fingers),
    "animated_typing_pixels": animated_fingers,
    "typing_patterns": typing_patterns,
    "decoded_gif_typing_patterns": len(gif_typing),
    "pixels_unchanged_from_needs_input_v2_outside_rear_and_fingertips": len(unchanged),
    "animated_main_glass_pixels": 209,
    "glass_pixels_with_content_motion_after_removing_pulse": content_pixels,
    "fixed_bezel_pixels": len(bezels),
    "amber_visor_indicator_pixels": len(visor),
    "fixed_visor_housing_pixels": len(housing),
    "forehead_has_no_duplicate_powered_visor_glow": True,
    "readable_symbol_pixels": len(symbol),
    "symbol": "9x7 filled warning triangle with dark exclamation mark",
    "warning_pixels_with_highlight_motion_independent_of_pulse": highlight_pixels,
    "warning_shape_and_dark_punctuation_fixed": True,
    "symbol_never_disappears": True,
    "pulse_matches_previous_needs_input_loop": True,
    "pulse_range": [min(pulses), max(pulses)],
    "traffic_types": sorted({t["sprite"] for t in spec["traffic"]["needs-input"]}),
    "traffic_lane_patterns": traffic_patterns,
    "all_previous_clips_manifests_metadata_and_previews_preserved": True,
    "fixed_decoded_gif_pixels_stable_across_loop": True,
    "native_and_enlarged_gif_pixels_match": True,
    "palette": "One newly fitted global palette for amber needs-input preview only; accepted GIFs copied unchanged.",
    "review": "Native full-scene contact and poster inspected; owner review pending.",
    "activation": "Separate review pack; no device upload or bridge/configuration changes.",
}
(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
