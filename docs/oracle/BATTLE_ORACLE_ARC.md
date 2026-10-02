# The Motavia arc's status and stat abilities, captured and replayed

What this ledger records: the forced captures that prove the status and stat
enemy abilities of [issue #58](https://github.com/TusanHomichi/PSIV/issues/58)'s
second half - VOICE, RIMIT, EVIL EYE, STASISBALL, DORAN, SEALS, GELUN, DEBAN,
VOL, GIRES and the Zol slugs' Fusion - the replay verdict of each, the carriers
that stay deferred and why, the harness changes the captures needed, and the
negative controls. The rules themselves, with their citations, are
[`../battle/ENEMY_EFFECT_ABILITIES.md`](../battle/ENEMY_EFFECT_ABILITIES.md) and
[`../battle/ENEMY_FUSION.md`](../battle/ENEMY_FUSION.md); the tooling is
[`BATTLE_ORACLE_FORCED.md`](BATTLE_ORACLE_FORCED.md) (the captures),
[`BATTLE_ORACLE_REPLAY.md`](BATTLE_ORACLE_REPLAY.md) (the comparator) and
[`BATTLE_ORACLE_SWEEP.md`](BATTLE_ORACLE_SWEEP.md) (the divergence manifest).

## 1. The captures

Every fixture is one `python3 -m oracle.force --formation F --durable
--max-rounds N --delay D` run from tape 07's own field prefix, extracted by
`python3 -m oracle.fixture` and finished by `python3 -m oracle.sweep.arc`
(section 4). `python3 -m oracle.sweep.arc --list` prints each command;
`--capture` and `--extract` reproduce the set. Each capture was run twice by the
tool and came back byte-identical, and `python3 -m oracle.rng_trace check` passed
for every one. The fixtures live in
`rust/psiv-core/src/battle/replay_fixtures/arc_motavia/` and are replayed by the
same one data-driven test as every other fixture
(`every_fixture_replays_as_recorded`); the divergence manifest
(`replay_fixtures/divergences.json`) is **empty**.

| fixture | formation | selector | battle frames | trace sha256 | log sha256 | rounds kept |
|---|---|---|---|---|---|---|
| `formation_D7_d0` | `$D7` three FlyScreamr (76) | `map 0x08C` LadeaTower, group 37, entry 3 | 24794-26565 | `5174febaa558` | `6f3506d98553` | 3 (the party is lost) |
| `formation_D5_d0` | `$D5` one FlyScreamr | the same map, entry 0 | 24794-28101 | `388c0e6c6870` | `bdc364c89475` | 8 |
| `formation_D6_d0` | `$D6` two FlyScreamr | the same map, entry 1 | 24794-27713 | `3e93b4ccb840` | `7928de808617` | 8 |
| `formation_D6_d1` | `$D6`, delay 1 | the same | 24794-27094 | `0068aacc259d` | `11ec6c2ae835` | 6 |
| `formation_100_d0` | `$100` one Blauzen (19) | `map 0x0CE` Nurvus_B1, group 41, entry 12 | 24794-28917 | `c5a7bc3892b0` | `d68a4dc18a12` | 10 |
| `formation_1A3_d0` | `$1A3` one Goldine (21) | `map 0x0C8` VahalFort, group 58, entry 0 | 24794-27848 | `07280d44bce5` | `87a15be60741` | 8 |
| `formation_1A5_d5` | `$1A5` one LifeDeletr (26), delay 5 | the same map, group 58, entry 2 | 24794-26591 | `46dc48af67e4` | `8bf1d7c985d2` | 1 |
| `formation_119_d0` | `$119` one Greneris (115) | `map 0x0CE`, group 41, entry 10 | 24794-29525 | `4904294b36f2` | `72ea5b9e0a14` | 3 |
| `formation_119_d2` | `$119`, delay 2 | the same | 24794-27479 | `322509141e80` | `61fd00171721` | 3 |
| `formation_119_d8` | `$119`, delay 8 | the same | 24794-27325 | `45b578a6b54c` | `a0339c030c40` | 3 |
| `formation_1C6_d0` | `$1C6` two BloodSaber (72) | `map 0x0F2` StrengthTower, group 61, entry 19 | 24794-26683 | `a7fa01f5c8cf` | `6d23d95cceb2` | 5 (the party is lost) |
| `formation_1C6_d1` | `$1C6`, delay 1 | the same | 24794-25708 | `2175adca6f6f` | `12a25bb6964d` | 3 (the party is lost) |
| `formation_1E1_d0` | `$1E1` one SoldrFiend (88) | `map 0x100` TheEdge, group 64, entry 10 | 24794-26416 | `dc6e327fabde` | `0906ff5c9bcc` | 1 |
| `formation_FB_d1` | `$FB` two Haunt (106) | `map 0x08E` LadeaTower_F2, group 38, entry 30 | 24794-27033 | `3ab5bab2a6c5` | `4b45172ad6e3` | 3 (the party wins) |
| `formation_FB_d2` | `$FB`, delay 2 | the same | 24794-26746 | `3b0c7ef9c2a7` | `9aa08a59541c` | 3 (the party wins) |
| `formation_16D_d1` | `$16D` two Spector (107), delay 1 | `map 0x172` AirCastle_Part2, group 52, entry 0 | 24794-27686 | `8810e9aa185b` | `5f34ed28910f` | 2 |
| `formation_12E_d10` | `$12E` two TechPlant (77), delay 10 | `map 0x158` MystVale, group 46, entry 0 | 24794-29135 | `7faa300fc96e` | `207603e083bc` | 2 |
| `formation_12E_d12` | `$12E`, delay 12 | the same | 24794-30081 | `08a2af687aa9` | `f4e4ef7efe8d` | 2 |
| `formation_D8_d0` | `$D8` one ShadowSabr (70) | `map 0x082` ZioFort, group 30, entry 24 | 24794-30469 | `d9fbfe7e124c` | `e0a7040bcc14` | 10 |
| `formation_D2_d2` | `$D2` two ZolSlug (34), delay 2 | `map 0x081` Passageway, group 29, entry 0 | 24794-27991 | `906cc2a77858` | `95ed3b81f3aa` | 8 |
| `formation_116_d2` | `$116` ZiosGuard (103) + TechMaster (100), delay 2 | `map 0x0CC` Nurvus_Part2, group 40, entry 12 | 24794-30551 | `97591e7ca78f` | `7874559bfecc` | 8 |

(The sha256 columns are the first twelve hex digits; the full values are in each
fixture's `provenance`.) The party is always tape 07's - Alys level 7, Chaz and
Hahn level 1 - with 999 HP each from `--durable`, so a formation that would end
the fight in round 1 shows several rounds. That is a fixture, not a route: no
player walks to these battles with this party.

A capture is cut (`--max-rounds`, then `oracle.sweep.arc`'s `rounds`) before the
first ability that no lane implements: Greneris's FORCEFLASH (lane a1),
LifeDeletr's MICROMISSL, SoldrFiend's BLADESHINE and HAKENBOLT, TechPlant's GIZAN,
Spector's CORRSION, TechMaster's ZAN. Those are why most fixtures of a
many-ability carrier keep two or three rounds. Delays were scanned
until a capture's first rounds held only abilities the port runs: the scan is
what `--delay` is for, and the failures are as informative as the keepers (every
TechPlant capture through delay 9 opened with GIZAN before RIMIT).

## 2. What each pair was seen doing

"Round" and "frame" are the log's: the frame the ability's `eN_ability` byte
moved on. "Rolls" is what the action drew (the ability roll plus one chance roll
per visited slot). `status` and `stats` are the cells the log shows moving in the
action.

| ability | enemy | fixture | round, frame | rolls | what the log shows |
|---|---|---|---|---|---|
| `$34` VOICE | 76 FlyScreamr | `D7_d0` | r3 f26139 | 2 | Hahn asleep (`$00`->`$08`); Alys and Chaz were already paralyzed, so one chance roll |
| | | `D5_d0` | r4 f26715, r6 f27211 | 4, 2 | all three asleep, then Alys only; the sleepers wake on the round-end rolls in r7 |
| | | `D6_d0` | r2 f25579, r3-r5, r6, r8 | 3, 2, 2, 2, 2, 2 | sleep landing one slot at a time, a wake in r6, a skipped sleeper in r8 (f27560, one roll) |
| | | `D6_d1` | r1 f25094, r2 f25356, r5 f26662 | 3, 3, 3 | two members slept at a time |
| `$2A` RIMIT | 115 Greneris | `119_d8` | r2 f25043 | 3 | Chaz and Hahn asleep (Alys was dead from VOL); both wake on r3's round-end rolls |
| | 77 TechPlant | `12E_d10`, `12E_d12` | r1 f25087, f25015 | 4, 4 | all three members asleep |
| `$4C` EVIL EYE | 106 Haunt | `FB_d1` | r1 f25004, r2 f25628, r3 f26236 | 2, 2, 3 | a miss, Chaz asleep, a miss |
| | | `FB_d2` | r2 f25709 | 2 | Alys asleep |
| | 107 Spector | `16D_d1` | r1 f24870, f24965, r2 f25132 | 2, 4, 2 | Chaz, Alys, then Hahn asleep |
| `$2F` VOL | 115 Greneris | `119_d8` | r1 f24859 | 2 | Alys dead (999 -> 0), no status or stat cell moves |
| | 72 BloodSaber | `1C6_d0`, `1C6_d1` | r1, r4, r5 and r1, r2, r3 | 2 each | one member killed each time; the party is wiped |
| | 88 SoldrFiend | `1E1_d0` | r1 f24880 | 2 | a miss: the roll is drawn, nothing moves (its kill arm is read, not observed) |
| `$0B` STASISBALL | 19 Blauzen | `100_d0` | r4 f26635, r6 f27499, r10 f28619 | 2, 2, 1 | Chaz then Alys paralyzed (`$02`) with `agi_bat` -> 1; the third use finds Alys already paralyzed and draws nothing |
| | 21 Goldine | `1A3_d0` | r5 f26619, r6 f27003, r8 f27547 | 2, 3, 3 | Alys, Chaz, Hahn paralyzed in turn |
| | 26 LifeDeletr | `1A5_d5` | r1 f24862 | 3 | Alys paralyzed; the ability roll re-rolled once |
| `$28` DORAN | 115 Greneris | `119_d0` | r2 f25115 | 4 | `agi_bat` Alys 15 -> 1 and Hahn 4 -> 1; Chaz unchanged |
| | | `119_d2` | r3 f26093 | 4 | Alys 15 -> 1 |
| | | `119_d8` | r3 f25167 | 6 | no `agi_bat` change survives the window: the two sleepers' wake rolls share it, and a wake restores agility |
| `$29` SEALS | 115 Greneris | `119_d0` | r1 f24859, r3 f25659 | 5, 1 | all three sealed (`$10`); the second cast finds them sealed and draws only the ability roll |
| | | `119_d2` | r1 f24973 | 4 | Hahn sealed |
| `$57` GELUN | 115 Greneris | `119_d2` | r2 f25501 | 4 | no cell the extractor reads moves; `oracle.sweep.arc` adds `atk_bat`: Alys 13 -> 0 and Chaz 18 -> 0 |
| `$2D` DEBAN | 70 ShadowSabr | `D8_d0` | r1 f25142 | 1 | nothing the log carries moves (the cell is `$2A`, which no column reads); every party hit on it afterwards deals the damage a defence of 26 gives, and from r2 it attacks instead of casting again |
| `$3E` GIRES | 100 TechMaster | `116_d2` | r2 f25775 | 17 | TechMaster's own HP 113 -> 120 (sixteen heal draws, capped) |
| `$12` Fusion | 34 ZolSlug | `D2_d2` | r1 f24865 | 1 | enemy slot 6: HP 50 -> 239, agility 14 -> 12, status bit 7 set on slots 6 and 7; the round ends there; seven more rounds against the MetaSlug replay |

Every row is an observed use. The fixtures replay **exactly**: every action's
queue, target list, verdict, damage, HP, deaths, status bits, `agi_bat` /
`atk_bat` / `dfs_bat` cell and every round's draw count, with the port's
`StatusInflicted` / `StatChanged` / `Died` / `EnemiesFused` events on one side and
the log's cells on the other.

## 3. Deferred carriers

| carrier | why |
|---|---|
| 116 Radhin: `$29` SEALS, `$2D` DEBAN | Every Radhin fight carries abilities nothing implements: `$26` SHIFT and `$27` SANER (`AbilityEffect_AttackUp` / `AgilityUp`, status/stat rows outside this issue) and `$56` FORCEFLASH (lane a1). The `$187` capture (two Radhin, group 55 via `map 0x19B`, kept locally, not committed) shows it: r1 opens with SEALS and ends with FORCEFLASH in the same round, r2 with SANER, SHIFT. No clean prefix exists, so the pair is not routed. `EnemyAttack_Juza` is the routine Greneris uses, so the arm is the same read; it is unobserved clean, which this ledger does not count. |
| 72 BloodSaber `$2D` DEBAN | The routine (`EnemyAttack_ShadowSabr`) and arm are ShadowSabr's, whose DEBAN is routed; no BloodSaber capture has the party *hit* it before its turn (it dodges every swing of the tape party), so `EnemyAI_PhysicalAtkReceived` never fires there and the cast was never seen. |
| 107 Spector, 131 DarkForce2, 135 ProfoundDarkness3 `$4C` | Spector is routed (`16D_d1`). DarkForce2 and ProfoundDarkness3 are the cutscene-gated routines of `ENEMY_ABILITIES.md` section 5 (the scripted-battle flag `$FFFFEE87` is tested first); no map reaches them. |
| `$2A` RIMIT / `$4C` EVIL EYE beyond the fixtures' prefixes | Their carriers' lists also hold abilities nothing runs yet (GIZAN, CORRSION, FORCEFLASH, ZAN, DEATH SPELL `$4E`), which is why each fixture is a prefix. |

## 4. Harness changes the captures needed

All in the write set; each has the control or test named.

- **`--durable` was placed on the wrong frame for a smaller formation.**
  `oracle/force/phases.py` planned the party-HP patch from the *probe's* start
  frame, but the probe builds whatever formation the group's own draw names, and
  the load writes one enemy slot per frame (`loc_7F2E`). Forcing a two-enemy
  formation out of a three-enemy draw put the patch a frame after the start state
  (`TechMaster`: "wrote 999 at f24818 but the capture reads 53 at f24817").
  `oracle/force/durable.py`'s `forced_start_frame` derives the forced
  formation's own frame; the capture's `durable.verify` is the check, and it now
  passes for the formations that failed. No python test: the tests directory is
  outside this lane's write set (see the report).
- **`oracle/sweep/arc.py`** is the committed recipe for these fixtures and
  adds the party's `atk_bat` / `dfs_bat` to each ability action's `effect.stats`
  (the extractor reads the derived `$24` / `$28`, which a buff never moves).
  `oracle/sweep/replay_pack.py` now takes several fixture directories and a
  `SPAWNED` map (Fusion brings enemy 36 in without any formation seating it), and
  `motavia_pack.json` was regenerated.
- **The comparator** (`replay/compare.rs`) used to ask only that *a*
  `StatusInflicted` existed for each status byte the log moved. It now compares
  the bits the log gained with the bits the port inflicted, both ways, ignoring
  transient bit 7 (the seat marker `loc_14D46` sets, answered by the turn skip);
  it checks `agi_bat`, `atk_bat` and `dfs_bat` cells against the port's
  `StatChanged` (and `EnemiesFused`, and the paralysis pin); it checks that the
  fighters an action took to zero HP are the ones the port's turn reports as
  `Died` (for an action with no damage word); and it accepts a queued fighter
  that cannot act as no swing - the log opens a window on it for the round
  tail's wake rolls.
- **`Battle::settle_outcome`** now ends the battle when every occupied party slot
  carries a bit of `$46` (`ps4.asm:9747-9757`); the Goldine, BloodSaber and
  FlyScreamr captures end exactly that way, and the SoldrFiend and BloodSaber
  ones by VOL.
- **The round-end routines no longer run in the round that decided the battle**
  (`engine.rs`): `Battle_RestoreStatsAtTurnEnd` and its sleeper wake rolls are
  reached only when the queue runs out (`ps4.asm:7596`); `FB_d2` ends in a victory
  with Alys asleep and the cartridge draws no wake roll.
- **A missed swing no longer marks the enemy as physically attacked**
  (`action.rs`): `Fighter_TakeDamage` returns before `Character_DamageEnemy` when
  the slot's hit byte is negative (`ps4.asm:3571-3572`). The port set the flag
  on misses; both BloodSaber captures (the party misses it for rounds and its
  `EnemyAI_PhysicalAtkReceived` DEBAN never fires) are the evidence.
- **`Battle::into_party`** clears asleep and tech-sealed at the exit
  (`Battle_LastMessage`, `ps4.asm:6366`).

## 5. Negative controls

Each line breaks one rule and shows that the replay (or the named unit test)
catches it; the break was undone after the run. The harness that ran them is not
committed (it edits source in place); the table is what it printed.

| break | caught by |
|---|---|
| VOICE sets no sleep bit | `D5_d0`: round 4's draw count (the wake rolls) |
| paralysis leaves agility alone | `100_d0` round 8: a damage value that depends on the pinned agility |
| SEALS rolls on a sealed target | `119_d0` round 3: status `$10` inflicted where the log gained nothing |
| DORAN without the floor of one | `119_d0` round 2: `agi_bat` 0 where the log has 1 |
| GELUN lowers nothing / lowers by one | `119_d2` round 2: `atk_bat` 13 / 12 where the log has 0 |
| DEBAN raises nothing | `D8_d0` round 2: it casts again instead of swinging |
| DEBAN raises by one | `D8_d0` round 2: a hit of 4 where the log has 1 |
| STASISBALL visits every slot | `100_d0` round 4: status gained by a slot the log never touched |
| ShadowSabr without its guard | `D8_d0` round 2 |
| a paralyzed party is not a defeat | `1A3_d0`: the fixture ends mid-battle |
| Fusion never runs | `D2_d2` round 1: `UnsupportedAbility $12` |
| the fused enemy acts the round it is seated | `D2_d2` round 1: 34 rolls where the log has 14 |
| GIRES is not a heal record | `116_d2` round 2: `UnsupportedAbility $3E` |
| the round-end wake rolls are not drawn | `D5_d0` round 4: 17 rolls where the log has 20 |
| VOL kills nothing | `119_d8` round 1: the log's Alys at zero HP, the port's turn reports no death |
| a miss marks the enemy physically attacked | `1C6_d0` round 4: the port casts DEBAN where the log has VOL |
| a decided battle still draws the round-end wake rolls | `FB_d2` round 3: 49 rolls where the log has 48 |

## 6. What this does not prove

- The party's `atk_bat` / `dfs_bat` are in the log; the enemies' are not (no
  column), so DEBAN's *size* is proven through the damage later hits deal, and
  GELUN's through the added cells. A fixture extractor change reading
  `$26` / `$2A` would make both direct (`oracle/fixture/observations.py` is
  outside this lane's write set).
- The captures are forced fixtures: the formations are real, the party is not a
  campaign party, and the encounters are not natural.
- Nothing here proves a sealed caster's fizzle (no way to seal an enemy yet) or
  the animations' timing.
- SoldrFiend's VOL kill, Spector's and TechPlant's wake-up through a round-end
  roll, and Haunt's other formations are read, not observed.
