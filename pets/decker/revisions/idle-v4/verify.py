"""Read-only native artwork checks; writes a JSON evidence record only."""
import hashlib
import json
import sys
from collections import Counter
from pathlib import Path

from PIL import Image, ImageSequence


def digest(image):
    return hashlib.sha256(image.tobytes()).hexdigest()


def frames(directory):
    paths = sorted(directory.glob("*.png"))
    assert [p.name for p in paths] == [f"{i:03}.png" for i in range(32)]
    images = [Image.open(p).convert("RGB") for p in paths]
    assert all(im.size == (64, 64) for im in images)
    return images


def check_sprite(path, images):
    sheet = Image.open(path).convert("RGB")
    assert sheet.size == (256, 512)
    for i, im in enumerate(images):
        x, y = i % 4 * 64, i // 4 * 64
        assert digest(sheet.crop((x, y, x + 64, y + 64))) == digest(im)


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "review"
spec = json.loads((root / "authoring.json").read_text())
old_root = root.parent / "idle-v3" / "review"
base = Image.open(root / spec["base"]).convert("RGB")
new = frames(review / "frames")
old = frames(old_root / "frames")
glass = {(x, y) for x, top, bottom in spec["monitor_aperture"] for y in range(top, bottom + 1)}
outside = [(x, y) for y in range(64) for x in range(64) if (x, y) not in glass]
bezel = [(x, y) for y in range(26, 53) for x in range(52, 64) if (x, y) not in glass]
assert len(glass) == 209
assert all(a.getpixel(p) == b.getpixel(p) for a, b in zip(new, old) for p in outside)
assert all(im.getpixel(p) == base.getpixel(p) for im in new for p in bezel)
assert all(len({im.getpixel(p) for im in new}) > 1 for p in glass)
assert digest(new[0]) == digest(old[0])
for x, top, bottom in spec["monitor_aperture"]:
    reference_colors = Counter(base.getpixel((x, y)) for y in range(top, bottom + 1))
    for im in new:
        assert Counter(im.getpixel((x, y)) for y in range(top, bottom + 1)) == reference_colors
    # The next frame after the last is one row forward, with no screen reset.
    assert all(new[-1].getpixel((x, y)) == base.getpixel((x, bottom if y == top else y - 1))
               for y in range(top, bottom + 1))
changed_per_frame = [sum(a.getpixel(p) != b.getpixel(p) for p in glass) for a, b in zip(new, old)]
wrap_changed = {(x, y) for y in range(64) for x in range(64)
                if new[-1].getpixel((x, y)) != new[0].getpixel((x, y))}
rear = {(x, y) for y in range(40, 47) for x in range(35, 46)}
assert wrap_changed <= glass | rear
for name in ["car-sprite.png", "second-car-sprite.png", "waveform-sprite.png", "hands-contact.png"]:
    assert (review / name).read_bytes() == (old_root / name).read_bytes()
qa = json.loads((review / "qa.json").read_text())
assert qa["registration_offsets"] == json.loads((old_root / "qa.json").read_text())["registration_offsets"]
check_sprite(review / "idle-sprite.png", new)

for kind, size in [("native", 64), ("preview", 512)]:
    reference = Image.open(old_root / f"idle-{kind}.gif")
    candidate = Image.open(review / f"idle-{kind}.gif")
    assert candidate.n_frames == reference.n_frames == 32
    assert candidate.size == (size, size)
    assert candidate.getpalette() == reference.getpalette()
    durations = []
    for i in range(32):
        candidate.seek(i)
        reference.seek(i)
        durations.append(candidate.info["duration"])
        a, b = candidate.convert("RGB"), reference.convert("RGB")
        scale = size // 64
        assert all(a.getpixel((x * scale, y * scale)) == b.getpixel((x * scale, y * scale)) for x, y in outside)
    assert sum(durations) == 2650 and set(durations) == {80, 90}
native, enlarged = Image.open(review / "idle-native.gif"), Image.open(review / "idle-preview.gif")
for i in range(32):
    native.seek(i)
    enlarged.seek(i)
    a, b = native.convert("RGB"), enlarged.convert("RGB")
    assert all(a.getpixel((x, y)) == b.getpixel((x * 8 + 4, y * 8 + 4)) for y in range(64) for x in range(64))
manifest = json.loads((review / "pet.json").read_text())
clip = manifest["animations"]["idle"]
assert clip["loop"] and clip["frame_duration_ms"] == 83
assert clip["source"]["frame_count"] == 32 and clip["source"]["columns"] == 4

report = {
    "method": "Read-only Pillow decode; exact comparisons against approved idle-v3 outside the full angled glass.",
    "frames": 32,
    "frame_ms": 83,
    "native_duration_ms": 2656,
    "gif_duration_ms": 2650,
    "glass_pixels": len(glass),
    "animated_glass_pixels": len(glass),
    "identical_per_frame_pixels_outside_glass": len(outside),
    "fixed_bezel_pixels": len(bezel),
    "original_column_colors_preserved": True,
    "first_frame_identical_to_approved_idle": True,
    "typing_traffic_waveform_and_character_preserved": True,
    "gif_palette_and_pixels_outside_glass_preserved": True,
    "native_and_enlarged_gif_pixels_match": True,
    "last_to_first_changes_confined_to_screen_interiors": True,
    "last_to_first_glass_step": "one cyclic source row in every column",
    "changed_glass_pixels_per_frame": changed_per_frame,
    "unique_complete_frames": len({digest(im) for im in new}),
    "activation": "Separate review pack; no device upload or active configuration change.",
}

combined = root / "combined"
if combined.exists():
    previous = root.parent / "work-v13" / "review"
    clip_names = ["idle", "working-enter", "working", "finished"]
    for name in clip_names:
        candidate = frames(combined / name)
        reference = frames(previous / name)
        assert all(a.getpixel(p) == b.getpixel(p) for a, b in zip(candidate, reference) for p in outside)
        check_sprite(combined / f"{name}-sprite.png", candidate)
        if name == "idle":
            assert all(digest(a) == digest(b) for a, b in zip(candidate, new))
        if name == "working":
            assert all(digest(a) == digest(b) for a, b in zip(candidate, reference))
            for kind in ["native", "preview"]:
                assert (combined / f"{name}-{kind}.gif").read_bytes() == (previous / f"{name}-{kind}.gif").read_bytes()
        for kind, size in [("native", 64), ("preview", 512)]:
            a, b = Image.open(combined / f"{name}-{kind}.gif"), Image.open(previous / f"{name}-{kind}.gif")
            assert a.n_frames == b.n_frames == 32 and a.size == (size, size)
            for i in range(32):
                a.seek(i)
                b.seek(i)
                scale = size // 64
                assert all(a.convert("RGB").getpixel((x * scale, y * scale)) == b.convert("RGB").getpixel((x * scale, y * scale))
                           for x, y in [(24, 28), (38, 56), (40, 43), (28, 19), (35, 12)])
    assert (combined / "motorized-motion.json").read_bytes() == (previous / "motorized-motion.json").read_bytes()
    clips = {name: frames(combined / name) for name in clip_names}
    for a, b in [(clips["working-enter"][0], clips["idle"][0]),
                 (clips["working-enter"][-1], clips["working"][0]),
                 (clips["finished"][0], clips["working"][0]),
                 (clips["finished"][-1], clips["idle"][0])]:
        assert digest(a) == digest(b)
    cycle = Image.open(combined / "work-cycle-preview.gif")
    assert cycle.n_frames == 160
    assert sum(f.info["duration"] for f in ImageSequence.Iterator(cycle)) == 13280
    report["combined_pack"] = {
        "clips": clip_names,
        "only_main_glass_changes_in_idle_start_finish": True,
        "working_rgb_and_gif_bytes_unchanged": True,
        "motorized_gesture_metadata_unchanged": True,
        "canonical_endpoints_unchanged": True,
        "cycle_frames": 160,
        "cycle_duration_ms": 13280,
    }

(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
