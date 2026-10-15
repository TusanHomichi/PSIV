# ROM extraction reference

[Project overview](../README.md) · [Documentation index](README.md)

The Python extractor decodes the verified US cartridge into local data and assets.
Commands below run from the repository root.

## Supported ROM

The extractor accepts only the verified US retail image with SHA-256:

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

The item and enemy `symbol` fields are stable identifiers transcribed from the public disassembly constants. Records also carry `display_name`, decoded from the cartridge's localized name tables; symbols remain alongside them because they disambiguate duplicate display names.

## Run it

From the project directory, with Python 3.10+:

```bash
python -m psiv_tools inspect "/path/to/Phantasy Star IV (USA).md"
python -m psiv_tools extract "/path/to/Phantasy Star IV (USA).md" generated
```

`extract` writes only the JSON tables. To rebuild the whole of `generated/`,
with its stamps, use [`regenerate`](#rebuild-the-generated-directory).

`extract` writes the JSON tables; `regenerate` also writes the PNG directories
and a stamp file for each. The whole of `generated/`:

```text
generated/
├── metadata.json                (carries the extract stamp, with every table's SHA-256)
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
├── planes.json
├── gfx/                         (art sheets, PNG)
│   └── extract_stamp.json
├── planes/                      (composed plane screens, PNG)
│   └── extract_stamp.json
├── battle_art/                  (enemies/ and characters/, PNG)
│   └── extract_stamp.json
└── layouts/                     (map renders, PNG; not rebuilt by regenerate, see below)
    └── extract_stamp.json
```

## Rebuild the generated directory

`generated/` is the output of four producers: `extract` (the JSON tables), `art`
(`gfx/`), `planes` (`planes/`) and `battle-art` (`battle_art/`). One command runs all of them, in that
order, into one directory:

```bash
python3 -m psiv_tools regenerate "/path/to/Phantasy Star IV (USA).md" generated
```

It prints each step's duration. This is the way to rebuild `generated/`; run it
after any change under `psiv_tools/` that the extractor reaches, and before a
tool that reads the tables.

Every output is stamped with the extractor that wrote it, so a reader cannot
use an older extract without noticing:

- The stamp is a SHA-256 over the source of every `psiv_tools` module the
  producing command can reach through its imports, with the per-module hashes
  beside it. The JSON tables share one stamp, in `metadata.json` under
  `extract_stamp`: `core` imports every extractor, so the import graph gives
  each table the same modules and a per-file stamp would claim precision the
  graph does not have. Each PNG directory carries its own `extract_stamp.json`,
  written by the exporter that writes the images (`export_art_pngs`,
  `export_plane_pngs`, `export_battle_art_pngs` and `export_map_pngs`).
- The stamp also lists every file its producer wrote, with the file's SHA-256, so
  it vouches for contents and not only for the directory. A table the stamp does
  not list, or whose bytes differ from the listed hash, is refused: an old table
  left behind by an extractor that dropped it, or a stale file copied over a
  current one, cannot load under a current stamp. A PNG directory that holds a
  file its stamp does not list, lacks one it lists, or holds a changed one is
  refused as a whole.
- A reader opens a table with `psiv_tools.extract_stamp.load_table`. A present
  extract with no stamp, with another source's stamp, or failing the content
  check above raises `StaleExtractError`, naming the file, what is wrong with it
  and the command above. A stale stamp refuses every table name, including one
  a newer extractor adds, and a table the stamp lists but which has been deleted
  is refused too. Only a checkout with no `generated/` at all (or a name nothing
  lists and no file holds) gives an ordinary `FileNotFoundError`, so a check that
  skips without the local inputs still skips. `tests/test_extract_stamp.py` fails when any other module reads
  `generated/`.
- Editing an extractor module, even a pure refactor, changes the stamp and
  makes the next read refuse until `regenerate` runs. That is the price of never
  missing a drift. `metadata.json` is written last, so an interrupted run leaves
  tables that refuse rather than ones that look current.
- Each producer first removes the files its *previous* stamp listed and then
  writes its own; a file the new run no longer produces does not stay behind. It
  removes nothing else and never clears a directory, so a file placed there by
  hand survives and a PNG directory holding one is refused until it is removed.
- **No code reads the PNG directories today.** Their stamps and
  `check_png_directory` are there for the first reader, which must call it. Only
  the JSON tables have readers (the battle-forcing pack, the route and player
  ability derivations, the replay pack, the sweep coverage and the dialogue
  census).
- `layouts/` is written by `export_map_pngs` for the maps someone chooses and
  has no command of its own, so `regenerate` does not touch it
  ([#106](https://github.com/TusanHomichi/PSIV/issues/106) tracks the missing
  producer). The exporter stamps it, and refuses to add a map to a directory
  that holds images from another source.

## Proven retail-layout tables

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
| Map→encounter-group table | `0x008050` | 1 | 416 (sic — see the source notes) |

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

`psiv_tools/nemesis.py` is transcribed from the game's own `NemDecomp` routine, the same way `kosinski.py` was. Its three load-bearing quirks are documented in `docs/source-notes/formats.md`; the short version is that output length comes from the header alone, the routine reads one lookahead byte it may never use (so consumed length is exact-or-plus-one), and XOR mode accumulates over the whole blob.

Located art: 12 named singletons (fonts, title/Sega art, window tiles), all 36 dialogue portraits via the pointer table at `0x06A4B0`, and 20 distinct battle backgrounds via the 32-entry table at `0x006ED4` — 68 blobs, 10,434 tiles. Battle backgrounds carry an internal length oracle: each art blob's Enigma plane mapping is stored immediately after it, so the compressed length is derivable from the cartridge alone. Portraits are 36 row-major tiles (48×48). Battle palettes are proven to occupy CRAM line 0 indices 1–13 with index 0 forced black.

`extract` emits only metadata (offsets, sizes, tile counts, sha256s, palette values) — decoded pixels never enter committed files. `python -m psiv_tools art <rom> <outdir>` renders the sheets to PNG locally.

Art composes now: `psiv_tools/enigma.py` (transcribed from the game's own `EniDecomp`, which implements a reduced Enigma — V/H flips only) decodes the plane mappings, and `psiv_tools/planes.py` composes art + mapping + palette into finished screens. Art without a proven palette still renders against a grayscale index ramp rather than a guessed line.

## Map layouts and collision

Per-map `MapUpdateJmpTbl` lists now ship as ordered `map_updates` programs,
with extracted CRAM tables/sine inputs, a 64-word palette shadow and additive
index-image sidecars. `manifest.map_updates` accounts for all 64 entries and
361 real maps; three entries explicitly name missing runtime buffers.
Original map images and all pre-existing map fields remain unchanged.
See [the census and frame contract](field/MAP_UPDATES.md) for retail citations,
supported classes, negative controls and full-pack comparison evidence.

Field maps are built from 32×32-pixel chunks: 16 Mega Drive pattern-name words each, Kosinski-compressed and loaded back to back into `Chunk_Table`. Each plane's layout is one byte per chunk, also Kosinski, and the map record says which plane the collision reader uses. Bit 14 of a chunk word — the bit that would be the high palette-select bit — is a collision flag the game masks off before the VDP sees it (so field tiles can only use CRAM lines 0–1), and the four flags of a 2×2-tile cell spell out a 4-bit collision type per 16 pixels (0 normal, 1 map change, 2 recovery, 8 solid, 9 water, $A sand, $B ice, $C shop; 8–C block). Proven on Piata, PiataItemShop and IslandCave; `generated/layouts/piata.png` renders the town from the cartridge, all six of its doorways land on collision type 1, and the shop-location table's counter position lands on a shop cell. The two overworlds use a paged format: uncompressed 1,024-byte pages of chunk ids streamed through a rolling 4KB window as the camera moves, 128×128 chunks per plane, wrapping at 4,096 px on both axes — the planet is a globe. Nine page-copy hooks rewrite layout cells from event flags (spaceports appearing, the Bio Plant sealing); the pack emits those as `layout_patches`. Priority tiles (bit 15, which survives the collision-bit mask) draw above sprites on hardware; every map with priority tiles gets a transparent overlay PNG so the renderer reproduces that ordering.

Live scene writes use the same `patch_tiles` atlas as load-time effects;
`map_effects.scene_patch_chunks` in the manifest records each map's extracted
scene chunk ids. The [live-layout census](scenes/LIVE_LAYOUT_WRITES.md) owns
their runtime semantics and the [crash helper's retail decode](scenes/43_CrashLanding.md#live-layout-write-retail-correction-67)
records why its `$50..$5F` tiles belong to Raja Temple, not Dezolis.

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

The extractor stays in Python so it remains executable and testable as the
research bench. The native runtime consumes its output through the Rust/Godot
workspace below; porting fixed-width big-endian readers and structs into that
runtime is intentionally boring once the formats are proven.
