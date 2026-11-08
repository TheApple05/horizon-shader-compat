# Horizon Shader Compat

Independent shader-focused development, based on official Horizon
`42d1e6d7d77ed300fcc4d644ce1444c25356085c`. The official history is preserved.
`origin` is TheApple05/horizon-shader-compat; `upstream` is horizon-rust/horizon.
The existing TheApple05/horizon project is separate.

## Source policy and architecture

Follow Horizon's MIT-compatible contribution policy. The initial source audit is
in [source-audit.json](source-audit.json). No foreign loader implementation was
read or reused. A permissive root license alone does not approve every file;
file-level checks precede any future adaptation. Private shader packs, converted
sources, prepared resources, accounts and worlds never belong in this repository.

`horizon-shaders` owns pack reading, preprocessing, source maps, Naga translation
and structured outcomes without wgpu. The renderer owns adapter capabilities,
pipeline validation and execution. Native WGSL packs retain their existing API.
The native `horizon-shader` command stays separate from `horizon-shader-compat`.

The player's loading path refuses an active required effect that cannot run and
uses existing vanilla recovery. Documented program fallbacks and optional material
variants retain their defined behavior. Outcomes identify selection, preprocessing,
translation, capabilities and backend compilation directly at their owning code.

## Build and use

Use Horizon's existing Rust and resource-preparation instructions. Build the
client and compatibility tool with:

```sh
cargo build --locked --profile fast -p horizon-client --bin horizon-client -p horizon-client-render --bin horizon-shader-compat
horizon-shader-compat inspect <pack> --json
horizon-shader-compat validate <pack> --backend vulkan --json
horizon-shader-compat validate <pack> --backend gl --json
horizon-shader-compat convert <pack> --backend vulkan --out <new-directory> --json
horizon-shader-compat report <new-directory>/compatibility-report.json
```

Selection in the game already uses conversion automatically. Optional CLI
arguments are `--dimension minecraft:the_nether`, `--profile <name>`, `--option GAIN=2` and
`--cache <directory>`. Mipmap macros use level 4 in standalone validation.
Explicit options override values selected by a profile.
`--hdr` validates the explicit [linear scene handoff](hdr-contract.md); packs
without a declaration are refused for that request. Game and CLI use the same
adapter-derived compiler feature macros.
Conversion bakes selected options into a separate compatibility pack and retains
generated preprocessing/source-map records. Original inputs are never modified.
Root license/notice files are copied; SOURCE.json identifies the unchanged original.
Native `.hshader` export is refused until equivalence is proven for the pack.

Exit 0 means the requested checks passed, 1 means incompatibility, and 2 means
invalid input or an unavailable environment. Backend selection is exact. Pipeline
checks use generated resources and production shader inputs; their success does
not establish scene execution or visual fidelity. Reports preserve this distinction.

The private preprocessing cache validates live include contents and option values
on each hit. Its namespace includes macros, adapter/device context and a hash of
the compiler sources and lockfile. Corrupt entries rebuild. GPU pipelines still
use the normal renderer/driver compilation path; no persistent GPU handles are saved.

## Validation and maintenance

Start with the original `compat-baseline` pack and independent expected pixels.
Run the explicit host-GPU tests serially, especially OpenGL on Windows:

```sh
cargo test --locked -p horizon-shaders -p horizon-shader-api
WGPU_BACKEND=vulkan cargo test --locked -p horizon-client-render --lib complete_chain_options_and_failure_recovery -- --ignored --nocapture --test-threads=1
WGPU_BACKEND=gl cargo test --locked -p horizon-client-render --lib complete_chain_options_and_failure_recovery -- --ignored --nocapture --test-threads=1
```

On PowerShell, set `$env:WGPU_BACKEND='vulkan'` or `'gl'` before the command.
The capability-refusal control additionally requires
`HORIZON_SHADER_PACK_WITHOUT=compute` or `fragment-storage` and the test filter
`required_compute_and_fragment_storage_refuse_when_forced_unavailable`.

Record exact pack hashes/options, commit, adapter/backend/driver and evidence.
Keep passed, failed, skipped and unavailable checks separate. Real-pack source
stays private; publish sanitized observations, never shader sources or transformed
artifacts. Reference screenshots require an observation process compatible with
the documented provenance rules.

Keep changes small and testable, merge fork PRs after their checks pass, and merge
upstream changes through the same baseline controls. Inherited publishing,
scheduled and self-hosted workflows are disabled on this dedicated repository.
The original HDR fixture has verified float readback and offscreen scRGB output.
Full-pack visual compatibility and physical HDR display output require separate
evidence; see the [HDR contract](hdr-contract.md).
