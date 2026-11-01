# Source / provenance notes

The extractor is grounded against the US retail ROM (SHA-256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`) and the
public `ps4disasm` work. What the repository holds from the game, and what stays
local, is listed in the [README](../../README.md#license-and-third-party-code).

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
| [saved-char-id-mem.md](saved-char-id-mem.md) | The `Saved_Char_ID_Mem` party bridge: retail's work-RAM window, the boot clear, and the native save field that fixes it | A new observation or a changed deviation |

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
- **DEVIATION — `Event_AngerTowerAlys` (`$62`) op 8, the leader's facing.**
  The cartridge writes Chaz's `facing_dir` raw (`move.w #0, $6(a4)` at
  `$071558`) and does not call `Event_UpdateObjFacing`, so his sprite is not
  refreshed; the port records it as `SceneOp::Face` down, which also turns
  the sprite. Nothing shows it: battle `$18` starts on the same frame and the
  field is rebuilt after it. Record [114](../scenes/114_AngerTowerAlys.md).

## Deliberate fixes to retail battle bugs

The owner's battle ruling (2026-08-15) fixes bugs that are plainly
unintended, under the rule in
[RUNTIME_DESIGN.md](../RUNTIME_DESIGN.md#fidelity-and-original-bugs). The
retail behaviour of each is in the `bugfixes=1` table of
[BATTLE_RULES.md](../battle/BATTLE_RULES.md#fork-contamination)
(`ps4.asm` line numbers there); the port takes the fixed branch:

- the level-99 pointer desync in the results loop (`ps4.asm:5926`);
- the shield-element read: a shield's element is applied as a weapon's
  (`ps4.asm:3822`, `3913`);
- the stat lag after a level-up: `UpdateCharModStats` is not re-run
  (`ps4.asm:6119`);
- the Defend and physical-resistance armour clobber of `physical_prop`
  (`ps4.asm:6368`, `9835`, `11287`);
- Telepipe and Escapipe consumed in battle (`ps4.asm:16109`);
- an ability effect id past the end of `AbilityEffectsOffs` (Zio3's `$2C`),
  rejected when the pack loads instead of dispatched.

The stat lag and the clobber are measured on hardware (tapes 10 and 14): Chaz
levels to 2, strength goes 8 to 9 and `atk_pow` stays 18 across 600 frames;
Defend rewrites `physical_prop`'s high byte 2 to 1 for the round. Quirks that
read as design stay faithful: the cure spells that also restore
agility and dexterity, and enemy paralysis clearing each turn.

## Other recorded deviations

- **The Menu-button credits skip is not ported.** `Cutscene_Ending`
  (`ps4.asm:158029-158033`) lets a player holding Menu at the scene's start jump
  to the credits; the port plays the whole ending. Recorded in
  [REVISION_AUDIT.md](../scenes/REVISION_AUDIT.md#recorded-deviation-the-menu-button-credits-skip).
- **The 16 oracle-receipt attack bodies ship no attack frames (#214).** The
  pack used to copy the indexed PNGs rendered from the receipts in
  `oracle/fixtures/battle_animation_remainder.json` as these bodies' attack
  frames. Those frames are not the routines' own art. Compared by pixel index
  (palette ignored), eight bodies (`$D526`, `$D998`, `$DCD2`, `$EFB2`,
  `$F088`, `$FCCA`, `$10C7C`, `$10E7A`) capture one identical frame sequence,
  three (`$E394`, `$F53C`, `$10AB4`) share a second, and `$E2BE` is the first
  sequence with one frame repeated. The shared frames show a humanoid figure
  with slash marks and tiles still missing mid-upload. That figure is most
  likely the targeted party member reacting, not the enemy's attack. The routines
  agree: `EnemyAttack_Prophallus` (`ps4.asm:19613`) only writes three palette
  words and an object routine and jumps to `loc_D200` (`ps4.asm:19395`), and
  after the `EnemyAttackOffs` dispatch the caller starts the target fighter's
  own presentation (`ps4.asm:19174-19184`, `move.w #8, $2(a0)`). The four
  unique sets (`$DE22`, `$E0EC`, `$ECDE`, `$10E38`) come from effect-object
  state machines that stream art and place themselves by target and by the
  ability chosen (`EnemyAttack_ProtectBit`, `ps4.asm:23637`, and
  `BattleObj_ProtectBitAtk`, `ps4.asm:30592`), which a static render does not
  reproduce. All 16 bodies therefore stay `deferred` in
  `battle/art/enemy_attacks.json` with no frames, and Godot keeps its
  body-flash fallback for them. Affected enemy ids: 4, 7, 14-16, 27-29, 56-58,
  78, 79, 91, 92, 96-101, 114-119, 121, 127, 128, 132, 151. The receipts and
  PNGs stay as oracle evidence only. `battle/enemy_animations.json` is
  unchanged.

- `Event_HuntersGuild`'s teardown (`$071D1A..$071D73`, [record](../scenes/124_HuntersGuild.md)): the
  cartridge's three `Window_Destroy` calls and `Map_LoadChunks` spend frames with no pad read. The port
  takes all three windows down on the press and ends the scene on that tick, so the field gets the
  pad back those frames sooner and the windows do not animate shut. No state differs. The frame count
  has not been measured on the oracle.

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
