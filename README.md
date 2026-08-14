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
└── shops.json
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

### Weird-but-useful duplicated level data

The primary level-table block occupies `0x2856B0..0x28A735` (20,614 bytes). The ROM contains a byte-for-byte identical mirror beginning at `0x2A3A42`, ending at `0x2A8AC7`, immediately before the initial character data. The extractor verifies this equality and reports it in `progression.json`.

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

1. The per-map encounter group tables (`Battle_EnemyFormationIndexes`, `Battle_MotaFormationGroupIndexes`, `Battle_DezoFormationGroupIndexes`), which map a field position to one of the 68 formation-index groups. The last two are Kosinski, so the decoder is already in place.
2. Shops and inventories.
3. Save/SRAM parsing and import.
4. Map/event/script structures.
5. Text/name decoding.
6. Graphics + palette decompression/export.

Kosinski was the first real compression boundary in this path and it is now crossed and proven. Once maps/events are normalized, the data boundary is large enough to start a tiny native runtime vertical slice without dragging Genesis-specific data handling into Godot.
