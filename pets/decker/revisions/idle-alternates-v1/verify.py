"""Read-only checks of exported artwork; writes a verification receipt only."""
import hashlib
import json
import sys
from pathlib import Path

from PIL import Image, ImageSequence


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pixels(rect):
    x, y, w, h = rect
    return {(px, py) for py in range(y, y + h) for px in range(x, x + w)}


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "review"
spec = json.loads((root / "authoring.json").read_text())
qa = json.loads((review / "qa.json").read_text())
pack = json.loads((review / "pet.json").read_text())
approved = root / spec["approved_pack"]
idle = [Image.open(approved / "idle" / f"{i:03}.png").convert("RGB") for i in range(32)]
all_pixels = {(x, y) for y in range(64) for x in range(64)}
glass = {(x, y) for x, top, bottom in spec["main_aperture"] for y in range(top, bottom + 1)}
rear = pixels(spec["rear_area"])
lamp = pixels(spec["lamp_area"])
eye = pixels(spec["eye_area"])
mouth = pixels(spec["mouth_area"])
traffic = set().union(*(pixels(r) for r in spec["car_clip"]))
buildings = set().union(*(pixels(r) for r in spec["building_areas"]))
layers = {
    "idle-flyby": [traffic, rear],
    "idle-yawn": [eye, mouth, lamp],
    "idle-message": [rear, glass],
    "idle-city": [buildings, lamp],
}
assert set(pack["animations"]) == {"idle", *layers}
assert pack["default_animation"] == "idle"
assert pack["schema_version"] == 1 and pack["canvas_size"] == 64
assert pack["animations"]["idle"] == json.loads((approved / "pet.json").read_text())["animations"]["idle"]
for i in range(32):
    name = f"idle/{i:03}.png"
    assert digest(review / name) == digest(approved / name)
for suffix in ["sprite.png", "native.gif", "preview.gif", "poster.png"]:
    name = f"idle-{suffix}"
    assert digest(review / name) == digest(approved / name)

checks = []
for name, regions in layers.items():
    paths = sorted((review / name).glob("*.png"))
    assert [p.name for p in paths] == [f"{i:03}.png" for i in range(40)]
    frames = [Image.open(path).convert("RGB") for path in paths]
    assert all(im.size == (64, 64) for im in frames)
    clip = pack["animations"][name]
    assert clip["frame_duration_ms"] == 100 and clip["loop"] is False
    source = clip["source"]
    assert source == {"type": "sprite_sheet", "path": f"{name}-sprite.png", "columns": 4, "frame_count": 40}
    sheet = Image.open(review / source["path"]).convert("RGB")
    assert sheet.size == (256, 640)
    allowed = set().union(*regions)
    outside = all_pixels - allowed
    different_layers = [0] * len(regions)
    for i, frame in enumerate(frames):
        ambient = idle[i * 32 // 39 % 32]
        assert all(frame.getpixel(p) == ambient.getpixel(p) for p in outside), (name, i)
        for j, region in enumerate(regions):
            different_layers[j] += sum(frame.getpixel(p) != ambient.getpixel(p) for p in region)
        x, y = i % 4 * 64, i // 4 * 64
        assert sheet.crop((x, y, x + 64, y + 64)).tobytes() == frame.tobytes()
    assert all(n > 0 for n in different_layers), (name, different_layers)
    assert frames[0].tobytes() == idle[0].tobytes()
    assert frames[-1].tobytes() == idle[0].tobytes()
    if name == "idle-yawn":
        # Check actual eyelid/mouth pixels, rather than just a changed face.
        assert max(frames[20].getpixel((25, 28))) < 80
        assert all(max(frames[20].getpixel((x, 35))) < 25 for x in range(23, 26))
        assert min(frames[20].getpixel((24, 34))) > 180
        assert all(frames[34].getpixel(p) == idle[34 * 32 // 39].getpixel(p) for p in eye | mouth)
    native = Image.open(review / f"{name}-native.gif")
    preview = Image.open(review / f"{name}-preview.gif")
    native_frames = [im.convert("RGB").copy() for im in ImageSequence.Iterator(native)]
    preview_frames = [im.convert("RGB").copy() for im in ImageSequence.Iterator(preview)]
    assert len(native_frames) == len(preview_frames) == 40
    assert native.size == (64, 64) and preview.size == (512, 512)
    for a, b in zip(native_frames, preview_frames):
        assert a.resize((512, 512), Image.Resampling.NEAREST).tobytes() == b.tobytes()
    for path in [review / f"{name}-native.gif", review / f"{name}-preview.gif"]:
        im = Image.open(path)
        assert [fr.info["duration"] for fr in ImageSequence.Iterator(im)] == [100] * 40
    unique = len({im.tobytes() for im in frames})
    assert unique >= 32
    checks.append({"clip": name, "frames": 40, "unique_frames": unique, "duration_ms": 4000,
                   "changes_in_each_layer": different_layers, "outside_layers_exact": True,
                   "canonical_endpoints": True, "sprite_order": True,
                   "native_and_enlarged_gifs_agree": True})

overview = Image.open(review / "alternates-overview.gif")
assert overview.size == (768, 768) and overview.n_frames == 40
assert sum(fr.info["duration"] for fr in ImageSequence.Iterator(overview)) == 4000
report = {"passed": True, "normal_idle_byte_identical": True, "clips": checks,
          "overview": {"frames": 40, "duration_ms": 4000},
          "hardware_playback_tested": False, "scheduling_implemented": False}
(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
