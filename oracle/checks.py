"""Shared helpers for verify.sh's check blocks.

verify.sh runs two lanes (see its header), each an independent python block, and
both need the same log loading, the same pass/fail reporting, and the same
transcription of the cartridge's RNG. Keeping those here means the two lanes
cannot drift apart in how they read a log or count a seed step.
"""
import csv
import sys

M32 = 0xFFFFFFFF


def load(path):
    """A log as a list of row dicts, minus the provenance comment lines."""
    return list(csv.DictReader(
        [l for l in open(path) if not l.startswith('#')]))


def by_frame(rows):
    return {int(r['frame']): r for r in rows}


def ok(msg):
    print(f"  ok    {msg}")


def bad(msg):
    print(f"  FAIL  {msg}", file=sys.stderr)
    sys.exit(1)


def update_rng_seed(seed):
    """UpdateRNGSeed, ps4.asm:86066-86092. See analyze_rng.py for the notes."""
    d1 = seed
    if (d1 & 0xFFFF) == 0:
        d1 = 0x2A6D365B
    d1 = (d1 * 41) & M32
    lo, hi = d1 & 0xFFFF, (d1 >> 16) & 0xFFFF
    return ((((lo + hi) & 0xFFFF) << 16) | lo) & M32


def rng_calls(a, b, max_calls=32):
    """How many UpdateRNGSeed applications turn seed a into seed b.

    Zero is a real answer: on frames where the vblank handler does not run the
    seed does not move. None means the transcription does not explain the step.
    """
    if a == b:
        return 0
    x = a
    for i in range(1, max_calls + 1):
        x = update_rng_seed(x)
        if x == b:
            return i
    return None
