# Cartridge formats

Scope: the byte-level formats the extractor decodes from the retail
image — the two font encodings, Kosinski, Nemesis and Enigma
decompression, palette encoding, plane mappings, and map records,
layouts and collision.

Index: [source and provenance notes](README.md).

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
