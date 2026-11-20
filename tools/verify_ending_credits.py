#!/usr/bin/env python3
"""Regenerate eight Ending credits oracle and native comparisons in one scene run.

Run in the dev shell: python3 tools/verify_ending_credits.py [--no-build].
Uses the real Ending fixture, empty isolated saves and the live credits planes
compositor. Acceptance and file identity are owned by psiv-native-verify in Rust.
Rendering pauses between observations; simulation never skips frames.
The oracle recipe is tape 48 plus the two tape-38 RAM patches. Comparison is
the full 320x224 retail frame (0,0,320,224), at a 1280x800 viewport.
All ROM-derived inputs and outputs stay local, under build/ending-credits/.
"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from oracle import host_binary
from tools.certify import GODOT_FLAGS, compare
from tools import native_verifier
from tools.gate import git_info, utc_stamp
from tools.godot import create_run_root, exit_leaks_binary, godot_command, logged_exit_leaks
from tools.heavy_lock import EXTENSION, GODOT, hold_heavy_lock


def run(command, log, env=None):
    print("running:", " ".join(map(str, command)), flush=True)
    with log.open("w") as stream:
        result = subprocess.run(command, env=env, stdout=stream, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError(f"exit {result.returncode}: {log}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true")
    args = parser.parse_args()
    # The machine slot precedes the repo lock, as it does for full gates.
    if not os.environ.get("HEAVY_JOBS"):
        os.execvp("heavy", ["heavy", "--", sys.executable, *sys.argv])
    root = Path(__file__).resolve().parent.parent
    os.chdir(root)
    output = root / "build" / "ending-credits" / utc_stamp(datetime.now(timezone.utc))
    output.mkdir(parents=True)
    started = time.monotonic()
    native_verifier.prepare()
    plan = native_verifier.verify({"operation": "ending-credits-plan"})
    with hold_heavy_lock(lock_class=EXTENSION):
        if not args.no_build:
            run(["cargo", "build", "--manifest-path", "rust/Cargo.toml",
                 "-p", "psiv-godot", "-p", "psiv-compare"], output / "build.log")
        exit_leaks_binary()
    oracle = output / "oracle"
    oracle.mkdir()
    command = [str(host_binary.ensure()),
               "--core", "oracle/core/genesis_plus_gx_libretro.so", "--rom", "Phantasy Star IV (USA).md",
               "--map", "oracle/ram_map.tsv", "--tape", plan["tape"],
               "--out", "/dev/null", "--dump-frames",
               ",".join(str(pair["frame"]) for pair in plan["pairs"]), "--dump-frames-dir", str(oracle)]
    for patch in plan["patches"]:
        command += ["--ram-patch", patch]
    run(command, output / "oracle.log")
    native_verifier.verify({"operation": "ending-credits-oracle", "directory": str(oracle)})
    with hold_heavy_lock(lock_class=GODOT):
        run_root = create_run_root(output / "run-root", source=root)
        try:
            saves = output / "saves"
            saves.mkdir()
            env = {k: v for k, v in os.environ.items()
                   if not k.startswith("PSIV_DEBUG_") and k not in ("PSIV_LOAD_SLOT", "XAUTHORITY")}
            env.update(PSIV_DEBUG_EVENT="0x8021", PSIV_DEBUG_ROUTE="1", PSIV_DEBUG_SCENE_TICKS="1",
                       PSIV_SAVE_DIR=str(saves), PSIV_CREDITS_OUTPUT=str(output),
                       PSIV_RUNTIME_PACK=str(run_root.pack), LIBGL_ALWAYS_SOFTWARE="1")
            command = godot_command(run_root, *GODOT_FLAGS, "--quit-after", "62200",
                                    "--script", str(root / "tools/native/native_ending_credits.gd"),
                                    prefix=("timeout", "1200", "xvfb-run", "-a"))
            run(command, output / "native.log", env)
            leaks = logged_exit_leaks(output / "native.log")
        finally:
            run_root.release()
    captures = native_verifier.verify({"operation": "ending-credits-observations", "directory": str(output)})
    comparisons = []
    for native, pair in zip(captures, plan["pairs"]):
        label, frame = pair["label"], pair["frame"]
        rmse, report = compare(output / f"{label}.png", oracle / f"frame_{frame}.png", plan["region"])
        comparisons.append({"label": native["label"], "rmse": rmse, "comparison": report})
    control = plan["negative_control"]
    rmse, report = compare(output / f"{control['native_label']}.png",
                           oracle / f"frame_{control['oracle_frame']}.png", plan["region"])
    request = {"operation": "ending-credits", "directory": str(output), "root": str(root),
               "comparisons": comparisons, "negative_control": {"rmse": rmse, "comparison": report},
               "exit_leaks": leaks, "metadata": {"git": git_info(root), "run_root": run_root.record,
               "elapsed_s": round(time.monotonic() - started, 2)}}
    (output / "comparisons.json").write_text(json.dumps(request, indent=2) + "\n")
    receipt = native_verifier.verify(request)
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, ValueError) as error:
        print(f"ending credits: {error}", file=sys.stderr)
        sys.exit(1)
