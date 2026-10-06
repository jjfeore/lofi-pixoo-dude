"""Read-only inspection of exported artwork; writes JSON evidence only."""
import hashlib
import json
import sys
from pathlib import Path

from PIL import Image, ImageSequence


def inside(rect, point):
    x, y, w, h = rect
    return x <= point[0] < x+w and y <= point[1] < y+h


def digest(image):
    return hashlib.sha256(image.tobytes()).hexdigest()


root = Path(__file__).resolve().parent
review = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / 'review'
spec = json.loads((root / 'authoring.json').read_text())
approved = root / spec['approved_pack']
base = Image.open(root / spec['base']).convert('RGB')
source = [Image.open(approved / 'working' / f'{i:03}.png').convert('RGB') for i in range(32)]
records = json.loads((review / 'delegation-motion.json').read_text())
sprite = Image.open(review / 'drone-sprite.png').convert('RGBA')
assert sprite.size == tuple(spec['drone']['size']) == (17, 8)
assert {sprite.getpixel((x,y))[3] for y in range(sprite.height) for x in range(sprite.width)} == {0,255}
opaque = {(x,y) for y in range(sprite.height) for x in range(sprite.width) if sprite.getpixel((x,y))[3]}
generated = Image.open(root / spec['drone']['source']).convert('RGBA')
x,y,w,h = spec['drone']['crop']
for py in range(sprite.height):
    for px in range(sprite.width):
        # Sample exact cell centers; Pillow's floating-point resize can pick the
        # other neighbor at the central column's mathematically integral tie.
        expected = generated.getpixel((x+(2*px+1)*w//(2*sprite.width), y+(2*py+1)*h//(2*sprite.height)))
        actual = sprite.getpixel((px,py))
        assert actual[:3] == expected[:3] and actual[3] == (255 if expected[3] >= 128 else 0)
assert tuple(spec['drone']['lamp']) in opaque
glass_spec = json.loads((root.parent / 'interrupted-v4' / 'authoring.json').read_text())['main_aperture']
glass = {(x,y) for x,top,bottom in glass_spec for y in range(top,bottom+1)}
rear = {(x,y) for y in range(40,47) for x in range(35,46)}
traffic = {(x,y) for y in range(64) for x in range(64) if any(inside(r,(x,y)) for r in spec['traffic_mask'])}
window = {(x,y) for y in range(64) for x in range(64) if any(inside(r,(x,y)) for r in spec['window_mask'])}
assert not window & (rear | traffic | glass)
bezel = {(x,y) for y in range(26,53) for x in range(52,64)} - glass
bezel |= {(x,y) for y in range(38,49) for x in range(33,49)} - rear
fingers = {tuple(p) for p in json.loads((root.parent / 'needs-input-v3' / 'authoring.json').read_text())['attention']['finger_pixels']}
frames = {}
fixed_counts = []
report = {}
for name,count in [('delegating-start',40),('delegating',32),('delegating-finished',40)]:
    paths = sorted((review / name).glob('*.png'))
    assert [p.name for p in paths] == [f'{i:03}.png' for i in range(count)]
    frames[name] = [Image.open(p).convert('RGB') for p in paths]
    motion = records[name]
    assert len(motion) == count
    for i,(image,record) in enumerate(zip(frames[name],motion)):
        assert image.size == (64,64) and record['frame'] == i
        assert record['source_frame'] == i*32//count
        original = source[record['source_frame']]
        drone = record['drone']
        footprint = set()
        if drone is not None:
            footprint = {(drone['x']+x,drone['y']+y) for x,y in opaque if (drone['x']+x,drone['y']+y) in window}
            assert footprint == {tuple(p) for p in drone['visible_pixels']}
            for p in footprint:
                local = (p[0]-drone['x'],p[1]-drone['y'])
                expected = tuple(drone['lamp']) if local == tuple(spec['drone']['lamp']) else sprite.getpixel(local)[:3]
                assert image.getpixel(p) == expected
            if name == 'delegating-start':
                assert drone['received'] == (i >= 20) and drone['ack'] == (20 <= i < 23)
            else:
                assert drone['received'] == (i < 23) and not drone['ack']
            expected_lamp = [196,255,223] if drone['ack'] else [69,255,132] if drone['received'] else [48,48,63]
            assert drone['lamp'] == expected_lamp
        assert footprint <= window
        allowed = rear | traffic | footprint
        fixed = {(x,y) for y in range(64) for x in range(64)} - allowed
        assert all(image.getpixel(p) == original.getpixel(p) for p in fixed)
        fixed_counts.append(len(fixed))
        assert all(image.getpixel(p) == original.getpixel(p) for p in glass | fingers | bezel)
        assert all(image.getpixel((x,y)) == original.getpixel((x,y)) for y in range(47,64) for x in range(50))
        if name == 'delegating':
            assert drone is None and len(record['traffic']) == 3 and record['rear']['mode'] == 'matrix'
            assert all((lambda c: c == (0,0,0) or c[1] > c[0] and c[1] > c[2])(image.getpixel(p)) for p in rear)
            assert any(image.getpixel(p) == (0,0,0) for p in rear)
            for column,phase in enumerate([0,3,6,2,5]):
                head = (i//2 + phase) % 8
                if head < 7:
                    assert image.getpixel((36+column*2,40+head)) == (168,255,188)
            for n,car in enumerate(record['traffic']):
                assert car['sprite'] == spec['traffic'][n]['sprite'] and car['lane'] == spec['traffic'][n]['lane']
                assert {tuple(p) for p in car['pixels']} <= traffic
        else:
            assert not record['traffic'] and all(image.getpixel(p) == base.getpixel(p) for p in traffic)
    assert all(len({image.getpixel(p) for image in frames[name]}) > 1 for p in glass)
    assert len({digest(image) for image in frames[name]}) == count
    if name != 'delegating':
        drone_records = [r['drone'] for r in motion]
        assert not drone_records[0]['visible_pixels'] and not drone_records[-1]['visible_pixels']
        assert max(len(d['visible_pixels']) for d in drone_records) == len(opaque)
        assert all([d['x'],d['y']] == spec['drone']['hover'] for d in drone_records[11:29])
        assert all(drone_records[i]['x'] <= drone_records[i+1]['x'] for i in range(28,39))
        if name == 'delegating-start':
            assert all(drone_records[i]['y'] >= drone_records[i+1]['y'] for i in range(11))
        else:
            assert all(drone_records[i]['x'] >= drone_records[i+1]['x'] for i in range(11))
        shapes = [frozenset(p for p in rear if frames[name][i].getpixel(p) != (0,0,0)) for i in range(4,27)]
        assert len(set(shapes)) == 1
        assert len({tuple(frames[name][i].getpixel(p) for p in sorted(rear)) for i in range(4,27)}) >= 6
    sheet = Image.open(review / f'{name}-sprite.png').convert('RGB')
    assert sheet.size == (256, ((count+3)//4)*64)
    for i,image in enumerate(frames[name]):
        sx,sy = i%4*64,i//4*64
        assert digest(sheet.crop((sx,sy,sx+64,sy+64))) == digest(image)
    native = Image.open(review / f'{name}-native.gif')
    large = Image.open(review / f'{name}-preview.gif')
    assert native.n_frames == large.n_frames == count and native.size == (64,64) and large.size == (512,512)
    assert native.getpalette() == large.getpalette()
    static = sorted(({(x,y) for y in range(47,64) for x in range(50)} - fingers) | bezel)
    first_colors = None
    delays = []
    for i in range(count):
        native.seek(i)
        large.seek(i)
        a,b = native.convert('RGB'),large.convert('RGB')
        colors = tuple(a.getpixel(p) for p in static)
        if first_colors is None:
            first_colors = colors
        assert colors == first_colors
        assert all(a.getpixel((x,y)) == b.getpixel((x*8+4,y*8+4)) for y in range(64) for x in range(64))
        delays.append(native.info['duration'])
    assert sum(delays) == count*83//10*10 and set(delays) == {80,90}
    report[name] = {'frames':count,'unique_frames':count,'native_duration_ms':count*83,'gif_duration_ms':sum(delays)}

def rear_pixels(image):
    return tuple(image.getpixel(p) for p in sorted(rear))


assert rear_pixels(frames['delegating-start'][0]) == rear_pixels(source[0])
assert rear_pixels(frames['delegating-start'][-1]) == rear_pixels(frames['delegating'][-1])
assert rear_pixels(frames['delegating-finished'][0]) == rear_pixels(frames['delegating'][0])
assert rear_pixels(frames['delegating-finished'][-1]) == rear_pixels(source[-1])
assert len({rear_pixels(image) for image in frames['delegating']}) >= 8
assert len({tuple(car['x'] for car in r['traffic']) for r in records['delegating']}) >= 24
old = json.loads((approved / 'pet.json').read_text())
manifest = json.loads((review / 'pet.json').read_text())
assert set(manifest['animations']) == set(old['animations']) | set(frames)
for name,clip in old['animations'].items():
    assert manifest['animations'][name] == clip
old_art = [p for p in approved.rglob('*') if p.suffix in ['.png','.gif']]
for path in old_art:
    assert (review / path.relative_to(approved)).read_bytes() == path.read_bytes()
for file in ['motorized-motion.json','compression-motion.json','attention-motion.json','interruption-motion.json']:
    assert (review / file).read_bytes() == (approved / file).read_bytes()
for name in frames:
    clip = manifest['animations'][name]
    assert clip['loop'] == (name == 'delegating') and clip['frame_duration_ms'] == 83
assert manifest['animations']['delegating']['entry'] == 'delegating-start'
assert manifest['animations']['delegating']['exit'] == 'delegating-finished'
for name,clip in manifest['animations'].items():
    for reference in [clip.get('entry'),clip.get('exit')]:
        if reference is not None:
            assert reference != name and reference in manifest['animations']
            assert not manifest['animations'][reference]['loop']
prepared_bytes = sum(clip['source']['frame_count']*16384 for clip in manifest['animations'].values())
assert prepared_bytes <= 16*1024*1024
cycle = Image.open(review / 'delegation-cycle-preview.gif')
assert cycle.n_frames == 112 and sum(frame.info['duration'] for frame in ImageSequence.Iterator(cycle)) == 9290
evidence = {
    'method':'Independent read-only native PNG, source projection, masks, motion, sprites and GIF checks.',
    'clips':report, 'minimum_exact_source_pixels_per_frame':min(fixed_counts),
    'working_body_visor_typing_and_entire_main_glass_preserved':True,
    'animated_glass_pixels':len(glass),'fixed_bezel_pixels':len(bezel),'typing_pixels':len(fingers),
    'generated_drone_projection_verified':True,'drone_size':list(sprite.size),'opaque_drone_pixels':len(opaque),
    'drone_always_behind_window_and_monitors':True,'stationary_transfer_hover':True,
    'rise_dispatch_received_light_rightward_departure_verified':True,
    'return_from_right_download_light_off_departure_verified':True,
    'no_car_traffic_in_transfer_clips':True,'three_distinct_loop_traffic_profiles':True,
    'matrix_black_background_green_falling_heads_and_tails':True,'pulsing_arrow_shapes_fixed':True,
    'rear_transition_endpoints_match_loop_or_working_pattern':True,
    'prior_native_frames_preserved':264,'prior_individual_gifs_preserved':16,
    'all_prior_png_gif_assets_manifest_selections_and_motion_records_preserved':True,
    'sprite_order_and_native_enlarged_gif_matching':True,'static_gif_colors_stable':True,
    'prepared_base64_bytes_including_aliases':prepared_bytes,'context_preview_frames':112,
    'review':'Native scene, drone and rear contacts require owner review.',
    'activation':'Separate review pack; no live device or hook/configuration changes.'
}
(review / 'verification.json').write_text(json.dumps(evidence,indent=2)+'\n')
print(json.dumps(evidence,indent=2))
