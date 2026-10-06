# Focused four-pose study inside the chest-to-keyboard return

Tool: built-in imagegen; transparent background. References: de-cells/002.png and 003.png.

Generate FOUR small intermediate COMPLETE-CHARACTER poses between these supplied two exact endpoint images. This is only the short movement of the foreground hand from chest height in reference1 toward the keyboard approach in reference2. Visor is already on the eyes and stays fixed. Do not begin with forehead/visor poses. Do not repeat the supplied endpoints.

Transparent RGBA 2x2 square-cell atlas, total 1024 square or equivalent equal cells, read top-left top-right bottom-left bottom-right. All head, ear, hair, visor, torso, near shoulder, far arm and far keyboard hand positions and scale identical to references. Same fixed camera/margins; no zoom or individual fitting. Draw each whole character using coherent full-width sleeves and stripe, not a disconnected limb.

The NEAR foreground hand is the hand at chest in reference1. It moves gradually downward AND slightly to the RIGHT on a smooth arc to reference2:
Cell1 near hand just below reference1, palm below collar line.
Cell2 near hand at mid-torso height, forearm tilted upward less, wrist directly above cuff.
Cell3 near hand at lower torso height, just above the stationary far hand's height, near sleeve in front of the far sleeve.
Cell4 near hand nearly at reference2, fingers extending toward keyboard, still above and IN FRONT of stationary far hand.
Four evenly spaced poses over the actual approximately 130-source-pixel descent in the 512-square references. No single large drop. Near wrist always connected to visible near cuff. Shoulder remains actual pivot, broad bicep sleeve and full-width purple band keep original volume. The forearm may overlap the far sleeve but NEVER passes behind it. At the final keyboard approach the near hand comes toward the LOWER/FRONT rest position, as shown in supplied second image. TWO hands only; stationary far hand never moves or duplicates. Reconstruct revealed arm behind the moving sleeve.

Only one bright cyan eye visor, forehead dark with no extra cyan. Adult male beard/profile unchanged, no body shrinking or spindly anatomy. Match reference's chunky hard pixel clusters. No antialiasing, atmospheric glow, halo, ground, desk, room, panels, dividers or labels. True transparent empty background outside the character.
