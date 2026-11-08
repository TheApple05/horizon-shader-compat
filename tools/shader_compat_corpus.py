#!/usr/bin/env python3
"""Run unchanged ZIP inputs privately; emit a source-free compatibility matrix."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import lab


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            result.update(chunk)
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--revision", required=True, help="source commit used to build the supplied binary")
    parser.add_argument("--out", required=True, type=Path, help="new PRIVATE evidence directory")
    parser.add_argument("packs", nargs="+", type=Path)
    args = parser.parse_args()
    if not re.fullmatch("[0-9a-f]{40}", args.revision):
        parser.error("revision must be the full source commit")
    binary = args.binary.resolve(strict=True)
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    result = {"schema": 1, "source_revision": args.revision, "binary_sha256": digest(binary), "results": []}
    for index, pack in enumerate(args.packs):
        pack = pack.resolve(strict=True)
        if not pack.is_file() or pack.suffix.lower() != ".zip":
            parser.error("corpus inputs must be unchanged ZIPs")
        source = digest(pack)
        for backend in ["vulkan", "gl"]:
            case = f"{index}-{backend}"
            home = output / case
            home.mkdir()
            env = {k: v for k, v in os.environ.items() if not k.startswith("HORIZON_")}
            env.update(HORIZON_TELEMETRY_URL="off")
            for key in ["HORIZON_HOME", "HORIZON_LAUNCHER_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"]:
                env[key] = str(home / key)
            env = lab.game_env(env)
            row = {"pack": pack.name, "archive_sha256": source, "requested_backend": backend,
                   "dimension": "minecraft:overworld", "options": "unchanged pack defaults", "profile": None}
            started = time.monotonic()
            with (home / "report.json").open("w", encoding="utf-8") as stdout, (home / "stderr.log").open("w", encoding="utf-8") as stderr:
                try:
                    ran = subprocess.run([str(binary), "validate", str(pack), "--backend", backend, "--cache", str(home / "cache"), "--json"],
                                         cwd=home, env=env, stdout=stdout, stderr=stderr, timeout=180)
                    data = json.loads((home / "report.json").read_text(encoding="utf-8"))
                    row.update(exit_code=ran.returncode, compatibility=data["compatibility"], evidence=data["evidence"],
                               adapter=data.get("adapter"), compiler_id=data["compiler_id"], compilation_ms=data.get("compilation_ms"),
                               effective_options=data["inventory"]["options"], failures=[{k: p[k] for k in ["requested", "phase", "capability"]}
                               for p in data["outcomes"]["programs"] if p["outcome"] == "failed"])
                except (subprocess.TimeoutExpired, json.JSONDecodeError) as error:
                    row.update(compatibility="untested", evidence={"validation": "failed"}, failure_category=type(error).__name__)
            row["wall_seconds"] = round(time.monotonic() - started, 3)
            assert digest(pack) == source, "corpus input changed during validation"
            result["results"].append(row)
            (output / "matrix.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            print(json.dumps({k: row.get(k) for k in ["pack", "requested_backend", "compatibility", "evidence", "failures"]}), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
