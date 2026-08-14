# Source / provenance notes

The extractor is grounded against Peter's verified US retail PSIV ROM and the public `ps4disasm` work. No ROM bytes are shipped in this repository beyond very short structural signatures used as tests.

## Public reverse-engineering references used

- `squidfeatures/ps4disasm` (`ps4.asm`, `ps4.constants.asm`), including the documented record layouts and stable symbolic IDs.
- `alechenninger/ps4disasm`, a later fork used as an additional orientation/reference source.

The public disassembly documents, among other things:

- initial character records as 66-byte entries
- inventory records as 22-byte entries
- enemy records as 48-byte entries
- enemy-skill records as 8-byte entries
- techniques, skills, and combo records as 8-byte entries
- vehicle records as 26-byte entries
- character level records as 22-byte entries, addressed through a per-character pointer table
- battle formations as variable-length `$FF`-terminated records inside Kosinski-compressed blobs

## Retail US offsets proven against this ROM

- Enemy data: `0x2816BC`
- Enemy skills: `0x28336C`
- Combo data: `0x285424`
- Vehicle data: `0x2855E2`
- Level pointer table: `0x004074`
- Primary Chaz level table / start of primary progression block: `0x2856B0`
- Mirrored progression block: `0x2A3A42`
- Initial character stats: `0x2A8ACA`
- Inventory data: `0x2A8E28`
- Technique data: `0x2A9BE8`
- Skill data: `0x2A9D28`

Kosinski-compressed blobs (end-exclusive):

- Battle formation indexes: `0x2836EC..0x283E67`
- Battle formation data 1: `0x283E6C..0x2842AE`
- Battle formation data 2: `0x2842BC..0x284713`
- Battle formation data 3: `0x28471C..0x284B7E`
- Battle formation data 4: `0x284B8C..0x284F7C`
- Boss formation data: `0x284F7C..0x285012`

- Shop inventories (`ShopInventories`): `0x0681A4..0x0682A3`, 49 `$FF`-terminated lists indexed by `GetOffsetByID_FF_Delim` (terminator counting, no pointer table, no bounds check — the bound lives in the data, corroborated by the 49-entry shopkeeper-portrait and greeting tables)
- Shop locations (`loc_68394`): `0x068394`, `$FFFF`-terminated 8-byte entries `(map id, x, y, group<<8|index)`; group 0 is the inn system with its own index space, groups 1–2 read `ShopInventories`

The offsets are accepted only after the full ROM SHA-256 matches the supported retail build, then short known signatures are checked again at those offsets before extraction.

## Important boundary

`ITEM_SYMBOLS` and `ENEMY_SYMBOLS` are disassembly identifiers, not decoded cartridge text. Records now also carry `display_name`, decoded from the cartridge's own name tables; the symbols stay because they disambiguate duplicates the ROM's display text does not (two skills both display as `FLAELI`; the ROM's `SHOOTINSTR` is the symbols' `Shootnstar`).

## Two font encodings

PSIV uses two charsets: `general/tables/wincharset.asm` for menu/battle name
tables and `script/charset.asm` for dialogue. They agree on `A`–`Z` and space
and disagree on everything else (`'a'` is 57 in the window font, 27 in the
dialogue font), so decoding a table with the wrong one garbles lowercase and
digits. The ROM cross-validates both: the 160 item names are stored twice,
once per encoding (`InventoryNames` window font at `0x2AAFB4`,
`InventoryNames2` dialogue font at `0x2ABA70`), and the decoded string sets
are identical.

Name-table retail ranges (end-exclusive): character names
`0x280CD0..0x280D07`, professions `0x280D08..0x280D42`, enemies
`0x280D42..0x2812E4`, enemy skills `0x2812E4..0x2816BC` (flush against the
enemy record table), combos `0x28554C..0x2855E1`, items `0x2AAFB4..0x2AB622`,
techniques `0x2AB622..0x2AB701`, skills `0x2AB702..0x2AB8A1`, places
`0x2AB8A2..0x2ABA70`, items-in-dialogue-font `0x2ABA70..0x2AC0DE`. No vehicle
name table exists; vehicles are inventory entries (`ItemID_LandRover = $96`).

Dialogue: 43 Kosinski-compressed trees back-to-back from `0x1DF600` to
`0x1FE655`, each zero-padded to a 16-byte boundary, chained into one
continuous length proof; the uncompressed "nothing interesting" message at
`0x1FE660` pins the region's end independently. Control codes are transcribed
from `TextCtrlCodesJmpTbl` and `TextActionsOffs`; zero unknown control bytes
and zero unmapped glyphs across all trees and tables, and the decoder raises
rather than emitting placeholders.

## Kosinski decompression

`psiv_tools/kosinski.py` is a transcription of `KosDecomp` in `ps4.asm`, not an
implementation written from a prose description of the format. Each branch of
the Python decoder names the asm label it corresponds to. Two details were
taken from the routine rather than assumed:

- Description fields are 16-bit little-endian and their bits are consumed
  LSB-first, but a field is refilled only *after* its sixteenth bit has been
  consumed and *before* that element's operand bytes are read (the `dbf d4`
  sits between the `lsr.w` and the operand fetch). A decoder that refills at
  the start of the sixteenth element desynchronises on real data.
- In the extended-count branch, the extra byte `0` ends the stream and `1`
  falls back into the main loop, consuming three bytes and two description
  bits without emitting output.

The decoder returns the number of compressed bytes consumed, which turns every
documented blob range into a verification: decompression must terminate on the
end marker at exactly the annotated end address.

## Nemesis decompression

`psiv_tools/nemesis.py` is a transcription of `NemDecomp` / `Nem_ProcessCompressedData` /
`Nem_BuildCodeTable` (`ps4.asm:84605-84797`), not an implementation from a prose
description. Three details were taken from the routine rather than assumed:

- Output length is fixed by the header alone (`(count & $7FFF) * 8` rows); the
  routine stops mid-run the instant the row counter hits zero, so decoding
  until input exhaustion overruns on real data.
- The bit-window refill runs before the pixels that emptied it are written, so
  the routine reads one lookahead byte it may never use. Consumed length is
  the stream length or length + 1 (observed on 23 of 68 retail blobs); the
  length checks accept exactly that pair and nothing else.
- XOR mode accumulates: output row N is the XOR of decoded rows 0..N, not a
  delta against the previous row alone.

The tile format is packed 4bpp (two pixels per byte, high nybble = left
pixel), proven by `Nem_PCD_WritePixel`'s `lsl.l #4 / or.b` construction.

## Palette format

CRAM colours are big-endian words `%0000BBB0GGG0RRR0`. Three-bit channels are
widened to 8 bits by bit replication (`v<<5 | v<<2 | v>>1`): 0 maps to 0, 7 to
255, it is monotone, and inverts exactly as `v = c >> 5` (the common `v * 36`
alternative tops out at 252). The original 0–7 levels are emitted alongside
the widened values, so the choice is never load-bearing. Battle palettes are
CRAM line 0, indices 1–13, index 0 forced black — proven from `loc_6C3C`,
which clears the first word and copies `$D` words after it.

Palette offsets: `Pal_Init` `0x09F2BC` (only line 2 carries colour);
`Pal_Init_Line_3` `0x296300` (byte-exact duplicate of that line, verified, the
same mirroring phenomenon as the level tables); title palettes `0x1D29BC` /
`0x1D2A3C` / `0x2F4994`; 31 contiguous battle palettes from `0x007054`.

## Enigma decompression

`psiv_tools/enigma.py` is a transcription of `EniDecomp` in `ps4.asm`. Six
details taken from the routine rather than assumed: PSIV implements a reduced
Enigma (V/H flip flags only — a stream declaring P or CC bits would
desynchronise, so `read_header` rejects flag bytes above 3); the format entry
is read as 7 bits and partly given back for the 6-bit modes; an inline value
straddling the bit window reloads the window wholesale rather than streaming;
base tile and flag bits are *added* to the value, not or-ed (invisible at the
ROM's 4–9 inline bits, load-bearing beyond); flag-bit reads skip the refill
check, which bounds the usable inline width (16-bit values with flags are
undecodable and rejected); and consumed length is exact after the routine's
rewind-and-round-to-even epilogue, so every length check is `==`.

## Plane mappings

35 mappings, 35,436 cells. Battle-background mappings sit in an adjacency
chain with their art (art, its mapping, next art), so `BattleBGArtPtrs` bounds
both halves from the cartridge alone; the title-screen chain ends exactly at
`Pal_TitleScreen` and the portrait chain at `Pal_TitleCharPortraits`. Tile
semantics proven four ways: `art_index = mapping_tile - art_vram_tile`, with
the mapping's base-tile low bits equal to the art's VRAM load tile. No retail
mapping carries per-cell palette bits; the base tile's line applies to the
whole mapping (battle backgrounds all line 0, matching the proven battle
palette placement). 82 of 83 disassembly `plane mappings/*.bin` payloads
decode with consumed == file size (the 83rd bundles 348 bytes of unrelated
sprite-mapping records after the stream — pinned).

## Map records

`FieldMapPtrs` at `0x100000` (417 longs, flush against its first record;
361 real maps totalling 59,512 bytes over `0x100684..0x1CB01A`, 56 ErrorTrap
nulls matching the `Null*` symbols). The record grammar is transcribed from
`GameMode_LoadFieldMap` and its subroutines; consumption rules per section are
documented in `psiv_tools/maps/records.py`. Notable proven quirks: MapID 0/1
(the overworlds) omit the 8-byte layout-pointer section entirely; `$FFFE` in
the sprite list is not a terminator but a decompress-to-RAM prefix; the scroll
section has a conditional 6/14/22-byte length; treasure-chest meseta is stored
in hundreds. Object/chest terminators are double-proven by replaying the
loader's alternate `GoPast_FFFF_Terminator` path over all 722 sections.

Encounter binding: `Battle_EnemyFormationIndexes` at `0x008050` is one byte
per map, indexed directly by MapID with no bounds check. Values >1 are the
encounter group; 0/1 selects the overworld position grids (Mota 64×64 cells of
64px decompressing to 4,096 bytes, Dezo 32×64 to 2,048), `$FF` means no
encounters and only works because those maps also clear `Random_Battles_Flag`.

## Map layouts and collision

Chunks are 16 big-endian pattern-name words (4×4 tiles, 32×32 px), Kosinski,
concatenated into `Chunk_Table`; layouts are one byte per chunk, row-major,
Kosinski, max 256 chunks addressable. Bit 14 of each chunk word is a collision
flag the game strips before the VDP sees it (`andi.w #$BFFF` — so field tiles
can only select CRAM lines 0–1), and the four flags of a 2×2-tile cell form a
4-bit collision type per 16-pixel cell (bits: TL=1, TR=2, BL=4, BR=8). Types:
0 normal, 1 map change (non-blocking — the walker steps on and the warp
fires), 2 recovery, 8 solid, 9 water, $A sand, $B ice, $C shop; 8–C block.
Which plane carries collision is per-map (the scroll section's first byte).
Proven on Piata / PiataItemShop / IslandCave with exact consumed-length chains
and two cross-table checks: all Piata doorways land on type-1 cells matching
the transition table, and the shop-location entry lands on a type-$C cell.

## Oracle methodology for graphics

No decompressed Nemesis source exists in the disassembly, so formations-style
round trips are unavailable. Substitutes, in decreasing strength:

- Battle backgrounds carry a ROM-internal exact-length oracle: each art blob's
  Enigma plane mapping is stored immediately after it, and all 20 blobs decode
  to exactly `mapping_ptr - art_ptr` consumed bytes.
- The disassembly stores Nemesis blobs already compressed; each `binclude`
  payload occurs exactly once in the retail image, fixing the offsets. The
  oracle tests match portraits and battle art to payloads by content, not by
  filename.
- Visual identification pins the decoders end to end: the font decodes to the
  Latin alphabet (letter-A bitmap pinned in a test), the Sega logo proves the
  XOR accumulator, Chaz's portrait proves row-major 48×48 composition, and the
  Mota desert background proves the pointer-table → art → palette path.
- sha256 pins: font, Sega logo, Chaz individually, plus one digest over the
  sorted digests of all 68 blobs.

## Oracle methodology for battle formations

`reference/ps4disasm/battles/{battle_formations_1..4,boss_formations}.asm`
contain the same formation data uncompressed, as plain `dc.b` listings. The
test suite parses those listings (rejecting any non-`dc.b` directive rather
than assembling anything) and asserts byte-for-byte equality against the
decompressed retail blobs. All five match exactly: 1,708 / 1,716 / 1,678 /
1,540 / 300 bytes. That equality, not the plausibility of the output, is the
acceptance criterion for the decoder.

The enemy/item id conventions used by formation records were proven the same
way. `ps4.constants.asm` defines `EnemyID_Helex = 0` and `ItemID_Dagger = 1`,
and the formation sources annotate every enemy byte with its symbol; the tests
check the extractor's resolution against those annotations for all 100+
distinct enemy ids that appear in formations.

## Discrepancies found against the disassembly's annotations

- `Battle_FormationData4`: the inline range annotation in `ps4.asm` gives
  `0x284B8C-0x284C3D`, which is only the first of many annotated chunks for
  that blob. The Kosinski stream actually runs to `0x284F7C`, immediately
  before `Battle_BossFormationData`, and the decompressor consumes exactly
  0x3F0 bytes.
- Formation `0x177` (block 3 record `$77`) declares four enemies in byte 4 but
  lists three enemy/position pairs, and its group bitmasks cover three slots.
  The disassembly's uncompressed source has the same bytes, so this is the
  ROM's own inconsistency rather than a decode error. Both the declared count
  and a mismatch flag are emitted. (Blast radius, proven from the code: the
  count byte's only reader is the Slasher weapon's hit-effect positioning, so
  the bug misplaces battle visuals in one rare Dezolis encounter and affects
  nothing else.)
- Shops: none. `ShopInventories`, its `ItemID_*` constants, and the
  surrounding labels match the retail bytes exactly. A clean oracle is itself
  a finding.
- `DialoguePortraitArtPtrs` is 7 entries longer in the fork than in retail:
  the retail table ends at index `$27` (Sekreas); the seven shopkeeper
  portraits exist in the ROM but are reached through the shop tables near
  `0x068000`, not through the portrait table.
- The fork's `revision!=0` branch of `Art_DialogueFont` includes a 6,720-byte
  italics font that appears nowhere in the retail image; retail carries the
  1,280-byte plain dialogue font.
- General caution: the `reference/` clone is configured as a Grand Cross hack
  build (`grand_cross=1`, `bugfixes=1`, `optional_fixes=1`,
  `no_random_battles=1`, `external_formation_data=1`). Its `revision`-gated
  and option-gated branches describe the hack, not necessarily retail; every
  slice must prove which branch matches the cartridge rather than trusting
  the default.
- `script/documentation.txt` omits control code `$FB` (extended event-flag
  check, 3 operand bytes) entirely.
- `$F6` (event) takes a 2-byte operand. The disassembly disagrees with itself
  (`GetEventFromDialogue` reads a word; `TextCtrlCode_Event` skips one byte);
  the ROM settles it — a 1-byte reading puts 43 corpus bytes outside the
  font, a 2-byte reading puts zero. The 1-byte skip is a dormant quirk: every
  retail `$F6` sits at an entry start, where the preprocessor handles it.
- `CharName_Chaz = "Shay"` in `ps4.constants.asm` is the Grand Cross hack's
  rename, applied unconditionally in this clone. The cartridge says `Chaz`.
- Five of the 43 `script/dialogue N.asm` sources do not reproduce retail
  bytes: tree 17 is wholesale rewritten by the fork (84 edits); trees 19, 29,
  31 and 34 each carry two single-byte defects that cancel in total length,
  so they pass a naive size check. The tests pin each defect's exact offset
  and shape so a future clone update fails loudly instead of silently
  widening the exception set. The other 38 trees round-trip byte for byte.
- RETAIL CARTRIDGE BUG: `GetChunkAndCollision` (`0x045AB8`) selects the
  layout base from `$FFFFEC24` correctly but takes the *opposite* plane's row
  size as its stride, contradicting `SetupChunksFG` (`0x054356`),
  `SetupChunksBG` (`0x0543A6`) and its own bounds check eleven instructions
  earlier. A crossed branch, confirmed in retail opcode bytes (all three
  sites pinned). Dormant because every map gives both planes the same width;
  our decoder takes the stride from the layout it is handed, i.e. the
  behavior the original intended.
- RETAIL CARTRIDGE BUG: `Battle_EnemyFormationIndexes` is 416 bytes for a
  417-map id space; MapID `$1A0` (`AirCastleSpace`) reads one byte past the
  end into `Character_Init` data, yielding nonexistent group 74. Dormant only
  because that map has random battles disabled. Relatedly, `ValleyMazeUnused`
  (`$02D`) stores `$FF` (no encounters) but leaves random battles *enabled* —
  rolling an encounter there would index far past the tables; dormant because
  the map is unused. `MystVale_Part4` and `AirCastleXeAThoulRoom` assign
  groups with battles off (wasted, harmless). All surfaced in
  `encounter_anomalies`.
- The fork's inline address annotations around the `0x8000` region are ~0x552
  off retail; `Battle_EnemyFormationIndexes` was located by content, not
  annotation. My earlier scouting note placing it at `0x0085A2` was wrong —
  that range is a 15-word palette.
- RETAIL CARTRIDGE BUG: `ClimCenter_F2`'s BG layout pointer targets
  `ClimCenter_F3`'s all-zero 32×32 buffer, but the map is 48×48 — 1,280 BG
  cells render whatever the previously loaded map left in `Map_Layout_BG`
  (zeros on a cold boot). Collision reads the FG plane there, so only the
  picture is affected. The pack zero-fills and records the anomaly.
- `InnerSanctuary_B1`'s BG layout names chunk `$FF` in exactly one cell while
  the map loads 128 chunks — the only such cell in the cartridge (all 718
  planes swept). `SetupChunksBG` has no bounds check, so hardware reads a
  `Chunk_Table` slot the map never wrote: stale data or, cold-booted, an
  empty definition. A stray byte in the original build data; the pack
  substitutes the empty definition and records the substitution.
- Seven Academy maps store zero-padded 32×32 layout buffers for 32×16 grids;
  the surplus is unreachable (`GetChunkAndCollision` wraps Y). The loader
  never checks blob lengths — the grid extent comes from the dimension bytes
  alone — and `layouts.decode_layout` now implements exactly that rule.
- `Zema_LockedDoorsOffs` is named backwards in the disassembly: the offsets
  are where the doors *open* (chunks with map-change cells are written in
  once `EventFlag_IgglanovaZema` is set; the stored layout holds solid
  chunks). Five doorway warps (four in Zema, one in BirthValley_B1) cover no
  type-1 cell in the stored layout for this reason — event-gated doors, not
  defects.
- Census facts (what retail data actually contains vs what the code
  permits): collision type `$7` exists (172 cells, 8 maps, every one
  adjacent to a `$9` water cell — a shoreline artifact of one chunk set) and
  is walkable via `TileColl_Empty`; types `$A` (sand) and `$B` (ice) never
  appear in any field map; dialogue-tree binding is 1-based (1..=43, never
  0); exactly one field object has a facing byte outside {0,4,8,$C}
  (AiedoPub object 2, byte $10); retail selects only 9 of the 15
  `XYRangeJmpTbl` routines. The pack manifest carries a `census` section so
  consumers assert against observed reality instead of hardcoding ranges.
- Nine Enigma call sites are revision-gated and the cartridge runs the `else`
  (English) branch — proven three ways: the retail code contains each
  mapping's `lea`/`move.w #base` pair exactly once with the retail base
  values; the decoded cell counts match only the retail dimensions; and the
  `revision=0` bases would point mappings outside their art blobs.
  `GetMapLayoutChunkFG/BG` hard-code the 128-byte overworld stride and are
  overworld-only helpers despite their general names. `MapEni_GameStartMotaBG`
  / `ArtNem_GameStartMotaBG` are unreferenced in the disassembly source; the
  cartridge reaches them at `0x073C4E`.
