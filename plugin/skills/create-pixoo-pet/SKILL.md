---
name: create-pixoo-pet
description: Create or adapt 64x64 pixel art, sprite sheets, and animations into Divoom Pixoo pet packs for the local Codex bridge. Use for characters on black or full scenes with backgrounds, including animation revisions and pack validation.
---

Create portable artwork packs, independent of device credentials or hook installation. The Pixoo receives full 64x64 RGB frames: black pixels are dark; alpha must be composited deliberately. Support character-on-black and full-scene artwork according to the user's choice.

## Base before motion

Read the user's visual direction, reference images, desired clips, and output location. Use available built-in image generation for creative raster work. References supply atmosphere or identity; preserve the user's requested changes. Do not substitute desktop-pet atlas geometry for a Pixoo scene.

Create one square canonical base. Design for a real 64x64 grid with a clear silhouette, broad color regions, and readable props; enlarged fine-detail art alone does not establish suitability. Prepare the output with the bridge's deterministic `prepare` command and inspect both the native 64x64 PNG and its nearest-neighbor preview.

**Show the base and obtain the user's approval before generating the full animation set or sprite sheet.** This project uses a review gate for composition and identity. Explicit prior approval of that base satisfies the gate; silence does not. Revise only the requested aspects and show the revised base when approval is still pending.

## Animation and consistency

Read [references/animation.md](references/animation.md) when authoring motion. Ground every clip in the approved base; keep camera, background, character identity, palette, and prop placement consistent. Prefer short loops and small purposeful changes. Generate raster motion through the available image-generation tool; use deterministic code for resizing, extracting cells, compositing, validation, and previews.

Supported logical selections are `idle`, `working`, `needs-input`, `finished`, `interrupted`, `compacting`, and `delegating`. All are optional. The user can choose other clip names through bridge configuration. `working-enter` is an optional single-play transition linked from `working`, not an additional Codex hook.

Aim for approximately 12 FPS when requested (`frame_duration_ms: 83`). Limit each uploaded clip to 40 frames or the lower configured limit. This ceiling is a conservative interoperability choice, not a guarantee for every firmware. Native looping avoids continuous frame streaming, but changing clips can show a device loading delay. Do not promise seamless video or native one-shot playback.

## Export and validation

Read [references/pack-format.md](references/pack-format.md) for the exact shared manifest and CLI. Locate `pixoo-pet.exe` in the workspace, release bundle, or user-specified location. It handles authoring preparation as well as bridge operation; an existing pack needs no image-generation runtime.

Export exact 64x64 frames or 64-pixel cells. A full-scene background stays opaque; a transparent character source is composited onto the manifest background. Save source images and prompts under the requested pack directory. Put previews in a separate subdirectory so directory import does not treat them as animation sources.

Run `validate`, inspect a contact sheet and animated preview when clips exist, and repair any misalignment or unreadable motion. Report normalization of GIF timing. Missing clips are valid and must not cause invented fallback artwork. Keep device IP/token and local hook paths out of distributable art packs. Do not activate a device or change Codex settings merely because artwork was requested.
