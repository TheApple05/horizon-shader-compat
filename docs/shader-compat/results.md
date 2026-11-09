# Experimental compatibility results

These are scoped observations on Windows with an RTX 3090 and NVIDIA 616.92.
The pinned upstream baseline is `42d1e6d7d77ed300fcc4d644ce1444c25356085c`.
Backend requests were checked against the actual adapter; an automatic switch to
another backend does not count as a pass. Original controls have rendered pixel
assertions. Real-pack compilation and live loading do not establish visual fidelity.

## Original controls

Eight forced-backend controls passed: four on Vulkan and four on OpenGL. Each backend exercised the
terrain → composite → final chain, sampled depth and common uniforms, the GAIN
option (1 → 2 → 1), all three dimensions, malformed reload and restoration.
Expected output was independently specified as `[32,128,128,255]` or
`[64,128,128,255]`, with a one-byte quantization allowance. A depth-only terrain
fragment also passed an independent depth readback assertion. Forced missing compute and fragment-storage capabilities
refused the active required effect, rather than silently reporting support.

The original HDR control retained `[4,1,0.25,0.375]` in its float scene target on
both backends. With the explicit assisted contract, Vulkan scRGB readback was
approximately `[3.9941,1,0.2496,1]`, with equivalent OpenGL readback within the
analytic test's tolerance; the pack-owned scene alpha remained `0.375`.
The SDR → HDR → SDR check recovered the same SDR values, approximately
`[0.7993,0.4995,0.1997,1]`, and checked HUD white independently. These are float
attachment/offscreen presentation checks, not physical HDR display evidence.
Only the original, fingerprinted `original-linear-v1` adapter is verified.

The exact test executable, source revision and per-case timings are recorded in
[validation-results.json](validation-results.json). The renderer suite passed
1,830 tests with 67 ignored. Eight decoded screenshots (vanilla and native Golden
Hour: water/day, glass, Nether and night lighting) were pixel-identical to the
pinned baseline on this machine's default adapter. That comparison did not force
both backends and does not validate GLSL pack appearance.

## Unchanged real-pack corpus

[corpus-results.json](corpus-results.json) records archive hashes, the exact
compiler executable/revision, settings identity, actual adapters and timings.
Every entry used unchanged pack defaults in the overworld, with no selected
profile or option overrides.

- Photon original `40c8871`: production translation and pipeline checks passed
  on Vulkan and OpenGL. Separate live-game runs loaded the unchanged archive,
  kept advancing frames and exited cleanly on both backends. Visual fidelity
  remains **untested**; there is no approved reference-image comparison.
- BSL 10.1.8, Complementary Reimagined r5.9.3 and MakeUp Ultra Fast 9.6:
  production translation and pipeline checks passed on both backends.
  GPU scene execution and visual fidelity remain **untested**.
- Rethinking Voxels r0.1-beta9: **incomplete** on both backends. The selected
  required final pass fails translation in its OptiFine/ACL error branch
  (`lib/textRendering/error_optifine_acl.glsl:3`, unresolved `beginTextM`). Backend
  compilation and scene execution were not run after that failure. Loader
  identity macros were not spoofed to suppress the failure. This observation
  does not justify replacing Naga or wgpu.

No third-party pack or generated third-party stage is included in the repository
or release. Download/observation rights are separate from implementation reuse;
the source audit records the decisions. These compilation checks found no shared
failure affecting multiple packs in the tested configuration. Wider effects and settings
need their own reduced regressions and evidence.

## Live-game recovery and measurements

The isolated original-pack session exercised dimension switching, day/night,
rain/clear, water, glass/transparency, entities, particles, held items and camera
movement. An edited GAIN option reloaded, malformed includes recovered to vanilla
without a retry loop, restoration loaded again, and switching to the original
HDR pack changed to a float frame and back to SDR. These interactions are smoke
coverage, not independent correctness assertions for every scene effect.

Separate Vulkan and OpenGL sessions removed only the copied original fixture,
issued F3+T, confirmed vanilla frames continued, restored the copy and selected it
again successfully. Reproduce with the control socket's `key f3 down`,
`key t tap`, `key f3 up`, then the Shader Packs UI. A retained selected row may
need clearing before selecting the restored pack again.

Live Photon initially exposed a Windows OpenGL context timeout while background
pipeline compilation competed with frame submission. OpenGL now completes the
outstanding frame and compiles pack pipelines serially on the render thread.
Both backend retests passed. Initial OpenGL compilation can pause rendering;
one observed Photon load took about 34.7 seconds. Vulkan keeps background builds.

Two short benchmark samples and process/GPU memory observations are preserved in
the JSON evidence. They were correctness runs at 640×360, render distance 2 and
a 60 FPS cap, with only 20 warmup ticks and concurrent compilation possible.
They are not a backend performance comparison. Same-machine repeatability and
visual tolerances for third-party packs have not been established.

## Reproduction and remaining checks

Follow [build/run instructions](README.md), then run:

```sh
python tools/shader_compat_gpu.py --help
python tools/shader_compat_game.py --help
python tools/shader_compat_corpus.py --help
python tools/pr_ready.py --help
```

Cache regressions cover edited includes/options/macros/context/compiler identity,
corrupt and valid-but-tampered entries. Export checks cover original immutability,
arbitrary include extensions, binary resources, source maps, notices, profile and
explicit-option precedence, overlapping paths and refused unproven native export.
The cache persists preprocessing and source analysis; native GPU objects are
recreated through the production renderer.

Physical HDR output, other GPUs/drivers, complete real-pack scene/effect fidelity,
all real-pack dimensions/options, a live window resize, and a long gameplay soak
remain unverified. The full 624-case vanilla parity harness was skipped because
its oracle/prepared harness configuration was not set up in this checkout; the
eight-picture baseline comparison above was run separately. Native Shader API
0.2 still has documented gates/incomplete behavior. No broad compatibility claim
is made by an experimental package or a successful CLI compilation.
