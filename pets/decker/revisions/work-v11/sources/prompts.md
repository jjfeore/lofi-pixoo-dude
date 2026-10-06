# Missing visor travel poses

Generated with the built-in image generation tool, using the approved work-v9 four-pose study and work-v10's corrected complete-character atlas as references. The output is `visor-travel.png`, eight transparent whole-character cells.

```text
Use case: illustration-story. Asset type: pixel art animation key poses, true transparent whole-character atlas.
Input image 1 is the approved four-pose scene study: preserve this adult male cyberpunk decker's anatomy, broad dark jacket, thick purple upper-arm band, chunky visor, hairstyle, beard, seated side profile facing right. Input image 2 is the previous transparent whole-character atlas: preserve its character scale and pixel style, but it lacks the actual hand-assisted visor travel.

Generate a NEW tightly registered atlas of EXACTLY 8 whole-character sprites in a borderless 4-column by 2-row grid, read left to right, then next row. All cells are square and equal sized. Transparent outside the character, no room, desk, keyboard, monitors, labels, grid lines, or background. Same unmoving head, neck, shoulder position, seated body and camera in every cell. Match the silhouettes and framing of the transparent reference's forehead-contact poses. Each sprite includes the ENTIRE decker from crown to lower jacket and BOTH arms/hands. Do not crop to a separate arm. The far arm stays extended on the keyboard level at lower right. The visible foreground arm bends at the real shoulder and elbow; keep its upper sleeve and purple band FULL and thick, with no shrinking.

These 8 poses show ONLY the missing continuous operation: foreground hand grips the SIDE of the ONE chunky visor on the forehead, then pulls it smoothly down to cover the eyes. The fingers must visibly touch the visor's near hinge / left edge in EVERY cell. The hand and visor move together throughout, never teleports, never an unattached visor. The hand does not cover the front cyan glass so visor position can be seen.
Cell 1: visor fully parked on the FOREHEAD above the eyebrow, foreground hand already reached all the way up and firmly gripping its near left side. Fingers at forehead height, not down beside the cheek. Face and eyes still exposed.
Cell 2: same grip; visor just starts to rotate/downward, about ONE native 64x64 pixel lower.
Cell 3: same grip; visor one small step farther down, approaching the eyebrow.
Cell 4: same grip; visor partly over the brow, not yet covering the eyes.
Cell 5: same grip; visor halfway down, visibly between the forehead position and eye position.
Cell 6: same grip; visor starts to cover the upper eye.
Cell 7: same grip; visor nearly seated across the eyes.
Cell 8: same grip; visor fully seated horizontally OVER THE EYES, ready to release. Hand still touches its left edge.

Critical: show EIGHT visibly different, evenly spaced visor heights/angles rather than repeating a raised pose followed by repeating a lowered pose. Total forehead-to-eyes travel is roughly 7-8 pixels in the eventual 64x64 scene, so each cell advances about 1 pixel. Keep the same generous chunky lens/housing, not thin sunglasses. There is exactly ONE visor in each cell. Once it moves, reconstruct plain dark HAIR where it previously sat; NO second goggles, cyan spot, glow, lens, or parked visor remains on the forehead. Lens cyan highlights belong ONLY to the moving visor. Maintain two hands total: lifted foreground hand on visor, far hand at keyboard, no spare foreground hand left on keyboard.

Hard-edged synthwave pixel art, dark navy/indigo with purple sleeve band, warm outlined skin and chunky cyan visor. Favor faithful anatomy/contact over decorative changes. Identical composition and pixel scale across all 8 cells, no individual bounding-box recentering or scale changes.
```
