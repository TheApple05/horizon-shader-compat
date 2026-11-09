#!/usr/bin/env python3
"""Isolated live-game regression for pack loading while frames submit on the requested backend."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import time
import lab


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", required=True, type=Path)
    parser.add_argument("--resources", required=True, type=Path)
    parser.add_argument("--backend", required=True, choices=["vulkan", "gl"])
    parser.add_argument("--pack", type=Path, default=lab.ROOT / "crates/horizon-shaders/tests/packs/compat-baseline")
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--revision", help="source revision corresponding to the supplied binaries")
    args = parser.parse_args()
    if args.revision and not re.fullmatch("[0-9a-f]{40}", args.revision):
        parser.error("revision must be the full source commit")
    pack = args.pack.resolve(strict=True)
    output = args.out.resolve()
    if output == pack or (pack.is_dir() and output.is_relative_to(pack)):
        parser.error("test output must stay outside the original pack")
    output.mkdir(parents=True, exist_ok=False)
    binaries = output / "bin"
    binaries.mkdir()
    for name in ["horizon-client.exe", "horizon-ctl.exe"]:
        shutil.copy2(args.bin_dir / name, binaries / name)
    if pack.is_dir() and not (pack / "shaders").is_dir():
        parser.error("pack folder must contain shaders/")
    target = output / "shaderpacks" / pack.name
    target.parent.mkdir()
    if pack.is_dir():
        shutil.copytree(pack, target)
    else:
        shutil.copy2(pack, target)
    (output / "options.txt").write_text("renderDistance:2\nsimulationDistance:2\nmaxFps:60\n", encoding="utf-8")
    api = "opengl" if args.backend == "gl" else "vulkan"
    (output / "horizon-graphics.txt").write_text(f"graphicsApi:{api}\nhdr:false\npickedShaderPack:{pack.name}\n", encoding="utf-8")
    env = {k: v for k, v in os.environ.items() if not k.startswith("HORIZON_")}
    for key in ["HORIZON_HOME", "HORIZON_LAUNCHER_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"]:
        env[key] = str(output / key)
    env = lab.game_env(env, WGPU_BACKEND=args.backend, HORIZON_TELEMETRY_URL="off")
    command = [str(binaries / "horizon-ctl.exe"), "--socket", str(output / "control.json")]
    calls = 0

    def call(*words):
        nonlocal calls
        calls += 1
        process = subprocess.run([*command, *map(str, words)], cwd=output, env=env, capture_output=True,
                                 text=True, encoding="utf-8", errors="replace", timeout=145,
                                 creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        (output / f"call-{calls}.json").write_text(process.stdout, encoding="utf-8")
        data = json.loads(process.stdout)
        assert process.returncode == 0 and not data.get("error"), (words, data)
        return data

    log = (output / "game.log").open("w", encoding="utf-8")
    game = subprocess.Popen([str(binaries / "horizon-client.exe"), "--world", "compat-smoke", "--creative", "--seed", "84326",
             "--offscreen", "--width", "640", "--height", "360", "--game-dir", str(output), "--resources", str(args.resources.resolve()),
             "--control-socket", str(output / "control.json")], cwd=output, env=env, stdout=log, stderr=subprocess.STDOUT,
             creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    try:
        deadline = time.monotonic() + 90
        while True:
            try:
                call("hello")
                break
            except (AssertionError, json.JSONDecodeError):
                assert game.poll() is None and time.monotonic() < deadline, "game did not open control"
                time.sleep(0.25)
        call("wait-ready", "120")
        deadline = time.monotonic() + 90
        while True:
            state = call("status")
            renderer = state["renderer"]
            assert renderer["backend"] == {"vulkan": "Vulkan", "gl": "Gl"}[args.backend]
            if not renderer["shader_pack_loading"]:
                break
            assert time.monotonic() < deadline, "pack load exceeded 90 seconds"
            time.sleep(0.25)
        assert renderer["shader_pack_kind"] == "iris", renderer
        assert not any(p["outcome"] == "failed" for p in renderer["shader_pack_compatibility"]["programs"])
        before = state["frames"]
        call("screenshot", output / "scene.png")
        call("wait", "20")
        state = call("status")
        assert state["frames"] > before and state["renderer"]["shader_pack_kind"] == "iris"
        try:
            call("gpu-passes", "on")
        except AssertionError:
            pass
        timings = [call("bench", "2"), call("bench", "2")]
        memory = call("memory")
        assert call("status")["renderer"]["shader_pack_kind"] == "iris"
        binary = binaries / "horizon-client.exe"
        digest = hashlib.sha256()
        with binary.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1 << 20), b""):
                digest.update(chunk)
        result = {"schema": 1, "source_revision": args.revision, "binary_sha256": digest.hexdigest(), "pack": pack.name,
                  "requested_backend": args.backend, "status": "passed", "renderer": state["renderer"], "visual_fidelity": "not_run"}
        result.update(timings=timings, memory=memory, warmup_ticks=20, frame_cap=60,
                      measurement_conditions="correctness run; other compilation may be active; not a performance comparison")
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"pack": pack.name, "backend": args.backend, "status": "passed", "frames_advanced": state["frames"] - before}))
    finally:
        if game.poll() is None:
            try:
                call("quit")
                game.wait(timeout=60)
            except (AssertionError, subprocess.TimeoutExpired, json.JSONDecodeError):
                game.kill()
                game.wait(timeout=60)
        log.close()


if __name__ == "__main__":
    main()
