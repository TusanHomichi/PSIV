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
└── vehicles.json
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

The 937 level records are split across 11 per-character tables. Their start addresses and starting levels are read from the pointer table at `0x004074`; the extractor does not hard-code each individual table address.

### Weird-but-useful duplicated level data

The primary level-table block occupies `0x2856B0..0x28A735` (20,614 bytes). The ROM contains a byte-for-byte identical mirror beginning at `0x2A3A42`, ending at `0x2A8AC7`, immediately before the initial character data. The extractor verifies this equality and reports it in `progression.json`.

## Data-model choices

- All records preserve `rom_offset` and `raw_hex` provenance.
- Equipment stat bonuses are decoded as signed 8-bit values. This matters: Shadow Blade encodes negative Mental/Agility/Dexterity modifiers.
- Item effect bytes whose semantics are not uniformly safe across every item class remain conservatively named as parameters instead of being over-interpreted.
- Enemy ability IDs are preserved *and* resolved to enemy-skill symbols; raw numeric IDs remain alongside them.
- Item/enemy symbols come from the disassembly constants, while actual localized game text remains a separate extraction problem.

## Tests

With the ROM fixture at the repository root (gitignored):

```bash
PYTHONPATH=. python -m unittest discover -s tests -v
```

The current suite covers ROM identity/checksum, known-layout signatures, dataset counts, representative character/ability/item/enemy records, signed equipment bonuses, level-table pointers, level 2 data, the full level-table mirror, and exact table boundaries.

## Why Python first?

The long-term runtime can still be Rust/Godot. This first pass is Python so the extractor can be executed and tested immediately. Once formats are proven, porting fixed-width big-endian readers and structs to Rust is intentionally boring.

## Next useful slice

1. Kosinski decompression for battle formations / encounter composition.
2. Shops and inventories.
3. Save/SRAM parsing and import.
4. Map/event/script structures.
5. Text/name decoding.
6. Graphics + palette decompression/export.

The public disassembly stores battle-formation sources uncompressed, but the retail ROM stores them Kosinski-compressed. That is the first real compression boundary in this path. Once battle formations and maps/events are normalized, the data boundary is large enough to start a tiny native runtime vertical slice without dragging Genesis-specific data handling into Godot.
