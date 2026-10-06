# Targeted pair of middle poses

Built-in image generation used original-resolution travel cells 4 and 5 as references. The returned two whole-character poses are in `middle-inbetweens.png`; actual native visor spacing is measured after projection rather than assumed from the prompt.

```text
Generate TWO transparent whole-character pixel art sprites in a borderless 2-column, 1-row grid of equally sized SQUARE cells. Same decker, camera, pixel scale, and framing as the two reference sprites. Preserve head, hair, torso, thick shoulder/sleeve and purple band, and far keyboard arm/hand exactly.

These are the TWO MISSING IN-BETWEEN POSES between reference 1 (visor at brow, tilted) and reference 2 (visor fully down over eyes, horizontal). The foreground hand GRIPS THE SAME LEFT / TEMPLE EDGE of the chunky cyan visor, and lowers it gradually. Hand and visor travel together.
LEFT sprite: visor and gripping hand about ONE native pixel lower than reference 1, lens partly overlaps eyebrow, eye still partly visible. Forearm follows hand naturally without shrinking. Lens stays a little tilted.
RIGHT sprite: visor and gripping hand another ONE native pixel lower, lens overlaps upper eye, very nearly horizontal. This is still ABOVE reference 2's final position.

Do not merely repeat either reference endpoint. The moving cyan lens CENTER should have intermediate heights around native y22.7 and y23.8, between the reference lens centers y21.6 and y24.9. Keep lens left/right placement, broad housing and hand grip consistent, with no teleport. No extra lens or glowing forehead hardware behind it. ONE visor and exactly TWO hands, foreground hand on visor, far hand stationary at keyboard. Full character from crown to lower jacket with consistent transparent margins, no background or captions.
```
