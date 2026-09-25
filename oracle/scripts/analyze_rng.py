#!/usr/bin/env python3
"""Classify each frame of an oracle log by how many RNG calls it made.

PSIV keeps one seed at $FFFFEF0C and updates it with UpdateRNGSeed
(ps4.asm:86066), transcribed below. Predicting that function forward from a
frame's seed and counting how many applications land on the next frame's seed
gives the number of times the game called the generator during that frame.
That turns the log into a behavioural census of RNG consumers without needing
to instrument the emulator or read every caller in the disassembly.
"""
import argparse
import csv
from collections import Counter

M32 = 0xFFFFFFFF


def update_rng_seed(seed):
    """UpdateRNGSeed, ps4.asm:86066-86092.

    Substitutes a fixed constant when the low word is zero, multiplies the
    longword by 41, then stores (low+high of the product) in the high word and
    the product's low word in the low word.
    """
    d1 = seed
    if (d1 & 0xFFFF) == 0:
        d1 = 0x2A6D365B
    d1 = (d1 * 41) & M32
    lo = d1 & 0xFFFF
    hi = (d1 >> 16) & 0xFFFF
    return ((((lo + hi) & 0xFFFF) << 16) | lo) & M32


def update_rng_seed2(seed):
    """UpdateRNGSeed2, ps4.asm:86098-86102: `ror` on the WORD at RNG_Seed."""
    hi = (seed >> 16) & 0xFFFF
    hi = ((hi >> 1) | ((hi & 1) << 15)) & 0xFFFF
    return ((hi << 16) | (seed & 0xFFFF)) & M32


def calls_between(a, b, max_calls=32):
    """How many UpdateRNGSeed applications turn a into b, or None.

    Zero is a real answer: on frames where the vblank handler does not run
    (fades and other busy-waits early in boot) the seed simply does not move.
    """
    if a == b:
        return 0
    x = a
    for k in range(1, max_calls + 1):
        x = update_rng_seed(x)
        if x == b:
            return k
    return None


def load(path):
    rows = list(csv.DictReader(
        [l for l in open(path) if not l.startswith('#')]))
    return rows, {int(r['frame']): r for r in rows}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('log')
    ap.add_argument('--phases', action='store_true',
                    help='break the census down by tape mark')
    a = ap.parse_args()

    rows, byf = load(a.log)
    seed = lambda f: int(byf[f]['rng_seed'], 16)
    frames = sorted(byf)

    census = Counter()
    for f in frames[:-1]:
        if f + 1 not in byf:
            continue
        census[calls_between(seed(f), seed(f + 1))] += 1
    total = sum(census.values())
    print(f"{a.log}: {total} frame transitions")
    for k, v in sorted(census.items(), key=lambda kv: (kv[0] is None, kv[0])):
        label = 'unexplained' if k is None else f'{k} UpdateRNGSeed call(s)'
        print(f"  {label:28s} {v:6d}  ({v / total:5.1%})")

    if a.phases:
        marks = [(r['mark'], int(r['frame'])) for r in rows if r['mark']]
        print("\nper-phase:")
        for i, (name, start) in enumerate(marks):
            end = marks[i + 1][1] if i + 1 < len(marks) else frames[-1]
            if end - start < 20:
                continue
            c = Counter(calls_between(seed(f), seed(f + 1))
                        for f in range(start, end) if f + 1 in byf)
            n = sum(c.values())
            desc = ", ".join(
                f"{'?' if k is None else k}x on {v}"
                for k, v in sorted(c.items(), key=lambda kv: (kv[0] is None,
                                                              kv[0])))
            print(f"  {name:22s} f{start:<6d} n={n:<5d} {desc}")


if __name__ == '__main__':
    main()
