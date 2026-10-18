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
covered here. Still open: world selection during later space-travel scenes
needs the original writes wired into the scene runtime ([#126](https://github.com/TusanHomichi/PSIV/issues/126)); general
map-entry follower alignment ([#127](https://github.com/TusanHomichi/PSIV/issues/127)); the exact retail teleport visuals and
window layout ([#128](https://github.com/TusanHomichi/PSIV/issues/128)); and the legacy standing-Y save-format discrepancy
([#129](https://github.com/TusanHomichi/PSIV/issues/129)).

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
connected play. Their Y word is `(standing_y-1)*16`, which accounts for the
native save-coordinate gap without modifying any campaign save.
