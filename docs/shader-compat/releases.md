# Experimental Windows packages

Only the dedicated compatibility CI is enabled for normal pushes. Inherited
scheduled, publishing and self-hosted workflows remain disabled. The fork's
`shader-compat-release.yml` is a separate manual workflow restricted to this
repository's main branch. It builds and tests the selected immutable commit,
packages three Rust executables, notices and original fixtures, and publishes
an experimental prerelease with matching source ZIP, BUILD.json and SHA256SUMS.
Hosted CI supplies CPU/build evidence. Host GPU and physical HDR checks must be
read from their exact revision/configuration records, rather than inferred from
the release workflow's success.

To package a checked local build from a clean checkout:

```sh
python tools/check_public_tree.py --history
cargo build --locked --profile fast -p horizon-client --bin horizon-client -p horizon-client-lib --bin horizon-ctl -p horizon-client-render --bin horizon-shader-compat
python tools/package_shader_compat.py --bin-dir target/fast --out .local/compat-release
```

The package includes a lockfile-derived dependency/license inventory and available
dependency notice files. Generated Minecraft resources, account data, worlds and
third-party shader packs are never bundled. Use your own prepared 26.3 resources:

```sh
horizon-client --resources <prepared-26.3-directory> --game-dir <separate-game-directory>
horizon-shader-compat validate original-packs/compat-baseline --backend vulkan --json
horizon-shader-compat validate original-packs/compat-hdr --backend gl --hdr --json
```

Copy a pack into that game directory's shaderpacks folder and select it in the
game's Shader Packs screen. Read BUILD.json and the compatibility reports before
using a real pack; pipeline compilation by itself is not a visual guarantee.
