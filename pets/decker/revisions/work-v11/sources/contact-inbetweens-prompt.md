# Focused forehead contact and middle travel poses

Built-in image generation supplied four complete character poses in `contact-inbetweens.png`. These augment the travel atlas rather than editing arm parts. The source cells are rectangular; each uses the same complete-character projection during native export.

```text
Use case: illustration-story. Asset type: transparent 2x2 whole-character pixel-art key pose atlas.
Reference image is an eight-pose atlas of the approved adult male cyberpunk decker. Match its character EXACTLY: same fixed seated side profile facing right, head/torso anchor, dark full broad jacket, thick purple upper-arm band, hair/beard, warm skin, SINGLE chunky cyan visor, one stationary far arm/hand at keyboard level at lower right. Entire character sprites, crown through jacket bottom, not arm sprites. Identical camera, framing and size in all four square cells. Each whole figure fills the cell like the reference, with the same transparent margins. No room, keyboard, desk, text or grid borders.

Create EXACTLY FOUR NEW poses in a 2-column by 2-row borderless grid. The sequence is deliberately four KEY poses, not a full animation. These fill missing contact positions in the reference:
TOP LEFT: FOREHEAD CONTACT. The chunky visor is raised on the forehead, eyes fully visible. The lifted FOREGROUND hand reaches all the way up and GRIPS THE LEFT, EAR-SIDE EDGE of that parked visor, next to the temple. The fingers are ABOVE the eyebrow, on the forehead, curling over the left side of the raised housing. The cyan glass projects to the RIGHT of the gripping fingers. The wrist is in front of the EAR, not the nose. The hand must NOT hold the RIGHT FRONT TIP of the visor. This is the same left/ear-side grip used when fully lowered in the reference's final cell, simply lifted up to forehead height. It is important that the hand genuinely TOUCHES the raised visor before it can move.
TOP RIGHT: SAME hand and SAME left edge grip, visor part way down, just above eyebrow; about 3 native pixels below its raised position. Broad lens still tilted slightly upward. Plain dark hair above the moving visor.
BOTTOM LEFT: SAME grip, visor one small step LOWER, at the eyebrow, approximately 4 native pixels below raised. Glowing glass still projects RIGHT from the fingers, eye not fully covered yet.
BOTTOM RIGHT: SAME grip, visor another small step LOWER, partially overlapping upper eye, approximately 5 native pixels below raised. This MUST be higher than the reference's fully-down eye-covering visor; a true intermediate between brow and fully-down, not another fully lowered pose.

These are same object/same hand/same grip at progressively lower heights. Never swap which edge is held; the hand accompanies the lens continuously. The parked lens occupies about native y17-20, intermediates y20-23, y21-24, y22-25; fully-down native y24-27 is NOT one of these four poses. Do not leave any extra cyan forehead goggles or glow behind. Cyan belongs ONLY to current moving lens.

Draw coherent whole-character anatomy for each pose, retaining broad upper arm and forearm with full purple band and real shoulder attachment. Keep the other far arm/hand stationary; exactly two hands total, no lifted hand copy remaining on keyboard. Faithful crisp pixel art, navy/indigo/purple/cyan synthwave palette. Transparent background throughout.
```
