# Whole-character transition sources

Generated with the built-in image generation tool. The owner approved the whole-character four-pose study before this full transition pass. The character atlas supplies all anatomy together; the background plate reconstructs room pixels hidden by the original figure. Offline Rust normalizes and composites these sources.

## Character atlas

```text
Use case: compositing / identity-preserve.
Asset type: a transparent 32-frame whole-character pixel-art sprite atlas for a 64x64 LED display.
Input 1: approved FOUR-POSE STUDY, primary character identity, jacket proportions, shoulder movement and fully revealed rear-arm reference.
Input 2: canonical idle scene, exact camera, character scale and placement reference.
Input 3: accepted steady working scene, chunky lowered visor and final resting working pose reference.
Draw the ENTIRE decker together in every frame, from hair through torso and both arms/hands. This is a coherent person animation, NOT separate limb pieces. Preserve the adult male face, dark hair and stubble, navy bulky jacket, broad violet sleeve band, consistent near-hand size, and synthwave pixel palette from the approved study.
Output a single 4-COLUMN by 8-ROW atlas of 32 equal square cells, no gutters, no grid lines, no labels. Overall canvas aspect ratio is 1:2. Every cell represents the same 64x64 viewport: hair/top of head near y=14, face centered around x=23 y=27, true shoulder seam near x=11 y=38, jacket torso reaching the left and bottom edges, keyboard-level hands around x=40 y=57-60. Same head, body size and camera in every cell. Keep the character registered in this viewport; do not crop each pose to its own bounding box.
Background must be genuinely transparent in all cells. Draw NO room, window, chair, desk, keyboard or monitors: these will be composited separately. They are position references only. Include all character anatomy normally visible in the scene, and RECONSTRUCT the entire far arm and its keyboard hand behind the moving near arm. Finish the concealed portions that become visible. No floating hand, no hole where a sleeve/forearm should connect.
Chronological action, READ LEFT TO RIGHT THEN TOP TO BOTTOM:
Row 1, frames 0-3: visor raised at forehead; both hands at keyboard, then the near/camera-facing hand begins lifting gently. Same broad upper sleeve as resting pose. Far hand stays at its keyboard position.
Row 2, frames 4-7: near elbow bends naturally at the true shoulder; near hand moves up from keyboard through chest level. Four DISTINCT gradual intermediate poses.
Row 3, frames 8-11: near hand rises from chest to the right side of the forehead visor. Thick upper sleeve and natural connected shoulder, forearm bending toward the head. Far forearm/wrist and one hand fully visible at keyboard.
Row 4, frames 12-15: near hand GRIPS the chunky visor at the forehead and starts tilting it down. Four small sequential changes. Visor still mostly above eyes.
Row 5, frames 16-19: with the near hand holding it, visor lowers across the eyes, then its cyan indicator illuminates. Keep the hand close to the face, constant hand size, chunky visor thickness.
Row 6, frames 20-23: visor stays down and illuminated. Near hand releases it and retreats from face toward chest in four gradual whole-body poses.
Row 7, frames 24-27: near hand descends toward keyboard, elbow opens naturally; purple band remains broad and follows substantial jacket cloth volume.
Row 8, frames 28-31: near hand settles back at keyboard alongside the far hand, visor remains down/glowing. End with the accepted ordinary steady-working posture from input 3, with both hands at keyboard.
Critical: exactly two hands, two connected arms in every frame. When near hand lifts, do not leave its duplicate on the keyboard. Far hand remains at keyboard throughout. Keep the upper arm as thick as the approved study, its violet band about THREE logical pixels thick; no spindly sleeve and no pinched purple line. Real shoulder rotates at the shoulder seam high on the torso, not a low hinge near the band. Shoulder, torso, cloth folds, both arms and hands must be drawn coherently together. No whole-arm shrinking or changes in camera distance.
Style: crisp coarse 64x64 pixel art, broad readable shapes and limited colors, no smooth illustration detail, no antialiasing, no painted checkerboard. Slight clothing fold changes are natural, but do not change costume or head placement.
Output only this genuinely transparent 4x8 atlas.
```

## Background plate

```text
Use case: precise-object-edit.
Asset type: fixed 64x64 pixel-art scene background plate for character animation.
Input 1 is the approved canonical decker room. Input 2 is the same approved four-pose character study for context only.
Remove ONLY the entire human character: hair, face, visor, neck, jacket, both arms and hands. Reconstruct the room, chair and desk that were behind the character, in the same synthwave/navy pixel palette and exactly the same camera/framing. The empty chair can remain where the character was sitting; show a connected desk and keyboard where the hands were, and the wall/window behind the head. There must be no person, body, sleeve, flesh, purple arm band or visor remaining.
Preserve the main monitor at the right edge, small rear monitor position, city window frame, distant skyline, existing wall lighting, desk and keyboard placement. Do not add props or move the room around. Preserve the same coarse 64x64 pixel grid and broad pixel shapes, no smooth shading.
Output one opaque square scene, pixel art conceived for a native 64x64 display. This is a static reconstructed background; no animation and no character.
```

