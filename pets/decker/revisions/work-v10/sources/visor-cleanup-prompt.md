# Single active visor cleanup

The owner noticed cyan forehead glow remaining after the visor had been lowered. This built-in image edit removes the duplicate parked visor glow in lowered poses. Final compositing samples only the forehead region from this edit, preserving the original character poses elsewhere.

```text
Use case: precise-object-edit.
Edit target: the attached genuinely transparent 4-column by 8-row, 32-cell whole-character decker atlas.
Make ONE targeted correction. In every pose where the blue visor has moved down over the eyes/face (cells 13 through 31, counted from zero in row-major order), REMOVE the leftover cyan/teal glowing visor or goggles from the FOREHEAD ABOVE IT. There must be only ONE visor, in its current lower position. Restore dark hair and a small unlit dark mounting bracket where the parked forehead visor used to be. No cyan, teal or bright blue indicator on the forehead in those lowered poses. The lowered visor over the eyes should retain its existing cyan illumination.
In cells 0 through 12, the visor is raised or being gripped at the forehead; preserve those poses and that single raised visor as-is.
Preserve every pose's face, hair silhouette, anatomy, sleeve thickness, violet band, hand positions, body placement, and the entire 4x8 atlas geometry. Do not move or resize characters or redraw the gesture. Change only the duplicate bright forehead visor above an already lowered glowing visor. No new props or lights.
Keep the genuine transparent background. Output the same 4-column by 8-row sheet, square cells, aspect ratio 1:2, no gutters or labels.
```
