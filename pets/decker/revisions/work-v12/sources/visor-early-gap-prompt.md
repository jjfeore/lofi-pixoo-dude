# Two-pose subdivision before the lower-forehead pose

Tool: built-in imagegen; transparent background. References: cd-cells/000.png and visor-lower-forehead.png.

Create ONLY TWO closely spaced intermediate whole-character pixel-art poses between the two supplied complete-character reference images. First is visor held high on forehead; second is the visor tilted partway over the lower forehead, NOT yet across the eye. We are NOT completing the lowering to eye level. Output true transparent RGBA atlas, exactly two equal square cells side by side (2 columns x 1 row, 1024x512 preferred). No labels.

Both new poses must lie strictly BETWEEN the supplied endpoints. LEFT cell lens one-third down that small gap; RIGHT cell lens two-thirds down it. The cyan plate rotates slightly down in front of the forehead, while the eye remains visible just below it in BOTH cells. At normalized 512px-cell scale, the lens center should be around y124 for the left cell and y135 for the right cell. Original high cyan plate around y108; supplied tilted endpoint around y145. Neither cell repeats the high or low endpoint. Neither has an eye-covering horizontal cyan lens.

Same fixed head/ear/hair/face/torso/shoulder locations, same complete chunky foreground arm and broad purple stripe, same cuff immediately below the gripping palm, stationary far hand resting at lower right. Near fingers maintain their grip at the near housing temple hinge, forearm stays IN FRONT of cheek. Never behind the ear/head, never a wrist descending from above palm. Character viewport/scale must match reference1 exactly; do not zoom or independently fit cells.

One cyan visor only, with its previous location restored to dark hair/mount as it moves. No cyan parked-glow remnant on forehead and no duplicate eye lens. Coherent thick sleeve and wrist in each complete pose, no floating hand, thin arm or extra third hand. Hard pixel edges identical to references. True transparent outside silhouette without halo, gradients, room, ground, grid lines or dividers.
