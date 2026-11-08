# Experimental GLSL scene-color handoff

Default GLSL packs retain their SDR final stage. Pack-owned alpha is not interpreted
as Horizon's legacy HDR light markers. There is no reconstruction from a generic
tone-mapped SDR image.

An explicit contract lives at `shaders/horizon-hdr.properties`:

```properties
contract=1
mode=authored
scene=colortex0
encoding=linear-bt709
white=paper-white
exposure=pack
tonemap=horizon
```

Version 1 deliberately supports one handoff: colortex0 contains linear BT.709 RGB,
where 1.0 is the user's calibrated paper white. The pack applies exposure once;
Horizon applies its existing display tone curve once. The scene attachment must
have at least RGB, 16-bit floating components; the selected adapter must supply
float scene and frame targets. Alpha and other data attachments remain pack-owned.
The final stage is replaced only for this declared HDR handoff; it gamma-encodes
scene RGB into Horizon's existing float frame. Horizon's existing tone curve,
black/white/peak calibration and HUD coverage presentation then run normally.
The original final stage runs for SDR. This contract requires an active final.

`mode=authored` is an author's declaration, not a visual compatibility certificate.
`mode=assisted` additionally requires `adapter=original-linear-v1` and the exact
normalized source fingerprint of the original `compat-hdr` fixture. Edited sources
are refused. The fingerprint normalizes UTF-8 BOM and CRLF; archive hashes in the
evidence remain exact. Additional adapters require their own reduced tests and
source review. No third-party assisted adapter is enabled.

Game execution retains the upstream `HORIZON_HDR_FRAME=float` gate and requires HDR
output actually offered by the renderer. The CLI's `validate --hdr` checks this
declared float pipeline without claiming execution or physical output. API 0.2
native WGSL packs retain their own runtime gates; this does not implement their
uncompleted HDR-output or linear-blending contracts. See [upstream updates](upstream-updates.md).

The original GPU control independently checks scene `[4, 1, 0.25, 0.375]`, gamma
handoff, scRGB `[4, 1, 0.25]` within 0.012, SDR `[0.8, 0.5, 0.2]`, switching
SDR/HDR/SDR, and HUD white. At paper white 80 nits and peak 800 nits, this scene is
below Horizon's tone-curve knee, so exact expected values detect double exposure,
double tone mapping and clipping. Readback allows 0.008 for half-float gamma
rounding. These tolerances are for analytic constants, not screenshot similarity.
Both explicit backends passed on RTX 3090 / driver 616.92. Physical display HDR,
other adapters and broad authored-pack appearance still need separate evidence.
