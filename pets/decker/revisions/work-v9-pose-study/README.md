# Whole-character pose study

Work-v8 improved upper-arm thickness but the owner still found a narrow upper sleeve, a thinning purple band, a hinge below the real shoulder and missing portions of the rear limb when revealed. This four-pose study tests drawing the complete decker in each pose instead of rotating separate limb textures. It is an anatomy and composition experiment, not a finished 32-frame transition or an active display pack.

The owner approved this study as fantastic and requested start/finish animations based on it. [Work-v10](../work-v10/README.md) develops the complete-character poses into 32-frame transitions with continuing ambient motion and targeted single-visor glow cleanup.

View the [native-export contact sheet](previews/pose-study-contact.png), [slow pose sequence](previews/pose-study.gif), or [enlarged visor-contact pose](previews/visor-contact.png). The sequence is resting, near hand lifting, hand gripping the forehead visor, then hand holding the lowered visor. Every exported pose is exactly 64x64 RGB. The source cells were larger; the bridge's sheet command mechanically normalized them. The preview deliberately holds each pose for 750 ms and has no in-between motion.

Built-in image generation used the approved base-v3 scene and accepted working poster as references. The [generated 2x2 source](sources/whole-character-poses.png) and [exact prompt](sources/prompts.md) are saved. The new poses give the model the torso, shoulder, jacket band, both arms and both hands together, including the rear limb newly visible under the lifted near arm. Native visual inspection suggests more coherent sleeve volume and shoulder attachment. Generated face, jacket and room details still drift; this is not proof of final temporal consistency or an automatic anatomy assessment.

For a final transition, preserve the existing room and ambient timelines, use complete character poses with a reconstructed backdrop, and align the head, shoulder seam and stationary keyboard limb before compositing. Include any newly revealed rear-arm and hand pixels; the old clean-plate rule that bans new skin in the removal region is unsuitable for this reconstruction. Inspect resting and early moving poses as well as visor contact. Build deliberate in-betweens at 32 frames and 83 ms, with approved canonical endpoints. Do not use whole-arm fitting or independently rescale sleeve components. Generation and cleanup may take longer; runtime still receives ordinary precomputed 64x64 RGB clips.

Reproduce the mechanical exports into a new directory:

```powershell
.\target\release\pixoo-pet.exe sheet .\pets\decker\revisions\work-v9-pose-study\sources\whole-character-poses.png --output .\local\whole-character-study-frames --columns 2 --frame-count 4
```

The saved manifest is intentionally named `pose-study`, not a lifecycle state. Bridge validation accepts its four frames (65,536 prepared bytes). Read-only image analysis verified all four dimensions, native-sheet ordering and four 750 ms GIF holds. Native contacts and the enlarged visor-contact pose were inspected. The owner approved the study as the basis for the next transitions. No runtime code, active pack, device content or commit was changed.
