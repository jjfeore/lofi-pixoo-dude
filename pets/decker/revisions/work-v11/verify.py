"""Read-only native art analysis; writes only the verification JSON record."""
import hashlib
import json
import sys
from pathlib import Path

from PIL import Image, ImageSequence


def inside(rect, x, y):
    rx, ry, width, height = rect
    return rx <= x < rx + width and ry <= y < ry + height


def digest(im):
    return hashlib.sha256(im.tobytes()).hexdigest()


def cyan(p):
    return p[3] >= 128 and p[1] > 105 and p[2] > 120 and p[1] > p[0] + 70


def skin(p):
    return p[0] > 85 and p[0] > p[1] + 15 and p[0] > p[2] + 15


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "review"
spec = json.loads((root / "authoring.json").read_text())
character = spec["character"]
assert spec["gesture"] is None
base = Image.open(root / spec["base"]).convert("RGB")
room = Image.open(review / "character-room-base.png").convert("RGB")
names = ["idle", "working-enter", "working", "finished"]
clips = {name: [Image.open(p).convert("RGB") for p in sorted((review / name).glob("*.png"))] for name in names}
old = root.parent / "work-v8" / "review"
for name in ["idle", "working"]:
    old_frames = [Image.open(old / name / f"{i:03}.png").convert("RGB") for i in range(32)]
    assert all(digest(a) == digest(b) for a, b in zip(clips[name], old_frames))
    for kind in ["native", "preview"]:
        assert (review / f"{name}-{kind}.gif").read_bytes() == (old / f"{name}-{kind}.gif").read_bytes()
joins = [
    (clips["idle"][0], clips["working-enter"][0]),
    (clips["working-enter"][-1], clips["working"][0]),
    (clips["working"][0], clips["finished"][0]),
    (clips["finished"][-1], clips["idle"][0]),
]
assert all(digest(a) == digest(b) for a, b in joins)
aperture = [(x, y) for x, top, bottom in spec["main_aperture"] for y in range(top, bottom + 1)]
main_bezel = [(x, y) for y in range(26, 53) for x in range(52, 64) if (x, y) not in aperture]
rear_bezel = [(x, y) for y in range(38, 49) for x in range(33, 49) if not inside(spec["rear_area"], x, y)]
finger_pixels = {tuple(p) for p in spec["finger_pixels"]}
dynamic_regions = [character["area"], *character["clear_regions"], spec["main_area"], spec["rear_area"], [20, 17, 12, 16], [25, 7, 35, 9], [34, 16, 26, 1]]
fixed = [(x, y) for y in range(64) for x in range(64) if (x, y) not in finger_pixels and not any(inside(r, x, y) for r in dynamic_regions)]
assert all(room.getpixel((x, y)) == base.getpixel((x, y)) for y in range(64) for x in range(64) if not any(inside(r, x, y) for r in character["clear_regions"]))
old_skin = [(x, y) for y in range(64) for x in range(64) if any(inside(r, x, y) for r in character["clear_regions"]) and skin(base.getpixel((x, y)))]
assert old_skin and all(not skin(room.getpixel(p)) for p in old_skin)
source_count = character["pose_count"] + sum(a["pose_count"] for a in character["additional_atlases"])
poses = [Image.open(review / "character-poses" / f"{i:03}.png").convert("RGBA") for i in range(source_count)]
assert len(poses) == 44 and all(p.size == (64, 64) for p in poses)
assert character["exit_poses"] == list(reversed(character["enter_poses"]))
assert len(set(character["enter_poses"])) >= 24
edit = character["forehead_edit"]
glow_counts, keyboard_skin, bands = [], [], []
for i, pose in enumerate(poses):
    glow = sum(cyan(pose.getpixel((x, y))) for y in range(14, 24) for x in range(20, 33))
    if i in edit["poses"]:
        assert glow == 0, ("duplicate forehead glow", i, glow)
    glow_counts.append(glow)
    far = sum(p[3] >= 128 and skin(p) for y in range(53, 64) for x in range(32, 48) if (p := pose.getpixel((x, y))))
    band = sum(p[3] >= 128 and p[0] > 55 and p[2] > 85 and p[0] > p[1] + 35 and p[2] > p[1] + 45 for y in range(38, 60) for x in range(1, 22) if (p := pose.getpixel((x, y))))
    assert far >= 24 and band >= 30, (i, far, band)
    keyboard_skin.append(far)
    bands.append(band)
report = {
    "method": "Complete generated character poses with additional grip/travel cells; fixed registration per source atlas, no articulated arm sprites.",
    "accepted_idle_and_working_rgb_and_gif_bytes_preserved": True,
    "canonical_transition_endpoints_match": True,
    "fixed_native_pixels": len(fixed),
    "projection_size": character["size"],
    "projection_origin": character["origin"],
    "additional_atlas_projections": [{"source": a["source"], "size": a.get("size", character["size"]), "origin": a.get("origin", character["origin"])} for a in character["additional_atlases"]],
    "generated_source_poses": source_count,
    "selected_source_poses": len(set(character["enter_poses"])),
    "character_action_reversed_for_finish": True,
    "ambient_timeline_forward_for_both_actions": True,
    "old_character_skin_replaced_with_reconstructed_room": True,
    "forehead_glow_counts_by_source_pose": glow_counts,
    "lowered_pose_count_with_no_forehead_glow": len(edit["poses"]),
    "keyboard_region_skin_pixels_min_max": [min(keyboard_skin), max(keyboard_skin)],
    "violet_sleeve_region_pixels_min_max": [min(bands), max(bands)],
    "anatomy_note": "Coverage checks guard obvious loss of hand/band pixels. They do not count anatomical hands or prove consistent transverse band width. Native contacts require visual review.",
    "clips": {},
}


def lens_component(pose):
    points = {(x, y) for y in range(14, 34) for x in range(19, 37) if cyan(pose.getpixel((x, y)))}
    components = []
    while points:
        todo = [points.pop()]
        part = set(todo)
        while todo:
            x, y = todo.pop()
            for q in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]:
                if q in points:
                    points.remove(q)
                    todo.append(q)
                    part.add(q)
        components.append(part)
    return max(components, key=len)


# Inspect the actual prop travel rather than assuming 32 exported frames imply
# enough in-betweens. White eye pixels are a smaller separate cyan component.
travel = []
for frame in range(11, 22):
    source_index = character["enter_poses"][frame]
    lens = lens_component(poses[source_index])
    center = [sum(x for x, y in lens) / len(lens), sum(y for x, y in lens) / len(lens)]
    travel.append({"entry_frame": frame, "finish_frame": 31 - frame, "source_pose": source_index, "cyan_lens_center": center, "cyan_pixels": len(lens)})
vertical_steps = [b["cyan_lens_center"][1] - a["cyan_lens_center"][1] for a, b in zip(travel, travel[1:])]
assert min(vertical_steps) >= -0.25 and max(vertical_steps) <= 2.1, vertical_steps
assert travel[-1]["cyan_lens_center"][1] - travel[0]["cyan_lens_center"][1] >= 7
assert len({round(t["cyan_lens_center"][1], 1) for t in travel}) >= 9
for source_index in [37, 38, 39, 41, 42]:
    assert glow_counts[source_index] == 0, ("extra parked glow after lowering", source_index)
report["contact_and_travel"] = {
    "entry_phases": {"reach": [1, 9], "forehead_contact_hold": [10, 11], "held_lowering": [12, 20], "eye_contact": [21, 21], "release_and_return": [22, 30], "canonical_work": [31, 31]},
    "finish_phases": {"reach": [1, 9], "eye_contact": [10, 10], "held_raising": [11, 19], "forehead_contact_hold": [20, 21], "release_and_return": [22, 30], "canonical_idle": [31, 31]},
    "travel": travel,
    "largest_vertical_lens_step_px": max(vertical_steps),
    "lens_vertical_travel_px": travel[-1]["cyan_lens_center"][1] - travel[0]["cyan_lens_center"][1],
    "note": "Lens centroids measure spacing, not anatomical contact. Whole-character head/hand contacts require visual review.",
}
previous_spec = json.loads((root.parent / "work-v10" / "authoring.json").read_text())["character"]
previous_centers = []
for frame in range(9, 22):
    index = previous_spec["enter_poses"][frame]
    pose = Image.open(root.parent / "work-v10" / "review" / "character-poses" / f"{index:03}.png").convert("RGBA")
    lens = lens_component(pose)
    previous_centers.append(sum(y for x, y in lens) / len(lens))
report["contact_and_travel"]["previous_largest_vertical_lens_step_px"] = max(b - a for a, b in zip(previous_centers, previous_centers[1:]))
assert report["contact_and_travel"]["previous_largest_vertical_lens_step_px"] > max(vertical_steps)

# Full-resolution RGBA references used for individual edits must be exact source
# crops; unlike preview normalization, extraction preserves original pixels.
original = Image.open(root / "sources" / "visor-travel.png").convert("RGBA")
for i in range(8):
    c, r = i % 4, i // 4
    crop = original.crop((c * original.width // 4, r * original.height // 2, (c + 1) * original.width // 4, (r + 1) * original.height // 2))
    extracted = Image.open(root / "sources" / "travel-original-cells" / f"{i:03}.png").convert("RGBA")
    assert crop.size == extracted.size and crop.tobytes() == extracted.tobytes()
report["original_resolution_rgba_cell_extraction_exact"] = True
window = [(x, y) for y in range(7, 17) for x in range(35, 60)]
gif_fixed = None
for name, frames in clips.items():
    assert len(frames) == 32 and all(frame.size == (64, 64) for frame in frames)
    assert all(frame.getpixel(p) == base.getpixel(p) for frame in frames for p in fixed + main_bezel)
    sheet = Image.open(review / f"{name}-sprite.png").convert("RGB")
    assert sheet.size == (256, 512)
    for i, frame in enumerate(frames):
        assert digest(frame) == digest(sheet.crop(((i % 4) * 64, (i // 4) * 64, (i % 4 + 1) * 64, (i // 4 + 1) * 64)))
        if name in ["working-enter", "finished"] and 0 < i < 31:
            indices = character["enter_poses"] if name == "working-enter" else character["exit_poses"]
            pose = poses[indices[i]]
            assert all(frame.getpixel((x, y)) == room.getpixel((x, y)) for x, y in old_skin if pose.getpixel((x, y))[3] == 0 and not inside(spec["main_area"], x, y) and not inside(spec["rear_area"], x, y))
            assert all(frame.getpixel(p) == base.getpixel(p) for p in rear_bezel if pose.getpixel(p)[3] == 0)
    native = Image.open(review / f"{name}-native.gif")
    preview = Image.open(review / f"{name}-preview.gif")
    assert native.n_frames == preview.n_frames == 32
    assert native.size == (64, 64) and preview.size == (512, 512)
    delays = []
    for i, gif_frame in enumerate(ImageSequence.Iterator(native)):
        delays.append(gif_frame.info["duration"])
        colors = [gif_frame.convert("RGB").getpixel(p) for p in fixed]
        if gif_fixed is None:
            gif_fixed = colors
        assert colors == gif_fixed
        preview.seek(i)
        assert all(preview.convert("RGB").getpixel((x * 8 + 4, y * 8 + 4)) == gif_frame.convert("RGB").getpixel((x, y)) for x, y in [(24, 28), (38, 56), (58, 35), (40, 43)])
    assert sum(delays) == 2650 and set(delays) == {80, 90}
    x, y, w, h = spec["rear_area"]
    rear_patterns = {digest(f.crop((x, y, x + w, y + h))) for f in frames}
    traffic_patterns = {tuple(f.getpixel(p) for p in window) for f in frames}
    assert len(rear_patterns) > 1 and len(traffic_patterns) > 1
    report["clips"][name] = {
        "frames": 32,
        "unique_frames": len({digest(f) for f in frames}),
        "native_frame_ms": 83,
        "native_duration_ms": 2656,
        "gif_duration_ms": sum(delays),
        "rear_monitor_patterns": len(rear_patterns),
        "window_traffic_patterns": len(traffic_patterns),
        "sheet_order_matches_frames": True,
        "fixed_scene_and_main_bezel_preserved": True,
    }
moving_glass = sum(len({f.getpixel(p) for f in clips["working"]}) > 1 for p in aperture)
assert moving_glass == len(aperture) == 209
report["main_glass_animated_pixels"] = moving_glass
cycle = Image.open(review / "work-cycle-preview.gif")
assert cycle.n_frames == 160 and cycle.size == (512, 512)
assert sum(f.info["duration"] for f in ImageSequence.Iterator(cycle)) == 13280
manifest = json.loads((review / "pet.json").read_text())
assert manifest["animations"]["working"]["entry"] == "working-enter"
assert not manifest["animations"]["working-enter"]["loop"] and not manifest["animations"]["finished"]["loop"]
report["cycle_frames"] = 160
report["cycle_duration_ms"] = 13280
report["review"] = "Native full-scene/head contacts and raised-hand posters inspected; owner review pending."
report["activation"] = "Separate review pack; no device upload or active config change."
(review / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))

