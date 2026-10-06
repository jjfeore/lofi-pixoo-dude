"""Inspect exported assets without editing pixels; write JSON evidence only."""
import hashlib
import json
import sys
from pathlib import Path

from PIL import Image, ImageSequence


def inside(rect, p):
    x, y, w, h = rect
    return x <= p[0] < x + w and y <= p[1] < y + h


def digest(im):
    return hashlib.sha256(im.tobytes()).hexdigest()


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / 'review'
spec = json.loads((root / 'authoring.json').read_text())
approved = root / spec['approved_pack']
paths = sorted((review / 'interrupted').glob('*.png'))
assert [p.name for p in paths] == [f'{i:03}.png' for i in range(40)]
frames = [Image.open(p).convert('RGB') for p in paths]
assert all(im.size == (64, 64) for im in frames)
source = {}
for name in ['idle', 'working', 'finished']:
    source[name] = [Image.open(approved / name / f'{i:03}.png').convert('RGB') for i in range(32)]
records = json.loads((review / 'interruption-motion.json').read_text())
assert len(records) == 40 and [r['frame'] for r in records] == list(range(40))
motor = json.loads((approved / 'motorized-motion.json').read_text())['frames']['finished']
base = Image.open(root / spec['base']).convert('RGB')
glass = {(x, y) for x, top, bottom in spec['main_aperture'] for y in range(top, bottom + 1)}
rear = {(x, y) for y in range(40, 47) for x in range(35, 46)}
traffic = {(x, y) for y in range(64) for x in range(64) if any(inside(r, (x, y)) for r in spec['traffic_mask'])}
expression = {(x, y) for y in range(64) for x in range(64) if inside(spec['expression_area'], (x, y))}
fingers = {tuple(p) for p in json.loads((root.parent / 'needs-input-v3' / 'authoring.json').read_text())['attention']['finger_pixels']}
ray_core = {tuple(p) for ray in spec['shock_rays'] for p in ray['core']}
ray_edge = {tuple(p) for ray in spec['shock_rays'] for p in ray['edge']}
sparks = ray_core | ray_edge
assert len(spec['shock_rays']) == 3 and len(sparks) == 24 and not ray_core & ray_edge
mx, my = spec['mouth']['origin']
mouth_mask = {(mx+x, my+y) for x,y in [(0,0),(1,0),(2,0),(0,1),(1,1),(2,1),(1,2),(2,2)]}
face_fixed = {(x,y) for y in range(31,44) for x in range(12,31)} - mouth_mask
assert spec['mouth']['stages'] == [0,1,2,3,3,3,3,2,1] + [0]*31
assert all(p is None for p in spec['expression_frames']) and spec['expression_study'] is None
previous = root.parent / 'interrupted-v2' / 'review'
unchanged_from_previous = {(x,y) for y in range(64) for x in range(64)} - expression - sparks
bezel = {(x, y) for y in range(26, 53) for x in range(52, 64) if (x, y) not in glass}
bezel |= {(x, y) for y in range(38, 49) for x in range(33, 49) if (x, y) not in rear}
assert len(glass) == 209 and len(bezel) == 214
assert digest(frames[0]) == digest(source['working'][0])
assert digest(frames[39]) == digest(source['idle'][0])
fixed_counts = []
expression_changes = {}
mouth_changes = {}
visible_colors = {v['id']: set() for v in spec['chase'] if v['police']}
maximum_visible = 0
for i, (im, rec) in enumerate(zip(frames, records)):
    name, j = ('working', i) if i < 8 else ('finished', i - 8)
    original = source[name][j]
    pose = motor[max(0, i - 8)]
    assert rec['source'] == name and rec['source_frame'] == j
    assert rec['visor_bounds'] == pose['visor_bounds'] and rec['lowered'] == pose['lowered']
    assert rec['expression'] == spec['expression_frames'][i]
    assert rec['mouth_stage'] == spec['mouth']['stages'][i]
    assert rec['shock_ray_count'] == (3 if i in [2,4,6] else 0)
    old_frame = Image.open(previous / 'interrupted' / f'{i:03}.png').convert('RGB')
    assert all(im.getpixel(p) == old_frame.getpixel(p) for p in unchanged_from_previous)
    assert all(im.getpixel(p) == source['working'][0].getpixel(p) for p in face_fixed)
    allowed = glass | rear | traffic
    allowed |= {(x, y) for y in range(64) for x in range(64) if inside(rec['visor_bounds'], (x, y))}
    if rec['expression'] is not None:
        allowed |= expression
        changes = sum(im.getpixel(p) != original.getpixel(p) for p in expression)
        assert changes > 0
        expression_changes[str(i)] = changes
    if rec['mouth_stage']:
        allowed |= mouth_mask
        changes = sum(im.getpixel(p) != original.getpixel(p) for p in mouth_mask)
        assert changes > 0
        mouth_changes[str(i)] = changes
    else:
        assert all(im.getpixel(p) == original.getpixel(p) for p in mouth_mask)
    if i in [2, 4, 6]:
        allowed |= sparks
        assert all(im.getpixel(p) == (255, 79, 108) for p in ray_core)
        assert all(im.getpixel(p) == (240, 27, 61) for p in ray_edge)
    else:
        assert all(im.getpixel(p) == original.getpixel(p) for p in sparks)
    fixed = [(x, y) for y in range(64) for x in range(64) if (x, y) not in allowed]
    assert all(im.getpixel(p) == original.getpixel(p) for p in fixed)
    assert all(im.getpixel(p) == original.getpixel(p) for p in fingers | bezel)
    assert all(im.getpixel((x, y)) == original.getpixel((x, y)) for y in range(47, 64) for x in range(50))
    fixed_counts.append(len(fixed))
    assert len(rec['chase']) == 6
    lead = rec['chase'][0]
    assert lead['id'] == 'getaway' and not lead['police']
    assert all(lead['x'] > car['x'] for car in rec['chase'][1:])
    if i:
        assert all(car['x'] >= records[i-1]['chase'][n]['x'] for n, car in enumerate(rec['chase']))
    visible = sum(car['visible_pixels'] > 0 for car in rec['chase'])
    maximum_visible = max(maximum_visible, visible)
    for n, car in enumerate(rec['chase']):
        assert car['id'] == spec['chase'][n]['id'] and car['lane'] == spec['chase'][n]['lane']
        if car['police'] and car['visible_pixels']:
            visible_colors[car['id']].add(car['beacon'])
            x = car['x'] + (1 if car['beacon'] == 'red' else 2)
            p = (x, car['lane'])
            if p in traffic:
                expected = (255, 38, 66) if car['beacon'] == 'red' else (45, 142, 255)
                assert im.getpixel(p) == expected
assert maximum_visible == 6
assert all(colors == {'red', 'blue'} for colors in visible_colors.values())
assert all(car['visible_pixels'] == 0 for car in records[0]['chase'] + records[-1]['chase'])
assert all(len({im.getpixel(p) for im in frames}) > 1 for p in glass)
assert all((lambda c: c[0] > c[1] + c[2])(frames[1].getpixel(p)) for p in glass)
powered = json.loads((root.parent / 'needs-input-v3' / 'authoring.json').read_text())['attention']['visor_pixels']
assert all((lambda c: c[0] > c[1] + c[2])(frames[1].getpixel(tuple(p))) for p in powered)
assert all(r['red_amount'] == 0 for r in records[34:])
assert all(r['rear_stop_strength'] == 0 for r in records[38:])
assert all(frames[i].getpixel(p) == source['finished'][i-8].getpixel(p) for i in range(34, 40) for p in glass)
assert all(frames[i].getpixel(p) == source['finished'][i-8].getpixel(p) for i in range(38, 40) for p in rear)
assert all(rec['lowered'] == 1 for rec in records[:13]) and all(rec['lowered'] == 0 for rec in records[28:])
assert all(records[i]['lowered'] >= records[i+1]['lowered'] for i in range(39))
open_mouth = [tuple(frames[i].getpixel(p) for p in sorted(mouth_mask)) for i in range(3,7)]
assert len(set(open_mouth)) == 1
assert len({tuple(im.getpixel(p) for p in sorted(mouth_mask)) for im in frames}) == 4
assert all(frames[4].getpixel(p) == (238,210,191) for p in [(mx+1,my),(mx+2,my)])
stop = {(40 + dx, 40 + y) for y, half in enumerate([2, 3, 4, 4, 4, 3, 2]) for dx in range(-half, half+1)}
bar = {(x, 43) for x in range(38, 43)}
assert all(frames[1].getpixel(p) == (255, 231, 233) for p in bar)
assert all((lambda c: c[0] > c[1] + c[2])(frames[1].getpixel(p)) for p in stop - bar)

sheet = Image.open(review / 'interrupted-sprite.png').convert('RGB')
assert sheet.size == (256, 640)
for i, im in enumerate(frames):
    x, y = i % 4 * 64, i // 4 * 64
    assert digest(sheet.crop((x, y, x+64, y+64))) == digest(im)
native = Image.open(review / 'interrupted-native.gif')
large = Image.open(review / 'interrupted-preview.gif')
assert native.n_frames == large.n_frames == 40 and native.size == (64, 64) and large.size == (512, 512)
assert native.getpalette() == large.getpalette()
delays = []
static = [(x, y) for y in range(47, 64) for x in range(50) if (x, y) not in fingers]
static += sorted(face_fixed)
static_colors = None
for i in range(40):
    native.seek(i)
    large.seek(i)
    a, b = native.convert('RGB'), large.convert('RGB')
    colors = tuple(a.getpixel(p) for p in static)
    if static_colors is None:
        static_colors = colors
    assert colors == static_colors
    assert all(a.getpixel((x, y)) == b.getpixel((x*8+4, y*8+4)) for y in range(64) for x in range(64))
    delays.append(native.info['duration'])
assert sum(delays) == 3320 and set(delays) == {80, 90}
cycle = Image.open(review / 'interruption-cycle-preview.gif')
assert cycle.n_frames == 104 and sum(f.info['duration'] for f in ImageSequence.Iterator(cycle)) == 8630

old = json.loads((approved / 'pet.json').read_text())
manifest = json.loads((review / 'pet.json').read_text())
assert set(manifest['animations']) == set(old['animations']) | {'interrupted'}
prefixes = set()
for name, clip in old['animations'].items():
    assert manifest['animations'][name] == clip
    prefix = clip['source']['path'].removesuffix('-sprite.png')
    prefixes.add(prefix)
    for i in range(32):
        file = Path(prefix) / f'{i:03}.png'
        assert (review / file).read_bytes() == (approved / file).read_bytes()
    for suffix in ['sprite.png', 'poster.png', 'contact.png', 'native.gif', 'preview.gif']:
        file = f'{prefix}-{suffix}'
        assert (review / file).read_bytes() == (approved / file).read_bytes()
for file in ['motorized-motion.json', 'compression-motion.json', 'attention-motion.json', 'work-cycle-preview.gif', 'compaction-cycle-preview.gif']:
    assert (review / file).read_bytes() == (approved / file).read_bytes()
clip = manifest['animations']['interrupted']
assert not clip['loop'] and clip['frame_duration_ms'] == 83 and clip['source']['frame_count'] == 40 and clip['source']['columns'] == 4
report = dict(method='Independent read-only PNG/GIF comparisons against accepted working/idle/finish and motion records.',
    frames=40, unique_frames=len({digest(im) for im in frames}), frame_ms=83, native_duration_ms=3320, gif_duration_ms=3320,
    canonical_working_start=True, canonical_idle_end=True, motorized_finish_path_preserved=True,
    minimum_fixed_source_pixels=min(fixed_counts), exact_arms_and_hands_against_corresponding_source=True,
    typing_pixels_preserved=len(fingers), fixed_bezel_pixels=len(bezel), animated_glass_pixels=len(glass),
    initial_red_visor_pixels=len(powered), screen_and_visor_red_end_frame=34, rear_restored_frame=38,
    expression_changed_pixels=expression_changes, mouth_changed_pixels=mouth_changes,
    mouth_mask_pixels=len(mouth_mask), fixed_lower_face_region_pixels=len(face_fixed),
    mouth_patterns=4, fully_open_mouth_held_without_shape_jitter=True,
    previous_interruption_pixels_preserved_outside_jaw_and_rays=len(unchanged_from_previous),
    shock_spark_frames=[2,4,6], shock_rays=3, shock_ray_pixels=len(sparks),
    chase_getaway_count=1, chase_police_count=5, maximum_visible_vehicles=maximum_visible,
    chase_moves_forward_without_wrap_or_overtaking=True, all_patrols_have_both_beacon_colors_while_visible=True,
    stop_symbol_pixels=len(stop), white_halt_bar_pixels=len(bar),
    prior_native_frames_preserved=len(prefixes)*32, prior_individual_gifs_preserved=len(prefixes)*2,
    previous_manifests_motion_records_and_previews_preserved=True, shared_gif_palette_and_native_enlarged_matching=True,
    static_arm_colors_stable_in_gif=True, context_preview_frames=104, context_preview_duration_ms=8630,
    review='Native full-scene, head and rear contacts inspected; owner review pending.',
    activation='Separate review pack; no live upload, runtime build or configuration change.')
(review / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
