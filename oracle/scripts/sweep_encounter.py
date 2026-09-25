#!/usr/bin/env python3
"""Sweep a tape's idle wait to find a value that dodges the random battles.

RunRandomBattles (ps4.asm:116840) rolls RNG_Seed & $1F == 0 on every completed
step after the first ten on a map, so a long walk is 1/32 per step of losing
the tape. Idling does not consume a step but does advance the seed, so varying
one wait re-rolls every check downstream without changing the route at all.

Usage: python3 -m oracle.scripts.sweep_encounter <tape> <mark> <lo> <hi> <step> [--jobs N]
where <mark> names the `N . <mark>` line whose frame count is swept.
"""
import argparse
import pathlib
import re
import subprocess
import sys

from oracle.checks import load

ROOT = pathlib.Path(__file__).resolve().parents[2]
ORACLE = ROOT / "oracle"


def variant(text, mark, frames):
    pat = re.compile(rf'^\d+ \. {re.escape(mark)}$', re.M)
    new, n = pat.subn(f'{frames} . {mark}', text)
    if n != 1:
        sys.exit(f'mark {mark!r} matched {n} lines, expected 1')
    return new


def run(tape, out):
    cmd = [str(ORACLE / 'bin' / 'psiv_oracle'),
           '--core', str(ORACLE / 'core' / 'genesis_plus_gx_libretro.so'),
           '--rom', str(ROOT / 'Phantasy Star IV (USA).md'),
           '--map', str(ORACLE / 'ram_map.tsv'),
           '--tape', str(tape), '--out', str(out),
           '--groups', 'core,pos']
    return subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def verdict(csv, want_cell):
    battles = []
    rows = load(csv)
    for row in rows:
        if row['game_mode'] == '0014':
            f = int(row['frame'])
            if not battles or f - battles[-1] > 1:
                battles.append(f)
    last = rows[-1]
    cell = (int(last['c1_x_px']) // 16, int(last['c1_y_px']) // 16)
    return battles, cell, cell == want_cell


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('tape')
    ap.add_argument('mark')
    ap.add_argument('lo', type=int)
    ap.add_argument('hi', type=int)
    ap.add_argument('step', type=int)
    ap.add_argument('--jobs', type=int, default=6)
    ap.add_argument('--cell', default='41,8')
    args = ap.parse_args()

    want = tuple(int(v) for v in args.cell.split(','))
    text = pathlib.Path(args.tape).read_text()
    values = list(range(args.lo, args.hi + 1, args.step))
    tmp = ORACLE / 'tmp_sweep'
    tmp.mkdir(exist_ok=True)

    for i in range(0, len(values), args.jobs):
        batch = values[i:i + args.jobs]
        procs = []
        for v in batch:
            tp = tmp / f'v{v}.tape'
            tp.write_text(variant(text, args.mark, v))
            procs.append((v, tp, tmp / f'v{v}.csv', run(tp, tmp / f'v{v}.csv')))
        for v, _, csv, p in procs:
            p.wait()
            battles, cell, hit = verdict(csv, want)
            print(f'{args.mark}={v:5d}  battles={battles}  end={cell}  '
                  f'{"CLEAN" if hit and not battles else ""}', flush=True)


if __name__ == '__main__':
    main()
