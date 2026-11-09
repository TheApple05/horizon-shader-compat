#!/usr/bin/env python3
"""Run the original shader controls serially on exact backends and retain evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import lab


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="horizon-client-render lib test executable")
    parser.add_argument("--out", type=Path, required=True, help="new evidence directory")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    prefix = "gfx::pack_post::compatibility_tests::"
    names = ["complete_chain_options_and_failure_recovery", "required_compute_and_fragment_storage_refuse_when_forced_unavailable", "explicit_hdr_handoff_retains_scene_range_and_pack_alpha"]
    listing = subprocess.run([str(binary), "--list"], capture_output=True, text=True, check=True).stdout
    assert all(prefix + name + ": test" in listing for name in names), "the supplied binary does not contain the required controls"
    digest = hashlib.sha256()
    with binary.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    results = {"schema": 1, "binary_sha256": digest.hexdigest(), "results": []}
    for backend in ["vulkan", "gl"]:
        for control in ["complete-chain", "compute", "fragment-storage", "hdr"]:
            mask = control if control in ["compute", "fragment-storage"] else ""
            case = backend + "-" + control
            home = output / case
            home.mkdir()
            env = {key: value for key, value in os.environ.items() if not key.startswith("HORIZON_")}
            env.update(WGPU_BACKEND=backend, HORIZON_SHADER_PACK_WITHOUT=mask, HORIZON_TELEMETRY_URL="off")
            for key in ["HORIZON_HOME", "HORIZON_LAUNCHER_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"]:
                env[key] = str(home / key)
            env = lab.game_env(env)
            name = names[2 if control == "hdr" else 1 if mask else 0]
            command = [str(binary), "--exact", prefix + name, "--ignored", "--nocapture", "--test-threads=1"]
            started = time.monotonic()
            row = {"case": case, "requested_backend": backend, "capability_mask": mask, "log": case + ".log"}
            try:
                process = subprocess.run(command, cwd=home, env=env, capture_output=True, text=True, timeout=60)
                log = process.stdout + process.stderr
                selected = re.search(r"backend: (Vulkan|Gl|Dx12|Metal)", log)
                selected = selected.group(1) if selected else None
                gpu = re.search(r'name: "([^"]+)"', log)
                driver = re.search(r'driver_info: "([^"]*)"', log)
                passed = process.returncode == 0 and "1 passed" in log and selected == {"vulkan": "Vulkan", "gl": "Gl"}[backend]
                row.update(status="passed" if passed else "unavailable" if "an explicit host GPU is required" in log else "failed", exit_code=process.returncode, actual_backend=selected, gpu=gpu.group(1) if gpu else None, driver=driver.group(1) if driver else None)
            except subprocess.TimeoutExpired as error:
                def decoded(value):
                    return value.decode(errors="replace") if isinstance(value, bytes) else value or ""
                log = decoded(error.stdout) + decoded(error.stderr)
                row.update(status="failed", reason="test exceeded 60 seconds and was stopped")
            row["elapsed_seconds"] = round(time.monotonic() - started, 3)
            (output / row["log"]).write_text(log, encoding="utf-8")
            results["results"].append(row)
            (output / "results.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
            print(json.dumps(row), flush=True)
    return int(any(row["status"] != "passed" for row in results["results"]))


if __name__ == "__main__":
    raise SystemExit(main())
