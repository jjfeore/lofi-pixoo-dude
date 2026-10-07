"""Read-only artwork checks; writes a verification receipt, never image pixels."""
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


def rows(values):
    return {(x, y) for y, left, right in values for x in range(left, right + 1)}


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "review"
spec = json.loads((root / "authoring.json").read_text())
refinement = spec["refinement"]
qa = json.loads((review / "qa.json").read_text())
pack = json.loads((review / "pet.json").read_text())
approved = root / spec["approved_pack"]
base = Image.open(root / spec["base"]).convert("RGB")
idle = [Image.open(approved / "idle" / f"{i:03}.png").convert("RGB") for i in range(32)]
all_pixels = {(x, y) for y in range(64) for x in range(64)}
glass = {(x, y) for x, top, bottom in spec["main_aperture"] for y in range(top, bottom + 1)}
rear = pixels(spec["rear_area"])
lamp = pixels(spec["lamp_area"])
eye = pixels(spec["eye_area"])
mouth = pixels(spec["mouth_area"])
traffic = set().union(*(pixels(r) for r in refinement["traffic_clear"]))
traffic_glass = rows(refinement["traffic_rows"])
flyby = rows(refinement["flyby_rows"])
floors = {tuple(p) for floor in refinement["floor_pixels"] for p in floor}
elevator = {tuple(p) for p in refinement["elevator_visible"]}
layers = {
    "idle-flyby": [flyby, rear, traffic],
    "idle-yawn": [eye, mouth, lamp, traffic],
    "idle-message": [rear, glass, traffic],
    "idle-city": [floors, elevator, lamp, traffic],
}
assert set(pack["animations"]) == {"idle", *layers}
assert pack["default_animation"] == "idle"
assert pack["schema_version"] == 1 and pack["canvas_size"] == 64
assert spec["frame_ms"] == qa["frame_ms"] == 83
assert all(clip["frame_duration_ms"] == 83 for clip in pack["animations"].values())
assert pack["animations"]["idle"] == json.loads((approved / "pet.json").read_text())["animations"]["idle"]
assert lamp == {(x, y) for x in [4, 5] for y in range(15, 25)}
assert floors == {(x, y) for x in [41, 42] for y in [23, 26, 29]}
assert elevator == {(49, y) for y in range(16, 27)}
assert traffic_glass <= traffic
for i in range(32):
    name = f"idle/{i:03}.png"
    assert digest(review / name) == digest(approved / name)
for suffix in ["sprite.png", "native.gif", "preview.gif", "poster.png"]:
    name = f"idle-{suffix}"
    assert digest(review / name) == digest(approved / name)

cars = {name: Image.open(root / path).convert("RGBA") for name, path in refinement["cars"].items()}
close_car = Image.open(review / "flyby-car-sprite.png").convert("RGBA")
delays = [((i + 1) * 83 // 10 - i * 83 // 10) * 10 for i in range(40)]
assert set(delays) == {80, 90} and sum(delays) == 3320
checks = []
traffic_signatures = set()
for name, regions in layers.items():
    paths = sorted((review / name).glob("*.png"))
    assert [p.name for p in paths] == [f"{i:03}.png" for i in range(40)]
    frames = [Image.open(path).convert("RGB") for path in paths]
    assert all(im.size == (64, 64) for im in frames)
    clip = pack["animations"][name]
    assert clip["loop"] is False
    source = clip["source"]
    assert source == {"type": "sprite_sheet", "path": f"{name}-sprite.png", "columns": 4, "frame_count": 40}
    sheet = Image.open(review / source["path"]).convert("RGB")
    assert sheet.size == (256, 640)
    allowed = set().union(*regions)
    outside = all_pixels - allowed
    different_layers = [0] * len(regions)
    records = next(c["motion"] for c in qa["clips"] if c["id"] == name)
    traffic_signatures.add(json.dumps([r["traffic"] for r in records], sort_keys=True))
    for i, frame in enumerate(frames):
        ambient = idle[i * 32 // 39 % 32]
        assert all(frame.getpixel(p) == ambient.getpixel(p) for p in outside), (name, i)
        for j, region in enumerate(regions):
            different_layers[j] += sum(frame.getpixel(p) != ambient.getpixel(p) for p in region)
        x, y = i % 4 * 64, i // 4 * 64
        assert sheet.crop((x, y, x + 64, y + 64)).tobytes() == frame.tobytes()
        # Independently reconstruct only the small traffic pixels from the
        # recorded paths and unchanged assets. No extra car or old traffic can remain.
        expected = {p: base.getpixel(p) for p in traffic}
        for j, vehicle in enumerate(refinement["traffic"][name]):
            record = records[i]["traffic"][j]
            assert record["sprite"] == vehicle["sprite"] and record["lane"] == vehicle["lane"]
            car = cars[vehicle["sprite"]]
            visible = set()
            for sy in range(car.height):
                for sx in range(car.width):
                    p = (record["x"] + sx, vehicle["lane"] + sy)
                    color = car.getpixel((car.width - 1 - sx if vehicle["mirror"] else sx, sy))
                    if p not in traffic_glass or color[3] == 0:
                        continue
                    visible.add(p)
                    previous = expected[p]
                    expected[p] = tuple((color[c] * color[3] + previous[c] * (255 - color[3]) + 127) // 255 for c in range(3))
            assert visible == {tuple(p) for p in record["pixels"]}
            if i:
                step = record["x"] - records[i - 1]["traffic"][j]["x"]
                assert step * (vehicle["end_x"] - vehicle["start_x"]) >= 0
            if i in [0, 39]:
                assert not visible
        exempt = elevator if name == "idle-city" else set()
        assert all(frame.getpixel(p) == color for p, color in expected.items() if p not in exempt), (name, i, "traffic")
    assert all(n > 0 for n in different_layers), (name, different_layers)
    assert frames[0].tobytes() == idle[0].tobytes()
    assert frames[-1].tobytes() == idle[0].tobytes()
    if name == "idle-flyby":
        extended = set()
        for i in range(3, 37):
            record = records[i]
            expected = {(record["car_x"] + sx, 17 + sy)
                        for sy in range(close_car.height) for sx in range(close_car.width)
                        if close_car.getpixel((sx, sy))[3] > 0
                        and (record["car_x"] + sx, 17 + sy) in flyby}
            assert expected == {tuple(p) for p in record["car_pixels"]}
            extended |= {p for p in expected if p[0] < 34}
            if i > 3:
                assert record["car_x"] < records[i - 1]["car_x"]
        assert extended and any(frames[i].getpixel(p) != idle[i * 32 // 39 % 32].getpixel(p)
                                for i in range(3, 37) for p in extended)
        assert not records[36]["car_pixels"]
    if name == "idle-yawn":
        assert max(frames[20].getpixel((25, 28))) < 80
        assert all(max(frames[20].getpixel((x, 35))) < 25 for x in range(23, 26))
        assert min(frames[20].getpixel((24, 34))) > 180
        assert all(frames[34].getpixel(p) == idle[34 * 32 // 39].getpixel(p) for p in eye | mouth)
    if name == "idle-message":
        # Check the complete ten-by-six outline after the requested +1,+1 shift.
        ox, oy = refinement["envelope_offset"]
        assert (ox, oy) == (1, 1)
        for yy in range(7):
            for xx in range(11):
                u, v = xx - ox, yy - oy
                outline = 0 <= u < 10 and 0 <= v < 6 and (v in [0, 5] or u in [0, 9]
                            or (v <= 4 and u in [v, 9 - v]))
                assert frames[20].getpixel((35 + xx, 40 + yy)) == ((116, 250, 202) if outline else (5, 18, 28))
        assert frames[20].getpixel((45, 40)) == (5, 18, 28)
        assert frames[20].getpixel((36, 41)) == frames[20].getpixel((45, 46)) == (116, 250, 202)
    if name == "idle-city":
        positions = []
        for i, frame in enumerate(frames):
            ambient = idle[i * 32 // 39 % 32]
            lit = {p for p in elevator if frame.getpixel(p) != ambient.getpixel(p)}
            assert len(lit) <= 1, (i, lit)
            positions.extend(sorted(lit))
            assert all(frame.getpixel((49, y)) == ambient.getpixel((49, y)) for y in range(27, 35))
        assert positions[0] == (49, 16) and positions[-1] == (49, 26)
        assert all(a[1] <= b[1] for a, b in zip(positions, positions[1:]))
        assert all(p in elevator for p in positions)
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
        assert [fr.info["duration"] for fr in ImageSequence.Iterator(im)] == delays
    unique = len({im.tobytes() for im in frames})
    assert unique >= 32
    checks.append({"clip": name, "frames": 40, "unique_frames": unique, "frame_duration_ms": 83,
                   "duration_ms": 3320, "gif_delays_ms": sorted(set(delays)),
                   "changes_in_each_layer": different_layers, "outside_layers_exact": True,
                   "canonical_endpoints": True, "sprite_order": True,
                   "one_pass_traffic_verified": True, "native_and_enlarged_gifs_agree": True})

assert len(traffic_signatures) == 4
overview = Image.open(review / "alternates-overview.gif")
assert overview.size == (768, 768) and overview.n_frames == 40
assert [fr.info["duration"] for fr in ImageSequence.Iterator(overview)] == delays
report = {"passed": True, "normal_idle_byte_identical": True, "clips": checks,
          "lamp_bounded_to_two_columns": True, "flyby_follows_head_outline": True,
          "envelope_enlarged_without_badge": True, "floors_on_existing_window_rows": True,
          "envelope_offset": refinement["envelope_offset"],
          "elevator_single_pixel_and_occluded": True, "four_distinct_traffic_patterns": True,
          "overview": {"frames": 40, "duration_ms": 3320},
          "hardware_playback_tested": False, "scheduling_implemented": False}
(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
