# Reference audit notes

Revisions and license-file blob IDs are pinned in source-audit.json. These are
documentation claims and reuse decisions, not independent compatibility results.
Only user documentation, metadata and license texts were inspected externally.
Implementation inputs remain Horizon's permitted source and original controls.

- **Iris:** documents OptiFine-format pack compatibility and links separate
  unsupported-pack and limitation lists. LGPL implementation source is excluded.
  Any reference observations must follow the recorded provenance process;
  this implementation session did not run an Iris comparison.
- **Vitrail:** documents load-time GLSL translation for Minecraft's native Vulkan
  renderer and links a pack comparison process. This is a useful validation lead;
  its LGPL implementation is excluded. Its game/backend results do not establish
  Horizon compatibility.
- **VulkanMod / Beryl:** VulkanMod documents a replacement renderer and directs
  shader users to Beryl. Beryl describes an integrated pack, with ARR licensing.
  Neither implementation is a code donor under this project's selected policy.
  The identification of the original name "ballaor" as Beryl remains tentative.
- **Photon-Horizon:** documents a standalone Windows distribution and renderer
  patch against `b33bc70c4fd2e18a5519a13189590cb7b106c556`. Its published RTX 3090
  Vulkan results belong to that distribution. Reproduce through its documented
  build/launcher and a fresh game directory; do not apply the historical patch
  wholesale to this baseline. Engine MIT terms and Photon's custom pack license
  remain separate. The unchanged local pack is a new-baseline test input only.
- **horizon-shader-mods:** documents five native-mod experiments pinned to
  `ba6344a`, with Vulkan-only testing and several partially tested effects.
  Its block-ID and options work must be compared with the newer upstream
  capabilities already present. No declared root license was found; code reuse
  is excluded. Its installation route requires the matching historical engine.
- **Horizon-Translator:** documents browser, desktop and command-line shader
  conversion, with remaining issues after conversion. No declared root license
  was found, so its implementation is excluded. Author-provided releases are a
  reproduction lead for a separately eligible observation process.
- **Meridian:** documents native WGSL API 0.1 packs, including a shadow-map pack
  and an older-engine compatibility variant. Its documented build tool creates
  both packages. MIT is declared, but relevant file headers still require review
  before adaptation. Recheck both packages with the current native checker and
  runtime before using their results as examples.
- **Aurora / HorizonShaders:** documents full and Lite native API 0.1 packs,
  checked against `0a95d1c047`. Its no-shadow claim describes that pack and old
  API snapshot, not current Horizon's renderer. Install the documented package
  and rerun the current checker/runtime; MIT file-level checks precede reuse.

No external implementation or shader-pack source was copied into this fork.
