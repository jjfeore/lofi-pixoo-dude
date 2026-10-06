# Pack format, schema 1

A pack directory contains `pet.json` and relative artwork files. Frames are exactly 64x64. Paths cannot escape the directory, including symlinks. All logical states are optional; malformed supplied clips, entry/exit references, or defaults are rejected.

```json
{
  "schema_version": 1,
  "id": "my-pet",
  "canvas_size": 64,
  "background": "#000000",
  "default_animation": "idle",
  "animations": {
    "idle": {
      "source": { "type": "image", "path": "idle.png" }
    },
    "working-enter": {
      "source": { "type": "sprite_sheet", "path": "enter.png", "columns": 8, "frame_count": 8 },
      "frame_duration_ms": 83,
      "loop": false
    },
    "working": {
      "source": { "type": "frames", "paths": ["working/000.png", "working/001.png"] },
      "frame_duration_ms": 83,
      "loop": true,
      "entry": "working-enter"
    }
  }
}
```

Source types:

- `image`: one 64x64 image, `path` required.
- `sprite_sheet`: row-major PNG grid, `path`, `columns`, and `frame_count` required. Dimensions equal `columns * 64` by `ceil(frame_count / columns) * 64`. Unused final-row cells are permitted.
- `frames`: explicit ordered `paths`, each exact 64x64.
- `gif`: `path`, full canvas exact 64x64. Constant timing is retained if no override is set. Variable timing requires an explicit `frame_duration_ms` to normalize it; the Pixoo transport has a single speed per clip.

`loop` defaults to true; frame duration defaults to 83 ms except retained GIF timing. Durations are 10-10000 ms. A static clip is valid. `entry` and optional `exit` name existing non-looping clips and cannot refer to the clip itself. The runtime uses working entry when aggregate work starts, delegation entry when the first observed child starts, and delegation exit when the final observed child stops. For delegation:

```json
"delegating": {
  "source": { "type": "sprite_sheet", "path": "delegating-sprite.png", "columns": 4, "frame_count": 32 },
  "frame_duration_ms": 83,
  "loop": true,
  "entry": "delegating-start",
  "exit": "delegating-finished"
}
```

Declare each referenced transition as its own complete non-looping clip. Both references are optional. Needs-input and compaction can preempt transfers; an early final-child stop or a new child can replace an unfinished transition. Parent interruption/completion does not synthesize a delegation return. A stopped child may subsequently continue, so these transitions represent observed hooks rather than guaranteed task completion. The new `exit` field requires the updated bridge executable; existing packs without it remain supported. `variants` maps `idle` and/or `working` to an existing clip, for example on `compacting`:

```json
"variants": { "idle": "compacting-idle", "working": "compacting-working" }
```

The variant clip has its own complete source. Resolution uses the aggregate base pose; nested variant traversal is not supported. Non-looping animation is emulated by a host timer and a final-frame hold or a return to base, not by a device-native one-shot instruction. Each clip has at most 40 frames. The pack has at most 64 clips and 16 MiB of prepared base64 data. Unknown manifest fields are rejected.

## Deterministic CLI

PowerShell examples (adjust the executable and output paths):

```powershell
.\pixoo-pet.exe prepare .\source.png --output .\pet\idle.png --preview .\pet\previews\base.png
.\pixoo-pet.exe import .\pet --id my-pet --frame-ms 83
.\pixoo-pet.exe validate .\pet
.\pixoo-pet.exe preview .\pet working --output .\pet\previews\working.gif --contact .\pet\previews\working-contact.png
```

`prepare` accepts a square raster and saves a nearest-neighbor 64x64 RGB PNG; `--background '#000000'` composites alpha. Its optional preview is 512x512. It refuses accidental overwrites. This conversion is a mechanical export; it does not fix a badly composed scene.

`import` recognizes state-named PNGs/GIFs and folders of lexically sorted PNG frames. Zero-pad numbered frames. PNG grids are treated as full grids; write the manifest directly if the last row contains unused cells. The importer marks `finished`, `interrupted`, `working-enter`, `delegating-start`, and `delegating-finished` non-looping. It links an available working entry and available delegation entry/exit clips. Other filenames remain usable custom clip names. It validates before writing and requires `--force` to replace an existing manifest. Re-run `validate` after manual edits.


## Generated grids and stable backgrounds

`sheet` exports generated cells into a numbered frame folder. The source aspect ratio must agree with the declared grid; it does not detect gutters or fix a malformed atlas. Check framing before extraction. For a 4x4 source:

```powershell
.\pixoo-pet.exe sheet .\generated.png --output .\pet\working --columns 4 --frame-count 16
```

Optional `--base` and repeated `--mask x,y,width,height` preserve a canonical 64x64 background while copying only explicitly animated regions. `--first-base` and `--last-base` can anchor a transition's endpoints to prepared poses. Determine masks from the actual scene and inspect their boundaries; don't apply one pet's coordinates to another. This is deterministic extraction/compositing of generated artwork, not creative replacement of an image-generation tool.

`preview` can also save a native-cell PNG using `--sheet output.png`, alongside its enlarged GIF and contact sheet. For a logical clip with variants, `--pose idle` selects its idle variant; working is the default.

Directory import reserves `sources/` and `previews/` for authoring assets and ignores them. It imports other PNG frame folders. An explicit manifest can reference any valid relative file.
