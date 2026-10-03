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

Usage: `python3 tools/certify.py [--no-build] [--only NAME ...] [--list]`.
The debug extension is rebuilt first by default, so a capture always comes from
the checked-out sources; `--no-build` skips that only when the caller has just
built this exact tree (a stale extension once certified the wrong code).
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

# `oracle` is a package of the repository root, not of `tools/`, so the root
# goes on the path too. The one thing this module takes from there is the host
# it regenerates a frame with: `oracle/host_binary.py` hands back a binary
# built from `oracle/host/` as it stands, so certification cannot compare
# against frames a stale host produced (issue #61).
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from oracle import host_binary  # noqa: E402

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

# Tape 32 is tape 07's battle, command idle at frame 25000, then Alys's
# command strip (cursor on TECH) and technique window. Both frames come from
# one oracle run (`oracle/tapes/32_battle_command_windows.tape`).
TAPE_32 = {
    "tape": "oracle/tapes/32_battle_command_windows.tape",
    "patches": [],
    "frames": [25134, 25400],
}

# The same tape's command-idle frame with three status bytes patched in
# `Character_Stats` one frame earlier: Chaz asleep ($08), Alys paralyzed ($02),
# Hahn tech-sealed ($10). The patches are fixture operations (oracle/README.md,
# "Deterministic scene fixtures"), not tape input.
TAPE_32_STATUS = {
    "tape": "oracle/tapes/32_battle_command_windows.tape",
    "patches": ["24999:FFFFF516:08", "24999:FFFFF596:02", "24999:FFFFF616:10"],
    "frames": [25000],
}

# The other three status inks: Chaz poisoned ($01, CRAM line 0), Alys dead
# ($04, line 2 and the death icon), Hahn asleep and sealed ($18).
TAPE_32_STATUS_2 = {
    "tape": "oracle/tapes/32_battle_command_windows.tape",
    "patches": ["24999:FFFFF516:01", "24999:FFFFF596:04", "24999:FFFFF616:18"],
    "frames": [25000],
}

# Tape 33 is a forced battle: tape 07's own field prefix, with the three RAM
# patches `python3 -m oracle.force --formation 0xD2` writes (the Passageway's
# encounter group at the encounter, the seed word that lands the draw on
# formation $D2, the map cells back), then its attack policy. Round 1 is the
# two Zol slugs' Fusion; the options reopen over the MetaSlug about frame 25496.
TAPE_33 = {
    "tape": "oracle/tapes/33_zol_fusion_metaslug.tape",
    "patches": ["24795:FFFFEC28:0081", "24818:FFFFEF0C:000A", "24820:FFFFEC28:0015"],
    "frames": [25560],
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
    # Sleep, paralysis and the seal on the status strip: the pane icons and
    # inks come from `Battle_DrawCommandIcons`; the seal draws nothing. Blink
    # seed 3/1 reads (17, 0) at tick 200, the oracle's $FFFF41D2/$FFFF41D4 at
    # frame 25000; overlay phase 5568 puts the enemy clock where the
    # oracle's Enemy_Sprites words say it is (docs/battle/BATTLE_COMMAND_UI.md).
    ("battle-status", {"PSIV_DEBUG_BATTLE_WINDOW": "top,blink=3/1,status=2/8,status=1/2,status=3/16",
                       "PSIV_DEBUG_BATTLE_PHASE": "5568"}, 200,
     "build/certify/oracle/battle-status/frame_25000.png",
     "f2037c2e7875ab85125a6ce20f392646bdfadd36ad3f0e7aaf35e24a74956da0", TAPE_32_STATUS),
    ("battle-status-2", {"PSIV_DEBUG_BATTLE_WINDOW": "top,blink=3/1,status=2/1,status=1/4,status=3/24",
                         "PSIV_DEBUG_BATTLE_PHASE": "5568"}, 200,
     "build/certify/oracle/battle-status-2/frame_25000.png",
     "3c0ac9db15083df8b18eb3b3458f91df6b9c76d19b7025782c80bd2a0392421c", TAPE_32_STATUS_2),
    # The fused MetaSlug. The clone plays the real round (the party defends,
    # the slugs' turn is Fusion) and the shot lands where the MetaSlug's overlay
    # clock reads what the oracle's Enemy_Sprites words say at frame 25560:
    # every piece at (frame 0, timer 2), one tick before the video frame.
    ("battle-fusion", {"PSIV_DEBUG_BATTLE_WINDOW": "top,round=defend",
                       "PSIV_DEBUG_BATTLE_FORMATION": "D2",
                       "PSIV_DEBUG_BATTLE_MAP": "81"}, 714,
     "build/certify/oracle/battle-fusion/frame_25560.png",
     "b10cb5aa7ce60c6c66eec714cab6de28d4ff85cad7f3a385aaa4cdc589ad8ef8", TAPE_33),
    # Clone tick t pairs with tape-07-family oracle frame 24800 + t: the
    # battle fixtures start at tick 30 with the overlay clock seeded to 20
    # (the command-idle pair's pin: tick 200 is frame 25000).
    ("battle-strip", {"PSIV_DEBUG_BATTLE_WINDOW": "strip,cursor=1,age=7", "PSIV_DEBUG_BATTLE_PHASE": "5567"}, 334,
     "build/certify/oracle/battle-windows/frame_25134.png",
     "b148f45abf5b9e80c779d64b9a7e88fa8f545dd2fbb455aad2df41fd82f95bcb", TAPE_32),
    # The blink seed (timer 14, phase 1) is solved so that the red cursor's
    # timers read what the oracle's RAM held at frame 25400 ($FFFF41D2 = 3,
    # $FFFF41D4 = 1) after the 571 frames the clone runs them.
    ("battle-tech", {"PSIV_DEBUG_BATTLE_WINDOW": "tech,cursor=0,blink=14/1", "PSIV_DEBUG_BATTLE_PHASE": "5567"}, 600,
     "build/certify/oracle/battle-windows/frame_25400.png",
     "7a6b67ab078e88db3ff9bbbe066a7e2e6070102ff352efc7fea039d16a66bcdc", TAPE_32),
]

# The shot hook saves and keeps running; quit a few frames after it.
QUIT_MARGIN = 30

RMSE = re.compile(r"rmse=([0-9.]+)")


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def regenerate(recipe, frame_path, log):
    out_dir = frame_path.parent
    out_dir.mkdir(parents=True, exist_ok=True)
    command = [str(host_binary.ensure()),
               "--core", "oracle/core/genesis_plus_gx_libretro.so",
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


#: Software GL under Xvfb renders a few frames a second on a loaded machine
#: (opening-p2 reached tick 3018 of 4580 in 300 s at load 8), so a capture's
#: budget scales with how far it must run instead of a flat cap.
TIMEOUT_BASE_S = 120
TIMEOUT_PER_TICK_S = 0.25


def capture_timeout(tick):
    """Seconds a capture to `tick` may take before it is called hung."""
    return int(TIMEOUT_BASE_S + (tick + QUIT_MARGIN) * TIMEOUT_PER_TICK_S)


def capture(name, env, tick, receipt):
    shot = receipt / f"{name}-t{tick}.png"
    # A fresh, empty save directory per run: the title shows CONTINUE (and the
    # autostart picks a different row) when any slot is valid.
    saves = receipt / f"{name}-saves"
    saves.mkdir(parents=True, exist_ok=True)
    full_env = {**os.environ, "LIBGL_ALWAYS_SOFTWARE": "1", "PSIV_DEBUG_SCENE_TICKS": "1",
                "PSIV_SAVE_DIR": str(saves),
                "PSIV_DEBUG_SHOT": str(shot), "PSIV_DEBUG_SHOT_FRAME": str(tick), **env}
    command = ["xvfb-run", "-a", "timeout", f"{capture_timeout(tick)}s", GODOT,
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


def build_parser():
    """The command line: build by default, opt out with `--no-build`."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-build", action="store_true",
                        help="skip `cargo build -p psiv-godot` (only when this tree was just built)")
    parser.add_argument("--build", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--only", nargs="+", metavar="NAME", help="run only these pairs")
    parser.add_argument("--list", action="store_true", help="print the pairs and exit")
    return parser


def main(argv=None):
    args = build_parser().parse_args(argv)
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
    if not args.no_build:
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
