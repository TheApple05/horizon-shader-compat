#!/usr/bin/env python3
"""Package a clean checked revision; never include game resources or third-party packs."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
        parser.error("commit the exact source and evidence before packaging")
    binaries = [args.bin_dir.resolve() / (name + ".exe") for name in ["horizon-client", "horizon-ctl", "horizon-shader-compat"]]
    for binary in binaries:
        if not binary.is_file():
            parser.error(f"missing binary: {binary.name}")
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))
    manifest = {"schema": 1, "source_revision": revision, "upstream_base": "42d1e6d7d77ed300fcc4d644ce1444c25356085c",
                "toolchain": {"rustc": subprocess.check_output(["rustc", "-vV"], text=True).strip().splitlines(),
                              "cargo": subprocess.check_output(["cargo", "--version"], text=True).strip()},
                "experimental": True, "build_command": "cargo build --locked --profile fast -p horizon-client --bin horizon-client -p horizon-client-lib --bin horizon-ctl -p horizon-client-render --bin horizon-shader-compat",
                "binaries": [{"file": p.name, "bytes": p.stat().st_size, "sha256": sha256(p)} for p in binaries],
                "dependencies": [{"name": p["name"], "version": p["version"], "license": p.get("license")} for p in metadata["packages"]],
                "support": "Original GPU fixtures only have rendered assertions. Consult docs/shader-compat results for corpus compilation evidence. Physical HDR is unverified. Prepare your own game resources."}
    package = output / "horizon-shader-compat-windows-x64.zip"
    with zipfile.ZipFile(package, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for binary in binaries:
            archive.write(binary, binary.name)
        for name in ["LICENSE", "THIRD_PARTY_NOTICES.md"]:
            archive.write(ROOT / name, name)
        for path in (ROOT / "docs/shader-compat").rglob("*"):
            if path.is_file():
                archive.write(path, path.relative_to(ROOT).as_posix())
        for fixture in ["compat-baseline", "compat-hdr"]:
            for path in (ROOT / "crates/horizon-shaders/tests/packs" / fixture).rglob("*"):
                if path.is_file():
                    archive.write(path, "original-packs/" + fixture + "/" + path.relative_to(ROOT / "crates/horizon-shaders/tests/packs" / fixture).as_posix())
        for package_index, dependency in enumerate(metadata["packages"]):
            folder = Path(dependency["manifest_path"]).parent
            notices = set()
            for path in folder.iterdir():
                if path.name.upper().startswith(("LICENSE", "LICENCE", "NOTICE", "COPYING")):
                    notices.update([path] if path.is_file() else [p for p in path.rglob("*") if p.is_file()])
            if dependency.get("license_file"):
                notices.add(folder / dependency["license_file"])
            for path in sorted(notices):
                if path.is_file():
                    try:
                        relative = path.resolve().relative_to(folder.resolve()).as_posix()
                    except ValueError:
                        relative = path.name
                    archive.write(path, f"licenses/{package_index}-{dependency['name']}-{dependency['version']}/" + relative)
        archive.writestr("BUILD.json", json.dumps(manifest, indent=2) + "\n")
    source = output / "horizon-shader-compat-source.zip"
    subprocess.run(["git", "archive", "--format=zip", "--prefix=horizon-shader-compat/", "-o", str(source), revision], cwd=ROOT, check=True)
    (output / "BUILD.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    files = [package, source, output / "BUILD.json"]
    (output / "SHA256SUMS").write_text("".join(f"{sha256(p)}  {p.name}\n" for p in files), encoding="utf-8")
    for path in [package, source]:
        with zipfile.ZipFile(path) as archive:
            assert archive.testzip() is None, "damaged package"
            assert all(not n.startswith("/") and ".." not in n.split("/") for n in archive.namelist()), "unsafe archive path"
    print(json.dumps({"source_revision": revision, "files": [p.name for p in files]}))


if __name__ == "__main__":
    main()
