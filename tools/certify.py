#!/usr/bin/env python3
"""Run the certified presentation pairs: native Godot capture vs GPGX oracle frame.

Each pair launches the debug Godot build under Xvfb with the capture doctrine
from docs/scenes/SCENE_PRESENTATION.md (software GL, X11, dummy audio,
`--fixed-fps 60`), takes one `PSIV_DEBUG_SHOT` at a pinned clone tick, and
compares it with a hash-pinned oracle frame through `presentation-rmse`
(`rust/psiv-compare`), over the whole frame or the region the pair's recipe
names. A certified pair passes only at `rmse=0.000000`.

Oracle frames are local, ignored assets. A missing frame that has a tape
recipe (MeetingRika, tape 28) is regenerated through the oracle host; every
frame is hash-checked before use, so a regenerated or edited frame that
differs from the certified one fails instead of silently re-baselining.

Usage: `python3 tools/certify.py [--no-build] [--jobs N] [--only NAME ...] [--list]`.
The debug extension and `presentation-rmse` are rebuilt first by default, so a
capture and its comparison always come from the checked-out sources; `--no-build` skips that only when the caller has just
built this exact tree (a stale extension once certified the wrong code).
Receipts land in `build/certify/<UTC>-<sha>/`. Exit 0 only when every
selected pair is 0.000000.

Certify takes the shared heavy lock itself (tools/heavy_lock.py), so it needs
no wrapper: the build in the extension class, then the captures holding one
godot-class slot per worker. `--jobs N` (default min(3, the godot slot cap))
runs the captures across N workers, longest first; each worker owns one Xvfb
server for its life and each capture its own save directory and output files,
so a pair's capture is the same bytes at any N. Under an exclusive holder both
steps are nested no-ops and N is capped by the slot cap; under a godot holder
(with `--no-build`: a build there is refused), by the slots it holds.

The captures run from one run root in the receipt directory
(tools/godot.py `create_run_root`): one snapshot of the built extension that
every worker loads, and one project copy per worker, so no capture maps the
checkout's library and `certify.json` names the library by sha256.
"""
import argparse
import ctypes
import functools
import hashlib
import json
import os
import queue
import re
import select
import signal
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack, contextmanager
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate import display_path, git_info, utc_stamp  # noqa: E402
from heavy_lock import (EXTENSION, GODOT as GODOT_CLASS, godot_slots,  # noqa: E402
                        hold_heavy_lock)

# `oracle` is a package of the repository root, not of `tools/`, so the root
# goes on the path too. The one thing this module takes from there is the host
# it regenerates a frame with: `oracle/host_binary.py` hands back a binary
# built from `oracle/host/` as it stands, so certification cannot compare
# against frames a stale host produced (issue #61).
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from oracle import host_binary  # noqa: E402

from godot import (RUN_ROOT_PACK, GodotError, create_run_root, exit_leaks_binary,  # noqa: E402
                   godot_command, logged_exit_leaks, require as require_godot)
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

# Tape 35 is tape 28's input schedule with the ship's destination menu entered
# by explicit fixture patches (`Cutscene_InsideSpaceship`, event `$800D`, at the
# Mota Spaceport with `EventFlag_AlysFound` set): frame 7000 reloads the map,
# frame 7200 puts the field routine in Events with `Event_Index = $800D`. The
# menu's windows are open by frame 7258 and the red cursor object and the
# marker's palette blink then run free; frames 7283..7308 are one cursor-hidden
# stretch whose frames are byte-identical, so frame 7293 is a settled target
# (`docs/scenes/41_InsideSpaceship.md`, "Oracle timeline").
TAPE_35 = {
    "tape": "oracle/tapes/35_ship_destination_menu.tape",
    "patches": ["7000:FFFFEC28:00BF", "7000:FFFFEC2A:0000", "7000:FFFFEC4E:02",
                "7000:FFFFEF00:0008", "7000:FFFFF101:80", "7200:FFFFECA8:800D",
                "7200:FFFFEC20:000C"],
    "frames": [7293],
}

# Tape 36 is tape 22's prelude with three fixture patches, a TELEPIPE, every
# Motavia town visited and `EventFlag_PrincipalConfession`, then ITEM, the
# TELEPIPE and USE: frame 8090 is the settled town list
# (docs/field/TRAVEL.md, "Teleport windows and effect").
TAPE_36 = {
    "tape": "oracle/tapes/36_teleport_windows.tape",
    "patches": ["7600:FFFFF410:84", "7600:FFFFF160:FFFF", "7600:FFFFF101:08"],
    "frames": [8090],
    # The town list's own window, (18,13) 14x12 cells: the ITEM path's
    # windows around it are not this pair's to certify.
    "region": [144, 104, 112, 96],
}

# The same tape's teleport message after Down to MILE and Speak, settled from
# 8300; its window $45, (7,21) 26x5 cells, over the list's lower rows.
TAPE_36_MESSAGE = {
    **TAPE_36,
    "frames": [8340],
    "region": [56, 168, 208, 40],
}

# The same tape's message closing after the acknowledgement on 8355: frame
# 8358 shows `DestroyAllWindows`'s third step, the blank window 18 cells wide
# at column 11, with the meseta and the town list's foot uncovered beside it.
TAPE_36_CLOSE = {
    **TAPE_36,
    "frames": [8358],
    "region": [56, 168, 208, 40],
}

# Then the town list's own close: 8365 is its second step, 10 cells wide at
# column 20, the field uncovered on both sides.
TAPE_36_CLOSE_TOWNS = {
    **TAPE_36,
    "frames": [8365],
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
    # The Fusion's own frames (#64) seat the MetaSlug 80 frames into the
    # slugs' turn rather than 36, and the options come back 20 frames sooner
    # after the seat: tick 758 puts the overlay where 714 did, and the blink
    # seed 21/0 is the old phase 20 red-cursor frames earlier.
    ("battle-fusion", {"PSIV_DEBUG_BATTLE_WINDOW": "top,round=defend,blink=21/0",
                       "PSIV_DEBUG_BATTLE_FORMATION": "D2",
                       "PSIV_DEBUG_BATTLE_MAP": "81"}, 758,
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
    # The ship's destination menu at the Mota Spaceport: the cursor hidden, the
    # marker blink and the clone's runtime clock aligned so the frame is the
    # oracle's frame 7293. The pair is the menu's own draw (the planetary map,
    # both windows, the cursor boxes), certified at 0.000000.
    # After 12 Window_Draw frames, tick 70 still has full windows, cursor and
    # marker hidden (session_destination); retain the fixed oracle frame.
    ("ship-menu", {"PSIV_DEBUG_EVENT": "0x800D"}, 70,
     "build/certify/oracle/ship-menu/frame_7293.png",
     "af4aaf169ea283c1784dad98822f02adcaa64684ccd3a5d037596df58f3300b8", TAPE_35),
    # The TELEPIPE town list against tape 36, its window's region only (#128).
    ("teleport-towns", {"PSIV_DEBUG_CAMP": "telepipe"}, 60,
     "build/certify/oracle/teleport-towns/frame_8090.png",
     "31176866ed7455c71d3126c9737798f74ddd85e3192ef5d919055fe833be2141", TAPE_36),
    ("teleport-message", {"PSIV_DEBUG_CAMP": "telepipe-message"}, 60,
     "build/certify/oracle/teleport-message/frame_8340.png",
     "654ca4af46c35ce3d428e8ae00d9e4f125c1484309b126f9e5f1edc6344bfccb", TAPE_36_MESSAGE),
    # The fixture acknowledges at tick 30 (8355); the shot at tick 31 shows
    # the first step (8356), so tick 33 is the third.
    ("teleport-close", {"PSIV_DEBUG_CAMP": "telepipe-close"}, 33,
     "build/certify/oracle/teleport-close/frame_8358.png",
     "3ebdba42970ec29494ef93a372381a97fd557b5ca707b21974291ddf64e48a2f", TAPE_36_CLOSE),
    ("teleport-close-towns", {"PSIV_DEBUG_CAMP": "telepipe-close"}, 40,
     "build/certify/oracle/teleport-close-towns/frame_8365.png",
     "28524733090391752cf562879b1fc59abfe4556ee94369a415edef289f7965c3", TAPE_36_CLOSE_TOWNS),
]

# The shot hook saves and keeps running; quit a few frames after it.
QUIT_MARGIN = 30

RMSE = re.compile(r"rmse=([0-9.]+)")
# The comparison, built beside the debug extension.
COMPARE = Path(__file__).resolve().parent.parent / "rust" / "target" / "debug" / "presentation-rmse"


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


#: The Xvfb screen `xvfb-run` gives a server by default, which every certified
#: capture was taken on; Godot's own window is the 1280x800 viewport regardless.
XVFB_ARGS = ["-screen", "0", "640x480x24", "-nolisten", "tcp"]
XVFB_START_S = 30
#: The X server program (a stand-in for the cases).
XVFB_COMMAND = ["Xvfb"]
PR_SET_PDEATHSIG = 1


def die_with_parent():
    """A `preexec_fn` that has the kernel send the child SIGTERM when this
    process dies, however it dies (SIGKILL included), so no Xvfb is orphaned.

    The signal follows the thread that forks the child: call it on the thread
    that outlives the server (`run_captures` starts every server on its caller's).
    """
    parent = os.getpid()
    libc = ctypes.CDLL(None, use_errno=True)

    def arm():
        libc.prctl(PR_SET_PDEATHSIG, signal.SIGTERM)
        if os.getppid() != parent:  # the parent died before the prctl
            os._exit(1)

    return arm


@contextmanager
def xvfb_display(log):
    """One Xvfb server for a worker's life; yields its DISPLAY (`:N`).

    `-displayfd` makes the server pick a free display number and report it,
    so concurrent workers (in this run or another worktree's) never collide.
    """
    read_end, write_end = os.pipe()
    with open(log, "w") as handle:
        server = subprocess.Popen([*XVFB_COMMAND, "-displayfd", str(write_end), *XVFB_ARGS],
                                  pass_fds=(write_end,), stdout=handle,
                                  stderr=subprocess.STDOUT, preexec_fn=die_with_parent())
    os.close(write_end)
    try:
        number = b""
        deadline = time.monotonic() + XVFB_START_S
        while not number.endswith(b"\n"):
            ready, _, _ = select.select([read_end], [], [], max(deadline - time.monotonic(), 0))
            chunk = os.read(read_end, 64) if ready else b""
            if not chunk:
                raise RuntimeError(f"Xvfb reported no display within {XVFB_START_S} s; log={log}")
            number += chunk
        yield f":{int(number)}"
    finally:
        os.close(read_end)
        server.terminate()
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()


def capture(name, env, tick, receipt, display, project, run_root):
    shot = receipt / f"{name}-t{tick}.png"
    # A fresh, empty save directory per run: the title shows CONTINUE (and the
    # autostart picks a different row) when any slot is valid.
    saves = receipt / f"{name}-saves"
    saves.mkdir(parents=True, exist_ok=True)
    full_env = {**{k: v for k, v in os.environ.items() if k != "XAUTHORITY"},
                "DISPLAY": display, "LIBGL_ALWAYS_SOFTWARE": "1",
                "PSIV_DEBUG_SCENE_TICKS": "1", "PSIV_SAVE_DIR": str(saves),
                "PSIV_DEBUG_SHOT": str(shot), "PSIV_DEBUG_SHOT_FRAME": str(tick), **env,
                # The run root's pack, resolved once: a caller's override would
                # win over `res://../runtime-pack` in the game (lib.rs `ready`).
                "PSIV_RUNTIME_PACK": str(project.parent / RUN_ROOT_PACK)}
    command = godot_command(run_root, "--log-file", str(receipt / f"{name}-godot.log"),
                            *GODOT_FLAGS, "--quit-after", str(tick + QUIT_MARGIN),
                            project=project, prefix=("timeout", f"{capture_timeout(tick)}s"))
    with open(receipt / f"{name}-run.log", "w") as handle:
        code = subprocess.run(command, env=full_env, stdout=handle,
                              stderr=subprocess.STDOUT).returncode
    return shot, code


def compare(shot, frame, region=None):
    crop = ["--region", ",".join(str(value) for value in region)] if region else []
    completed = subprocess.run([str(COMPARE), *crop, str(shot), str(frame)],
                               capture_output=True, text=True)
    match = RMSE.search(completed.stdout)
    return (match.group(1) if match else None), (completed.stdout + completed.stderr).strip()


def build_parser():
    """The command line: build by default, opt out with `--no-build`."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-build", action="store_true",
                        help="skip `cargo build -p psiv-godot -p psiv-compare` "
                             "(only when this tree was just built)")
    parser.add_argument("--build", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--jobs", type=int, metavar="N",
                        help=f"capture workers, one godot slot each (default: min({DEFAULT_JOBS}, "
                             "the godot slot cap))")
    parser.add_argument("--only", nargs="+", metavar="NAME", help="run only these pairs")
    parser.add_argument("--list", action="store_true", help="print the pairs and exit")
    return parser


#: Workers when `--jobs` is not given, before the godot slot cap applies.
DEFAULT_JOBS = 3


def job_count(requested, cap):
    """Workers for a run: `requested` (default min(DEFAULT_JOBS, cap)), or a
    ValueError when it is not 1..cap, since more could never be granted."""
    if requested is None:
        return min(DEFAULT_JOBS, cap)
    if not 1 <= requested <= cap:
        raise ValueError(f"--jobs {requested}: the godot slot cap is {cap} "
                         "(PSIV_GODOT_SLOTS raises it)")
    return requested


def run_captures(tasks, jobs, receipt, display, take, results=None, *, projects):
    """Capture every `(name, env, tick)` task across `jobs` workers, longest
    tick first; fills and returns `results`, `{name: (shot, godot exit, seconds)}`.

    Each worker owns one display from `display(log)` (`xvfb_display`) and one
    of `projects` (the run root's project copies, at least `jobs` of them) for
    its life. `take(name, env, tick, receipt, display, project)` makes one
    capture: `capture` with the run root bound, which `main` passes; it has no
    default, since `capture` alone cannot run without that run root. When a capture
    raises or the wait is interrupted (Ctrl-C), the queued captures are
    cancelled, the running ones finish, and the exception propagates with
    `results` holding every capture that finished.
    """
    jobs = min(jobs, len(tasks))
    if len(projects) < jobs:
        raise ValueError(f"{jobs} workers need {jobs} projects, not {len(projects)}")
    workers = queue.Queue()
    results = {} if results is None else results
    with ExitStack() as stack:
        for worker in range(jobs):
            workers.put((stack.enter_context(display(receipt / f"xvfb-{worker}.log")),
                         projects[worker]))

        stop = threading.Event()  # set by the first failure; a worker checks it first

        def run(task):
            if stop.is_set():
                return
            name, env, tick = task
            screen, project = workers.get()
            try:
                start = time.monotonic()
                shot, code = take(name, env, tick, receipt, screen, project)
                results[name] = (shot, code, round(time.monotonic() - start, 1))
            except BaseException:
                stop.set()
                raise
            finally:
                workers.put((screen, project))

        ordered = sorted(tasks, key=lambda task: -task[2])
        pool = ThreadPoolExecutor(max_workers=max(jobs, 1))
        try:
            for future in [pool.submit(run, task) for task in ordered]:
                future.result()
        except BaseException:
            stop.set()
            raise
        finally:
            pool.shutdown(wait=True, cancel_futures=True)
    return results


def main(argv=None):
    args = build_parser().parse_args(argv)
    root = Path(__file__).resolve().parent.parent
    os.chdir(root)
    pairs = [p for p in PAIRS if not args.only or p[0] in args.only]
    if args.list or not pairs:
        for name, env, tick, frame, _, _ in PAIRS:
            print(f"{name}: tick {tick} vs {frame} {env}")
        return 0 if args.list else 2
    try:
        require_godot()
        exit_leaks_binary()
    except GodotError as error:
        print(f"certify: refused, {error}")
        return 2
    try:
        jobs = min(job_count(args.jobs, godot_slots()), len(pairs))
    except ValueError as error:
        print(f"certify: refused, {error}")
        return 2
    if not args.no_build:
        with hold_heavy_lock(lock_class=EXTENSION):
            built = subprocess.run(["cargo", "build", "-p", "psiv-godot", "-p", "psiv-compare",
                                    "--manifest-path", "rust/Cargo.toml"])
        if built.returncode != 0:
            return built.returncode
    info = git_info(root)
    receipt = receipt_dir(root, info)
    receipt.mkdir(parents=True, exist_ok=True)
    with hold_heavy_lock(lock_class=GODOT_CLASS, slots=jobs) as hold:
        jobs = min(jobs, hold.slots)
        print(f"certify: {len(pairs)} pairs on {jobs} worker(s), "
              f"{'under the holder of' if hold.nested else 'holding'} {hold.lock_class} "
              f"{hold.path}", flush=True)
        try:  # one library snapshot, one prepared project per worker
            run_root = create_run_root(receipt / "run-root", projects=jobs, source=root)
        except GodotError as error:
            print(f"certify: refused, {error}")
            return 2
        print(f"certify: run root {display_path(run_root.root, root)}, library sha256 "
              f"{run_root.library_sha256} ({run_root.record['created_s']} s)", flush=True)
        started = time.monotonic()
        records, tasks = [], []
        for name, env, tick, frame, digest, recipe in pairs:
            frame_path, error = oracle_frame(name, frame, digest, recipe, receipt)
            record = {"pair": name, "tick": tick, "oracle": frame,
                      "oracle_sha256": digest, "env": env}
            if recipe and recipe.get("region"):
                record["region"] = recipe["region"]
            records.append((record, frame_path, error))
            if error is None:
                tasks.append((name, env, tick))
        prepared = time.monotonic()
        shots, interrupted = {}, None
        try:
            run_captures(tasks, jobs, receipt, xvfb_display,
                         functools.partial(capture, run_root=run_root), results=shots,
                         projects=run_root.projects)
        except BaseException as stop:  # Ctrl-C too: record what finished, then re-raise
            interrupted = stop
        finally:
            run_root.release()  # certify.json keeps its record
        timing = {"oracle_s": round(prepared - started, 1),
                  "capture_s": round(time.monotonic() - prepared, 1)}
    results = []
    for record, frame_path, error in records:
        if error is None and record["pair"] not in shots:
            error = "interrupted before its capture finished"
        if error is None:
            shot, code, seconds = shots[record["pair"]]
            record["godot_exit"] = code
            record["seconds"] = seconds
            if shot.exists():
                record["capture_sha256"] = sha256(shot)
                record["rmse"], record["compare"] = compare(shot, frame_path,
                                                            record.get("region"))
            else:
                error = f"no capture (godot exit {code})"
            record["exit_leaks"] = logged_exit_leaks(receipt / f"{record['pair']}-run.log",
                                                     receipt / f"{record['pair']}-godot.log")
            if error is None and record["exit_leaks"]:
                error = "Godot reported at exit: " + "; ".join(record["exit_leaks"])
        record["error"] = error
        record["pass"] = error is None and record.get("rmse") == "0.000000"
        results.append(record)
        print(f"{record['pair']:13} {'PASS' if record['pass'] else 'FAIL'} "
              f"rmse={record.get('rmse')} {error or ''}".rstrip(), flush=True)
    stopped = None if interrupted is None else f"{type(interrupted).__name__}: {interrupted}"
    (receipt / "certify.json").write_text(json.dumps(
        {**info, "jobs": jobs, **timing, "interrupted": stopped, "run_root": run_root.record,
         "pairs": results},
        indent=2) + "\n")
    if interrupted is not None:
        print(f"certify: interrupted ({stopped}); {len(shots)}/{len(tasks)} captures finished; "
              f"receipt={display_path(receipt, root)}", flush=True)
        raise interrupted
    passed = sum(r["pass"] for r in results)
    print(f"certify: {passed}/{len(results)} pairs at 0.000000; captures {timing['capture_s']} s "
          f"on {jobs} worker(s), oracle frames {timing['oracle_s']} s; "
          f"receipt={display_path(receipt, root)}")
    return 0 if passed == len(results) else 1


def receipt_dir(root, info):
    """`build/certify/<UTC stamp>-<short sha>/` under `root`."""
    return root / "build" / "certify" / f"{utc_stamp(datetime.now(timezone.utc))}-{info['short_sha']}"


if __name__ == "__main__":
    sys.exit(main())
