# Native field travel

`psiv_tools/travel.py` extracts the ROM tables into the ordinary JSON runtime
pack. `psiv-data` validates their version, ROM hash, selectors and uniqueness.
The Rust runtime now consumes those tables; Godot's TECH menu exposes HINAS
and RYUKA when the character has learned them. ITEM exposes TELEPIPE and
ESCAPIPE without a character picker. Runtime play reads JSON, with
no ROM or emulator access.

RYUKA is technique 39 and costs 8 TP; HINAS is technique 40 and costs 4 TP in
the US pack. Learned-slot order and the original field caster rules apply:
death and paralysis block casting, while technique seal does not. Browsing
or cancelling RYUKA destinations is free. Choosing a visited destination pays
once and opens the teleport message. HINAS pays when an eligible exit message
opens. Confirm, cancel, Enter or controller Start acknowledges the paid message
and performs the ordinary map transition; acknowledging it is not a refund.
All four travel paths consume no RNG draws themselves.

TELEPIPE (item 132) uses the same town list without a caster, TP, status or
learned-spell requirement. Browse/cancel and blocked use preserve every item.
Confirming an available town removes the selected inventory slot, then runs
the original first-hole shift. ESCAPIPE (item 133) uses the remembered dungeon
exit and consumes its selected slot only when that exit is valid. A pending
message cannot consume twice. These item paths play the teleport SFX on
acknowledgement, without the technique-cast sound.

The source is `Win_ItemActionMain` ($5B6CC), `Win_TelepipePlaceList`'s
confirmation ($5CA0C), and `Win_ItemTeleportMsg` ($5CAD2). `GetInventoryOffset`
uses the selected eight-item page/row. Unlike a full chest's discard swap,
ordinary item use does not search for the first matching item ID. The town
and dungeon eligibility/destination routines are shared with TECH.

## Retail sources and coordinates

`ProcessTownTeleportData` allows RYUKA only when Dungeon_Teleport_Index and
Town_Teleport_Flag are both zero, and the World_Index BYTE is 0, 1 or 2.
The previously reversed town-byte helper is corrected. World_Index is at
$F400: it is the high byte of the raw word retained by the save serializer.

RYUKA reads Motavia town bits 0..15, Dezolis bits 16..24, or Rykros bit 25.
Motavia starts at $6115A; its 16-row loop overlaps the first three Dezolis
rows at $611C2. Those aliases are retained. Rykros has one row at $6120A.
The destination's table map becomes Field_Map_Index_2, and the current world's
overworld becomes Field_Map_Index. Coordinates are eight-pixel Map_Start
units; the runtime applies its normal standing-cell Y shift.

`ProcessDungeonTeleportData` reads 34 eight-byte rows at $61212..$61321.
Index zero is unavailable. The departing map becomes Field_Map_Index_2;
the selected row supplies the new map, start and facing.

`FieldRoutine_PlaceName` reads 62 six-byte records at $664BC..$6662F,
terminated by $FFFF. It matches current and previous map. A nonnegative
selector registers a town flag and clears the dungeon exit. $FE clears the
exit without registering a town. Other negative selectors set selector & $7F.
Passageway index 7 becomes 8 at actor X $520. Runtime construction and map
entry now apply these rules; normal warps update the previous-map word and
scene LoadMap honors its explicit prev_map.

A negative dungeon byte skips the store in both original map-load routines.
The JSON null shape is retained, with its documented meaning corrected to
inherit the current exit. This preserves the Valley Maze's entrance through
its interior maps.

## Save extension and limits

The cartridge keeps Dungeon_Teleport_Index at $ED51, outside its SRAM payload.
Native saves now preserve it in unused logical header bytes $F022..$F025:
ASCII TR, the index, and its complement. Retail payload and checksum bytes
are unchanged by this extension. Loading uses it only for inherited-exit
maps and only for a recognized selector; legacy files remain readable.
A legacy save deep inside an inherited-exit dungeon cannot always reconstruct
its entrance, so its exit defaults to zero unless the place-entry table
identifies it. This extension is a deliberate native restart improvement,
not a claim that the original cartridge saves $ED51.

World selection during later space-travel scenes still needs the explicit
original writes wired into the scene runtime. Native Motavia travel and the
raw saved world-byte interpretation are covered here. The existing general
map-entry follower alignment, the exact retail teleport visuals/window layout,
and the legacy standing-Y save-format discrepancy are
separate remaining work. Do not infer full travel or campaign fidelity.

## Verification

Three Python tests compare extracted coordinates, table aliases and opposite
maze entrances against ROM bytes. Seven Rust tests cover schema rejection,
town registration, previous-map save words, free cancellation, once-only costs,
status/TP/world gating, Passageway's X-dependent exit, both Valley Maze exits,
and fresh native save reload of inherited exits. These are data and runtime
fixtures. The broader workspace check is recorded under
`build/native-tonoe/workspace-travel-tests.log`.

`build/native-travel/route/receipt.json` is the completed native fixture:
ordinary TECH input casts HINAS from Tonoe basement, cancel acknowledges its
paid message, RYUKA browsing and cancellation remain free, and choosing MILE
lands on the correct overworld cell. Chaz TP is 50 -> 46 -> 46 -> 38; other
party state and money stay unchanged. The menus and both arrival renders were
inspected. SAVE writes slot 2 while the fixture's slot 1 stays unchanged.

`build/native-travel/continued/receipt.json` then selects slot 2 through the
actual title CONTINUE in a fresh process, walks south, and saves slot 3. The
input slot is unchanged; only the leader Y changes in the logical payload.
These are isolated ability fixtures derived from the native Tonoe save;
they do not grant these spells or change any campaign save.

The full Rust workspace passes 849 tests, strict Clippy passes, and eight
Python pack tests pass. Logs are under `build/native-tonoe`; native source and
binary hashes are recorded under `build/native-travel`. Software rendering
and dummy audio do not establish physical controller, sound or frame pacing
quality.

## Pipe verification

`build/native-pipes/runtime-tests.log` passes all nine runtime travel tests:
the six technique/entry/save cases plus selected-duplicate pipe consumption,
free cancellation and rejection, inherited dungeon exits without TP, and
revalidation after the inventory changes. Strict workspace Clippy passes in
`build/native-pipes/clippy.log`. The isolated `pipe_fixture` preserves the
native Tonoe campaign roster and gives five explicitly labelled test items;
it does not modify the campaign source file.

The first native input run completed blocked TELEPIPE, ESCAPIPE, TELEPIPE
browse/cancel/reopen/confirm, and SAVE. Its captures exposed cramped town-list
spacing; the corrected list uses two tile rows and a separate cursor column.
The corrected `build/native-pipes/route/receipt.json` completed blocked use,
ESCAPIPE, free town browse/cancel, selection of the later duplicate TELEPIPE,
MILE arrival and SAVE slot 2. Only the two selected pipes were consumed;
party HP/TP/status and money stayed unchanged. The corrected captures were
inspected. `build/native-pipes/continued/receipt.json` then used the actual title
CONTINUE for slot 2, walked down once and saved slot 3 in a fresh process.
Only leader Y differs in the logical save payload ($0660 -> $0670); both input
slots and the original Tonoe campaign save remain unchanged. Source/binary
hashes and byte comparisons are recorded alongside the receipts. These runs
use software rendering and dummy audio, not a physical-controller/audio test.
