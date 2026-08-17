#!/usr/bin/env bash
# Verification run for the PSIV emulator oracle.
#
# Everything the harness claims is re-derived here from the cartridge, so a
# green run means the numbers in oracle/README.md are still true.
#
# Two lanes:
#
#   ./oracle/verify.sh          fast lane - the structural checks and the short
#                               tapes. Well under five minutes; this is what to
#                               run routinely and before handing work over.
#   ./oracle/verify.sh --full   everything, adding the battle tapes and the
#                               level-up grind. Tape 10 alone is ~55k frames,
#                               so the full lane runs for tens of minutes.
#
# The split is by tape cost, not by importance: the fast lane still proves
# determinism, both byte-order accessors, walk timing, talk behaviour, the RNG
# transcription and the wander rule. The full lane adds everything that needs a
# battle, which is where the frames go.
#
# Checks are ordered cheapest-first; any failure aborts.
set -euo pipefail

LANE=fast
case "${1-}" in
	--full) LANE=full ;;
	"") ;;
	*) printf 'usage: %s [--full]\n' "$0" >&2; exit 2 ;;
esac

cd "$(dirname "$(realpath "$0")")/.."
ORACLE=oracle
CORE=$ORACLE/core/genesis_plus_gx_libretro.so
ROM="Phantasy Star IV (USA).md"
ROM_SHA=511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
BIN=$ORACLE/bin/psiv_oracle
OUT=$ORACLE/logs
pass() { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1" >&2; exit 1; }

printf '== inputs (%s lane) ==\n' "$LANE"
[ -f "$ROM" ] || fail "ROM not found: $ROM"
got=$(sha256sum "$ROM" | cut -d' ' -f1)
[ "$got" = "$ROM_SHA" ] || fail "ROM sha256 is $got, expected $ROM_SHA"
pass "ROM sha256 matches the dump the RAM map was written against"

[ -f "$CORE" ] || fail "core missing: $CORE (run oracle/build_core.sh)"
pass "Genesis Plus GX core present"

echo "== build =="
mkdir -p "$ORACLE/bin" "$OUT"
	gcc -O2 -Wall -Wextra -o "$BIN" \
		"$ORACLE/host/psiv_oracle.c" \
		"$ORACLE/host/frame_dump.c" \
		"$ORACLE/host/ram_patch.c" \
		"$ORACLE/host/ram_dump.c" \
		"$ORACLE/host/core_vdp.c" \
		"$ORACLE/host/state_dump.c" \
		"$ORACLE/host/tape.c" -ldl
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

# Each run logs only the groups its checks read: the full 656-column map makes
# 60MB logs per tape for no benefit here.
run() { "$BIN" --core "$CORE" --rom "$ROM" --map "$ORACLE/ram_map.tsv" \
                --tape "$1" --out "$2" "${@:3}"; }

echo "== determinism =="
# The whole point of the harness: same tape in, same log out, bit for bit.
run "$ORACLE/tapes/01_newgame_to_first_control.tape" "$OUT/verify_run1.csv" \
    --groups core,pos,input,party,collision
run "$ORACLE/tapes/01_newgame_to_first_control.tape" "$OUT/verify_run2.csv" \
    --groups core,pos,input,party,collision
cmp -s "$OUT/verify_run1.csv" "$OUT/verify_run2.csv" \
	|| fail "two runs of tape A differ - the harness is not deterministic"
pass "tape A replays byte-identically ($(sha256sum <"$OUT/verify_run1.csv" | cut -c1-16)...)"

# --- fast lane tapes -------------------------------------------------------
run "$ORACLE/tapes/02_walk_timing.tape" "$OUT/verify_walk.csv" \
    --groups core,pos,collision
run "$ORACLE/tapes/03_npc_talk.tape" "$OUT/verify_talk.csv" \
    --groups core,pos,window
run "$ORACLE/tapes/08_rng_characterization.tape" "$OUT/verify_rng.csv" \
    --groups core,rng
run "$ORACLE/tapes/08_rng_characterization.tape" "$OUT/verify_objects.csv" \
    --groups core,rng,objects
run "$ORACLE/tapes/04_alys_joins.tape" "$OUT/verify_alys.csv" \
    --groups core,party
run "$ORACLE/tapes/11_beside_press.tape" "$OUT/verify_beside.csv" \
    --groups core,pos,window,text
run "$ORACLE/tapes/16_page_boundary.tape" "$OUT/verify_page.csv" \
    --groups core,window,text
run "$ORACLE/tapes/17_flag_alias.tape" "$OUT/verify_flags.csv" \
    --groups core,flagbytes
run "$ORACLE/tapes/18_flag_round_trip.tape" "$OUT/verify_roundtrip.csv" \
    --groups core,objects,flagbytes
run "$ORACLE/tapes/19_chest_map_objects.tape" "$OUT/verify_chestmap.csv" \
    --groups core,pos,window,objects,flagbytes,chars
run "$ORACLE/tapes/20_chest_round_trip.tape" "$OUT/verify_chesttrip.csv" \
    --groups core,pos,window,objects,flagbytes,chars
run "$ORACLE/tapes/21_second_chest.tape" "$OUT/verify_chest2.csv" \
    --groups core,pos,window,objects,flagbytes,chars,party

echo "== findings =="
PYTHONPATH="$ORACLE" python3 - "$OUT" <<'PY'
import pathlib, sys
sys.path.insert(0, 'oracle')
from checks import load, by_frame, ok, bad, rng_calls
OUT = pathlib.Path(sys.argv[1])

rows = load(OUT/'verify_run1.csv')
byf  = by_frame(rows)

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
byf  = by_frame(rows)
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
byf  = by_frame(rows)
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
# algorithm as written in checks.py is not what the cartridge runs.
rows = load(OUT/'verify_rng.csv')
byf = by_frame(rows)
frames = sorted(byf)
seed = lambda f: int(byf[f]['rng_seed'], 16)
counts = {}
for f in frames[:-1]:
    if f + 1 not in byf:
        continue
    counts[f] = rng_calls(seed(f), seed(f + 1))
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

# NPC object columns: slot i must be the map's npc record i at spawn, and the
# third field-mode RNG call must be a wander decision.
import json as _json
rows = load(OUT/'verify_objects.csv')
byf = by_frame(rows)
rec = pathlib.Path('runtime-pack/maps/013_PiataAcademy_F1.json')
if rec.exists():
    npcs = _json.loads(rec.read_text())['npcs']
    spawn = next(r for r in rows
                 if r['map_index'] == '0013' and r['o00_id'] != '0000')
    for i, npc in enumerate(npcs):
        x, y = int(spawn[f'o{i:02d}_x_px']), int(spawn[f'o{i:02d}_y_px'])
        oid = int(spawn[f'o{i:02d}_id'], 16) & 0x7FFF
        if (x, y, oid) != (npc['x_pixels'], npc['y_pixels'], npc['object_id']):
            bad(f"object slot {i} is ({x},{y}) id {oid}, pack npcs[{i}] is "
                f"({npc['x_pixels']},{npc['y_pixels']}) id {npc['object_id']}")
    ok(f"object slot i == pack npcs[i] for all {len(npcs)} map $13 NPCs "
       f"at spawn (f{spawn['frame']})")
else:
    print("  skip  slot/pack mapping (runtime-pack not built)")

LO, HI = 6456, 7610
SL = range(32)
wander = [i for i in SL
          if any(byf[f][f'o{i:02d}_id'] != '0000' and byf[f][f'o{i:02d}_timer'] != '0'
                 for f in range(LO, HI) if f in byf)]
matched = total = 0
for f in range(LO, HI):
    if f + 1 not in byf:
        continue
    a = byf[f]
    expired = sum(1 for i in wander
                  if a[f'o{i:02d}_id'] != '0000' and a[f'o{i:02d}_timer'] == '0')
    total += 1
    if rng_calls(int(a['rng_seed'], 16),
                 int(byf[f + 1]['rng_seed'], 16)) == 2 + expired:
        matched += 1
if matched != total:
    bad(f"wander RNG rule held on only {matched}/{total} field-control frames")
ok(f"RNG in field control == 2 + (wandering objects with timer 0) on all "
   f"{total} frames; {len(wander)} of 8 map $13 objects wander")

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

# Beside press: identical text to the open-ground press means it took the
# "nothing here" path and never reached the adjacent NPC. The window flag
# cannot show this - Interaction_DoPlayerNothingMsg opens a window too.
rows = load(OUT/'verify_beside.csv')
byf = by_frame(rows)
me = next(int(r['frame']) for r in rows if r['mark'] == 'speak_empty')
mb = next(int(r['frame']) for r in rows if r['mark'] == 'speak_beside')
txt = lambda f: ' '.join(byf[f][f'tb{i:02d}'] for i in range(24))
if txt(me + 250) != txt(mb + 250):
    bad("the beside press drew different text from the open-ground press - "
        "it may have reached the NPC; re-check the staged geometry")
ok(f"beside press at f{mb} (leader facing {byf[mb]['c1_facing']}) draws the "
   "same text as the open-ground press: it hit 'nothing here', not the NPC")

# Page advance is edge-triggered: holding Speak accelerates the page being
# drawn but must NOT carry past its end. Two claims in one tape - the hold
# draws page 1 at one character per frame, and page 2 never starts.
rows = load(OUT/'verify_page.csv')
byf = by_frame(rows)
NT = 24
text = lambda f: tuple(byf[f][f'tb{i:02d}'] for i in range(NT))
# Only the dialogue matters: the prelude's title screens, intro cutscene and
# menus all write the text buffer too, and scanning the whole tape mixes them
# in with the page being measured.
speak = next(int(r['frame']) for r in rows if r['mark'] == 'speak')
draws = [f for f in range(speak + 1, speak + 1500)
         if f in byf and f - 1 in byf and text(f) != text(f - 1)]
if not draws:
    bad("tape 16 recorded no text draws at all")
gaps = [b - a for a, b in zip(draws, draws[1:])]
if sorted(set(gaps)) not in ([1], [1, 2], [1, 2, 3], [1, 3]):
    bad(f"tape 16 draw gaps were {sorted(set(gaps))}; holding Speak should "
        "draw one character per frame")
# Under the hold, page 1's 58 characters land within ~60 frames of the first
# draw. The tape then runs on for another ~1300 frames with the button still
# down, so any draw well past that window means the page advanced.
late = [f for f in draws if f > draws[0] + 300]
if late:
    bad(f"tape 16 drew text at f{late[0]}, long after page 1 finished - a held "
        "button appears to have advanced the page")
ok(f"page advance is edge-triggered: holding Speak drew page 1's {len(draws)} "
   f"characters at 1/frame (ending f{draws[-1]}) and never advanced to page 2")

# Where "temp" event flags land, on hardware. SOURCE_NOTES proves from ROM
# bytes that the clone's fifth bank at $F156 does not exist in retail and that
# "temp" flag writes go through the door at $F140 - the one the constants file
# labels Chest_Flags. That label is wrong about chests (tapes 20 and 21 below
# show chests writing $F120 instead) but right about the address these writes
# use. TempEveFlag_Xanafalgue = $13 fires in the Piata basement, and the shared
# bit rule puts id N at byte N>>3, mask 1 << (7 - (N&7)), so $13 must land as
# $10 at $FFFFF142.
rows = load(OUT/'verify_flags.csv')
sets = [r for a, r in zip(rows, rows[1:])
        if a['chestb2'] == '00' and r['chestb2'] == '10']
if not sets:
    bad("tape 17 never set $FFFFF142 bit 4 - TempEveFlag_Xanafalgue did not "
        "land in the $F140 bank")
setf = int(sets[0]['frame'])
dead = {k for k in ('tempb0', 'tempb1', 'tempb2', 'tempb3')
        if {r[k] for r in rows} != {'00'}}
if dead:
    bad(f"the $F156 bank was written ({sorted(dead)}); retail should never "
        "touch it")
# Rule out the innocent explanation: a chest opening during this tape. The
# basement's chests write $F123 (tape 21), so that is the byte to watch.
if {r['extb3'] for r in rows} != {'00'}:
    bad("$FFFFF123 changed - a basement chest was opened, so the byte-2 bit "
        "cannot be attributed to the temp-flag write")
if '0024' in {r['game_mode_routine'] for r in rows}:
    bad("FieldRoutine_ItemFound ran - a chest was opened during tape 17")
ok(f"TempEveFlag_Xanafalgue ($13) sets $FFFFF142 bit 4 at f{setf}; the $F156 "
   "bank stays 00 and no chest was opened")

# The round trip: the bit clears on the way out and the Xanafalgue respawns on
# the way back, so the set/clear cycle is repeatable. This is a property of the
# $F140 temp bank alone; it says nothing about chests, which never write here.
rows = load(OUT/'verify_roundtrip.csv')
seq, prev = [], None
for r in rows:
    if r['chestb2'] != prev:
        seq.append((int(r['frame']), r['chestb2']))
        prev = r['chestb2']
vals = [v for _, v in seq]
if vals[:3] != ['00', '10', '00']:
    bad(f"chestb2 did not go 00 -> 10 -> 00 across the round trip; saw {vals[:5]}")
cleared = seq[2][0]
# Object slot 0 on map $15 is the Xanafalgue (pack object_id 388). It must be
# gone after the flee and present again after re-entry.
back = [r for r in rows if int(r['frame']) > cleared
        and r['map_index'] == '0015'
        and (int(r['o00_id'], 16) & 0x7FFF) == 388]
if not back:
    bad("the Xanafalgue never respawned after re-entering with the flag clear")
ok(f"round trip: $FFFFF142 bit 4 clears at f{cleared} on leaving, and the "
   f"Xanafalgue respawns at f{back[0]['frame']} on return - the set/clear "
   "cycle is repeatable")

# Object slots on a chest map: NPCs fill the pool first, chests follow in
# record order, so a chest's slot is npc_count + chest_index. Tape 02's map has
# no chests, so this is the only tape that exercises the offset.
rows = load(OUT/'verify_chestmap.csv')
# Objects populate over several frames, so sample once the pool has settled
# rather than on the first frame slot 0 appears - an early frame catches the
# chest slots still empty.
settle = next(int(r['frame']) for r in rows if r['mark'] == 'settle')
spawn = next(r for r in rows if int(r['frame']) >= settle
             and r['map_index'] == '0015' and r['o02_id'] != '0000')
if (int(spawn['o00_id'], 16) & 0x7FFF) != 388:
    bad(f"map $15 slot 0 is {spawn['o00_id']}, expected the Xanafalgue (388)")
for slot in (1, 2):
    got = int(spawn[f'o{slot:02d}_id'], 16) & 0x7FFF
    if got != 0xA0:
        bad(f"map $15 slot {slot} is {got:#x}, expected a chest ($A0)")
    if spawn[f'o{slot:02d}_timer'] != '0':
        bad(f"chest in slot {slot} has a non-zero wander timer "
            f"({spawn[f'o{slot:02d}_timer']}) - chests must never wander")
ok("chest map object pool: slot 0 is the NPC (388), slots 1-2 are chests "
   "($A0) in record order, both with timer 0")

# The chest open itself: the item is granted, and the flag write lands in the
# $F120 bank, not the $F140 one the constants file calls Chest_Flags. An
# earlier revision of this check pinned the ABSENCE of a $F143 write as a
# finding; that negative was true but useless, because it was watching the
# wrong eight bytes. Both halves are asserted together now so the same mistake
# cannot recur silently: a positive somewhere and a negative everywhere else.
byf = by_frame(rows)
op = next(int(r['frame']) for r in rows if r['mark'] == 'open_chest')
found = [r for r in rows if int(r['frame']) >= op and r['inv0'] == '7D']
if not found:
    bad("opening the chest never put the Dimate ($7D) in Inventory[0]")
grant = int(found[0]['frame'])
wrote = [r for r in rows if r['extb3'] == '80']
if not wrote:
    bad("$FFFFF123 bit 7 was never set - the chest flag write did not land in "
        "the $F120 bank")
w = int(wrote[0]['frame'])
if w != grant:
    bad(f"chest flag set at f{w} but the item was granted at f{grant}; they "
        "were measured as simultaneous")
if {r['chestb3'] for r in rows} != {'00'}:
    bad("$FFFFF143 changed - chest flags were measured to live at $F120, not "
        "$F140; re-check the README's chest section")
ok(f"chest open: Dimate reaches Inventory[0] and chest flag 24 sets "
   f"$FFFFF123 bit 7, both at f{grant}; $FFFFF143 stays 00")

# ---------------------------------------------------------------------------
# Tape 20: the same chest, pressed twice and revisited.
# ---------------------------------------------------------------------------
rows = load(OUT/'verify_chesttrip.csv')
byf = by_frame(rows)
EXT = [f'extb{n}' for n in range(32)]

# The bank is not empty at a new game. Event_GameStart preloads eleven ids into
# $F120-$F13F, and the runtime pack's own game_start snapshot carries the same
# thirty-two bytes - a check that the extractor and the cartridge agree about
# the bank's identity, not just its contents.
import json, pathlib as _p
pack = _p.Path('runtime-pack/manifest.json')
preload = next(r for r in rows if int(r['frame']) >= 1000)
live = ''.join(preload[c].lower() for c in EXT)
if pack.is_file():
    want = json.loads(pack.read_text())['game_start']['flag_banks'] \
        ['extended_event_flags']['raw_hex'].lower()
    if live != want:
        bad(f"$F120-$F13F reads {live} at f{preload['frame']} but the pack's "
            f"game_start snapshot says {want}")
    ok(f"new-game preload: $F120-$F13F matches the pack's game_start snapshot "
       f"byte for byte ({sum(bin(int(preload[c],16)).count('1') for c in EXT)} "
       "ids set)")

# Pressing an opened chest again, standing still, on the same visit. The
# routine IS entered - so the guard is not LoadTreasureChests refusing to spawn
# an interactable object - but nothing is granted and no bit moves.
rp = next(int(r['frame']) for r in rows if r['mark'] == 'repress_same_visit')
after = [r for r in rows if rp <= int(r['frame']) <= rp + 120]
if '0024' not in {r['game_mode_routine'] for r in after}:
    bad("re-pressing an opened chest never entered FieldRoutine_ItemFound "
        "($24) - the guard would then be at the spawn site, not in ItemFound")
if {r['inv1'] for r in after} != {'00'}:
    bad("re-pressing an opened chest granted a second item")
if {r['extb3'] for r in after} != {'80'}:
    bad("re-pressing an opened chest moved $FFFFF123")
ok(f"same-visit re-press at f{rp}: ItemFound is entered, no item is granted "
   "and $FFFFF123 stays $80 - the guard is the flag test inside ItemFound")

# Leave the map, come back: the chest is still open, because the bit that
# decides its facing was written on the first visit and never cleared.
ret = next(int(r['frame']) for r in rows if r['mark'] == 'returned')
spawn = next(r for r in rows if int(r['frame']) > ret
             and r['map_index'] == '0015' and r['o01_id'] != '0000')
if spawn['o01_facing'] != '4':
    bad(f"chest slot 1 respawned facing {spawn['o01_facing']} at "
        f"f{spawn['frame']}; an opened chest must come back open (facing 4)")
if spawn['o02_facing'] != '0':
    bad(f"chest slot 2 respawned facing {spawn['o02_facing']}; the untouched "
        "chest must come back closed (facing 0)")
if spawn['extb3'] != '80':
    bad("$FFFFF123 bit 7 did not survive the map transition")
# Nothing a chest does may touch the $F140 bank. The one bit that moves there
# is the Xanafalgue's temp flag, byte 2 - so byte 3, where ids 24 and 25 would
# land under the mislabelling, is the byte to pin.
if {r['chestb3'] for r in rows} != {'00'}:
    bad("$FFFFF143 moved during the chest round trip")
ok(f"leave and return: the opened chest respawns open at f{spawn['frame']} "
   "(facing 4) while the untouched one respawns closed, and $FFFFF143 never "
   "moves")

# ---------------------------------------------------------------------------
# Tape 21: the second chest, which is what separates the bit rule from luck.
# ---------------------------------------------------------------------------
rows = load(OUT/'verify_chest2.csv')
op = next(int(r['frame']) for r in rows if r['mark'] == 'open_chest')
hit = [r for r in rows if int(r['frame']) >= op and r['extb3'] == '40']
if not hit:
    bad("opening chest flag 25 did not set $FFFFF123 bit 6 - the bit rule "
        "base $F120, byte id>>3, mask 1 << (7 - (id & 7)) does not hold")
if {r['chestb3'] for r in rows} != {'00'}:
    bad("$FFFFF143 moved while opening chest 25")
before = next(r for r in rows if int(r['frame']) == op - 1)
paid = int(rows[-1]['current_money']) - int(before['current_money'])
if paid != 100:
    bad(f"chest 25 paid {paid} meseta, expected 100")
ok(f"chest flag 25 sets $FFFFF123 bit 6 at f{hit[0]['frame']} and pays 100 "
   "meseta - two ids, two neighbouring bits, one bank: the chest system's "
   "door is $F120")
PY

if [ "$LANE" = fast ]; then
	echo
	echo "fast lane passed (run with --full for the battle tapes)"
	exit 0
fi

# --- full lane tapes -------------------------------------------------------
echo "== full lane tapes =="
run "$ORACLE/tapes/07_first_battle.tape" "$OUT/verify_battle.csv" \
    --groups core,battle,bhit,enemy,chars,rng
run "$ORACLE/tapes/09_second_battle.tape" "$OUT/verify_battle2.csv" \
    --groups core,battle,bhit,enemy,chars,rng
run "$ORACLE/tapes/10_levelup.tape" "$OUT/verify_levelup.csv" \
    --groups core,battle,bhit,enemy,chars,rng
run "$ORACLE/tapes/12_escape.tape" "$OUT/verify_escape.csv" \
    --groups core,battle,chars
run "$ORACLE/tapes/14_defend.tape" "$OUT/verify_defend.csv" \
    --groups core,battle,bcmd,chars

echo "== battle findings =="
PYTHONPATH="$ORACLE" python3 - "$OUT" <<'PY'
import pathlib, sys
sys.path.insert(0, 'oracle')
from checks import load, by_frame, ok, bad
OUT = pathlib.Path(sys.argv[1])

# Battle ground truth. These numbers are what the damage-formula work is fitted
# against, so they are pinned rather than merely reported.
rows = load(OUT/'verify_battle.csv')
byf = by_frame(rows)
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
seen = []
for f in range(bf, bl + 1):
    r = byf.get(f)
    if r and (not seen or r['battle_exp_total'] != seen[-1]):
        seen.append(r['battle_exp_total'])
if '12' not in seen or '24' not in seen:
    bad(f"battle_exp_total never reached 12 then 24; saw {seen[:8]}")
gains = {c: int(rows[-1][f'{c}_exp']) - int(byf[bf][f'{c}_exp'])
         for c in ('chaz', 'alys', 'hahn')}
if set(gains.values()) != {8}:
    bad(f"exp gains were {gains}, expected 8 each (24 total / 3 living)")
money = int(rows[-1]['current_money']) - int(byf[bf]['current_money'])
if money != 6:
    bad(f"meseta gain was {money}, expected 6 (2 x ZoranBult meseta_reward 3)")
ok("rewards: battle_exp_total 24 (2 x 12), 8 exp each to 3 living members, "
   "+6 meseta (2 x 3) - all matching enemies.json")

# Second battle: a different seed path, a different formation, and a critical.
rows = load(OUT/'verify_battle2.csv')
byf = by_frame(rows)
inb = [r for r in rows if r['game_mode'] in ('0010', '0014')]
if not inb:
    bad("tape 09 never entered a battle")
b2f, b2l = int(inb[0]['frame']), int(inb[-1]['frame'])
mid = byf[min(b2f + 240, b2l)]
if mid['e1_id'] != '9' or mid['e2_id'] != '10':
    bad(f"battle 2 formation is e1={mid['e1_id']} e2={mid['e2_id']}, "
        "expected 9 (Xanafalgue) and 10 (ZoranBult)")
totals = []
for f in range(b2f, b2l + 1):
    r = byf.get(f)
    if r and (not totals or r['battle_exp_total'] != totals[-1]):
        totals.append(r['battle_exp_total'])
if '9' not in totals or '21' not in totals:
    bad(f"battle 2 exp total never stepped 9 then 21; saw {totals[:8]}")
g2 = {c: int(rows[-1][f'{c}_exp']) - int(byf[b2f][f'{c}_exp'])
      for c in ('chaz', 'alys', 'hahn')}
if set(g2.values()) != {7}:
    bad(f"battle 2 exp gains were {g2}, expected 7 each (21 / 3)")
m2 = int(rows[-1]['current_money']) - int(byf[b2f]['current_money'])
if m2 != 5:
    bad(f"battle 2 meseta gain was {m2}, expected 5 (2 + 3)")
crit = [int(r['frame']) for r in rows if b2f <= int(r['frame']) <= b2l
        and any(r[f'hit_{i:02d}'] == '01' for i in range(9))]
if not crit:
    bad("battle 2 recorded no critical hit (Fighters_Hit_Flags $01)")
ok(f"battle 2 f{b2f}-{b2l}: Xanafalgue+ZoranBult, exp 21/3 = 7 each, "
   f"+5 meseta, critical hit at f{crit[0]}")

# Level up, and the equipment stat-lag bug that the fidelity policy turns on.
rows = load(OUT/'verify_levelup.csv')
byf = by_frame(rows)
lvl = next((int(b['frame']) for a, b in zip(rows, rows[1:])
            if a['chaz_level'] == '1' and b['chaz_level'] == '2'), None)
if lvl is None:
    bad("tape 10 never levelled Chaz to 2")
before, after = byf[lvl - 1], byf[min(lvl + 600, int(rows[-1]['frame']))]
expect = {'chaz_maxhp': ('25', '31'), 'chaz_maxtp': ('10', '13'),
          'chaz_str': ('8', '9'), 'chaz_agi': ('7', '8'), 'chaz_dex': ('5', '6')}
for k, (b0, a0) in expect.items():
    if before[k] != b0 or after[k] != a0:
        bad(f"{k} went {before[k]}->{after[k]}, expected {b0}->{a0}")
ok(f"level up at f{lvl}: maxhp 25->31, maxtp 10->13, str 8->9, agi 7->8, "
   "dex 5->6, all matching progression.json level 2")
lag = {k: (before[k], after[k]) for k in
       ('chaz_atk', 'chaz_dfs', 'chaz_str_mod', 'chaz_agi_mod', 'chaz_dex_mod',
        'chaz_mdef') if before[k] != after[k]}
if lag:
    bad(f"equipment-derived stats refreshed on level up: {lag} - the retail "
        "stat-lag bug did not reproduce")
ok("stat lag confirmed: atk_pow stays 18 and the *_mod / magic_dfs stats do "
   "not refresh when the base stats rise")

# A miss must be present: an action that resolves with no HP change at all.
misses = 0
for a, b in zip(rows, rows[1:]):
    if a['game_mode'] not in ('0010', '0014'):
        continue
    if all(b[f'hit_{i:02d}'] == 'FF' for i in range(9)) and \
       any(a[f'hit_{i:02d}'] != 'FF' for i in range(9)):
        misses += 1
if misses == 0:
    bad("tape 10 recorded no all-FF hit-flag frame (no miss sample)")
ok(f"miss samples present in tape 10 ({misses} all-FF resolutions)")

# Escape leaves battle mode.
rows = load(OUT/'verify_escape.csv')
cf = next(int(r['frame']) for r in rows if r['mark'] == 'confirm_run')
left = [r for r in rows if int(r['frame']) > cf
        and r['game_mode'] not in ('0010', '0014')]
if not left:
    bad("tape 12 never left battle mode after confirming RUN")
ok(f"escape confirmed at f{cf} leaves battle at f{left[0]['frame']}")

# DEFEND, and the physical_prop clobber the bugfix policy hinges on.
rows = load(OUT/'verify_defend.csv')
byf = by_frame(rows)
cd = next(int(r['frame']) for r in rows if r['mark'] == 'confirm_defend')
if byf[cd + 150]['cmd0_index'] != '5':
    bad(f"defend selection wrote command {byf[cd + 150]['cmd0_index']}, expected 5")
props = {r['alys_phys_prop'] for r in rows if cd <= int(r['frame']) <= cd + 3000}
if '258' not in props or '514' not in props:
    bad(f"alys physical_prop never toggled 514<->258 while defending; saw {sorted(props)}")
saves = {r['alys_phys_prop_save'] for r in rows}
if saves != {'0'}:
    bad(f"physical_prop_save was written ({sorted(saves)}); retail leaves it 0")
others = {r['chaz_phys_prop'] for r in rows if cd <= int(r['frame']) <= cd + 3000}
if others != {'514'}:
    bad(f"a non-defending member's physical_prop changed: {sorted(others)}")
ok("defend: command 5 selected; physical_prop 514->258 on the defender only, "
   "physical_prop_save stays 0 (the retail clobber)")
PY

echo
echo "full lane passed"
