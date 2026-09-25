#!/usr/bin/env python3
"""Collect same-matchup damage samples from the basement fight.

The ratified RNG design swaps the HV-counter term for a surrogate, and the risk
is that the surrogate pins damage into a low-variance regime. This measures the
real spread: replay tape 07's battle with the command timing shifted by a frame
at a time, which changes the seed path without changing the formation, and
record every attack's damage by attacker and defender.

Shifting the *fight* presses rather than the patrol is deliberate: the
encounter has already fired by then, so the formation stays 2x ZoranBult and
every run is the same matchup.
"""
import argparse
import csv
import pathlib
import subprocess
from collections import defaultdict

ROOT = pathlib.Path(__file__).resolve().parents[2]
O = ROOT / "oracle"
BIN = O / "bin" / "psiv_oracle"
CORE = O / "core" / "genesis_plus_gx_libretro.so"
ROM = ROOT / "Phantasy Star IV (USA).md"
MAP = O / "ram_map.tsv"

# Fighter index (from $FFFF4142) to a readable attacker name. 1-5 are the
# party in slot order, 6-9 the enemies.
PARTY = {1: 'Alys', 2: 'Chaz', 3: 'Hahn'}


def s16(v):
    return v - 65536 if v > 32767 else v


def split_tape7():
    """Tape 07, cut at its battle_start mark.

    Everything before the mark walks to the encounter; everything after is the
    fight. Inserting frames at the seam shifts the command timing without
    touching the patrol, so the encounter that fires is the same one.
    """
    text = (O / 'tapes' / '07_first_battle.tape').read_text()
    head, tail = text.split('120 . battle_start')
    return head, tail


def run(shift, tag, head, tail):
    tape = O / 'tmp_census.tape'
    tape.write_text(f"{head}{120 + shift} . battle_start{tail}")
    out = O / 'logs' / 'census.csv'
    subprocess.run([str(BIN), '--core', str(CORE), '--rom', str(ROM),
                    '--map', str(MAP), '--tape', str(tape),
                    '--groups', 'core,battle,bhit,enemy,chars', '--out', str(out)],
                   check=True, capture_output=True)
    rows = list(csv.DictReader([l for l in open(out) if not l.startswith('#')]))
    byf = {int(r['frame']): r for r in rows}
    inb = [r for r in rows if r['game_mode'] in ('0010', '0014')]
    if not inb:
        return []
    bf, bl = int(inb[0]['frame']), int(inb[-1]['frame'])
    # Enemy ids for this battle, read once the formation has loaded.
    mid = byf[min(bf + 400, bl)]
    eid = {e: mid.get(f'e{e}_id') for e in (1, 2, 3, 4)}

    tracked = [(f'e{e}_hp', f"enemy{e}", f"id{eid[e]}") for e in (1, 2, 3, 4)] + \
              [(f'{c}_hp', c, c) for c in ('chaz', 'alys', 'hahn')]
    prev = {k: s16(int(byf[bf][k])) for k, _, _ in tracked if k in byf[bf]}
    samples = []
    for f in range(bf, bl + 1):
        r = byf.get(f)
        if not r:
            continue
        actor = int(r['battle_actor'], 16) if r.get('battle_actor') else 0
        for key, slot, who in tracked:
            if key not in r:
                continue
            v = s16(int(r[key]))
            p = prev.get(key)
            if p is not None and v != p and abs(v - p) < 500 and v < p:
                crit = any(r[f'hit_{i:02d}'] == '01' for i in range(9))
                attacker = PARTY.get(actor, f'enemy{actor - 5}' if 6 <= actor <= 9
                                     else f'#{actor}')
                samples.append({'shift': shift, 'frame': f,
                                'attacker': attacker, 'defender': who,
                                'damage': p - v, 'crit': crit})
            prev[key] = v
    return samples


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--runs', type=int, default=20)
    a = ap.parse_args()

    head, tail = split_tape7()
    all_samples = []
    for shift in range(a.runs):
        s = run(shift, str(shift), head, tail)
        all_samples += s
        print(f"  shift {shift:>3}: {len(s)} damage events", flush=True)

    print(f"\n{len(all_samples)} damage samples over {a.runs} runs\n")
    groups = defaultdict(list)
    for s in all_samples:
        groups[(s['attacker'], s['defender'])].append(s)

    print(f"{'matchup':<28} {'n':>4} {'min':>4} {'max':>4} {'mean':>6} "
          f"{'crits':>6}  histogram")
    for (atk, dfn), rows_ in sorted(groups.items(),
                                    key=lambda kv: -len(kv[1])):
        dmg = [r['damage'] for r in rows_]
        crits = sum(1 for r in rows_ if r['crit'])
        hist = defaultdict(int)
        for d in dmg:
            hist[d] += 1
        h = " ".join(f"{k}:{v}" for k, v in sorted(hist.items()))
        print(f"{atk + ' -> ' + dfn:<28} {len(dmg):>4} {min(dmg):>4} "
              f"{max(dmg):>4} {sum(dmg) / len(dmg):>6.2f} {crits:>6}  {h}")

    out = O / 'logs' / 'damage_census.csv'
    with open(out, 'w', newline='') as fh:
        w = csv.DictWriter(fh, fieldnames=['shift', 'frame', 'attacker',
                                           'defender', 'damage', 'crit'])
        w.writeheader()
        w.writerows(all_samples)
    print(f"\nwrote {out}")


if __name__ == '__main__':
    main()
