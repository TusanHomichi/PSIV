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
PY

echo
echo "all checks passed"
