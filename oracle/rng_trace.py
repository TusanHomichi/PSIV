#!/usr/bin/env python3
"""Check an RNG trace from `psiv_oracle --rng-trace` against its RAM log.

    python3 oracle/rng_trace.py check <trace.csv> <ram-log.csv>

The trace has one row per call of UpdateRNGSeed2 (ps4.asm:86097):

    frame,call_index_in_frame,pc,hv,frame_count,seed_before,roll,seed_after

and the RAM log of the same run has the seed and the frame counter the host
sampled around every frame (`--groups rng`). This command re-derives the
trace's own columns and chains every frame's calls, then insists that

  * each row is the cartridge's arithmetic:
    roll = (hv + frame_count - seed_high) & $FFFF, and
    seed_after = ror16(seed_before's high word) with the low word carried;
  * each frame's calls chain, i.e. the next call starts from the word the
    previous one left;
  * a frame's first call starts from the seed the frame opened with, or from
    that seed after one UpdateRNGSeed - the two possibilities are the VBlank
    handler's block having run or not, which the log's Main_Frame_Count step
    says independently;
  * a frame's last seed_after is the seed the frame ended with, and every
    row's frame_count is the log's Main_Frame_Count for that frame.

What it proves: the rows are the rolls the cartridge computed, and the calls
the trace holds account for every seed change the log records between the
first and last call of a frame - a missing or extra call, a wrong seed, or a
multiply in the wrong place would break the chain.

What it does not prove: that the emulation itself matches the cartridge (that is
oracle/verify.sh's job), nor anything about frames the log does not cover; those
are counted and reported as skipped. The first mismatch is printed and the exit
status is 1, so a run whose trace cannot be replayed cannot pass silently.
"""
import argparse
import csv
import sys

try:  # imported as oracle.rng_trace, or run as oracle/rng_trace.py
    from oracle.checks import update_rng_seed
except ImportError:  # pragma: no cover - depends on how this file is invoked
    from checks import update_rng_seed

M16 = 0xFFFF
M32 = 0xFFFFFFFF

TRACE_COLUMNS = ("frame", "call_index_in_frame", "pc", "hv", "frame_count",
                 "seed_before", "roll", "seed_after")
LOG_COLUMNS = ("frame", "rng_seed", "main_frame_count")


class Mismatch(Exception):
    """The first thing that does not add up, with where it was seen."""


def load_rows(path):
    """A trace or RAM log as row dicts, minus its provenance comment lines."""
    with open(path) as handle:
        return list(csv.DictReader(
            line for line in handle if not line.startswith("#")))


def by_frame(rows):
    return {int(row["frame"]): row for row in rows}


def rotate_seed(seed):
    """`ror (RNG_Seed).w`: the high word rotates, the low word is carried."""
    hi, lo = (seed >> 16) & M16, seed & M16
    return ((((hi >> 1) | ((hi & 1) << 15)) & M16) << 16) | lo


def roll_for(hv, frame_count, seed):
    """UpdateRNGSeed2's roll (ps4.asm:86097, ROM $04239E):

        move.w  $8(a5), d0              ; the VDP HV counter
        add.w   (Main_Frame_Count).w, d0
        sub.w   (RNG_Seed).w, d0        ; d0 = the roll

    `(RNG_Seed).w` is an absolute-short word operand at $FFFFEF0C, where
    `RNG_Seed` (ps4.constants.asm:2328) is a longword, so the 68000 subtracts
    the word at $FFFFEF0C/$FFFFEF0D - the longword's *high* half, and the word
    `ror (RNG_Seed).w` (ROM $0423AA) rotates next. Subtracting the low half at
    $FFFFEF0E gives a per-frame-constant shift of this, not a roll
    (docs/oracle/BATTLE_ORACLE_REPLAY.md settles it against the cartridge).

    The host computes the same thing once, in `rng_trace_roll`
    (oracle/host/rng_trace.h); tests/test_oracle_rng_trace.py builds a program
    against that header and compares it with this function, so this file
    cannot quietly re-derive a convention of its own - which is how the
    low-half roll went unnoticed when both sides made the same mistake."""
    return (hv + frame_count - ((seed >> 16) & M16)) & M16


def roll_arithmetic(row):
    """The row's roll and seed_after, recomputed from its own other columns."""
    hv = int(row["hv"], 16)
    frame_count = int(row["frame_count"])
    seed = int(row["seed_before"], 16)
    return roll_for(hv, frame_count, seed), rotate_seed(seed)


def group_frames(rows):
    """{frame: [rows]} in file order, insisting the file groups frames."""
    frames = []
    for row in rows:
        frame = int(row["frame"])
        if not frames or frames[-1] != frame:
            if frame in frames:
                raise Mismatch(f"frame {frame} rows are not written together")
            frames.append(frame)
    return {f: [r for r in rows if int(r["frame"]) == f] for f in frames}


def check_frame(frame, rows, log, state):
    """Chain one frame's calls and compare the ends with the RAM log."""
    previous = log.get(frame - 1)
    current = log.get(frame)
    if previous is None or current is None:
        state["skipped"] += 1
        return

    opened = int(previous["rng_seed"], 16)
    closed = int(current["rng_seed"], 16)
    frame_count = int(current["main_frame_count"])

    for index, row in enumerate(rows):
        if int(row["call_index_in_frame"]) != index:
            raise Mismatch(f"f{frame}: call {index} is numbered "
                           f"{row['call_index_in_frame']}")
        if int(row["frame_count"]) != frame_count:
            raise Mismatch(f"f{frame} call {index}: frame_count "
                           f"{row['frame_count']} is not the log's "
                           f"Main_Frame_Count {frame_count}")
        seed = int(row["seed_before"], 16)
        want_roll, want_after = roll_arithmetic(row)
        if int(row["roll"], 16) != want_roll:
            raise Mismatch(f"f{frame} call {index}: roll {row['roll']} is not "
                           f"(hv + frame_count - seed_high) & $FFFF = "
                           f"{want_roll:04X}")
        if int(row["seed_after"], 16) != want_after:
            raise Mismatch(f"f{frame} call {index}: seed_after "
                           f"{row['seed_after']} is not ror16 of seed_before "
                           f"{row['seed_before']} = {want_after:08X}")
        if index and int(rows[index - 1]["seed_after"], 16) != seed:
            raise Mismatch(f"f{frame} call {index}: starts from "
                           f"{seed:08X}, the previous call left "
                           f"{int(rows[index - 1]['seed_after'], 16):08X}")

    first = int(rows[0]["seed_before"], 16)
    possibilities = [("kept", opened), ("multiplied", update_rng_seed(opened))]
    anchors = [name for name, seed in possibilities if seed == first]
    if not anchors:
        raise Mismatch(f"f{frame} call 0: starts from {first:08X}, neither the "
                       f"frame's seed {opened:08X} nor it after UpdateRNGSeed "
                       f"{update_rng_seed(opened):08X}")
    # The VBlank handler bumps Main_Frame_Count and the seed in one block, so
    # the anchor and the log's counter step say the same thing - unless the
    # multiply leaves the seed where it was, which is why both are allowed
    # when the anchors coincide.
    stepped = frame_count == (int(previous["main_frame_count"]) + 1) & M16
    if len(anchors) == 1 and (anchors[0] == "multiplied") != stepped:
        raise Mismatch(f"f{frame}: the calls start from a "
                       f"{'multiplied' if anchors[0] == 'multiplied' else 'kept'}"
                       f" seed, but Main_Frame_Count "
                       f"{'stepped' if stepped else 'did not step'}")
    if len(anchors) == 2:
        state["ambiguous"] += 1

    last = int(rows[-1]["seed_after"], 16)
    if last != closed:
        raise Mismatch(f"f{frame}: {len(rows)} call(s) chain to {last:08X}, "
                       f"the log's rng_seed for the frame is {closed:08X}")
    state["frames"] += 1
    state["rolls"] += len(rows)


def check(trace_path, log_path, out=sys.stdout):
    """Returns an exit status; prints what it checked and the first mismatch."""
    rows = load_rows(trace_path)
    if not rows:
        print(f"  FAIL  {trace_path} has no rolls to check", file=out)
        return 1
    missing = [c for c in TRACE_COLUMNS if c not in rows[0]]
    if missing:
        print(f"  FAIL  {trace_path} has no {', '.join(missing)} column",
              file=out)
        return 2
    log_rows = load_rows(log_path)
    if not log_rows:
        print(f"  FAIL  {log_path} has no frames", file=out)
        return 1
    missing = [c for c in LOG_COLUMNS if c not in log_rows[0]]
    if missing:
        print(f"  FAIL  {log_path} has no {', '.join(missing)} column "
              f"(run the oracle with --groups rng)", file=out)
        return 2
    log = by_frame(log_rows)

    state = {"frames": 0, "rolls": 0, "skipped": 0, "ambiguous": 0}
    try:
        frames = group_frames(rows)
    except Mismatch as mismatch:
        print(f"  FAIL  {mismatch}", file=out)
        return 1
    print(f"  ok    {trace_path}: {len(rows)} roll(s) in {len(frames)} "
          f"frame(s), frames {min(frames)}-{max(frames)}", file=out)
    try:
        for frame in sorted(frames):
            check_frame(frame, frames[frame], log, state)
    except Mismatch as mismatch:
        print(f"  FAIL  {mismatch}", file=out)
        return 1

    pcs = sorted({row["pc"] for row in rows})
    print(f"  ok    every row is the cartridge's arithmetic "
          f"(roll and ror16 of the seed)", file=out)
    print(f"  ok    {state['frames']} frame(s) chain to the log's rng_seed "
          f"exactly, {state['rolls']} roll(s) in total", file=out)
    print(f"  ok    HV reads come from {'/'.join('$' + pc for pc in pcs)}",
          file=out)
    if state["skipped"]:
        print(f"  note  {state['skipped']} frame(s) skipped: the log has no "
              f"seed for them or for the frame before them", file=out)
    if state["ambiguous"]:
        print(f"  note  {state['ambiguous']} frame(s) where the frame's seed "
              f"and its UpdateRNGSeed are the same", file=out)
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Check a PSIV RNG trace against its RAM log.")
    subparsers = parser.add_subparsers(dest="command", required=True)
    checker = subparsers.add_parser(
        "check", help="chain the trace's rolls against the RAM log")
    checker.add_argument("trace", help="CSV from psiv_oracle --rng-trace")
    checker.add_argument("log", help="CSV from the same run (--groups rng)")
    arguments = parser.parse_args(argv)
    return check(arguments.trace, arguments.log)


if __name__ == "__main__":
    sys.exit(main())
