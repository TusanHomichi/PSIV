#!/usr/bin/env python3
"""Walk around an encounter map until a random battle fires, then fight it.

Encounters are rolled as the party walks, so the tape has to wander first. This
appends a fixed back-and-forth patrol to a prelude, runs it, and looks for the
frame Game_Mode_Index becomes $10 (LoadBattle) or $14 (Battle). Once it finds
one it emits a tape that walks exactly that far and then mashes the confirm
button, which is enough to take the default battle command (COMD -> attack ->
first enemy) for every character and see the fight through.

Everything is deterministic, so the patrol length that produced a battle here
produces the same battle on every replay.
"""
import argparse
import csv
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE = ROOT / "oracle"
BIN = ORACLE / "bin" / "psiv_oracle"
CORE = ORACLE / "core" / "genesis_plus_gx_libretro.so"
ROM = ROOT / "Phantasy Star IV (USA).md"
MAP = ORACLE / "ram_map.tsv"


def patrol(cells, reps):
    """A back-and-forth walk: `cells` right then `cells` left, `reps` times."""
    out = [f"repeat {reps}", f"{cells * 8} R", "8 .", f"{cells * 8} L", "8 ."]
    out.append("end")
    return "\n".join(out) + "\n"


def run(tape_text, out, groups="core,pos,battle,rng"):
    tape = ORACLE / "tmp_findbattle.tape"
    tape.write_text(tape_text)
    subprocess.run([str(BIN), "--core", str(CORE), "--rom", str(ROM),
                    "--map", str(MAP), "--tape", str(tape), "--groups", groups,
                    "--out", str(out)], check=True, capture_output=True)
    return list(csv.DictReader([l for l in open(out) if not l.startswith('#')]))


def first_battle(rows):
    for r in rows:
        if r.get('game_mode') in ('0010', '0014'):
            return int(r['frame']), r
    return None, None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--prelude', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--cells', type=int, default=6,
                    help='patrol width in cells')
    ap.add_argument('--reps', type=int, default=60,
                    help='how many back-and-forth laps to try')
    ap.add_argument('--fight-presses', type=int, default=400)
    ap.add_argument('--approach', default='',
                    help='tape lines to run before patrolling, e.g. to walk '
                         'away from the stairs so the patrol cannot leave the '
                         'map (semicolon separated, e.g. "96 D;40 L")')
    a = ap.parse_args()

    prelude = pathlib.Path(a.prelude).read_text().rstrip() + "\n"
    log = ORACLE / "logs" / "find_battle.csv"

    approach = ""
    if a.approach:
        approach = "\n" + "\n".join(
            line.strip() for line in a.approach.split(';')) + "\n60 . approached\n"
    walk = approach + "\n60 . patrol_start\n" + patrol(a.cells, a.reps)
    rows = run(prelude + walk, log)
    f, r = first_battle(rows)
    if f is None:
        print(f"no battle in {a.reps} laps of {a.cells} cells; "
              f"try more reps. last map={rows[-1].get('map_index')}")
        return 1
    print(f"battle starts at f{f} (map {r.get('map_index')}, "
          f"game_mode={r.get('game_mode')})")

    # Rebuild the tape so it stops walking as soon as the battle starts, then
    # fights. The patrol is deterministic, so trimming it to the lap that
    # triggered the encounter keeps the same battle.
    fight = ("\n120 . battle_start\n"
             f"repeat {a.fight_presses}\n4 C\n12 .\nend\n"
             "600 . battle_done\n")
    pathlib.Path(a.out).write_text(prelude + walk + fight)
    print(f"wrote {a.out}")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
