#!/usr/bin/env python3
"""Run the certified presentation pairs: native Godot capture vs GPGX oracle frame.

Each pair launches the debug Godot build under Xvfb with the capture doctrine
from docs/scenes/SCENE_PRESENTATION.md (software GL, X11, dummy audio,
`--fixed-fps 60`), takes one `PSIV_DEBUG_SHOT` at a pinned clone tick, and
compares it with a hash-pinned oracle frame through
`psiv_tools/presentation_rmse.py`. A certified pair passes only at
`rmse=0.000000`.

Oracle frames are local, ignored assets. A missing frame that has a tape
recipe (MeetingRika, tape 28) is regenerated through the oracle host; every
frame is hash-checked before use, so a regenerated or edited frame that
differs from the certified one fails instead of silently re-baselining.

Usage: `python3 tools/certify.py [--build] [--only NAME ...] [--list]`.
Receipts land in `build/certify/<UTC>-<sha>/`. Exit 0 only when every
selected pair is 0.000000. Refuses while a process has the debug extension
mapped (rebuilding or loading under a live Godot can crash it).
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate import EXTENSION_PATH, git_info, loaded_extension_pids, utc_stamp  # noqa: E402

GODOT = os.environ.get("PSIV_GODOT", str(Path.home() / ".local/bin/psiv-godot-4.7.1"))
ROM = "Phantasy Star IV (USA).md"
GODOT_FLAGS = ["--display-driver", "x11", "--rendering-method", "gl_compatibility",
               "--rendering-driver", "opengl3", "--audio-driver", "Dummy", "--fixed-fps", "60"]
SCENE = {"PSIV_DEBUG_AUTOCLOSE_SCENE": "1", "PSIV_DEBUG_RETAIL_PACE": "1"}

# Tape 28 enters MeetingRika by explicit fixture patches (oracle/README.md,
# "Deterministic scene fixtures").
TAPE_28 = {
    "tape": "oracle/tapes/28_meeting_rika_retail_probe.tape",
    "patches": ["7000:FFFFEC28:00AC", "7000:FFFFEC2A:0000", "7000:FFFFEC4E:02",
                "7000:FFFFEF00:0008", "7000:FFFFF406:01F0", "7000:FFFFF408:01A0",
                "7000:FFFFF40A:00010203", "7200:FFFFECA8:8007", "7200:FFFFEF00:000C"],
    "frames": [7250],
}

# The opening is certified through the title's real START (`Runtime::new_game`
# then Event_GameStart), not a hand-built fixture: a fixture builds its own
# state and rots when the map-entry triggers or the new-game state change
# (#44). Tape 27 is the power-on-to-first-control schedule that produced both
# frames (tape 01 is the same schedule and yields byte-identical frames).
OPENING = {"PSIV_DEBUG_TITLE_SHOT": "1", "PSIV_DEBUG_TITLE_AUTOSTART": "1", **SCENE}
TAPE_27 = {
    "tape": "oracle/tapes/27_opening_scene_presentation.tape",
    "patches": [],
    "frames": [4000, 5200],
}

# name, clone env, shot tick, oracle frame, frame SHA-256, regeneration recipe.
# The opening hashes were pinned from the local frames on 2026-10-01 (the
# 2026-08-17 certification recorded no hash for them); the others are the
# hashes recorded in SCENE_PRESENTATION.md and oracle/README.md.
PAIRS = [
    ("opening-p1", OPENING, 3550,
     "oracle/frames/opening/frame_4000.png",
     "4df2ef0f63697bdb0cb7be6bd80c6e23a6ac91942e150b0763ea0d865b890455", TAPE_27),
    ("opening-p2", OPENING, 4550,
     "oracle/frames/opening/frame_5200.png",
     "d80b93dbd866e990b611ee358380622531d007d9b9b3873d58318225182e2b74", TAPE_27),
    ("meeting-rika", {"PSIV_DEBUG_EVENT": "0x8007", **SCENE}, 160,
     "build/certify/oracle/meeting-rika/frame_7250.png",
     "d8fc26ae6987e416ee75c02cd10ea4975e9be22485b8feeda84161aa489888c9", TAPE_28),
    ("title", {"PSIV_DEBUG_TITLE_SHOT": "1"}, 480,
     "oracle/frames/title/frame_450.png",
     "8cebd30d62a7b5b0c3ad634ec6efc5ab6ab62e088d6166835d447c843df18c10", None),
    ("battle-0x88", {"PSIV_DEBUG_BATTLE": "0x88"}, 200,
     "oracle/frames/frame_25000.png",
     "761fb241a2360d222fdf1538be1af89b7bfb9cd09376a6157733fd8f71e773c4", None),
    ("camp-root", {"PSIV_DEBUG_CAMP": "1"}, 60,
     "oracle/frames/frame_7675.png",
     "9bf283d9f48b4c0d959eb297ca0b1a997b62f227385ec25cae921e078ceaeca0", None),
]

# The shot hook saves and keeps running; quit a few frames after it.
QUIT_MARGIN = 30

RMSE = re.compile(r"rmse=([0-9.]+)")


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def regenerate(recipe, frame_path, log):
    out_dir = frame_path.parent
    out_dir.mkdir(parents=True, exist_ok=True)
    command = ["oracle/bin/psiv_oracle", "--core", "oracle/core/genesis_plus_gx_libretro.so",
               "--rom", ROM, "--map", "oracle/ram_map.tsv", "--tape", recipe["tape"],
               "--out", "/dev/null", "--dump-frames", ",".join(str(f) for f in recipe["frames"]),
               "--dump-frames-dir", str(out_dir)]
    for patch in recipe["patches"]:
        command += ["--ram-patch", patch]
    with open(log, "w") as handle:
        return subprocess.run(command, stdout=handle, stderr=subprocess.STDOUT).returncode


def oracle_frame(name, frame, digest, recipe, receipt):
    path = Path(frame)
    if not path.exists() and recipe is not None:
        code = regenerate(recipe, path, receipt / f"{name}-oracle.log")
        if code != 0:
            return None, f"oracle regeneration exited {code}"
    if not path.exists():
        return None, f"missing oracle frame {frame}"
    actual = sha256(path)
    if actual != digest:
        return None, f"oracle frame hash {actual} != certified {digest}"
    return path, None


def capture(name, env, tick, receipt):
    shot = receipt / f"{name}-t{tick}.png"
    # A fresh, empty save directory per run: the title shows CONTINUE (and the
    # autostart picks a different row) when any slot is valid.
    saves = receipt / f"{name}-saves"
    saves.mkdir(parents=True, exist_ok=True)
    full_env = {**os.environ, "LIBGL_ALWAYS_SOFTWARE": "1", "PSIV_DEBUG_SCENE_TICKS": "1",
                "PSIV_SAVE_DIR": str(saves),
                "PSIV_DEBUG_SHOT": str(shot), "PSIV_DEBUG_SHOT_FRAME": str(tick), **env}
    command = ["xvfb-run", "-a", "timeout", "300s", GODOT,
               "--log-file", str(receipt / f"{name}-godot.log"), *GODOT_FLAGS,
               "--path", "godot", "--quit-after", str(tick + QUIT_MARGIN)]
    with open(receipt / f"{name}-run.log", "w") as handle:
        code = subprocess.run(command, env=full_env, stdout=handle,
                              stderr=subprocess.STDOUT).returncode
    return shot, code


def compare(shot, frame):
    completed = subprocess.run([sys.executable, "psiv_tools/presentation_rmse.py",
                                str(shot), str(frame)], capture_output=True, text=True)
    match = RMSE.search(completed.stdout)
    return (match.group(1) if match else None), (completed.stdout + completed.stderr).strip()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--build", action="store_true", help="cargo build -p psiv-godot first")
    parser.add_argument("--only", nargs="+", metavar="NAME", help="run only these pairs")
    parser.add_argument("--list", action="store_true", help="print the pairs and exit")
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parent.parent
    os.chdir(root)
    pairs = [p for p in PAIRS if not args.only or p[0] in args.only]
    if args.list or not pairs:
        for name, env, tick, frame, _, _ in PAIRS:
            print(f"{name}: tick {tick} vs {frame} {env}")
        return 0 if args.list else 2
    users = loaded_extension_pids(root / EXTENSION_PATH)
    if users:
        print(f"certify: refused, {EXTENSION_PATH} is mapped by PIDs {users}; close Godot first")
        return 3
    if args.build:
        built = subprocess.run(["cargo", "build", "-p", "psiv-godot", "--manifest-path",
                                "rust/Cargo.toml"])
        if built.returncode != 0:
            return built.returncode
    info = git_info(root)
    receipt = root / "build" / "certify" / f"{utc_stamp(datetime.now(timezone.utc))}-{info['short_sha']}"
    receipt.mkdir(parents=True, exist_ok=True)
    results = []
    for name, env, tick, frame, digest, recipe in pairs:
        frame_path, error = oracle_frame(name, frame, digest, recipe, receipt)
        record = {"pair": name, "tick": tick, "oracle": frame, "oracle_sha256": digest, "env": env}
        if error is None:
            shot, code = capture(name, env, tick, receipt)
            record["godot_exit"] = code
            if shot.exists():
                record["capture_sha256"] = sha256(shot)
                record["rmse"], record["compare"] = compare(shot, frame_path)
            else:
                error = f"no capture (godot exit {code})"
        record["error"] = error
        record["pass"] = error is None and record.get("rmse") == "0.000000"
        results.append(record)
        print(f"{name:13} {'PASS' if record['pass'] else 'FAIL'} "
              f"rmse={record.get('rmse')} {error or ''}".rstrip(), flush=True)
    (receipt / "certify.json").write_text(json.dumps({**info, "pairs": results}, indent=2) + "\n")
    passed = sum(r["pass"] for r in results)
    print(f"certify: {passed}/{len(results)} pairs at 0.000000; receipt={receipt.relative_to(root)}")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
