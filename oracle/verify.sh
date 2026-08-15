#!/usr/bin/env bash
# Verification run for the PSIV emulator oracle.
#
# Everything the harness claims is re-derived here from the cartridge, so a
# green run means the numbers in oracle/README.md are still true. Checks are
# ordered cheapest-first; any failure aborts.
set -euo pipefail

cd "$(dirname "$(realpath "$0")")/.."
ORACLE=oracle
CORE=$ORACLE/core/genesis_plus_gx_libretro.so
ROM="Phantasy Star IV (USA).md"
ROM_SHA=511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
BIN=$ORACLE/bin/psiv_oracle
OUT=$ORACLE/logs
pass() { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1" >&2; exit 1; }

echo "== inputs =="
[ -f "$ROM" ] || fail "ROM not found: $ROM"
got=$(sha256sum "$ROM" | cut -d' ' -f1)
[ "$got" = "$ROM_SHA" ] || fail "ROM sha256 is $got, expected $ROM_SHA"
pass "ROM sha256 matches the dump the RAM map was written against"

[ -f "$CORE" ] || fail "core missing: $CORE (run oracle/build_core.sh)"
pass "Genesis Plus GX core present"

echo "== build =="
mkdir -p "$ORACLE/bin" "$OUT"
gcc -O2 -Wall -Wextra -o "$BIN" "$ORACLE/host/psiv_oracle.c" -ldl
pass "host builds clean"

# The C host reads ram_map.tsv, but ram_map.json is the source of truth. If the
# generated file were allowed to drift, the logs would silently describe the
# wrong addresses, so regenerate and demand no diff.
cp "$ORACLE/ram_map.tsv" "$ORACLE/ram_map.tsv.check"
python3 "$ORACLE/gen_ram_map.py" >/dev/null
if ! diff -q "$ORACLE/ram_map.tsv" "$ORACLE/ram_map.tsv.check" >/dev/null; then
	mv "$ORACLE/ram_map.tsv.check" "$ORACLE/ram_map.tsv"
	fail "ram_map.tsv is stale; regenerate it with gen_ram_map.py and re-run"
fi
rm -f "$ORACLE/ram_map.tsv.check"
pass "ram_map.tsv matches ram_map.json"

run() { "$BIN" --core "$CORE" --rom "$ROM" --map "$ORACLE/ram_map.tsv" \
                --tape "$1" --out "$2" "${@:3}"; }

echo "== determinism =="
# The whole point of the harness: same tape in, same log out, bit for bit.
run "$ORACLE/tapes/01_newgame_to_first_control.tape" "$OUT/verify_run1.csv"
run "$ORACLE/tapes/01_newgame_to_first_control.tape" "$OUT/verify_run2.csv"
cmp -s "$OUT/verify_run1.csv" "$OUT/verify_run2.csv" \
	|| fail "two runs of tape A differ - the harness is not deterministic"
pass "tape A replays byte-identically ($(sha256sum <"$OUT/verify_run1.csv" | cut -c1-16)...)"

run "$ORACLE/tapes/02_walk_timing.tape" "$OUT/verify_walk.csv"
run "$ORACLE/tapes/03_npc_talk.tape" "$OUT/verify_talk.csv"
run "$ORACLE/tapes/08_rng_characterization.tape" "$OUT/verify_rng.csv"
run "$ORACLE/tapes/04_alys_joins.tape" "$OUT/verify_alys.csv"
run "$ORACLE/tapes/07_first_battle.tape" "$OUT/verify_battle.csv"

echo "== findings =="
python3 - "$OUT" <<'PY'
import csv, sys, pathlib
OUT = pathlib.Path(sys.argv[1])

def load(p):
    return list(csv.DictReader([l for l in open(p) if not l.startswith('#')]))

def ok(m):   print(f"  ok    {m}")
def bad(m):  print(f"  FAIL  {m}", file=sys.stderr); sys.exit(1)

rows = load(OUT/'verify_run1.csv')
byf  = {int(r['frame']): r for r in rows}

# Byte-order proof. TitleRoutine_PickOption writes Field_Map_Index =
# MapID_PiataAcademy ($11, ps4.constants.asm:1084) before handing off. If the
# host's word accessor were byte-swapped this would read $1100, so a correct
# $0011 here proves the 16-bit path against a value the cartridge chose.
m = byf[640]['map_index']
ok(f"word accessor: Field_Map_Index reads {m} at f640 (= MapID_PiataAcademy)") \
    if m == '0011' else bad(f"Field_Map_Index reads {m} at f640, expected 0011")

# Byte-order proof for the 8-bit path. Joypad_Held is the low byte of the word
# at $FFFFEF02 and Joypad_Pressed the high byte; START is bit 7 ($80). Reading
# $80 out of joy_held on the frame we assert START proves ram[A^1] indexing,
# because the swapped reading would put the value in the neighbouring field.
h = byf[401]['joy_held']
ok(f"byte accessor: Joypad_Held reads {h} on the first START frame (bit7=START)") \
    if h == '80' else bad(f"Joypad_Held reads {h} at f401, expected 80")

# First controllable frame of a new game.
n = len(rows)
first = None
for i, r in enumerate(rows):
    if int(r['frame']) < 6230:
        continue
    if r['game_mode'] == '000C' and r['game_mode_routine'] == '0000' and \
       all(rows[j]['game_mode_routine'] == '0000'
           for j in range(i, min(i + 120, n))):
        first = r
        break
if not first:
    bad("tape A never reaches a sustained FieldRoutine_Controls frame")
exp = {'frame': '6456', 'map_index': '0013', 'c1_x_px': '768',
       'c1_y_px': '288', 'c1_facing': '0', 'party_slots': '00FFFFFF'}
for k, v in exp.items():
    if first[k] != v:
        bad(f"first control {k} is {first[k]}, expected {v}")
ok("first control at f6456: map $13, (768,288) = cell (48,18), facing DOWN, "
   "party = Chaz alone")

# Walk timing: every leg of tape B must come out at 8 frames per 16px cell.
rows = load(OUT/'verify_walk.csv')
byf  = {int(r['frame']): r for r in rows}
mk   = {r['mark']: int(r['frame']) for r in rows if r['mark'].startswith('walk_')}
for leg, ax in [('walk_down', 'c1_y_px'), ('walk_right', 'c1_x_px'),
                ('walk_up', 'c1_y_px'), ('walk_left', 'c1_x_px')]:
    s = mk[leg]
    seg = [byf[f] for f in range(s - 1, s + 200) if f in byf]
    vals = [(int(r['frame']), int(r[ax])) for r in seg]
    start = vals[0][1]
    stop = next((vals[i][0] for i in range(len(vals) - 1, 0, -1)
                 if vals[i][1] != vals[i - 1][1]), None)
    moved = abs(vals[-1][1] - start)
    if not moved or not stop:
        bad(f"{leg}: no movement recorded")
    fpc = (stop - s + 1) / (moved / 16)
    if abs(fpc - 8.0) > 1e-9:
        bad(f"{leg}: {fpc} frames/cell, expected exactly 8")
ok("walk timing: 8.00 frames per 16px cell on all four legs of tape B")

# Talking: Speak while facing an adjacent NPC must open a dialogue window.
rows = load(OUT/'verify_talk.csv')
byf  = {int(r['frame']): r for r in rows}
sp   = next(int(r['frame']) for r in rows if r['mark'] == 'speak')
seg  = [byf[f] for f in range(sp, sp + 300) if f in byf]
opened = [r for r in seg if r['windows_opened'] != '0']
if not opened:
    bad("Speak while facing the NPC did not open a dialogue")
o = int(opened[0]['frame'])
ok(f"talk: Speak at f{sp} opens the dialogue at f{o} ({o - sp} frames later)")
# It must then stay open until a press closes it, never time out on its own.
closed = [r for r in seg if int(r['frame']) > o and r['windows_opened'] == '0']
if closed:
    bad(f"dialogue closed by itself at f{closed[0]['frame']} with no press")
ok("dialogue stays open for 300 frames with no press (does not auto-close)")

# RNG. The transcription of UpdateRNGSeed must explain every frame transition
# in the log as an exact number of calls; a single unexplained frame means the
# algorithm as written in analyze_rng.py is not what the cartridge runs.
import importlib.util
spec = importlib.util.spec_from_file_location(
    "analyze_rng", pathlib.Path(__file__).parent / "analyze_rng.py")
try:
    rows = load(OUT/'verify_rng.csv')
except FileNotFoundError:
    bad("verify_rng.csv missing")
byf = {int(r['frame']): r for r in rows}

M32 = 0xFFFFFFFF
def upd(seed):
    d1 = seed
    if (d1 & 0xFFFF) == 0:
        d1 = 0x2A6D365B
    d1 = (d1 * 41) & M32
    lo, hi = d1 & 0xFFFF, (d1 >> 16) & 0xFFFF
    return ((((lo + hi) & 0xFFFF) << 16) | lo) & M32
def calls(a, b, k=32):
    if a == b:
        return 0
    x = a
    for i in range(1, k + 1):
        x = upd(x)
        if x == b:
            return i
    return None

frames = sorted(byf)
seed = lambda f: int(byf[f]['rng_seed'], 16)
counts = {}
for f in frames[:-1]:
    if f + 1 not in byf:
        continue
    counts[f] = calls(seed(f), seed(f + 1))
unexplained = [f for f, c in counts.items() if c is None]
if unexplained:
    bad(f"{len(unexplained)} frames not explained by UpdateRNGSeed "
        f"(first at f{unexplained[0]})")
ok(f"RNG: UpdateRNGSeed explains all {len(counts)} frame transitions exactly")

# Field control must run the generator twice a frame; the camp menu once.
def phase_counts(name, span):
    s = next(int(r['frame']) for r in rows if r['mark'] == name)
    return [counts[f] for f in range(s, s + span) if f in counts]
fi = phase_counts('field_idle', 300)
mi = phase_counts('menu_idle', 300)
wa = phase_counts('walk_a', 96)
if set(fi) - {2, 3}:
    bad(f"field idle RNG calls/frame were {sorted(set(fi))}, expected 2 (occasionally 3)")
if set(wa) - {2, 3}:
    bad(f"field walking RNG calls/frame were {sorted(set(wa))}, expected 2 (occasionally 3)")
if set(mi) != {1}:
    bad(f"camp menu RNG calls/frame were {sorted(set(mi))}, expected exactly 1")
ok(f"RNG per frame: field idle {fi.count(2)}x2/{fi.count(3)}x3, "
   f"walking {wa.count(2)}x2/{wa.count(3)}x3, camp menu {len(mi)}x1")
ok("RNG: walking does not advance the seed beyond the field-mode baseline")

# Alys joining is the first story gate and the first party change, so it is
# worth pinning: slot 1 becomes Alys (id 1) and Chaz (id 0) moves to slot 2.
# Only after first control: the intro cutscene stages the party as 0100FFFF
# for its own reasons well before Alys actually joins, so an unqualified search
# finds a frame in the opening movie instead of the event.
rows = [r for r in load(OUT/'verify_alys.csv') if int(r['frame']) >= 6456]
before = [r for r in rows if r['party_slots'] == '00FFFFFF']
after = [r for r in rows if r['party_slots'] == '0100FFFF']
if not before or not after:
    bad("tape 04 did not show the party going from 00FFFFFF to 0100FFFF")
j = int(after[0]['frame'])
if j != 7478:
    bad(f"Alys joins at f{j}, expected f7478")
if rows[-1]['party_slots'] != '0100FFFF':
    bad(f"party ended as {rows[-1]['party_slots']}, expected 0100FFFF")
ok(f"Alys joins at f{j}: Current_Party_Slots 00FFFFFF -> 0100FFFF "
   "(Alys slot 1, Chaz slot 2)")

# Battle ground truth. These numbers are what the damage-formula work is fitted
# against, so they are pinned rather than merely reported.
rows = load(OUT/'verify_battle.csv')
byf = {int(r['frame']): r for r in rows}
inb = [r for r in rows if r['game_mode'] in ('0010', '0014')]
if not inb:
    bad("tape 07 never entered a battle")
bf, bl = int(inb[0]['frame']), int(inb[-1]['frame'])
if bf != 24794:
    bad(f"battle starts at f{bf}, expected f24794")
ok(f"battle f{bf}-{bl} on map $15 (Academy Basement)")

mid = byf[min(bf + 240, bl)]
if mid['e1_id'] != '10' or mid['e1_hp'] != '25' or mid['e1_atk'] != '16' \
        or mid['e1_dfs'] != '2' or mid['e1_agi_bat'] != '6' \
        or mid['e1_str_bat'] != '18' or mid['e1_dex_bat'] != '8':
    bad(f"enemy 1 stats are not the ZoranBult record: id={mid['e1_id']} "
        f"hp={mid['e1_hp']} atk={mid['e1_atk']} dfs={mid['e1_dfs']} "
        f"agi={mid['e1_agi_bat']} str={mid['e1_str_bat']} dex={mid['e1_dex_bat']}")
ok("enemy RAM matches generated/enemies.json id 10 (ZoranBult): hp 25, "
   "atk 16, def 2, agi 6, str 18, dex 8")

# EXP: total accumulates 12 per kill, then divides by the living party count.
exp_totals = []
for f in range(bf, bl + 1):
    r = byf.get(f)
    if r and r['battle_exp_total'] not in [x for _, x in exp_totals[-1:]]:
        exp_totals.append((f, r['battle_exp_total']))
seen = [v for _, v in exp_totals]
if '12' not in seen or '24' not in seen:
    bad(f"battle_exp_total never reached 12 then 24; saw {seen[:8]}")
last_row = rows[-1]
gains = {c: int(last_row[f'{c}_exp']) - int(byf[bf][f'{c}_exp'])
         for c in ('chaz', 'alys', 'hahn')}
if set(gains.values()) != {8}:
    bad(f"exp gains were {gains}, expected 8 each (24 total / 3 living)")
money = int(last_row['current_money']) - int(byf[bf]['current_money'])
if money != 6:
    bad(f"meseta gain was {money}, expected 6 (2 x ZoranBult meseta_reward 3)")
ok("rewards: battle_exp_total 24 (2 x 12), 8 exp each to 3 living members, "
   "+6 meseta (2 x 3) - all matching enemies.json")
PY

echo
echo "all checks passed"
