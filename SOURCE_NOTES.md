# Source / provenance notes

The extractor is grounded against Peter's verified US retail PSIV ROM and the public `ps4disasm` work. No ROM bytes are shipped in this repository beyond very short structural signatures used as tests.

## Retail bug ledger index

The numbered index follows the ledger numbering established in the commit
history. It has **13 entries** as of 2026-09-12. The detailed records below are
deliberately chronological and retain their corrections, retractions, and
superseded interpretations; this index is only a map, not a replacement for
that evidence. Classifications are kept explicit: a retail finding is not
quietly promoted to a confirmed cartridge bug, and a candidate is not counted
as one.

1. **RETAIL FINDING / ROM bug — formation `0x177`.** The record declares four
   enemies but contains three enemy/position pairs; the retail source carries
   the same inconsistency.
2. **RETAIL CARTRIDGE BUG — `GetChunkAndCollision`.** The routine uses the
   opposite plane's row stride; the defect is dormant while retail plane widths
   agree.
3. **RETAIL CARTRIDGE BUG — `Battle_EnemyFormationIndexes`.** The table is 416
   bytes for a 417-map space; `AirCastleSpace` reads past it, with the related
   unused `ValleyMazeUnused` encounter footgun recorded in the same entry.
4. **RETAIL FINDING / stray build data — `InnerSanctuary_B1`.** One BG cell
   names chunk `$FF` although the map loads only 128 chunks.
5. **RETAIL CARTRIDGE BUG — `ClimCenter_F2`.** Its BG layout points at the
   wrong, smaller map buffer, leaving 1,280 cells dependent on stale layout RAM.
6. **RETAIL CARTRIDGE BUG — world-map viewer.** The viewer starts 64 bytes
   before the planet page data and renders the pointer table as terrain.
7. **RETAIL CARTRIDGE BUG — `Battle_ProcessRUN`.** It passes an uninitialised
   critical threshold to `Battle_CalculateChances`; the sign-only caller makes
   the result dormant in retail.
8. **RETAIL CARTRIDGE BUG — BLACK WAVE effect `$2C`.** The effect is beyond the
   dispatch table; the later Zio3 correction leaves whether the scripted fight
   reaches the crash path unsettled.
9. **RETAIL CARTRIDGE BUG — Wren Charge pose.** Decompression writes 42 words
   into a 36-word plane buffer and spills six into the next party slot.
10. **RETAIL CARTRIDGE BUG — `Battle_BackgroundIndexes`.** A second 416-byte
    table serves the 417-map id space and reads past its end at `0x1A0`.
11. **RETAIL CARTRIDGE BUG — basement repeatable un-looter.** The historical
    consequence is marked retracted by the final flag model; the measured
    set/clear/respawn cycle remains in the detailed record.
12. **RETAIL CARTRIDGE BUG — `Battle_OrderTurns`.** The max-agility scan reads
    ten words from a nine-entry table and consumes one word past its end.
13. **RETAIL CARTRIDGE BUG — VISION reads the caster's name as its power.**
    Stat selector zero addresses the first encoded name byte. The native
    implementation preserves Hahn's normal +8 dexterity without depending
    on his name; see the measured record below.

### Other classified records in the chronology

- **RETAIL FINDING — FLAG MODEL FINAL.** Retail has four flag banks; `$F120`
  is the shared chest/extended-event bank, `$F140` is temp, and `$F156` is
  fiction. This is the current flag model and supersedes the earlier chain.
- **CANDIDATE RETAIL BUG — `TempEveFlag_BioPlantAlarm`.** The earlier alias
  interpretation is superseded by FLAG MODEL FINAL; the historical candidate
  remains visible below.
- **CANDIDATE RETAIL BUG — invisible Academy Basement blockers.** The port
  treats invisible no-dialogue blockers as solid and silent; hardware
  confirmation remains pending.

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

Scene-level fork divergences (the 17 ungated Grand Cross scene rewrites) are
documented per scene in `docs/scenes/*.md`, each with retail byte ranges and
byte-diff analyses — including `AlysFound`, where the fork kept the retail
prologue/epilogue and cut a hole in the middle, omitting the dialogue call
and the flag-set whose absence would re-fire the trigger forever.


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
- RETAIL CARTRIDGE BUG: the in-game world-map viewer reads its chunk ids 64
  bytes early. `FieldRoutine_WorldMap` (`0x0666CC`) loads the BG *pointer
  table* address (`loc_10BE02`) and streams 16×1,024 bytes from there as raw
  chunk ids, but the page data starts 64 bytes later at `0x10BE42` — so the
  minimap draws the pointer table's own bytes as terrain in its first
  half-row and shifts the whole planet by 64 chunks. Same shape for Dezolis
  (`0x0666DA`). Settled by rendering both readings: aligned yields Motavia
  with the Nurvus crater centered; retail's yields a split continent with a
  garbage strip. Confined to that viewer screen.
- Census correction (overworld data): collision types `$A` (sand) and `$B`
  (ice) are absent from the interiors but NOT the cartridge — Motavia has
  632 sand cells (no ice), Dezolis 1,704 ice cells (no sand). The earlier
  "never appear in any field map" note was interior-only truth.
- Overworld facts: the paged layout format is uncompressed (1,024-byte pages
  of chunk ids, a rolling 4KB / four-page window keyed by camera row; chunk
  definitions still Kosinski via the record's normal list); both planets are
  128×128 chunks wrapping at 4,096 px on both axes; Dezolis stores only 8
  distinct pages and aliases its bottom half from its own rim (emitted
  as-is, recorded as an anomaly). Nine page-copy hooks rewrite layout cells
  from event flags — 12 patches, 51 writes, 147 cells — which is how the
  spaceports, Machine Center and The Edge doors appear and how the Bio Plant
  seals; the pack emits them as `layout_patches`, proven by applying each
  patch and watching doorway cells become type 1. Priority tiles (bit 15,
  which survives the collision-bit mask) draw above sprites on hardware; 339
  maps carry an overlay, and all 22 maps without one have exactly zero
  priority tiles.
- Field sprite facts, all read from the cartridge: walking is 8 frames per
  16px cell (`FieldObj_MovementsTbl` `0x047AA8`, constant `$0200`; slow/fast
  blocks `$0100`/`$0400` exist, and retail's selector mask is `#3` where the
  clone's is `#7`). The sprite-mapping record's piece count is stored minus
  one while animation sequence counts are stored exactly — two opposite
  conventions three bytes apart. Mapping byte 5 (mirror-builder X offset) is
  real data no retail field path ever reads. Pattern words are added with a
  genuine 16-bit carry (1,056 composed pieces depend on it). A sprite's CRAM
  line comes solely from its routine's `$13(a4)` byte; all 11 party routines
  store line 2, which `loc_53F14` splices from `Pal_Init_Line_3` on every
  map — why the party's colors never change. Animation free-runs (Chaz's
  4-frame cycle is 44 frames against 8-frame steps) and idle is walking's
  frame 0, not a separate sequence. AiedoPub object 2's facing byte `$10`
  sends `FieldObj_Animate` past its own table; the pack names those
  sequences `idle_facing_0x10`/`walk_facing_0x10` so nobody reads the byte
  as a direction.
- New-game findings (instruction-level provenance in the pack's
  game_start.json): retail hands over control with **Chaz alone** on
  PiataAcademy_F1 at cell (48,19) facing down — Alys is the NPC he finds;
  she leads only after Event_AlysFound. 500 meseta, empty inventory, event
  flag 7 set by the opening event, 11 unnamed extended event flags and three
  town flags preloaded by the initialiser (probably dialogue-state; recorded
  as ids, not guessed). The clone mislabels the initialiser's flag-table
  copy target as Chest_Flags where retail writes Extended_Event_Flags — a
  reading under which every new game would start with 11 chests looted —
  and carries Grand Cross start-position edits ($58/$22/right vs retail's
  $60/$24/down, self-documented by "; was" comments).
- Nine Enigma call sites are revision-gated and the cartridge runs the `else`
  (English) branch — proven three ways: the retail code contains each
  mapping's `lea`/`move.w #base` pair exactly once with the retail base
  values; the decoded cell counts match only the retail dimensions; and the
  `revision=0` bases would point mappings outside their art blobs.
  `GetMapLayoutChunkFG/BG` hard-code the 128-byte overworld stride and are
  overworld-only helpers despite their general names. `MapEni_GameStartMotaBG`
  / `ArtNem_GameStartMotaBG` are unreferenced in the disassembly source; the
  cartridge reaches them at `0x073C4E`.
- RETAIL CARTRIDGE BUG: `Battle_ProcessRUN` calls `Battle_CalculateChances`
  (`$00B5A6`) with `d5` — the critical threshold — uninitialised
  (`ps4.asm:7704-7712` loads only `d1-d4`). Dormant: the caller tests only
  the result's sign, and both "normal" (0) and "critical" (1) verdicts are
  non-negative, so whatever garbage `d5` holds cannot change the escape
  outcome. A port must document this rather than silently invent a value.
- RETAIL CARTRIDGE BUG: enemy skill 112 `BLACK WAVE` declares effect `$2C`,
  one past the end of the 44-entry `AbilityEffectsOffs` table (`$0061BE`,
  ids `$00-$2B`). The `TRAP #2` dispatcher (`$000216`) has no bounds check;
  dispatching it would jump to the odd address `$00B033` and raise a 68000
  address error — a crash. Dormant: the skill's only user is enemy 152
  `Zio3` (16383 HP, all-255 defences — a debug/leftover boss) which appears
  in zero of the 504+27 formations. Verdict for the port: reject effect ids
  outside `$00-$2B` at data-load time and record the one offender as a
  census anomaly. Full battle fact base: `docs/BATTLE_SCOUT.md`.
- RETAIL CARTRIDGE BUG: Wren's battle pose 3 (Charge) decompresses 42 words
  into a 36-word plane buffer. The per-slot buffers (`loc_9A80`) are `$48`
  bytes apart and the draw loop (`loc_86D6`) stamps 6x6, so the surplus 6
  words land in the next party slot's buffer and are never drawn — dormant
  unless the neighbouring slot isn't redrawn afterwards. Every other pose
  across all eleven characters is exactly 36 words. Found by
  `psiv_tools/battle_art.py`, which pins the anomaly rather than widening
  its size rule.
- [PARTIALLY CORRECTED — see the FLAG MODEL FINAL entry] RETAIL FINDING (2026-08-15, triple-verified): the cartridge has FOUR flag
  banks, not five. The clone defines `Temp_Event_Flags = $FFFFF156` with its
  own Test/Set/Clear doors — but the retail image contains ZERO instructions
  addressing $F156 (no `lea (xxx).w` = 41F8F156, no absolute-long
  0000F156, no word-lea to any address in $F141-$F15F), while the $F140
  (Chest_Flags) door shows exactly the site count the clone splits across
  its chest AND temp labels (Test/Set at four banks each — event, extended,
  $F140, town — Clear at three; nothing ever clears a town flag). So the
  retail "TempEveFlags" calls dispatch through the chest bank's door.
  ANSWERED by call-site bytes: every retail `move.w #$13,d0` (the clone's
  TempEveFlag_Xanafalgue) jsr-targets the $F140 doors — set 0x04AEA4 ->
  0x05767A, test 0x051E18 -> 0x057638, clear 0x0522BE -> 0x0576BC — with
  the id RAW, no offset. Retail temp flag N and chest flag N are the same
  bit. CANDIDATE RETAIL BUG (behavioral test pending): TempEveFlag_
  BioPlantAlarm = 8 = ChestFlag_Alshline, so the Bio Plant alarm's
  set-on-trigger / clear-on-exit plausibly re-arms or force-loots the
  Alshline chest on hardware. psiv-core's five-bank model must merge temp
  into the $F140 bank (one 256-id space) to reproduce retail. Found by the map-effects decoder
  (docs/MAP_EFFECTS.md finding 5); ROM byte sweeps re-verified
  independently by the lead.
- RETAIL CARTRIDGE BUG (a second instance of a known one):
  `Battle_BackgroundIndexes` (0x006CA8) is 416 bytes for the 417-map id
  space — the identical off-by-one `Battle_EnemyFormationIndexes` has.
  MapID `$1A0` reads the byte past the end (the first byte of the 32-entry
  post-step selector at `loc_6E48`, value 0 → MotaDesert). Dormant for the
  same reason: that map has random battles disabled. Two independently
  authored 416-byte tables over a 417-map space suggests the id-space count
  itself was wrong somewhere in Sega's build tooling. Found during battle
  background emission (battle/art/backgrounds).
- [RETRACTED — see the FLAG MODEL FINAL entry] The flag-bank alias (above) collides on SIX id pairs in actual use, not
  one: temp $08 BioPlantAlarm = chest Alshline, and temp $09-$0D — Vahal Fort's two
  moving platforms and two conveyor-direction terminals plus a Weapon
  Plant platform — = the PsycoWand, ControlKey, Canceller, EclpsTorch
  and AeroPrism chests. All six pairs live in ONE byte, $FFFFF141
  (masks $80/$40/$20/$10/$08/$04, MSB-first), already logged as the
  oracle's chestb1 column — the future mid-game experiment needs route
  reach only, and a single platform round trip demonstrates the alias
  BIDIRECTIONALLY (down sets, up clears the paired chest bit).
  Platform state and treasure state are one bit each on retail hardware —
  a platform left mid-cycle plausibly marks a chest looted (or re-arms
  it) elsewhere in the world. Full table and behavioural pin in
  psiv-core/src/state.rs; hardware confirmation queued with the oracle
  (those dungeons are late-game, so it waits for deeper routes).
  WIDENED (map-load-clear scout): six was the count among transcribed
  trigger tables only — the constants file shows essentially every temp
  id across $08..$1D doubling as a chest id (e.g. $14 GrbkTwEyeball =
  GrbrkTwStarDew, $18 ChazHouse = PiataMonomate), so the hardware-test
  framing is "the whole range", not an enumeration. Seven map-load
  routines clear twenty of these ids as explicit immediates
  (psiv-core/src/map_load.rs carries the transcribed table).
- [INTERPRETATION CORRECTED — the measurement stands; see the FLAG MODEL FINAL entry] The flag-bank alias is CONFIRMED ON HARDWARE (tape 17): the Xanafalgue
  temp flag ($13) lands at $FFFFF142 bit 4 — byte 2 of the CHEST bank,
  exactly where the reversed-bit arithmetic (`bset 7-(id&7)`) predicts —
  while the clone's fifth bank at $F156 stays zero for all 24,880 frames.
  The innocent explanation is ruled out, not assumed away: ItemFound never
  runs and the basement's own chests live in byte 3, which never moves.
  Measured consequence: walking the opening-act Piata basement WRITES THE
  GARUBERK TOWER MOON SLASHER CHEST'S FLAG (ChestFlag_GrbrkTwMoonSlashr =
  $13). Bounded claims: whether that chest reads as looted at Garuberk
  (unreachable by tape), and whether anything clears the bit on leaving
  the basement, are both untested — a leave-and-reenter tape is queued.
- [CHEST CONSEQUENCE RETRACTED — see the FLAG MODEL FINAL entry; the measured set/clear/respawn cycle stands] RETAIL CARTRIDGE BUG (measured both directions, tape 18): the basement
  round trip is a REPEATABLE UN-LOOTER. The Xanafalgue's flee sets chest
  bit $13 at despawn (same frame); arriving on the destination map clears
  it 39 frames after load (the clear is tied to LOADING the destination,
  not to leaving — whether every map clears id $13 or only some is
  untested); returning to the basement respawns the Xanafalgue. So a
  player who owns the Garuberk Tower Moon Slasher and later walks into
  the Piata Academy Basement and back out has that chest's flag CLEARED
  and can take the item again — reachable by ordinary backtracking, no
  sequence break. Remaining untested: that Garuberk's chest-open check
  reads this bit (route unreachable by tape; follows from the measured
  one-bank model). Bonus transcription datum: the fleeing Xanafalgue
  runs at 4 frames/cell — the fast step table. ENGINE NOTE: the flag
  set/clear/respawn cycle is the real model for the Xanafalgue's
  despawn; the runtime's interim session-permanent despawn ledger
  diverges (no respawn) and is slated for replacement by the map-effects
  flag gates plus the yet-unread map-load flag-clear routine.
- Item byte $12 has TWO readings (UpdateCharElems 0x05FD2A): held by a
  weapon hand it is the attack element Battle_LoadWpnAttackElem reads;
  worn as a shield/head/body item it instead writes resistance value 1
  into the element property it names — and the grant is unconditional,
  so armour can DOWNGRADE a character's innate immunity (0) to mere
  resistance (1). Reachable in play; the pack emits element.role per
  item and finished element_props per character as conformance vectors.
- Correction (2026-09-15): dual wielding is available through the normal
  hand selector at `loc_5F93A..loc_5FAEE`. Types 1, 2 and 5 offer right/left
  choice; `loc_5F99E` replaces the tentative default dispatch with that
  selection before inventory commit. The earlier initial-data-only claim
  stopped at `EquipItemType_OneHanded` and missed this later path. A left
  weapon and a right shield are both valid. `docs/EQUIP_SCOUT.md` records
  the repaired native flow and its verification.
- Citation correction: UpdateCharModStats is retail $05F754, not
  $05F880 ($05F880 is the unrelated routine UpdateEquipment tail-calls).
  The behaviour transcribed everywhere is correct; two docs carried the
  wrong address.
- CORRECTION to the BLACK WAVE entry: Zio3 IS fielded — boss formation
  `event_battle_index` 4, the scripted Zio fight (the earlier "zero of
  the 504+27 formations" searched only the 504 normal ones). Whether
  retail can actually dispatch effect $2C there (address-error crash) is
  UNSETTLED — thirty years without crash reports suggests the scripted
  fight ends before the AI reaches the skill, or the dispatch path
  differs; an oracle experiment waits on mid-game routes. The port's
  guard moves accordingly: the loader cannot hard-reject the record
  (that refuses the retail pack), so usable()/rejected() splits plus the
  runtime UnsupportedAbility event hold the line.
- Retail equipment bonuses go genuinely negative (agility to -5, mental
  and dexterity to -10), making the two-adder split live: add.b without
  sign extension for the four byte stats, ext.w signed for the three
  derived words. Both reproduced; the 11 conformance vectors exercise it.
- RETAIL CARTRIDGE BUG: Battle_OrderTurns' max-agility scan reads TEN
  words of the nine-entry Battle_Turn_Order table (ps4.asm:7779-7788) —
  one word past the end, into whatever follows at $FFFFEFD4. Dormant in
  effect (the stray word only matters if it exceeds every real agility);
  the port implements the intended nine and documents the deviation.

- FLAG MODEL FINAL (2026-08-15, three independent evidence lines): the
  clone mislabelled TWO banks, not one. Retail's four flag banks are:
  $F100 event (256), $F120 CHEST + "extended event" — ONE bank, two
  name-spaces on the same bits — $F140 TEMP (what the clone calls
  Chest_Flags), $F160 town. $F156 remains fiction. Evidence: (1) the
  chest system's three call sites all target the $F120 doors
  (LoadTreasureChests test 0x537E0, ItemFound entry test 0x66B2A,
  ItemFound set 0x66DE0; ItemFound's type dispatch is 0->F100,
  1->F120, else->F140), byte-verified twice independently; (2) tape 20's
  whole-64KB RAM diff — found blind, before the byte reading reached the
  oracle — shows ChestFlag_PiataMonomate (24) landing at $FFFFF123 bit 7,
  simultaneous with the item grant at ItemFound+10, and NOTHING in
  $F140-$F17F; (3) the pack census: the new-game initialiser's 11
  preloaded "extended event flags" are, eleven for eleven, real chests'
  flag ids (BioPlant_B2 x3, HuntersGuildStorage, KadaryStorageRoom,
  Nurvus_B3, Hangar, Kuran_F1, ClimCenter_F3, WeaponPlant_F2,
  TonoeBasement_B3) — RETAIL PRE-SETS ELEVEN CHESTS AT NEW GAME.
  MECHANISM CORRECTED by the clear-door sweep: there is NO arming
  scheme, because NOTHING in the cartridge clears a $F120 bit — the
  clear door's sole caller is the disassembly's own annotated dead
  routine. Chest flags are permanent, and the eleven are permanently
  DISABLED chests: seven hold HuntKnife across unrelated maps (plus
  100mst, ShortCake) — placeholder/duplicate records with defaulted
  items, switched off at boot so they never spawn lootable. And the
  REAL proof of one-bank-two-names is a second behavioural 11/11:
  every literal-id $F120 test site in story code is a real chest's
  flag ($08-$0D = EclpsTorch/FradeMantl/Canceller/PalmaRing/AeroPrism/
  RepairKit; $A1-$A5 = the five tower rings, MapUpdate_CourageTwChests
  its own routine) — chest writes, story READS. Deliberate reuse: the
  game asks "did the player take this item" by testing the chest's own
  bit. Port caution: retail's $F120 door takes the RAW id (no
  subi.w #$100 — the clone's is fiction), so any caller convention
  mixing $127-style combined ids with $27-style bank ids is where an
  off-by-$100 would hide; the engine pins the arithmetic. Consequences:
  temp and chest flags never collide (the six-pairs table and the
  basement-un-looter bug 11 retract — the measured $F142 set/clear/
  respawn cycle stands, it just gates only the Xanafalgue); the LIVE
  alias is chest <-> extended-event, same-id same-bit; chest open state
  renders via the object's facing_dir byte, derived from the flag at
  load; grant and flag write are simultaneous. Oracle process lesson
  kept in oracle/README: a negative about RAM is only as good as the
  watched range — whole-RAM diff before ever reporting an absence.
- CORRECTION to the shop-counter reading: the counter is NOT gated on
  collision type $C — loc_65D12 never touches the collision grid; the
  location table is keyed by the SHOPKEEPER OBJECT's position, and the
  $C correlation is incidental (52 counters on $C, 13 on ordinary floor,
  3 on solid — a runtime gating on collision would break thirteen
  shops). The pack binds each counter to its object, fail-closed: 65 of
  68 rows land exactly on a placement.
- Three shops were cut in development and their rows left behind: table
  rows 21-23 name Tonoe at (0,0), which no object occupies — shop
  inventories 9/10/11 are referenced only by those rows and are
  unreachable in play. The location table's 0..$30 coverage stands as
  bytes; the REACHABLE shop set is 46, not 49. Tonoe's only live
  counter is its inn.
- Shop economy facts (docs/SHOPS.md): sell price is exactly half,
  rounding down (read from the lsr, not assumed); stock is structurally
  unlimited (buy lists re-read from ROM, nothing decrements); the inn
  bill is rate x occupied party slots, DEAD MEMBERS BILLED, and a night
  is the game's only full-party revive (hp/tp/status incl. death/skill
  uses, party then vehicles); the Aiedo inn (selector 6) runs
  Event_GirlsSneakingOut instead of a night while Zio and GirlsCaught
  are both clear; there is NO church/clinic mechanism anywhere — three
  counter groups only, cure and revival are otherwise items/techniques.
  Shopkeeper greetings bypass the dialogue tree entirely (selector-
  assembled text + portrait tables, most shop objects carry
  dialogue_id 0).
- CANDIDATE RETAIL BUG (engine deviates under the bug policy): the talk
  probe has no invisible-object filter, so the invisible blocker walls
  stacked on the Academy Basement bosses (type $74, bit 3 set, dialogue
  id 0) plausibly open dialogue tree entry 0 — the principal's chain —
  when pressed at on hardware. The port makes invisible no-dialogue
  objects solid-but-silent (docs/FIELD_STATE.md, invisible-blockers
  section). Hardware confirmation tape filed with the oracle.


### 2026-09-12 — VISION and opening character skills

**RETAIL CARTRIDGE BUG 13 — VISION reads the caster's name.** The verified
US skill record 47 at `$2A9E98` uses stat selector zero and effect `$26`.
`Effect_SetupSkillParams` resolves zero through `AbilityStatsOffs` to stats
offset zero, then reads one byte there. `AbilityEffect_DexterityUp` adds
that byte to each eligible target's modified dexterity. Hahn's initial
encoded `H` is `$08`, so normal Vision adds 8, independently of his level
or mental stat. It replaces the previous dexterity buff.

Fresh oracle RAM probes on 2026-09-12 verified this on the US retail ROM.
Starting from tape 07's battle idle at frame 25000, a patched command round
has Alys and Chaz defend while Hahn uses Vision. At frame 26000, Chaz/Alys/
Hahn dexterity is `13/21/13`, up from `5/13/5`; Hahn's uses fall from 5 to 4.
Repeating with only Hahn's first name byte patched to `$1A` changes the
results to `31/39/31`, a +26 buff. These are controlled RAM-patched oracle
fixtures, not natural menu-input captures. The local receipts are
`build/native-skills/{vision,renamed-vision}.ram` and the corresponding
`oracle-*-vision.csv` logs.

The native fix gives Vision a constant +8 when its stat selector is zero.
A mod that supplies another supported stat selector uses that stat instead.
This follows the owner's existing obvious-bug policy in
`docs/RUNTIME_DESIGN.md`; the original JSON remains unchanged.

**RETAIL FINDINGS — VORTEX and EARTH.** A second patched command round uses
Vortex on enemy 6, Earth on enemy 7, and Vision on the party. Enemy 6's HP is
raised to 512 to expose separate hits. It drops once to 452 at frame 25181.
Enemy 7 gains status `$08` at frame 25284 while retaining agility 6; both
before and after, its HP is 25. Final remaining uses are Earth 2, Vortex 4,
Vision 4. See `build/native-skills/oracle-skills.csv`, `oracle-changes.json`,
and `skills.ram`. Vortex's three projectile sprites culminate in one damage
application; Earth's successful object timer sets sleep without the generic
sleep object's agility reduction. `Battle_RestoreStatsAtTurnEnd` rolls once
per occupied sleeping fighter, wakes on odd, restores modified agility, then
clears enemy paralysis without a roll.

**PORT BUG — alphabetical resistance census.** `psiv_tools/battle_pack.py`
emits `enemies.properties` as sorted names. The Rust bridge incorrectly used
that census as element-id order for both enemies and newly seated characters.
For example, Earth (element 11, psychic) read the mechanical immunity at
alphabetical slot 11, so ZoranBult could never be put to sleep. The bridge
now places named values in `ELEMENT_NAMES` cartridge order, validates the
complete name set, and checks all 153 enemy and eleven initialized character
records. This is a port repair, not a cartridge bug or a pack format change.

### 2026-09-12 — Battle items and Fission

**RETAIL FINDING — revival details.** Moon-Dew restores a dead human to
one-quarter maximum HP and preserves technique seal. A controlled US-ROM
command probe changes Hahn from HP 0/status `$1D` to HP 5/status `$90` at
maximum HP 21: seal plus a transient sprite flag. Sol-Dew on a living human
restores HP without curing ailments. Repair-Kit is disposable type 8,
although several equipment activations reuse other item-effect dispatchers.
All 160 item activation headers and disposal types were verified against
the ROM; local evidence is in `build/native-items/`.

**PORT BUG — dormant Igglanova neighbors.** `EnemyInit_Igglanova`
(`ps4.asm:18275`) destroys the adjacent fighter objects without clearing
their cached stats. The old port counted and rendered those full-HP records
as already present. A fresh, input-only retail Academy replay confirms the
boss is initially alone (frame 40650), then grows right/left Xanafalgues
(frames 41200/41850). See `build/native-fission/oracle-receipt.json`.

**RETAIL FINDING — Fission uses cached neighbor identity.**
`EnemyAI_EmptySpace` (`ps4.asm:23524`) uses one random parity draw only when
both sides are empty. `BattleObj_IgglanovaFission` / `loc_14CBE` restores
the chosen slot through `Battle_FillEnemyStats`, retaining its cached enemy
ID. In Guilgenova's formation this restores Gicefalgue, despite the nominal
Fission2 target byte. A replacement begins with status bit 7; the turn
executor's `$EE` mask (`loc_5772`) prevents a newly revived fighter from
acting on an old queued turn. The native engine uses an active-slot state
and same-round revival events, without leaking the transient bit into saves.

### 2026-09-12 — Dialogue continuations, choices and resting

**PORT BUG — scene dialogue ignored its caller's yield.** Retail `$F7`
reaches `TextCtrlCode_Terminate3` and saves the following text address before
returning to the event. The old Godot window immediately read the next chunk,
while the runtime automatically acknowledged `RunDialogueResume`. The
post-Igglanova entry (tree 33, entry 18) has two such breaks: Chaz turns
before the second chunk, and Hahn moves before the third. The native window
now preserves the original tree and cursor while those scene ops execute.

**RETAIL FINDING — choice offsets start after both operands.**
`TextCtrlCode_YesNo` (`ps4.asm:142780`) stores the selection in `Yes_No_Option`,
advances past both operands, and calls `GetOffsetByID`. Zero continues in
the current entry; positive values count subsequent `$FF` delimiters. Cancel
selects NO. `Event_ChazHouse` (`:149074`) consults that saved answer after the
response closes. Both branches set its temporary visit flag, but only YES
calls recovery. The port previously discarded the text branches and forced
the scene branch to YES.

**PORT BUG — rests left skills exhausted.** Both native scene recovery and
inn recovery omitted skill uses. `RecoverStats` / `DoCharRecovery`
(`ps4.asm:136503`) refill the current party's HP, TP and eight skill counts
and clear status. `DoVehicleRecovery` then refills all three saved vehicle
use banks; it does not write vehicle HP. The two native paths now share this
core operation, with real-pack tests for YES, NO, deferred answers, and inns.

### 2026-09-12 — Battle rewards across a connected Academy route

**PORT BUG — displayed meseta never reached the purse.**
`Battle_VictoryMessage` (`ps4.asm:4796..4806`, retail `$30E6` region)
adds the zero-extended 16-bit `$FFFF41D0` pool to `Current_Money` and
caps the result at 9,999,999. The native battle emitted that pool, and Godot
displayed it, but the runtime only absorbed stats and awarded experience.
The consumed battle now pays its pool on confirmed victory, including
vehicle battles; escaping with accumulated kills pays none. Runtime tests
cover the cap, save/reload, and duplicate result-close calls. The connected
START-to-Motavia development route now retains all 86 meseta from its four
battles, ending with 986 after Hahn's 100 and the principal's 300.

**PORT BUG — camp healing used a fixed midpoint and battle-like masks.**
The verified ROM contains `HealingEffectData` at `$67FA4` with exact bytes
`120f0000130e8000140d8000150088001600000017000000180f0000`.
`CalcHealingValue` (`$67FC0`) draws from UpdateRNGSeed, rejects repeats of
the previous masked value (initial zero), and accepts sixteen values.
`ItemUsed_AllAlliesLoop` calls it before inspecting a slot, including the
first empty slot; that empty slot then ends the loop.
Field masks therefore differ from combat: Moon Dew and Sol Dew clear all
status on eligible humans; ordinary healing masks with `0F`. Repair Kit's
explicit ID branch allows only Demi/Wren, while other restorative items
reject androids. Native camp now follows those rules. `Win_ItemUseCharListMain`
also calls `ReorderInventory` after clearing a used slot; the port had left
the hole as if this were a battle command.


### Native battle return and field ability recovery (2026-09-12)

`GameMode_LoadFieldMap` (`ps4.asm:107505`) retains field-object RAM when
`Map_Load_Flags` bit 0 is set, skips `LoadMapObjects` initialization
(`110924`), and still invokes `MapDataManager` (`107597`). Native battle
return now preserves the live cast while applying that map-data walk. The
Academy flag-$0B boss and invisible-block despawns were verified in Godot.

Field TECH/SKILL follow `loc_5FF98`, `Win_LoadTechList`, `SubtractTP`,
`Win_TechUsedMsg`, `loc_61AD4`, `loc_620D2`, and `GetItemTechSkillEffect`.
The field death test is bit 2, distinct from battle's combined death mask.
Both group recovery loops calculate before checking the first `$FF` party
slot, then return immediately. Skills read their selected modified stat;
RECOVER uses strength, MEDICE/MIRACLE mental, MEDIC PW strength. The latter
heals and revives humans, including living targets, while skipping androids.
All 17 field-usable records were compared to local retail ROM bytes; the
two teleport techniques still await implementation.

### Birth Valley enemy gameplay (2026-09-12)

`EnemyAttack_FlattrPlnt` selects Acid Breath for `$33`; the ROM ability at
`$2834FC` is `01 01 08 18 06 01 00 00`. `BattleObj_AcidBreath` guards its
single damage request with bit 1. Its child is animation-only.
`loc_B75A` supplies a normal hit without an accuracy draw, then
`Enemy_DamageCharacter` / `loc_26DA` / `loc_26F8` select battle strength,
defense and physical resistance. The existing 16-draw damage formula applies.
The objects write MoleAttack `$D5` and EnemyAttack4 `$D8`; native cues are
event-aligned, with the original object timings/art still pending.

Acid Breath's remaining carriers (2026-09-23). `EnemyAttackOffs` (`ps4.asm:19206`)
routes 75 FlattrPlnt, 76 FlyScreamr and 77 TechPlant to `EnemyAttack_FlattrPlnt`
and 85 Piercer and 86 HakenLeft to `EnemyAttack_Piercer` (`ps4.asm:21518`).
`generated/enemies.json` holds ability 51 in the regular lists of 75, 76, 85 and
86 only. That `$33` arm (`loc_F5BE`) has no enemy-id test and no write to
`Current_Target_Index` — unlike its `$34`/`$2A` arms and its `loc_F722` fallback —
so 75 and 76 run the same object chain against the drawn party target.
`EnemyAttack_Piercer`'s `$33` arm (`loc_F2A0`, `ps4.asm:21532`) also leaves
`Current_Target_Index` alone, but loads a different chain: object `$35C`
(`loc_23998`, `ps4.asm:47264`) with the target in `$38(a1)`, then child `$360`
(`loc_24FD2`, `ps4.asm:48883`), copied by `loc_24A20` (`ps4.asm:48452`). The
child makes the reaction write (`move.w #5, $2(a3)`, `$1C = $E`) and raises
`($FFFFEE80)` when it ends; `loc_23AB6` (`ps4.asm:47344`) then makes the same
single damage request as `BattleObj_AcidBreath`'s `loc_24AEC` (`ps4.asm:48507`)
exit — `move.w #$C, $2(a3)` into `Fighter_TakeDamage` (`ps4.asm:3564`), guarded
by bit 7 so it fires once, with `($FFFF416C)` as the handshake. Piercer's arm
writes `SFXID_MoleAttack` `$D5` and then `SFXID_EnemyAttack4` `$D8`;
FlattrPlnt's writes the same pair. `Enemy_DamageCharacter` reads the shared
`EnemySkillData` record, so both arms take the caster's own strength (88
FlyScreamr, 128 Piercer, 164 HakenLeft), the target's defense and its physical
resistance through the same formula, and neither adds a physical attack or a
status. Native: `ACID_BREATH_CARRIERS` in `rust/psiv-core/src/battle/enemy_skill.rs`
now covers 75/76/85/86; 77 TechPlant stays out because its US list never rolls
`$33`, so the gate cannot be reached by an unproven carrier.

`loc_5A98` tests target status `$C4` and `loc_5ACE` redraws from living party
members before the enemy ability roll. `Character_Dead` at `loc_84DE` masks
status with `$10`, then sets bit 6 for androids or bit 2 for humans.
`Battle_DoAttackEffect` only dispatches a plain attack's status after damage
and a successful hit. All nonzero enemy attack-status records are 27/28.
`AbilityEffect_Poison` / `AbilityEffect_Paralyze` pass target offset `$48`;
`Effect_DoPhysicalAttack` compares battle strength to battle strength with
threshold `$70`. Existing poison/paralysis skips the draw, while immunity
still runs it. Paralysis clears sleep bit 3 and sets agility/dexterity to 1.

### Post-Rika overworld crossing and recovery (2026-09-23)

US retail `$053D16..$053D39` (`loc_53D16`) tests event `$35` before writing
FG `$00` and BG `$48` at chunk `(42,33)`. This is a paged-overworld loader
hook, not a MapDataManager entry. It opens Motavia cells `(84..85,66..67)`:
the unpatched collision is water `$9`, and the final BG chunk supplies `$0`.
The prior native runtime ignored the extracted hook list. The repaired pack
and Rust consumer retain the source records, compose both planes and apply
flag-gated collision, raw chunk identity and render patches on map build.
Same-map live page streaming after a flag change remains unimplemented; see
[MAP_EFFECTS.md](docs/MAP_EFFECTS.md#12-native-overworld-page-hook-consumption-2026-09-23).

Zema's inn row `$068116` is `01 14`, rate 20 per occupied party slot; its
keeper row is `$0683A4`. Five members pay 100. `RecoverStats` starts at
`$0662DA`, not `$0662FE`: the latter is its call to `DoVehicleRecovery`.
`DoCharRecovery` at `$066306` restores HP, TP, persistent status and all eight
skill-use counters. The older address in SHOPS.md and shop_flow.py's docstring
is corrected; no recovery rule changed.

Raw byte receipts and the retained native failure live under
`build/native-post-rika-20260923/`. The original bridge video comparison uses
explicitly isolated SRAM position fixtures, not connected cartridge play.
Their Y word is `(standing_y-1)*16`, accounting for the separately documented
legacy native save-coordinate gap without modifying any campaign save.

### FloatMine2's regular Fission2 roll (2026-09-24)

**RETAIL FINDING — `EnemyAttack_FloatMine`'s fall-through spends the turn.**
`EnemyAttackOffs` `$32` (line 19257) sends 50 FloatMine2 to
`EnemyAttack_FloatMine` (`ps4.asm:22675`), whose only arms test `$24(a4)` for
`$14` (line 22677), `$18` (22690), `$19` (22707) and `$1A` (22766). Its eight
regular slots are `07 07 07 07 17 17 17 17` (`$58(a3,d0.w)`, line 19153 in
`Enemy_Attack`, `ps4.asm:19138`; `generated/enemies.json` id 50): the index roll
always lands on one of those two, neither of which is an arm, so this enemy
never loads an attack object at all and reaches the fall-through `loc_10406`
(`ps4.asm:22781`) every time:

```
	movea.l	$38(a1), a0
	clr.w	(Current_Target_Index).l
	clr.w	$24(a4)
	move.w	#$16, (Battle_Routine).l
	subq.w	#2, $2(a4)
	rts
```

No object, no `LoadPLC1`, no palette write, no `Sound_Index`, no message window.
`$16` is `Battle_DoAttackEffect` (`ps4.asm:8553`; table entry at line 7536,
`BattleRoutines` `ps4.asm:7524`); the ability word it reads is the one just
cleared and `loc_B6A2` (`ps4.asm:17492`) left all nine `Fighters_Hit_Flags`
(`$FFFF4150`, `ps4.constants.asm:2019`) at `$FF`, so it takes `loc_5DD2`
(`ps4.asm:8625`) → `Battle_Routine` `$12` = `loc_6672` (`ps4.asm:9689`) → `$1E` =
`loc_66B8` (`ps4.asm:9709`). `Ability_GetEffectAndRange` is never called for that
turn: no damage, no status, no ailment, and the `$12`/`$1E` pair does the ordinary
end-of-action wait and advances the turn order. The actor's `fighter_routine`
drops 6 → 4 (`Fighter_DoNothing`) exactly as the shared `loc_D200`
(`ps4.asm:19395`) tail does after a real attack, so the turn is *spent*, not
skipped. `$17` Waiting is the same path for 44 FloatMine, 46 VopalSphre and 50
FloatMine2 — which is what the ability's name says — while `$19` on 45 CommndBall
(the routine's fourth carrier) has an arm and is unaffected.

**Draw accounting — nothing after the ability index.** `Enemy_Attack` spends the
index roll (`loc_CFE6`, rerolling the index only), then the AI instruction block
at `$50(a3)`..`$53(a3)`; all four of FloatMine2's bytes are 7 =
`EnemyAI_PhysicalAtkReceived` (`ps4.asm:22816`, table `ps4.asm:19364`), which
draws nothing and only replaces the rolled id when `reaction_flags` bit 0 is set.
Back in `Enemy_Attack`, `loc_B6A2` clears the flags and, with
`Current_Target_Index` now 0, runs its slot loop **once** for `d6 = 0`
(`moveq #0, d7` then `bpl.s loc_B6D4`): `Battle_GetFighterAddr(0)` is
`Obj_Fighters - $40` = `$FFFF43C0`, the phantom slot 0 of the 1-based shadow
array `loc_5C04` (line 8454) indexes into. The battle start wipes
`$FFFF4000`-`$FFFF47FF` (lines 9989-9993; `Trap00Exception`, line 161, clears
`d7+1` longwords), and every write to that array in the disassembly lands at
`$FFFF43C0 + n*$40` with n ≥ 1 (`loc_2515C`, line 49017, pre-increments;
`loc_2A88`, line 4241, starts at `$40(a1)`), so the phantom slot reads 0: empty
slots consume no chance roll, and the verdict byte lands in
`Fighters_Hit_Flags[-1]` (`$FFFF414F`), which nothing reads. Nine ordering rolls,
four enemy-target rolls, the ability index — then nothing.

**Fork check.** No `if bugfixes`/`if grand_cross` arm exists between line 18356
and the routine at 22675, nor inside it: the body above is the disassembly's one
unconditional body, so there is no alternative arm a retail build would take and
no fork-only branch to strip. The enemy ability bytes and the routine's arm ids
agree with the ROM-extracted `generated/` tables. No emulator, tape or hardware
capture backs this entry, and none is claimed: it is a static read of the
disassembly plus the extracted records.

**PORT CHANGE.** `enemy_skill::resolve_no_effect_turn` now witnesses that
fall-through: it requires the record to be `$07` Fission2 (`is_fission`) or `$17`
Waiting (`is_waiting`, record 23 = `22 00 00 00 00 00 00 00` at `0x28341C`) and
the actor's enemy id to be one of the four `EnemyAttack_FloatMine` carriers (44,
45, 46, 50), emits `BattleEvent::EnemyAbilityWasted { actor, ability, name }` and
clears the actor's ability slot (`clr.w $24(a4)`), so the physical fallback no
longer runs for those two ids. 12/13 Fission and everything else keep their
existing path, `fission_neighbor` included. Evidence: core
`enemy_skill_tests::floatmine2_rolling_fission2_spends_the_turn_without_an_effect`
(14 draws for the round; no `Attacked`, `Resolved` or `UnsupportedAbility`),
`waiting_spends_the_turn_for_every_float_mine_carrier` (44/46/50),
`the_no_effect_witness_needs_the_traced_record_and_carrier`, and the negative
controls `an_unproven_float_mine_arm_still_falls_back_to_a_physical_attack` and
`a_non_carrier_keeps_the_fallback_for_the_fission2_roll`; runtime
`combat_fission::floatmine2_formations_spend_fission2_and_waiting_turns_without_a_swing`
drives pack formation 263 (FloatMine2, Tower, FloatMine2 — Tower's slots are all
zero) and sees both `$07` and `$17` in the trace, and
`a_float_mine_arm_on_a_real_formation_keeps_the_physical_fallback` pins formation
292 (45 CommndBall's `$19`).

**Two deliberate limits.** (1) The port emits one event per turn and models no
per-frame timing; `loc_6672` also sets the `$FFFF418A` wait to `$F` when the
ability slot is empty, and `loc_66B8` decrements it once a frame, so retail
advances the turn sixteen frames later than after an attack. That is pacing from
the unimplemented object-timing layer (`docs/BATTLE_ANIMATIONS.md`), not a rule.
(2) `EnemyAI_PhysicalAtkReceived` and
`reaction_flags` are not modelled: a FloatMine2 hit physically since its last
action has its rolled `$07` replaced in retail by the `$18` Explosion conditional
(`$54(a3)`), an arm whose object is still untraced, so the port spends the turn
there too. `docs/ENEMY_ABILITIES.md` records both limits under Port gaps.
