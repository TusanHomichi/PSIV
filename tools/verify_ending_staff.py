#!/usr/bin/env python3
"""Regenerate six Ending staff oracle and native comparisons in one scene run.

Run in the dev shell: python3 tools/verify_ending_staff.py [--no-build].
Uses the real Ending fixture, empty isolated saves and the shared RunText2
consumer. Rendering pauses between observations; simulation never skips frames.
The oracle recipe is tape 46 plus the two tape-38 RAM patches. Comparison is
psiv-compare's exact staff rectangle (24,64,256,88), at a 1280x800 viewport.
All ROM-derived inputs and outputs stay local, under build/ending-staff/.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from oracle import host_binary
from tools.certify import GODOT_FLAGS, compare, sha256
from tools.gate import git_info, utc_stamp
from tools.godot import create_run_root, exit_leaks_binary, godot_command, logged_exit_leaks
from tools.heavy_lock import EXTENSION, GODOT, hold_heavy_lock

# Oracle hashes pin a fresh regeneration; none is an asset distributed in Git.
PAIRS = [
    ("before", 42503, "779f77ad9bb3403f7714ccd9996486fa80fa41c4165bce2d6596afc57844f94d"),
    ("fall", 42525, "805c7aaa712e730d6eee66a4d31f6cad1927ea3df32bff087cf09062c64b945f"),
    ("dark", 42555, "0baa1fbd6c9ce8a7db30ce57b22d69bb07eee13e7fd8811e3226e49a79d193a6"),
    ("rise", 42945, "75a610dbc87ff7760c47fe2254fd33c6a551fbf9d989945bcaed2657b54aa6dc"),
    ("after", 42963, "779f77ad9bb3403f7714ccd9996486fa80fa41c4165bce2d6596afc57844f94d"),
    ("cleared", 42966, "779f77ad9bb3403f7714ccd9996486fa80fa41c4165bce2d6596afc57844f94d"),
]
REGION = [24, 64, 256, 88]


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
    output = root / "build" / "ending-staff" / utc_stamp()
    output.mkdir(parents=True)
    started = time.monotonic()
    with hold_heavy_lock(lock_class=EXTENSION):
        if not args.no_build:
            run(["cargo", "build", "--manifest-path", "rust/Cargo.toml",
                 "-p", "psiv-godot", "-p", "psiv-compare"], output / "build.log")
        exit_leaks_binary()
    oracle = output / "oracle"
    oracle.mkdir()
    command = [str(host_binary.ensure()),
               "--core", "oracle/core/genesis_plus_gx_libretro.so", "--rom", "Phantasy Star IV (USA).md",
               "--map", "oracle/ram_map.tsv", "--tape", "oracle/tapes/46_ending_staff.tape",
               "--out", "/dev/null", "--ram-patch", "7200:FFFFECA8:8021",
               "--ram-patch", "7200:FFFFEC20:000C", "--dump-frames",
               ",".join(str(frame) for _, frame, _ in PAIRS), "--dump-frames-dir", str(oracle)]
    run(command, output / "oracle.log")
    for _, frame, digest in PAIRS:
        if sha256(oracle / f"frame_{frame}.png") != digest:
            raise RuntimeError(f"oracle frame {frame} hash changed")
    with hold_heavy_lock(lock_class=GODOT):
        run_root = create_run_root(output / "run-root", source=root)
        try:
            saves = output / "saves"
            saves.mkdir()
            env = {k: v for k, v in os.environ.items()
                   if not k.startswith("PSIV_DEBUG_") and k not in ("PSIV_LOAD_SLOT", "XAUTHORITY")}
            env.update(PSIV_DEBUG_EVENT="0x8021", PSIV_DEBUG_ROUTE="1", PSIV_DEBUG_SCENE_TICKS="1",
                       PSIV_SAVE_DIR=str(saves), PSIV_STAFF_OUTPUT=str(output),
                       PSIV_RUNTIME_PACK=str(run_root.pack), LIBGL_ALWAYS_SOFTWARE="1")
            command = godot_command(run_root, *GODOT_FLAGS, "--quit-after", "45200",
                                    "--script", str(root / "tools/native/native_ending_staff.gd"),
                                    prefix=("timeout", "1200", "xvfb-run", "-a"))
            run(command, output / "native.log", env)
            if logged_exit_leaks(output / "native.log"):
                raise RuntimeError(f"Godot exit leaks: {output / 'native.log'}")
        finally:
            run_root.release()
    captures = json.loads((output / "captures.json").read_text())
    if [row["label"] for row in captures] != [row[0] for row in PAIRS]:
        raise RuntimeError("missing or reordered native observations")
    results = []
    for native, (label, frame, _) in zip(captures, PAIRS):
        rmse, result = compare(output / f"{label}.png", oracle / f"frame_{frame}.png", REGION)
        results.append({**native, "oracle_frame": frame, "rmse": rmse, "comparison": result})
    receipt = {"git": git_info(root), "inputs": {
        name: sha256(root / name) for name in ("Phantasy Star IV (USA).md", "oracle/tapes/46_ending_staff.tape",
                                             "runtime-pack/dialogue/font.png")},
        "run_root": run_root.record, "region": REGION, "results": results,
        "elapsed_s": round(time.monotonic() - started, 2)}
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))
    return 0 if all(row["rmse"] == "0.000000" for row in results) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, ValueError) as error:
        print(f"ending staff: {error}", file=sys.stderr)
        sys.exit(1)
