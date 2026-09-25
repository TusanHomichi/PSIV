# Source / provenance notes

The extractor is grounded against Peter's verified US retail PSIV ROM and the public `ps4disasm` work. No ROM bytes are shipped in this repository beyond very short structural signatures used as tests.

## Topic files

The detailed records live in the topic files below: the ledger index's pointers
to "the record below" mean the topic file whose scope covers it. Add a new note
to the topic file whose scope fits, and add a topic file when none fits; this
index lists every topic file.

| File | Scope | Where new notes go |
| --- | --- | --- |
| [formats.md](formats.md) | The two font encodings, Kosinski/Nemesis/Enigma decompression, palettes, plane mappings, map records, layouts and collision | A new format rule, or a correction to a decoder's proven behaviour |
| [oracle-methodology.md](oracle-methodology.md) | How the graphics and battle-formation decoders are pinned against the ROM | A new oracle technique or a changed acceptance criterion |
| [disassembly-discrepancies.md](disassembly-discrepancies.md) | Where the fork's annotations and the retail bytes disagree, and the dated retail-bug chronology | A dated record, correction or retraction |
| [battle-enemy-abilities.md](battle-enemy-abilities.md) | Enemy damage skills, their targeting arms and measured routes | A dated enemy-ability record |
| [battle-party.md](battle-party.md) | Party actions, items, rewards, battle entry/exit and the attack passes | A dated party or battle-flow record |
| [field-and-dialogue.md](field-and-dialogue.md) | Dialogue continuations, choices, resting and overworld field travel | A dated dialogue or field-travel record |

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
