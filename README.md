# psiv-tools proof of concept

A deliberately small, dependency-free extractor for the verified US retail ROM of **Phantasy Star IV**.

This is an archaeology/provenance tool, not an emulator and not a game engine. It proves that documented PSIV structures can be decoded directly from the retail cartridge into ordinary modern data without shipping Sega's ROM or extracted assets in the tool itself.

## Supported ROM

This PoC intentionally accepts only the verified US retail image with SHA-256:

`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`

The ROM is **not included** and should never be committed to this project.

## What it extracts now

- Mega Drive / Genesis ROM metadata and checksum
- 11 initial character records
- 160 inventory/equipment records
- 153 enemy records
- 112 enemy-skill records, with enemy AI ability IDs resolved to stable symbols
- all 937 character level-progression records through level 99
- 40 techniques
- 54 skills
- 15 combo-effect records
- 3 vehicle records
- 504 battle formations plus 27 boss formations, Kosinski-decompressed from the retail blobs
- 68 encounter formation-index groups (32 candidate formations each)
- 49 shop inventories plus the shop-location table binding each shop to a map position (inns hold a separate index space)
- 68 Nemesis-compressed art blobs (10,434 tiles): all 36 dialogue portraits, 20 battle backgrounds, fonts, title art — decoded and verifiable, with a `python -m psiv_tools art` command rendering them to PNG sheets locally
- Mega Drive CRAM palettes: the init/title palettes plus the 31-entry battle-background palette table, with the original 3-bit levels preserved next to widened RGB
- 10 localized name tables decoded straight from the cartridge (153 enemies, 112 enemy skills, 160 items twice in two fonts, 40 techniques, 54 skills, 54 places, 14 combos, 11 characters, 8 professions), attached to their records as `display_name` alongside the disassembly `symbol`
- all 43 Kosinski-compressed dialogue trees: 2,736 entries with control codes fully identified and preserved (zero unknown bytes across the corpus)
- all 417 map records (361 real, 56 null): tilesets, sprites, dimensions, warps, NPC objects, treasure chests, tile animations, dialogue-tree binding, interaction areas, events, palettes, and per-map flags, walked with the loader's own grammar
- per-map encounter binding: the 416-byte map→group table plus the two Kosinski position grids for the overworlds, cross-validated against the 68 encounter groups
- map layouts and collision: 32×32-pixel chunks, per-plane layouts, and the 4-bit-per-cell collision grid, proven by rendering Piata/PiataItemShop/IslandCave and cross-checking warp doorways against collision type 1
- 35 Enigma-compressed plane mappings (35,436 cells): 20 battle backgrounds, the title screen, the Sega logo and the four title portraits, composed against their art and palettes into finished PNGs by `python -m psiv_tools planes`
- a runtime pack (`python -m psiv_tools pack`, gitignored output): all 361 maps — every interior plus both overworld planets — as lean per-map JSON + composed PNG + a priority-overlay PNG for the Rust runtime — collision grids, warp trigger rects transcribed from the 15 `XYRangeJmpTbl` routines (retail uses 9), NPCs, treasure, and a census of what the data actually contains vs what the code permits
- field sprites (pack format 1): all 11 party sheets and 183 deduplicated NPC sheets with animation sequences transcribed from the cartridge's own `SprMapsPtrs_*`/`FieldObj_Animate` tables — per-frame tick durations, H-flip mirrors, per-object CRAM-line palettes proven from the `$13(a4)` byte, and the 91 invisible trigger objects classified with reasons instead of drawn
- exact ROM offsets and raw bytes for every extracted record
- signature checks tying the implementation to known bytes in this exact retail build
- an exact-mirror check for the duplicated 20,614-byte character level-table block

The item and enemy `symbol` fields are stable identifiers transcribed from the public disassembly constants. They are **not yet cartridge-localized display strings**. A later text-decoder slice should recover the actual encoded in-game names.

## Run it

From the project directory, with Python 3.10+:

```bash
python -m psiv_tools inspect "/path/to/Phantasy Star IV (USA).md"
python -m psiv_tools extract "/path/to/Phantasy Star IV (USA).md" generated
```

The extract command writes:

```text
generated/
├── metadata.json
├── layout_validation.json
├── tables.json
├── characters.json
├── items.json
├── enemies.json
├── enemy_skills.json
├── progression.json
├── techniques.json
├── skills.json
├── combos.json
├── vehicles.json
├── formations.json
├── formation_indexes.json
├── shops.json
├── graphics.json
├── names.json
├── dialogue.json
├── maps.json
├── encounters.json
└── planes.json
```

## Proven retail-layout tables used by this PoC

| Table | ROM offset | Record size | Count |
|---|---:|---:|---:|
| Enemy data | `0x2816BC` | 48 | 153 |
| Enemy skills | `0x28336C` | 8 | 112 |
| Character level pointer table | `0x004074` | 6 | 11 |
| Primary character level records | `0x2856B0` | 22 | 937 total | 
| Combos | `0x285424` | 8 | 15 |
| Vehicles | `0x2855E2` | 26 | 3 |
| Initial character stats | `0x2A8ACA` | 66 | 11 |
| Inventory/equipment | `0x2A8E28` | 22 | 160 |
| Techniques | `0x2A9BE8` | 8 | 40 |
| Skills | `0x2A9D28` | 8 | 54 |
| Shop inventories | `0x0681A4` | variable, `$FF`-terminated | 49 |
| Shop locations | `0x068394` | 8 | 68 entries incl. 18 inns |
| Map pointer table (`FieldMapPtrs`) | `0x100000` | 4 | 417 |
| Map→encounter-group table | `0x008050` | 1 | 416 (sic — see SOURCE_NOTES) |

The 937 level records are split across 11 per-character tables. Their start addresses and starting levels are read from the pointer table at `0x004074`; the extractor does not hard-code each individual table address.

### Kosinski-compressed blobs

These are variable-length compressed streams, not fixed-size record tables. Ranges are end-exclusive.

| Blob | ROM range | Compressed | Decompressed | Records |
|---|---|---:|---:|---:|
| `Battle_FormationIndexes` | `0x2836EC..0x283E67` | 1,915 | 4,352 | 68 groups |
| `Battle_FormationData1` | `0x283E6C..0x2842AE` | 1,090 | 1,708 | 128 |
| `Battle_FormationData2` | `0x2842BC..0x284713` | 1,111 | 1,716 | 128 |
| `Battle_FormationData3` | `0x28471C..0x284B7E` | 1,122 | 1,678 | 128 |
| `Battle_FormationData4` | `0x284B8C..0x284F7C` | 1,008 | 1,540 | 120 |
| `Battle_BossFormationData` | `0x284F7C..0x285012` | 150 | 300 | 27 |
| `DialogueTree1..43` | `0x1DF600..0x1FE655` | 43 blobs | — | 2,736 entries |

### Weird-but-useful duplicated level data

The primary level-table block occupies `0x2856B0..0x28A735` (20,614 bytes). The ROM contains a byte-for-byte identical mirror beginning at `0x2A3A42`, ending at `0x2A8AC7`, immediately before the initial character data. The extractor verifies this equality and reports it in `progression.json`.

## Graphics

`psiv_tools/nemesis.py` is transcribed from the game's own `NemDecomp` routine, the same way `kosinski.py` was. Its three load-bearing quirks are documented in `SOURCE_NOTES.md`; the short version is that output length comes from the header alone, the routine reads one lookahead byte it may never use (so consumed length is exact-or-plus-one), and XOR mode accumulates over the whole blob.

Located art: 12 named singletons (fonts, title/Sega art, window tiles), all 36 dialogue portraits via the pointer table at `0x06A4B0`, and 20 distinct battle backgrounds via the 32-entry table at `0x006ED4` — 68 blobs, 10,434 tiles. Battle backgrounds carry an internal length oracle: each art blob's Enigma plane mapping is stored immediately after it, so the compressed length is derivable from the cartridge alone. Portraits are 36 row-major tiles (48×48). Battle palettes are proven to occupy CRAM line 0 indices 1–13 with index 0 forced black.

`extract` emits only metadata (offsets, sizes, tile counts, sha256s, palette values) — decoded pixels never enter committed files. `python -m psiv_tools art <rom> <outdir>` renders the sheets to PNG locally.

Art composes now: `psiv_tools/enigma.py` (transcribed from the game's own `EniDecomp`, which implements a reduced Enigma — V/H flips only) decodes the plane mappings, and `psiv_tools/planes.py` composes art + mapping + palette into finished screens. Art without a proven palette still renders against a grayscale index ramp rather than a guessed line.

## Map layouts and collision

Field maps are built from 32×32-pixel chunks: 16 Mega Drive pattern-name words each, Kosinski-compressed and loaded back to back into `Chunk_Table`. Each plane's layout is one byte per chunk, also Kosinski, and the map record says which plane the collision reader uses. Bit 14 of a chunk word — the bit that would be the high palette-select bit — is a collision flag the game masks off before the VDP sees it (so field tiles can only use CRAM lines 0–1), and the four flags of a 2×2-tile cell spell out a 4-bit collision type per 16 pixels (0 normal, 1 map change, 2 recovery, 8 solid, 9 water, $A sand, $B ice, $C shop; 8–C block). Proven on Piata, PiataItemShop and IslandCave; `generated/layouts/piata.png` renders the town from the cartridge, all six of its doorways land on collision type 1, and the shop-location table's counter position lands on a shop cell. The two overworlds use a paged format: uncompressed 1,024-byte pages of chunk ids streamed through a rolling 4KB window as the camera moves, 128×128 chunks per plane, wrapping at 4,096 px on both axes — the planet is a globe. Nine page-copy hooks rewrite layout cells from event flags (spaceports appearing, the Bio Plant sealing); the pack emits those as `layout_patches`. Priority tiles (bit 15, which survives the collision-bit mask) draw above sprites on hardware; every map with priority tiles gets a transparent overlay PNG so the renderer reproduces that ordering.

## Battle formations

Formation records are variable length and concatenated with no index. The game finds record N by counting `$FF` terminators from the start of the decompressed block, and formation ids are global across the four blocks: block 1 holds `0x000..0x07F`, block 2 `0x080..0x0FF`, block 3 `0x100..0x17F`, block 4 `0x180..0x1F7`. Boss formations are a separate id space keyed by event battle index.

The decompressor is transcribed branch-for-branch from `KosDecomp` in the public disassembly rather than from a general description of the format, and it reports how many compressed bytes it consumed so every blob's length can be checked against the ROM map.

Two things worth knowing about the source data:

- The disassembly's inline range annotation for `Battle_FormationData4` stops at `0x284C3D`, which is only the end of its first annotated chunk. The stream actually runs to `0x284F7C`, where `Battle_BossFormationData` begins. The extractor verifies this by requiring the decompressor to consume exactly the documented range.
- Formation `0x177` (block 3 record `$77`) declares four enemies in byte 4 but lists three enemy/position pairs. The disassembly's uncompressed source carries the same bytes, so this is the ROM's own inconsistency. Records expose both the declared `enemy_count` and an `enemy_count_matches_entries` flag rather than silently trusting one of them.

## Data-model choices

- All records preserve `rom_offset` and `raw_hex` provenance.
- Equipment stat bonuses are decoded as signed 8-bit values. This matters: Shadow Blade encodes negative Mental/Agility/Dexterity modifiers.
- Item effect bytes whose semantics are not uniformly safe across every item class remain conservatively named as parameters instead of being over-interpreted.
- Enemy ability IDs are preserved *and* resolved to enemy-skill symbols; raw numeric IDs remain alongside them.
- Item/enemy symbols come from the disassembly constants, while actual localized game text remains a separate extraction problem.
- Enemy ids are 0-based (`EnemyID_Helex = 0`) while item ids are 1-based (`ItemID_Dagger = 1`). Both conventions come from `ps4.constants.asm` and both are asserted in the tests; formation records use both, so the difference is load-bearing.
- Decompressed records carry their containing blob's ROM range and a SHA-256 of its compressed bytes, since a decompressed record has no ROM offset of its own.

## Tests

With the ROM fixture at the repository root (gitignored):

```bash
PYTHONPATH=. python -m unittest discover -s tests -v
```

The current suite covers ROM identity/checksum, known-layout signatures, dataset counts, representative character/ability/item/enemy records, signed equipment bonuses, level-table pointers, level 2 data, the full level-table mirror, and exact table boundaries.

The Kosinski decoder tests need no ROM: they run against hand-built streams covering literal runs, inline matches, full matches, extended counts, the extra-byte 0/1 distinction, the end marker, and the description-field reload boundary. The formation tests additionally round-trip every decompressed blob against the uncompressed `dc.b` sources in `reference/ps4disasm/battles/`, which is the acceptance criterion for the decoder.

`reference/` is a local, gitignored clone of the public disassembly. The two oracle-backed tests skip with an explicit message when it is missing; restore it with:

```bash
git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm
```

## Why Python first?

The long-term runtime can still be Rust/Godot. This first pass is Python so the extractor can be executed and tested immediately. Once formats are proven, porting fixed-width big-endian readers and structs to Rust is intentionally boring.

## Next useful slice

Filed from the shops slice: the per-shop/per-inn greeting selectors (`loc_68136`, 49 words; `loc_68112`, 18 words) choose dialogue strings and belong with the text-decoder work, not with shops.

Filed from the graphics slice: the seven shopkeeper portraits reached via the shop tables; uncompressed field/battle character sprites and the field map palettes.

Filed from wave 2: the 43 character battle-sprite mappings (decode cleanly, not yet composed with their art); the Sega-logo/GameStartMotaBG palettes; `MapDataManager`/`MapUpdate`/`RunEvents` handler-name transcription (a MapDataManager lane would close all event-gated doors at once — Zema's, the overworld spaceports, and Dezolis' chunk-definition swaps); `Map_General_Var` semantics; whether any NPC routine sets the high-priority sprite bit (bit 4 of the flag byte — such a sprite belongs above the priority overlay).

Filed from the sprite slice: NPC wander boundaries (73 object types carry roam boxes; the random-direction routine and cadence are untranscribed, and half a model is worse than none); vehicle sprites (spawn from `Vehicle_Index`, never map-placed); sprite priority (unused by placed retail objects).

Filed from the oracle slice: a psiv-core tape-replay entry point emitting the oracle's CSV columns, so golden logs become CI fixtures with no emulator in the loop (comparator design in oracle/README.md); the two remaining tapes (accept-press during the 9-frame open animation; a staged beside-talk press); scroll-arrow timing.

Filed from the interaction slice: shop-counter reach (`Interaction_ChkObjsSpecial` extends talk range one further cell across a `$C` counter tile and routes to the shop system — belongs with the shop-UI slice; until then shopkeepers behind counters are out of range); the per-object interactable runtime flag (bit 3 of `$2(a3)`, not in the pack; every object currently answers).

Filed from the text slice: binding dialogue ids to the maps/NPCs that speak them (the `dc.l DialogueTreeN` pointers live in map headers); resolving `$F5`/`$FA`/`$FB` relative branch targets to absolute dialogue ids; the id spaces behind `$F2` action operands (panels, sounds, event flags).

1. Save/SRAM parsing and import (needs a real emulator save as a fixture).
2. The filed items above, as needed by the runtime.

Maps, layouts, collision, encounter binding, and all three of Sega's compression formats are done and proven.

## The Rust runtime

`rust/` holds a Cargo workspace (see `docs/RUNTIME_DESIGN.md` for the design record): `psiv-data` (fail-closed schema over the runtime pack), `psiv-core` (the deterministic field engine — integer-only, dependency-free, floats banned by the compiler), and `psiv-runtime` (the bridge and game shell that `psiv-godot` will drive). The headless golden path passes: spawn in Piata, walk into the academy doorway, and the engine warps to `MapID_PiataAcademy` at exactly the cell and facing the cartridge's transition table stores; all 359 packed maps convert into live engine maps. `cargo test` from `rust/`.
