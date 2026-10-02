# Aiedo arrival (`$54`) — campaign segment ledger

The post-Rika checkpoint at Motavia `$00 (84,64)` is the north bank of the
crossing verified in [the travel ledger](../field/TRAVEL.md#post-rika-northern-crossing-2026-09-23).
This ledger establishes the ordinary-input route from there to **Aiedo `$54`**,
the entry and story conditions on the way and on arrival, the resource budget,
and the port's readiness — then opens one bounded graph for the arrival itself
with ordinary SAVE and fresh-process CONTINUE.

Nothing here moves a party. Every number below is read from the US cartridge,
the frozen runtime pack, or a command whose output is saved under
`build/lane-evidence/` (hashes in that directory's `EVIDENCE-SHA256.txt`).

## Inputs

| Input | Identity | Check |
| --- | --- | --- |
| Checkpoint save | `build/aiedo-research/saves/slot_1.sram` | SHA256 `2590e0e97ff3125774951ed2c474eba832892e3644cb79c8cdbb443d6499379e`, 0x1600 bytes, re-hashed before every read |
| ROM | `Phantasy Star IV (USA).md` | SHA256 `511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a` = `runtime-pack/manifest.json` `rom.sha256` |
| Pack | `runtime-pack/` | manifest SHA256 `018df2227406af1f09412b9ec3550724a2f9b8688aa0400c1cd707f5b4d05650` (the pack frozen by the northern-crossing gate) |
| Base revision | `d39d9cdd26dcd38d5580d1ff9f42104621faa78b` | plus this ledger's edits |

The checkpoint decode (`build/lane-evidence/checkpoint-save.json`,
`build/lane-evidence/probe_save.py`) matches the brief: signature
`PHANTASY STAR 4` at logical `$F002..$F010` (`ps4.asm:134718-134732`), slot 0,
checksum valid, world 0, map `0x000`, leader pixels `$0540/$0400` = cell
`(84,64)`, party slots `[4,1,0,2,5]` = Gryz/Alys/Chaz/Hahn/Rika
(`ps4.constants.asm:2398-2409`), 1129 meseta, every status byte 0, temp bank
`{7,15}`, town bank `{5,7,23,30}`.

## 1. The route

```bash
python3 -m oracle.route --pack runtime-pack --from-map 0x00 --from-cell 84,63 --to-map 0x54
```

Acknowledge the brief's exact command line: it was run first, unchanged, as
`--from-cell 84,64`, and its raw output is
`build/lane-evidence/route-54.stdout.txt` (exit 0). `oracle/route.py` works in
the cartridge's `pixel // 16` rows — it re-adds the standing-cell shift itself
when it indexes the pack's collision grid (`GRID_Y_BIAS`) — so the party's
standing cell `(84,64)` is row `84,63` there. Both runs are kept
(`build/lane-evidence/route-54-both-rows.txt`); they produce the same corridor
and differ by exactly that one cell, and the table below uses the correct row.

**One map transition, one warp, 67 walking steps.** Every leg was then
re-walked over the pack's own collision grid and warp records
(`build/lane-evidence/probe_route.py` → `route-54-analysis.json`).

| Leg | Direction × cells | Arrives at (cartridge row) | Standing cell | Collision |
| ---: | --- | --- | --- | --- |
| 1 | Up × 13 | `(84,50)` | `(84,51)` | 0 |
| 2 | Left × 20 | `(64,50)` | `(64,51)` | 0 |
| 3 | Down × 1 | `(64,51)` | `(64,52)` | 0 |
| 4 | Left × 1 | `(63,51)` | `(63,52)` | 0 |
| 5 | Down × 3 | `(63,54)` | `(63,55)` | 0 |
| 6 | Left × 1 | `(62,54)` | `(62,55)` | 0 |
| 7 | Down × 1 | `(62,55)` | `(62,56)` | 0 |
| 8 | Left × 1 | `(61,55)` | `(61,56)` | 0 |
| 9 | Down × 1 | `(61,56)` | `(61,57)` | 0 |
| 10 | Left × 16 | `(45,56)` | `(45,57)` | 0 |
| 11 | Down × 1 | `(45,57)` | `(45,58)` | 0 |
| 12 | Left × 8 | `(37,57)` | `(37,58)` | **1 = map change** → warp |

Reading convention, because the two spaces differ by one row: the pack stores
`WarpSource.y_byte` as "one row above the cell it means" and `y_cell` with the
standing shift applied (`rust/psiv-data/src/map/warp.rs`), and the native save
writes the standing cell times 16 where retail writes `(standing_y-1) * 16`
(`docs/camp/SAVE_SCOUT.md`, "Native inherited-dungeon exit extension"). The
`collision` column is the pack grid indexed by the standing cell; the probe
applies `GRID_Y_BIAS` when it walks in cartridge rows. The route is 27 cells
north, 46 west — plain overworld, no vehicle, no ledge, no door but the one
warp.

### The warp

Map `$00` transition index 7, record `0x100706`, ROM bytes
`22 39 00 01 00 54 2f 52 04 04`:

| Field | Value | Note |
| --- | --- | --- |
| source x/y byte | `34` / `57` | `XYPlus40` → the cells `x 34..37, y 57..60` in cartridge rows = standing rows `58..61` |
| range id | `1` = `XYPlus40` | `ps4.asm:112603-112622`, four cells square |
| target map | `0x0054` = `Aiedo` | `Target: Aiedo` |
| destination | x byte `47`, y byte `82` | `Map_Start_*_Pos = byte * 2` in 8-pixel units (`ps4.asm:112566-112575`) = cartridge row 82 → **standing cell (47,83)** |
| facing | `4` = up | the party arrives facing into town |
| character alignment | `4` | not decoded (`map/warp.rs`) |
| table | `2` = `Map_Change` | the doorway table, reached only from a standing collision type 1 |

Cross-checked in the pack: the same record is
`runtime-pack/maps/000_Motavia.json` warp index 7, `trigger: map_change_tile`,
`rect {x:34,y:58,width:4,height:4}`, `destination {x_cell:47,y_cell:83}`,
`record_offset 0x100706`, and the pack's collision grid holds type 1 at
standing `(34..37, 58..61)` — the warp the port will match. The landing cell
`(47,83)` is walkable (collision 0) and the two gate guards stand at
`(46,83)` and `(49,83)` (`runtime-pack/maps/054_Aiedo.json` NPCs 0 and 1), so
the party lands between them with `(47,82)` open northward.

### Aiedo, the arrival map

| Field | Value |
| --- | --- |
| record | `runtime-pack/maps/054_Aiedo.json`, ROM `0x146260..0x146458` |
| size | 96 × 96 cells; `bg_row_size` 47 (`0x1462AA` = `2f 2f 2f 2f`) |
| music | 132 `MotabiaTown` |
| flags | `00 00 00 00` at `0x146450`: **no random battles**, no poison — the whole town is fight-free |
| warps | 20: four town-edge exits (table 1, `any_walkable_tile`, all → Motavia standing `(36,61)`), sixteen doorway warps (table 2) to the bakery, hunters' guild, weapon shop, prison, Chaz's house, houses 2–7, supermarket, pub and Rocky's house |
| objects | 13 NPCs, 4 interaction areas |
| exit band | the south edge covers standing rows 85–95, two rows below the landing cell |

The route therefore ends inside a town that cannot roll an encounter, and the
only way back out is the edge band or a doorway — neither is on the arrival
path.

## 2. Entry and story conditions

### Nothing gates the path or the door

* The warp record carries no flag: ten bytes of coordinates, target, facing and
  alignment. `RunMapTransitions` tests the player's position only
  (`ps4.asm:90923`, `112525-112560`), so Aiedo's door is unconditional.
* No flag-gated overworld patch touches the route. Map `$00` carries six
  `overworld_patches` and nine `layout_patches`; all of them sit on 32-pixel
  page chunks outside the route's own chunks (`build/lane-evidence/route-54-analysis.json`
  → `patch_overlap`, every `route_overlap` empty). The Rika bridge patch
  (`EventFlag_RikaJoined`, chunk `(42,33)` = standing cells `(84..85,66..67)`)
  is behind the checkpoint.
* The overworld has no objects and no chests (`objects.count: 0`,
  `treasure_chests: []` in the map record), so no NPC or chest can block a step.

The route is open for the checkpoint's flag state, and would be for any state:
the base collision grid already carries it, so no patch has to *open* anything.

### No scene triggers on the way or on arrival

Both maps' `RunEventsJmpTbl` lists were read from the ROM
(`0x1007F0` = `06 3a 3b 3c 5b ff` for Motavia, `0x14644A` = `27 ff` for Aiedo)
and each entry was evaluated against the checkpoint's own flags
(`build/lane-evidence/probe_trigger_conditions.py` →
`trigger-conditions.json`). Every condition is the transcription in
`rust/psiv-core/src/trigger_table.rs`, which cites its `ps4.asm` line.

| Map | Index | Routine | Requires | Checkpoint | Fires |
| --- | --- | --- | --- | --- | --- |
| `$00` | `$06` | `RunEvent_MachineCenter` (`115178`) | event `$42` set, `$43` clear, x `$710..$750`, y `$AD0` | `$42` clear | no |
| `$00` | `$3A` | `RunEvent_MeetingSeth` (`116068`) | event `$C1` clear, x `$770`, y `$9B0` | flags hold; the route's pixels are x `544..1375`, y `800..1040` | no |
| `$00` | `$3B` | `RunEvent_AeroPrism` (`116081`) | chest `$0D` (event `$10D`) set, event `$C5` clear | `$10D` clear | no |
| `$00` | `$3C` | `RunEvent_DarkForce3Defeated` (`116092`) | event `$C5` set | `$C5` clear | no |
| `$00` | `$5B` | `RunEvent_ReenterPiata` (`116441`) | event `$0C` clear | `$0C` **set** | no |
| `$54` | `$27` | `RunEvent_ClrChazHouseRest` (`115818`) | temp `$18` set | temp `$18` clear | no |

**Aiedo has no arrival scene.** Its one listed trigger is
`RunEvent_ClrChazHouseRest`, which only resets the Chaz-house rest flag on the
way to `Event_LeavingChazHouse` (`$003C`) — verified in the cartridge at
`ps4.asm:115818-115824` (`beq.w RunEvent_NoEvent` when the temp flag is clear).
That scene is already transcribed in
[docs/scenes/29_LeavingChazHouse.md](../scenes/29_LeavingChazHouse.md) and is
registered in the port (`rust/psiv-core/src/scenes/mod.rs:246`, driven by
`rust/psiv-runtime/tests/next_arc_scenes.rs`), so nothing is missing even if a
future segment rests at Chaz's house.

The four interaction areas on Aiedo are `Interaction_DisplayDialogue`
(router byte 0, `ps4.asm:118287-118295`) with dialogue id 138; they answer a
confirm press at their own rectangles and are not arrival triggers. The town
flag the place-name routine registers for Aiedo is **7**
(`runtime-pack/travel.json`, entry `AIEDO`, `place_id` 12), and the checkpoint
already holds town flag 7 from the opening act — arriving writes no save byte
there.

### What the arrival map does on load

Aiedo's `MapDataManager` list is `0083 ffff` (`0x146454`) → entry `$83`,
`MapDataMan_Aiedo`. Decoded from the cartridge, its whole body is
`ps4.asm:110753-110760` and the ROM bytes at `0x534F4` are byte-identical:
`43 f8 b7 06` `12 bc 00 06` `13 7c 00 06 00 03` `7e 00` `4e 75` —
`lea ($FFFFB706).w,a1` / `move.b #$06,(a1)` / `move.b #$06,$3(a1)` /
`moveq #0,d7` / `rts`. `$FFFFB000` is `Map_Layout_BG` (`ps4.constants.asm:2083`),
Aiedo's stride is `bg_row_size + 1` = 48 (`GetMapLayoutOffset`,
`ps4.asm:110783-110795`), so the two writes land on **BG chunk `(22,37)` and
`(25,37)`** = the two 32×32 chunks over standing cells `(44..45,74..75)` and
`(50..51,74..75)`. The record's static BG layout (Kosinski at `0x14819C`,
2304 = 48×48 bytes) holds **28** at both, so the routine stamps chunk **6**
over them on every load.

The pack decodes entry `$83` as `decoded: true` with `kinds: []` and
`paths: []` — the two writes name no flag bank the model tracks — and the port
applies no write for it. Measured consequence
(`build/lane-evidence/probe_map_records.py`, `probe_route.py` family):
composing map `$54`'s FG plane puts **1024 of 1024 non-transparent pixels** on
both chunks, so the BG bytes underneath are not visible on screen, and the
arrival viewport (cartridge rows 76–90, x 37–57, saved as
`build/lane-evidence/aiedo-arrival-viewport-baked.png`) does not contain them
at all. This is recorded as a fidelity note, not a blocking gap: see §4, G1.

## 3. Resource budget

### What the route rolls

The checkpoint stands on map `$00`, whose `Battle_EnemyFormationIndexes` byte is
`0` — the "world map" sentinel — so on foot the group comes from
`Battle_MotaFormationGroupIndexes` (Kosinski at `0x28501C`), indexed
`(y_pixels >> 6) * 64 + (x_pixels >> 6)`
(`psiv_tools/maps/encounters.py`, `rust/psiv-runtime/src/encounters.rs`).
**All 67 landings resolve to the same grid byte, `2`** — one encounter group for
the whole walk, with no fallback square (`route-54-analysis.json` →
`encounter_squares`).

`RunRandomBattles` (`$05784E`) grants ten free steps after a map load or a
battle and then rolls 1-in-32 per step, and it skips steps that stand on or
next to a map-change tile; the route has one of each at its end
(`rust/psiv-runtime/src/encounters.rs`, `field_tick.rs`). Two of the 67
landings are therefore suppressed and one is the warp itself, leaving 65
rolling-eligible steps. Walking them is a renewal process — nine free steps,
then a 1-in-32 roll per step, re-armed by ten free steps after each battle:
**expected 1.39 fights**, `0: 16.9%`, `1: 39.7%`, `2: 31.9%`, `3: 10.3%`,
`4+: 1.2%` (`build/lane-evidence/route54-budget.json`).

Aiedo itself rolls nothing (`random_battles: 0`), so arrival ends the exposure.

### Group 2's ten formations

`runtime-pack/battle/formations.json` (`build/lane-evidence/route54-enemies.json`).

| Formation | Rolls /32 | Occupancy | Enemy HP | Meseta | EXP |
| ---: | ---: | --- | ---: | ---: | ---: |
| 17 | 5 | LOCUSTA | 68 | 21 | 78 |
| 18 | 5 | LOCUSTA ×2 | 136 | 42 | 156 |
| 31 | 2 | CRAWLER ×2 + LOCUSTA | 128 | 51 | 174 |
| 32 | 2 | LOCUSTA + CRAWLER ×2 | 128 | 51 | 174 |
| 33 | 4 | SCORPIRUS | 150 | 121 | 256 |
| 34 | 4 | SCORPIRUS ×2 | 300 | 242 | 512 |
| 35 | 1 | SCORPIRUS ×3 | 450 | 363 | 768 |
| 36 | 5 | INFANTWORM ×2 | 100 | 126 | 336 |
| 37 | 3 | INFANTWORM ×3 | 150 | 189 | 504 |
| 38 | 1 | INFANTWORM ×4 | 200 | 252 | 672 |

No formation in the group drops an item (`drop_item: null`, `drop_rate: 0` for
all ten), so **the inventory cannot change on this route**; money can only grow.
The four enemies' records (`runtime-pack/battle/enemies.json` ids 14, 30, 67,
150; ROM `0x28195C`, `0x281C5C`, `0x28234C`, `0x2832DC`):
LOCUSTA hp 68 atk 26 def 5 agi 12, CRAWLER hp 30 atk 16 def 4 agi 5,
SCORPIRUS hp 150 atk 60 def 2 agi 12, INFANTWORM hp 50 atk 50 def 1 agi 18.
SCORPIRUS is the one that matters: 150 HP, the highest attack power, physical
property 1 (resistant), and a plain attack whose `attack.status_effect` is 27 =
poison.

### The party the checkpoint fields

| Member | Lv | HP | TP | ATK | DEF | STR | AGI | DEX | Weapons |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Gryz | 7 | 75/76 | 22 | 52 | 43 | 20 | 7 | 12 | BROAD-AXE |
| Alys | 8 | 60/61 | 43 | 22 | 28 | 14 | 17 | 15 | SLASHER ×2 |
| Chaz | 6 | 53/53 | 28 | 29 | 25 | 15 | 12 | 10 | STEL-SWORD |
| Hahn | 6 | 45/45 | 50 | 12 | 25 | 10 | 8 | 11 | DAGGER |
| Rika | 1 | 39/39 | 25 | 37 | 40 | 13 | 13 | 11 | CLAW ×2 |

272 HP and 168 TP in total, every status byte 0. Inventory: nine equippables —
BOOMERANG, HUNT-KNIFE ×2, TITN-AXE, LTHR-CLOTH ×3, LTHRSHIELD, LTHR-BAND —
**no consumables at all**, so no healing item and no antidote. Recovery is the
party's own magic: **RES** (technique 24, power byte 16, 3 TP, `battle_and_field`)
on Chaz, Hahn and Rika, and **ANTI** (technique 34, 2 TP, `battle_and_field`) on
Hahn. That is 34 casts of RES across the three casters, healing roughly
`mental_battle + 16` per cast at the mean roll — 27 for Chaz and Rika, 32 for
Hahn (`Battle_CalcHealing` `$00B5FE`) — and 2 TP per status cure.

### Can the party make it without recovery?

Per-hit numbers at the mean roll of `Battle_CalculateDamage` (`$00B5CA`:
`(((S+8) * ATK >> 6) + ATK + 2*bonus) * element >> 2 - DEF`, `S` mean 56), with
the hit roll from `loc_B6A2` (`$00B716`, scale 2, miss ≤ 8), the enemies'
element factor being the party's physical property (2 for all five), and the
party's own attacks taking the best of both hands against the target's property
(`build/lane-evidence/route54-budget.json` → `damage_tables`):

| Attacker | vs Gryz | vs Alys | vs Chaz | vs Hahn | vs Rika |
| --- | ---: | ---: | ---: | ---: | ---: |
| LOCUSTA (atk 26) | 1 | 1 | 1 | 1 | 1 |
| CRAWLER (atk 16) | 1 | 1 | 1 | 1 | 1 |
| INFANTWORM (atk 50) | 7 | 22 | 25 | 25 | 10 |
| SCORPIRUS (atk 60) | 17 | 32 | 35 | 35 | 20 |

The party's own mean damage per attack: ~127 a round against LOCUSTA/CRAWLER/
INFANTWORM formations and ~60 a round against SCORPIRUS ones (its physical
resistance halves the weapons), so the common fights last about one round and
the SCORPIRUS family two to eight:

| Formation | Rounds (five plain attacks) | Incoming per round | Incoming over the fight |
| --- | ---: | ---: | ---: |
| 17 / 18 (LOCUSTA) | 0.6 / 1.2 | 11 / 22 | 6 / 26 |
| 31 / 32 (CRAWLER + LOCUSTA) | 1.0 | 33 | 33 |
| 33 / 34 (SCORPIRUS ×1–2) | 2.5 / 5.0 | 11 / 22 | 28 / 111 |
| **35 (SCORPIRUS ×3)** | **7.5** | 33 | **249** |
| 36 / 37 / 38 (INFANTWORM ×2–4) | 0.8 / 1.2 / 1.7 | 22 / 33 / 44 | 18 / 41 / 73 |

Weighted by the roll table, one fight costs about **43 HP** of incoming damage
and the whole expected 1.39-fight route about **60 HP** — spread over 272, with
34 RES casts worth roughly 950 HP of healing behind it. The verdict: **the
party reaches Aiedo without an inn**, on the strength of the expected fight
count and the healing capacity, not on any per-fight certainty.

Two things the plain estimate does not capture, both worth naming:

* Formation 35 (3 × SCORPIRUS, 1/32 of rolls, so about 1.4% of runs) is the one
  that hurts: 249 incoming against 272 HP, and a focused round can put 105
  damage on Hahn's 45. No revive exists in this party — the inventory has no
  item and RES/ANTI do not revive — so a death there fails the node and is
  recorded as a failure, not repaired. The driver's own battle input heals any
  member under 70% with RES and spends skills on durable enemies
  (`tools/native/native_bioplant.gd`, `battle_input`), which is the mitigation.
* SCORPIRUS's plain attack carries the **poison** rider (record byte
  `attack.status_effect` 27). The rider is one `Battle_CalculateChances` roll
  against the target's efess property with `miss ≤ $70`, which for a strength
  25 attacker is 19% (Gryz) to 34% (Hahn) per landed hit
  (`rust/psiv-core/src/battle/action.rs`, `resolve_enemy_attack_status`).
  Poison survives the battle and ticks HP away on the field
  ([field status](PLAYABILITY_HOLT_TONOE.md#native-tonoe-attempt-and-field-status-repair-2026-09-13)),
  so the run must cure it with ANTI before walking on; the driver's inherited
  `field_poison_input` path does exactly that and asserts the 2 TP charge.

## 4. Port readiness

### What the route needs, and where it stands

| Mechanic | Port state | Evidence |
| --- | --- | --- |
| Map change through the doorway table | implemented | `rust/psiv-data/src/map/warp.rs` (`TransitionTable`), `rust/psiv-core/src/field.rs:523`, `rust/psiv-runtime/src/map_change.rs`; the same mechanism carried the Zema entry and exit in the northern-crossing run (`tools/native/native_post_rika.gd` route legs) |
| The town-edge `any_walkable_tile` table | implemented | `rust/psiv-core/src/field.rs:529` (`WarpTrigger::NormalGround`) — only needed if a later segment leaves town |
| Overworld encounters from the position grid | implemented | `rust/psiv-runtime/src/encounters.rs` `EncounterTable::select` (`Source::Grid`, `cell >> 2`), `field_tick.rs` roll; `EncounterClock::suppressed` matches the cartridge's gates |
| Group 2's four enemies: regular and conditional abilities | **no gap** | all four records carry all-zero `ai.regular_ability_ids`, `ai.conditional_ability_ids` and `ai.condition_ids` (`route54-enemies.json`); `docs/battle/ENEMY_ABILITIES.md`'s 83-id universe is therefore untouched by this route, and `rust/psiv-core/src/battle/enemy_ai.rs` is never asked for an arm |
| Their plain attack and the attack-status rider | implemented | `rust/psiv-core/src/battle/action.rs` `resolve_enemy_attack_status` (effects 27 poison, 28 paralysis) with `attack_status_tests.rs`'s retail-threshold test; the all-zero lists take the `Ok(false)` fall-through in `enemy_ai.rs`/`engine.rs` without an `UnsupportedAbility` event |
| Enemy battle art, animations and sound | complete for all four | manifest census `enemy_attack_sheets {exact: 153, partial: 0, deferred: 0}`, `sprite_sheet_exact: 153`, `movement_exact: 153`, `frame_sequence_deferred: 0`; `runtime-pack/battle/enemy_animations.json` composition `exact` for ids 14/30/67/150 |
| Field poison and field healing (RES/ANTI) | implemented | `rust/psiv-core/src/field_status.rs`, `field_healing.rs`, `rust/psiv-runtime/src/field_status.rs`; native evidence in the Holt/Tonoe ledger |
| Arrival scene | none exists; the only Aiedo trigger is transcribed | §2 table; `docs/scenes/29_LeavingChazHouse.md`, `scenes/mod.rs:246`, `rust/psiv-runtime/tests/next_arc_scenes.rs` |
| Ordinary SAVE and fresh-process CONTINUE | implemented | `rust/psiv-runtime/src/save.rs`, `camp` STATE/SAVE, `tools/native/native_continue.gd`, `tools/native/verify_native_continue.py` |
| Aiedo's map record loads | implemented | `rust/psiv-data/tests/runtime_pack.rs` `the_real_pack_loads_and_validates` loads every map and asserts each warp's facing |

### Gaps and findings

**No readiness gap blocks the route.** The gaps found are these, each with its
disposition:

| # | Finding | Evidence | Disposition |
| --- | --- | --- | --- |
| G1 | Aiedo's only `MapDataManager` entry (`$83`, `MapDataMan_Aiedo`) stamps BG chunk 6 over chunks `(22,37)` and `(25,37)`; the pack models no write for it and the port applies none | ROM `0x146454` = `0083 ffff`; ROM `0x534F4` = `43 f8 b7 06 12 bc 00 06 13 7c 00 06 00 03 7e 00 4e 75`; `ps4.asm:110753-110760`; `Map_Layout_BG` `ps4.constants.asm:2083`; static layout `0x14819C` = 28 at both cells; pack entry `kinds: []` | **not blocking**: the FG plane is 1024/1024 opaque over both chunks and neither is inside the arrival viewport, so the omission is not expected to be visible. Kept as a fidelity note for a future whole-scene comparison |
| G2 | Aiedo's inn (`AiedoSupermarket`, selector 6) runs `Event_GirlsSneakingOut` instead of an ordinary night when `EventFlag_Zio` and `EventFlag_GirlsCaught` are clear — both are clear in the checkpoint — and the port returns `InnResult::AiedoEventPending` without running the scene or billing | `docs/camp/SHOPS.md` "Finding 3" (`$066148`); `rust/psiv-runtime/src/shop.rs:67-79` ("The event runner is not owned by the shop lane yet") and `:126`; test `aiedo_special_is_flagged_before_the_bill_is_deducted` | **not on the arrival path**: §3 shows the route needs no rest. It blocks only a later segment that rests at Aiedo, and needs its own assignment before one does |
| G3 | The overworld battle background falls back to asset 0 on Motavia, so every fight on this route renders the wrong backdrop | [Travel ledger](../field/TRAVEL.md#visual-evidence-and-remaining-limits): "The route log still reports the existing Motavia battle-background fallback to asset 0" | **presentation only**; state, SAVE and CONTINUE are unaffected. Kept by the roadmap's presentation section |
| G4 | Aiedo's per-frame map update is unmodelled: the map record lists `MapUpdateJmpTbl` index 2 = `MapUpdate_MotaTownsWaterPal`, the pack emits no `map_updates` key for any map, and no port code reads one | ROM `0x1462BE` = `02 ff`; `ps4.asm:112885-112888`; `runtime-pack/maps/054_Aiedo.json` has no `map_updates`; no consumer in `rust/` | **presentation only, but it constrains claims**: the arrival viewport contains the town's water and fountains (`build/lane-evidence/aiedo-arrival-viewport-baked.png`), whose palette animation the port does not run. No capture from this segment may be claimed as whole-scene parity |
| G5 | `docs/scenes/12_ArcTriggerCensus.md:16` lists Aiedo's dialogue tree as 5; the cartridge's record points at `DialogueTree11` (`0x14641C` → `0x1E6FE0`), and the pack agrees. The same column disagrees for Molcum, ZioFort F4, Ladea Tower F2/F5, BioPlant Part2, Mota Spaceport, Nurvus B4 Part2 and Zelan | `build/lane-evidence/map-records.json`; pack `dialogue_tree` per map | **unresolved documentation discrepancy**, outside this segment; the column's exact meaning is not established, so this ledger cites the ROM and pack instead. Worth an issue when documentation work is next authorized |

## 5. Aiedo arrival task graph

Opened 2026-09-25. Format and field meanings:
[the workflow](../AGENT_WORKFLOW.md#one-canonical-task-graph). Owner roles are
the workflow's roles; which host model fills a role is untracked, host-local
configuration and is not recorded here.

```yaml
outcome: "Reach Aiedo $54 from the north-bank checkpoint with ordinary input, SAVE ordinarily, and prove a fresh-process CONTINUE"
canonical_record: "docs/campaign/AIEDO.md#5-aiedo-arrival-task-graph"
base_revision: "d39d9cdd26dcd38d5580d1ff9f42104621faa78b plus this ledger's edits"
authority: "Owner 2026-09-23 campaign request (docs/ROADMAP.md section 1): local implementation, focused checks and scoped repairs; no commit, push, merge, deployment or paid-service grant is recorded for campaign changes (docs/AGENT_WORKFLOW.md#authority-effort-and-continuation)"
effort_policy: "Continue scoped repairs until acceptance passes; no fixed cycle limit (inherited)"
common_inputs: ["docs/campaign/AIEDO.md sections 1-4", "docs/field/TRAVEL.md#post-rika-northern-crossing-2026-09-23", "docs/camp/SAVE_SCOUT.md", "docs/DEVELOPMENT.md", "build/aiedo-research/saves/slot_1.sram (SHA256 2590e0e97ff3125774951ed2c474eba832892e3644cb79c8cdbb443d6499379e)", "runtime-pack manifest 018df2227406af1f09412b9ec3550724a2f9b8688aa0400c1cd707f5b4d05650"]
exclusions: ["Zio's Fort, Krup, Saya, shopping, grinding, debug or fixture injection, model-driven gameplay, commits, pushes, merges, releases", "no whole-scene visual-parity claim (gaps G3 and G4)"]
next_action: "AI-01: freeze the ordinary-input Aiedo driver and its run directory"
nodes:
  - id: AI-01
    outcome: "A frozen ordinary-input driver walks section 1's route table and stops on Aiedo $54, with a run directory that copies and hashes the checkpoint before any movement"
    depends_on: []
    owner: "bounded delivery (routine implementation through checks and closeout)"
    inputs: ["section 1 route table and warp record", "tools/native/native_post_rika.gd", "tools/native/native_bioplant.gd (battle_input, field_poison_input, field_healing_input, save flow)", "tools/native/native_continue.gd", "tools/native/verify_native_continue.py", "checkpoint save copy at build/aiedo-research/saves/slot_1.sram"]
    acceptance: "New tools/native/native_aiedo.gd extends the post-Rika driver with the 12-leg table in standing cells, arrival assertions for map 0x54 at cell (47,83) facing up, healing = true so the inherited ANTI/RES field path runs, and no shopping; the local runner copies the source save into a fresh directory under build/, hashes it, sets PSIV_SAVE_DIR, and refuses to launch on a hash mismatch; the driver parses under Godot --headless --check-only with exit 0; a negative control (a deliberately wrong expected party/money receipt) stops before movement with a non-zero exit and leaves every save copy unchanged; driver and runner hashes are recorded in the run receipt"
    state: ready
    evidence: []
    effort: inherited
  - id: AI-02
    outcome: "Focused coverage that group 2's ten formations fight in the port, with a negative control on the SCORPIRUS rider"
    depends_on: [AI-01]
    inputs: ["section 3 roster and roll table", "AI-01's formation list", "runtime-pack/battle/{formations,enemies}.json", "rust/psiv-runtime/tests/combat_enemy_poison.rs as the pattern"]
    acceptance: "A new psiv-runtime integration test builds each of the ten formations from the pack and asserts: no BattleEvent::UnsupportedAbility for any of the four enemies (their ability lists are all zero), the plain attack resolves damage, and SCORPIRUS's attack-status rider sets the poisoned bit exactly at the retail threshold draw and leaves it clear one draw below; the test is run with CARGO_BUILD_JOBS=1 and --test-threads=1 and passes; a mutation that gives SCORPIRUS a nonzero regular ability id makes it fail (negative control, reverted before freeze)"
    state: pending
    evidence: []
    effort: inherited
  - id: AI-03
    outcome: "Walk the route with ordinary input and arrive at Aiedo $54"
    depends_on: [AI-02]
    inputs: ["AI-01 freeze", "AI-02 pass", "a fresh copy of the checkpoint save"]
    acceptance: "The route receipt records every leg's observed (map, cell) equal to section 1's standing cells and the arrival observation map 0x54 at cell (47,83); all five members alive with status byte 0 at SAVE time; event flags, party order and inventory byte-identical to the checkpoint; money equals 1129 plus the encounter rewards listed in the same receipt; each battle's formation is recorded with its group (2); the input copy's SHA256 is unchanged; 1280x800 captures of the arrival field and of any battle are stored in the run directory; no fixture, debug or resource injection appears in the driver or the receipt"
    state: pending
    evidence: []
    effort: inherited
  - id: AI-04
    outcome: "Ordinary SAVE at Aiedo, then fresh-process CONTINUE with byte validation"
    depends_on: [AI-03]
    inputs: ["AI-03 save and route receipt", "tools/native/native_continue.gd", "tools/native/verify_native_continue.py"]
    acceptance: "A new process selects the arrival save through the real title CONTINUE, the state matches the arrival save for party, resources, inventory, flags, map and money, one ordinary step is taken, and SAVE 2 succeeds; python3 tools/native/verify_native_continue.py exits 0 reporting only the standing-Y word in the logical payload plus header slot/checksum offsets; the arrival save and the checkpoint copy are unchanged; the receipts are read back and their hashes recorded"
    state: pending
    evidence: []
    effort: inherited
  - id: AI-05
    outcome: "Review the evidence, dispose of the gaps, advance the roadmap and archive this graph"
    depends_on: [AI-04]
    inputs: ["AI-03 and AI-04 receipts and captures", "section 4 gap table"]
    acceptance: "Every acceptance above is re-read from the raw artifacts (not from summaries), the arrival ledger records the run's save hashes, party state, money and captures, gaps G1-G5 keep or lose their disposition on the new evidence (including whether the arrival capture shows the two chunks of G1), docs/ROADMAP.md section 1's next action names the next segment, this graph is marked archived here with its evidence paths, and no commit, push or merge is made without a new authorization"
    state: pending
    evidence: []
    effort: inherited
```

## Evidence index

Local, ignored and not committed (AGENTS.md, "Protect local inputs and
evidence"). `build/lane-evidence/EVIDENCE-SHA256.txt` has every hash.

| File | What it holds |
| --- | --- |
| `route-54.stdout.txt` / `route-54-both-rows.txt` | the brief's route command and the standing-cell row, raw |
| `route-54-analysis.json` | legs, per-cell collision and encounter suppression, the warp record, encounter squares, patch-overlap check, arrival record — from `probe_route.py` |
| `checkpoint-save.json` | the full save decode — from `probe_save.py` |
| `route54-enemies.json` | group 2's ten formations, four enemies, AI lists and skills — from `probe_encounters.py` |
| `route54-budget.json` | damage tables, per-formation rounds and incoming totals, heal capacity, fight-count distribution — from `probe_budget.py` |
| `map-records.json` | the `$00` and `$54` ROM records: events, `MapDataManager` lists, dimensions, flags — from `probe_map_records.py` |
| `trigger-conditions.json` | every route and arrival trigger against the save's flags — from `probe_trigger_conditions.py` |
| `aiedo-arrival-viewport-baked.png` | the 320×224 baked arrival viewport (rows 76–90, x 37–57) |
| `aiedo-chunks-22-25-37.png`, `aiedo-two-chunks.png`, `aiedo-chunk6-sample.png`, `aiedo-chunk28-sample.png`, `aiedo-base-vs-over-gate.png` | the G1 regions: the two written chunks, chunk 6 and chunk 28 samples, and the base against the overlay plane |

## Reproduction

```bash
python3 -m oracle.route --pack runtime-pack --from-map 0x00 --from-cell 84,63 --to-map 0x54
python3 build/lane-evidence/probe_route.py
python3 build/lane-evidence/probe_save.py build/aiedo-research/saves/slot_1.sram
python3 build/lane-evidence/probe_encounters.py
python3 build/lane-evidence/probe_budget.py
python3 build/lane-evidence/probe_map_records.py
python3 build/lane-evidence/probe_trigger_conditions.py
python3 tools/check_docs.py
python3 tools/size_guard.py
```

The probes are read-only: they open the checkpoint copy, the ROM and the pack,
and write nothing but stdout. `build/aiedo-research/saves/slot_1.sram` is the
only save they touch, and only to read.
