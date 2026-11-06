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

Native Motavia travel and the raw saved world-byte interpretation are
covered here. The cartridge writes `World_Index` in eight places: the new-game
initializer, the ship's destination menu and Sabotage's copy of it, and five
scenes. `Runtime::set_world_index` cites each one it makes,
`rust/psiv-runtime/tests/world_index_writes.rs` holds that list to the disassembly, and the
whole-route runner test checks the world and map of every chapter that lands
on a new planet. Still open: general map-entry follower alignment
([#127](https://github.com/TusanHomichi/PSIV/issues/127)); and the teleport's
window teardown ([#128](https://github.com/TusanHomichi/PSIV/issues/128),
[below](#teleport-windows-and-effect)).

## Teleport windows and effect

Measured on the oracle with
[`oracle/tapes/36_teleport_windows.tape`](../../oracle/tapes/36_teleport_windows.tape):
Tape 22's prelude to Chaz alone on Piata Academy F1, then the fixture patches
(see [the oracle guide](../../oracle/README.md#deterministic-scene-fixtures))
`7600:FFFFF410:84` (a TELEPIPE), `7600:FFFFF160:FFFF` (every Motavia town
visited) and `7600:FFFFF101:08` (`EventFlag_PrincipalConfession`, which keeps
`Event_PiataGuardsReprimand` from carrying the party back into Piata), and
ITEM, the TELEPIPE, USE, Down to MILE, Speak, Speak.

**The town list** (`Win_TelepipePlaceList`, `ps4.asm:123998-124213`; RYUKA's
copy `Win_RyukaPlaceList`, `:129219`) shows the visited towns five to a page
from `$FFFFE270`. Each page is its own window from records `$36..$44`
(`ps4.asm:140340-140388`; `$66..$74` for RYUKA): 14 cells wide, two cells per
town plus the border, at (18,13) for the first page and one cell further right
for each later one, drawn over the pages before it. Every row has a cursor box
at the window's second column and the name at its fourth; `TO?` sits on the
bottom border five cells in (`loc_2AA892`). A page with one behind it carries
the left arrow on its top-left corner and one with a page ahead the flipped
arrow on its top-right. Up and Down wrap within the page; Right *held* opens
the next page and Left *held* closes the current one, each with the cursor on
the first row (`ps4.asm:124111-124158`); Start closes every window.

**The message** is window `$45` (`$65` for techniques), the ITEM message's
rectangle, with one of three lines chosen by `$FFFFE284` (`loc_5CB66`,
`loc_60B26`): `Somehow the body` / `has become lighter.` (two rows apart),
`Cannot be used here!` and `Not enough TP!`. Its acknowledgement plays the
teleport sound and closes every window.

**The effect**, frame by frame on tape 36 (the acknowledgement on 8355): the
seven windows close one at a time, the last at 8385; `MenuExit_Teleport`
(`ps4.asm:117166-117169`) fades the palette out in two-frame steps from 8391
to black at 8403; `GameMode_LoadFieldMap` runs from 8404 for 61 rows (map 0
with the music branch taken: `field_reload_rows`), black until its
`Pal_FadeIn` lights 8452 and clears 8464; control returns on 8465, the party
standing at (61,102) on the overworld outside MILE.

The port draws the list, its pages, arrows and label, and the message lines
from those records (`rust/psiv-godot/src/camp/abilities.rs`, `layout.rs`); the
session pages the list as retail does (`session/camp/travel.rs`); and a
teleport's `RuntimeEvent::Teleported` carries the destination's load rows to
the `Teleport` transition, which reproduces the fade levels above
(`transitions.rs`, `a_teleport_fades_out_then_spends_the_destinations_load_rows`).
Two certified pairs (`tools/certify.py`, region-only) compare the native
captures with tape 36 in the 320×224 field viewport: `teleport-towns`, the
list's window (pixels 144,104 to 256,200) at frame 8090 against
`PSIV_DEBUG_CAMP=telepipe`, and `teleport-message`, window `$45` (56,168 to
264,208) at frame 8340 against `PSIV_DEBUG_CAMP=telepipe-message`. Both pass
at RMSE 0. The rest of those frames is the ITEM path's windows, which are
not these pairs' to certify.

Not reproduced yet: the camp windows close at once rather than one at a time,
so the fade starts on the acknowledgement instead of 35 frames later; and the
fade-out darkens the destination, as every native doorway does, where the
cartridge darkens the field it left.

## Verification

Python tests compare extracted coordinates, table aliases and opposite maze
entrances against ROM bytes. Rust tests cover schema rejection, town
registration, previous-map save words, free cancellation, once-only costs,
status/TP/world gating, Passageway's X-dependent exit, both Valley Maze exits,
fresh save reload of inherited exits, selected-duplicate pipe consumption,
blocked and rejected pipe use, and revalidation after the inventory changes.
Native input for HINAS, RYUKA, TELEPIPE and ESCAPIPE was checked from isolated
ability fixtures with SAVE and a fresh-process CONTINUE.

## The northern bridge after Rika joins

Once Rika has joined (event `$35`), an overworld page hook opens a bridge north
of the BioPlant, and Zema's inn is the party's recovery before the crossing.
US retail `$053D16..$053D39` (`loc_53D16`) tests event `$35` before writing
FG `$00` and BG `$48` at chunk `(42,33)`. This is a paged-overworld loader
hook, not a MapDataManager entry. It opens Motavia cells `(84..85,66..67)`:
the unpatched collision is water `$9`, and the final BG chunk supplies `$0`.
The pack keeps the source records, and the runtime composes both planes and
applies the flag-gated collision, raw chunk identity and render patches when it
builds the map; how is [map effects §12](MAP_EFFECTS.md#12-native-overworld-page-hook-consumption).
Same-map live page streaming after a flag change is not implemented.

Zema's inn row `$068116` is `01 14`, rate 20 per occupied party slot; its
keeper row is `$0683A4`, so five members pay 100. The rest is `RecoverStats`
(`$0662DA`; what it restores is [SHOPS.md](../camp/SHOPS.md#what-a-night-restores)).

The bridge's video comparison uses isolated SRAM position fixtures, not
connected play. Their Y word is the cartridge's `(standing_y-1)*16`, the word
a native SAVE writes too ([SAVE_FORMAT.md](../camp/SAVE_FORMAT.md#standing-y)).
