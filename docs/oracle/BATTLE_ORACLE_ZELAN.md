# The Zelan route's status and stat abilities, captured and replayed

What this ledger records: the forced captures that prove the status, stat and
conditional enemy abilities of the route from Zelan to Kuran
([issue #58](https://github.com/TusanHomichi/PSIV/issues/58)'s class, lane A4,
2026-10-04) - POISONMIST, SLEEP GAS, SHADOWBIND, the refill WARNING, Dark Force 1's
latch-gated first action and, as the evidence for the handler move, THREAD - the
replay verdict of each, what stays deferred and why, the harness changes the
captures needed, and the negative controls. The abilities are the ones
`python3 -m oracle.sweep.route_abilities` derives
([`ENEMY_ABILITIES.md`](../battle/ENEMY_ABILITIES.md) section 6); the rules are
[`ENEMY_EFFECT_ABILITIES.md`](../battle/ENEMY_EFFECT_ABILITIES.md) section 7; the
tooling is [`BATTLE_ORACLE_FORCED.md`](BATTLE_ORACLE_FORCED.md) (the captures),
[`BATTLE_ORACLE_REPLAY.md`](BATTLE_ORACLE_REPLAY.md) (the comparator) and
[`BATTLE_ORACLE_SWEEP.md`](BATTLE_ORACLE_SWEEP.md) (the divergence manifest). The
Motavia arc's twin of this file is [`BATTLE_ORACLE_ARC.md`](BATTLE_ORACLE_ARC.md).

## 1. The captures

Every fixture is one `python3 -m oracle.force --durable` run from tape 07's own field
prefix (`--formation F`, or `--event N` for an event battle,
[section 8 of the forced ledger](BATTLE_ORACLE_FORCED.md)), extracted by
`python3 -m oracle.fixture` and finished by `python3 -m oracle.sweep.zelan`.
`python3 -m oracle.sweep.zelan --list` prints each command; `--capture` and
`--extract` reproduce the set. Each capture was run twice by the tool and came back
byte-identical, and `python3 -m oracle.rng_trace check` passed for every one. The
fixtures live in `rust/psiv-core/src/battle/replay_fixtures/arc_zelan/` and are
replayed by the same data-driven test as every other fixture
(`every_fixture_replays_as_recorded`); the divergence manifest
(`replay_fixtures/divergences.json`) is **empty**.

| fixture | formation | enemies (slot:record) | selector | battle frames | trace sha256 | log sha256 | rounds kept |
|---|---|---|---|---|---|---|---|
| `formation_61_d0` | 0x61 | 1:57, 2:57 | group 11 via world map 1, position-grid cell (7,14) | f24794-29748 | 4a545f5aa1e6 | b5f0c5fbcf70 | 8 |
| `formation_11E_d0` | 0x11E | 1:63, 2:63, 3:63 | group 44 via map 351 (Hangar) | f24794-30404 | 28de83373935 | 369bc142026c | 8 |
| `formation_124_d0` | 0x124 | 1:50, 2:45, 3:50 | group 45 via map 400 (Kuran) | f24794-25476 | 9e1a1dbf797e | 7edf6fef1c54 | 1 |
| `formation_124_d2` | 0x124 | 1:50, 2:45, 3:50 | group 45 via map 400 (Kuran) | f24794-25702 | 77a9f36dba6d | a0e9301f8999 | 2 |
| `formation_170_d0` | 0x170 | 1:111 | group 53 via map 389 (AirCastleInner_B1) | f24794-25140 | efd5e818d031 | 166c6686bf0e | 1 |
| `event_08_d9` | event 8 | 1:138 | Event_Battle_Index = 8 | f24794-26589 | 7981efbfbf67 | b2874a855953 | 3 |
| `event_09_d0` | event 9 | 1:130 | Event_Battle_Index = 9 | f24794-25396 | 6093b7b9672e | 0a130bade79a | 1 |
| `formation_9A_d0` | 0x9A | 1:31, 2:31, 3:31 | group 18 via map 155 (ValleyMaze) | f24794-27752 | d7cb640f9fc1 | a02e7eba2577 | 4 |

(The sha256 columns are the first twelve hex digits; the full values are in each
fixture's `provenance`.) The party is always tape 07's - Alys level 7, Chaz and Hahn
level 1 - with 999 HP each from `--durable`; fighter 1 is Alys, 2 Chaz, 3 Hahn, 6 to 9
the enemy slots. That is a fixture, not a route: no player walks to these battles
with this party, and the event battles are forced by writing `Event_Battle_Index`
over a random encounter.

A capture is cut (`--max-rounds`, then the recipe's `rounds`) before the first ability
that no lane implements: FloatMine2's EXPLOSION `$18` and CommndBall's DETONATION
`$19` (formation `$124`), ChaosSorcr's FLAELI `$5A` and HEWN `$4F`, Dark Force 1's
FLARE SHOT `$1C`. The delay scan that chose them is the capture tool's own
`--delay`: event 8 was scanned over delays 0 to 12 and only delay 9 uses SHADOWBIND
before FLAELI or HEWN; formation `$124` over 0 to 3, where all four open with
WARNING and delay 2 is the one whose second round is also all supported.

## 2. What each pair was seen doing

"Round" and "frame" are the log's: the frame the action's ability byte moved on (for a
fixture round, the first frame of the action). "Rolls" is what the action drew (the
ability roll plus one chance roll per visited slot). `status` and `agi_bat` are the cells
the log shows moving in the action; "nothing the log carries moves" is a landing on a
target that already carried the status, a miss, or an arm that changes no cell. The
table is generated: `python3 -m oracle.sweep.observed`.

| fixture | ability | round | frame | actor | rolls | what the log shows |
|---|---|---|---|---|---|---|
| `event_08_d9` | `$4B` | 1 | f24980 | 6 | 4 | fighter 1 agi_bat 15->1; fighter 3 agi_bat 4->1 |
| `event_08_d9` | `$4B` | 3 | f26004 | 6 | 4 | fighter 2 agi_bat 7->1 |
| `formation_11E_d0` | `$25` | 1 | f25293 | 8 | 5 | fighter 3 status $00->$08 |
| `formation_11E_d0` | `$25` | 4 | f27451 | 6 | 4 | fighter 1 status $01->$09; fighter 2 status $00->$08; fighter 3 status $00->$08 |
| `formation_11E_d0` | `$25` | 4 | f27653 | 8 | 2 | nothing the log carries moves |
| `formation_11E_d0` | `$25` | 7 | f29483 | 8 | 3 | fighter 2 status $01->$09; fighter 3 status $01->$09 |
| `formation_11E_d0` | `$25` | 8 | f30067 | 7 | 3 | fighter 3 status $01->$09 |
| `formation_124_d0` | `$14` | 1 | f24971 | 7 | 2 | nothing the log carries moves |
| `formation_124_d2` | `$14` | 1 | f24868 | 7 | 4 | nothing the log carries moves |
| `formation_124_d2` | `$14` | 2 | f25101 | 7 | 1 | nothing the log carries moves |
| `formation_170_d0` | `$4B` | 1 | f24875 | 6 | 4 | fighter 1 agi_bat 15->1; fighter 2 agi_bat 7->1; fighter 3 agi_bat 4->1 |
| `formation_61_d0` | `$24` | 1 | f24886 | 7 | 2 | fighter 2 status $00->$01 |
| `formation_61_d0` | `$24` | 3 | f25883 | 6 | 1 | nothing the log carries moves |
| `formation_61_d0` | `$24` | 5 | f27131 | 7 | 1 | nothing the log carries moves |
| `formation_61_d0` | `$24` | 6 | f27755 | 6 | 2 | fighter 1 status $00->$01 |
| `formation_61_d0` | `$24` | 7 | f28459 | 7 | 1 | nothing the log carries moves |
| `formation_61_d0` | `$24` | 7 | f28519 | 6 | 1 | nothing the log carries moves |
| `formation_9A_d0` | `$10` | 2 | f26050 | 7 | 2 | fighter 2 agi_bat 7->1 |
| `formation_9A_d0` | `$10` | 3 | f26598 | 8 | 3 | nothing the log carries moves |

Rows the table cannot show, by hand:

| ability | enemy | fixture | round, frame | what the log shows |
|---|---|---|---|---|
| Dark Force 1's first action (`$818`) | 130 DarkForce1 | `event_09_d0` | r1 f24891 | the party is ambushed (priority `$FF`), the queue is the enemy alone, and the enemy's action reads ability `$00` with no slot resolved and no cell moving: one draw (the ability roll), nothing else. Rounds 2 and 3 of the same capture show FLARE SHOT at f25398 and are not kept |
| `$17` WAITING (the spent turn, first observation) | 50 FloatMine2 | `formation_124_d2` | r2 f25179 | the refilled slot 8 acts: two draws (the ability roll and one re-roll), no hit pass, ability byte `$00` because `loc_10406` writes and clears it inside one frame |
| `$14` WARNING's refill | 45 CommndBall | `formation_124_d2` | r1 f24868 -> r2 queue | round 1's queue is `[7]` alone (both FloatMine2 were cleared by `EnemyInit_Tower`); after the action slot 8 carries status `$80` (the seat marker) and is in round 2's queue, where it acts; the second WARNING (r2 f25101) refills slot 6 |

Every row is an observed use. The fixtures replay **exactly**: every action's queue,
target list, verdict, damage, HP, deaths, status bits, battle-agility cell and every
round's draw count.

## 3. Deferred and read-only carriers

| carrier | why |
|---|---|
| 48 Siren386 `$1D` BARRIER (conditional, arm `$08`) | Fires only after the Siren is hit by magic (`EnemyAI_MagicDamageReceived`, reaction flag bit 1). The capture tool has no policy that commands a technique ([forced ledger](BATTLE_ORACLE_FORCED.md) section 1.3: it needs a `tech` policy, a party that knows a single-target technique, and an extractor that records the command kind - the last in `oracle/fixture/`, outside this lane), so no capture can show the arm. The handler (`AbilityEffect_MagicDefenseUp`, range 2, no roll) is not written without an observed use; until then `UnsupportedAbility` and the physical swing. The arm guards on its own derived and battle magic defence (`move.w $2E(a2), d3 / cmp.w $2C(a2), d3 / bgt.w loc_10016`, `ps4.asm:22568-22570`) like ShadowSabr's DEBAN. |
| 42 SatMinion `$17` WAITING (conditional, arm `$05`) | Implemented by reading: `EnemyAttack_ArmDrone`'s `$17` arm `loc_10468` (`ps4.asm:22806`) is the six instructions of the FloatMine fall-through `loc_10406`, whose behaviour `formation_124_d2` now shows. Every formation that triggers it (`$123`, SatMinion, CRayTube, SatMinion) carries CRayTube's CHARGCNNON `$15` in round 1, a damage ability another lane owns, so no fixture can be kept. Re-capture `$123` once CHARGCNNON is routed. |
| 58 FlameNewt `$24`, 105 ShadMirage and 132 DarkForce3 `$4B` | Share a routine and arm with a routed pair, are not on the route, were not captured. |
| 39 Tower `$14` | Same init (`EnemyInit_Tower`) and record as 45 CommndBall, not on the route, not captured; the init gate covers it, the refill record is shared, no fixture. |
| `$FFFFEE87`'s other first-action readers (133 ProfoundDarkness1 `$864`, 131 DarkForce2 `$83C`, 129 CarnivorousTree `$808`) | Not on this route (Dezolis's later arc); their objects' chains are unread. |

## 4. Harness changes the captures needed

All in the write set; each has the control or test named.

- **`python3 -m oracle.force --event N`** (`oracle/force/`): forces an event battle by
  writing `Event_Battle_Index`. See [the forced ledger, section 8](BATTLE_ORACLE_FORCED.md).
  The model check is part of the run (`phases.event_probe`: the probe must build the boss
  formation's enemies and HP); the committed fixtures `event_08_d9` and `event_09_d0`
  are its evidence. No python test: `tests/` is outside this lane's write set (see the
  report).
- **The durable patch's frame** (`oracle/force/durable.py`): three GerotLux load 1 + 2 on
  the frames of the start state, so the estimated frame was one late and
  `durable.verify` refused the capture. The patch is written on every frame from the
  draw to the estimate; `tests.test_oracle_force_battle_run` (existing) still pins the
  last write.
- **`oracle/sweep/arc.py`** is now `run_recipe` plus its own entries, shared by
  `oracle/sweep/zelan.py`; `Entry` carries `event`. `oracle/sweep/replay_pack.py`
  regenerated `motavia_pack.json` (49 enemy records, 54 ability records).
- **The comparator** (`replay/compare.rs`) takes two more answers for an *attack* the
  log shows resolving nothing: `FirstZioAction` (a scripted first action clears the
  ability byte) and `EnemyAbilityWasted` (the FloatMine fall-through writes and clears
  its id inside one frame, so the byte reads zero). The round's own draw count is what
  tells them from a swing that missed: a swing draws a hit roll, these draw the ability
  roll and nothing after.
- **The replay builder** (`replay/build.rs`) no longer asks the port to seat the
  enemies a formation's init routine clears: the log still holds the slot's record
  (`enemy_count` 3), the first round's queue says it is not fighting
  (`EnemyInit_Igglanova`/`EnemyInit_Tower`, `ps4.asm:18275`, `18243`).
- **`oracle/sweep/route_abilities.py`** and **`observed.py`** are the two generators this
  ledger and the inventory use; neither decides a behaviour.

## 5. Negative controls

Each line breaks one rule and shows the replay catches it; the break was undone after
the run. The harness that ran them edits source in place and is not committed; the
table is what it printed.

| break | caught by |
|---|---|
| POISONMIST not routed | `formation_61_d0` round 1: `UnsupportedAbility $24` and a physical swing |
| SLEEP GAS not routed | `formation_11E_d0` round 1: `UnsupportedAbility $25` |
| THREAD not routed (the crawler handler move) | `formation_9A_d0` round 2: `UnsupportedAbility $10` |
| SHADOWBIND not routed for 138 / for 111 | `event_08_d9` / `formation_170_d0` round 1: `UnsupportedAbility $4B` |
| a poisoned target still takes the roll | `formation_61_d0` round 3: the log's action resolves one target and the port's draws fall short of it |
| WARNING is not a refill | `formation_124_d0` round 1: `UnsupportedAbility $14` |
| CommndBall's neighbours stay in the formation | `replay/build.rs`: the formation seats the cartridge's enemies |
| Dark Force 1 reads no latch | `event_09_d0` round 1: a physical swing of 204 where the log has nothing |
| the agility-down floor is 0 (the handler SHADOWBIND shares with DORAN and THREAD) | `arc_motavia/formation_119_d0` round 2, the first fixture in file order: `agi_bat` 0 where the log has 1 |

## 6. Cartridge findings

- **WARNING is a refill, not a spent turn.** `EnemyInit_Tower` clears the objects beside
  a CommndBall and its `$14` object ends in `loc_14CBE`, Fission's refill; the first
  reading (an alarm and nothing else) was refuted by the capture's round-1 queue.
- **The FloatMine fall-through is now observed** (`$17` on 50 FloatMine2): the log's
  ability byte is `$00` for it, which is why the extractor files it as an attack.
- **BURSTROC `$63` is a damage ability**, not scripted/custom: object `$82C` jumps into
  `loc_24A9E`, the all-party request tail (`ps4.asm:64263`, `48483`).
- **Dark Force 1's first action** replaces the rolled ability (byte cleared) with object
  `$818`, which requests nothing; the battle is an ambush because the latch is up.
- **Siren386 carries `$1D` BARRIER as a conditional**, which `ENEMY_ABILITIES.md` section 2
  did not list (it names 49 Browren486 only).
- **STRNGLIGHT is not on this route**; section 4 of the inventory listed it with the
  Dezolis abilities, its only carrier (Shrieker) is on the Island Cave maps.
- **A formation's slot records survive their init routine**: a cleared neighbour keeps
  its record and `enemy_count` counts it.

## 7. What this does not prove

- The captures are forced fixtures: the formations are real, the party is not a
  campaign party, the event battles are random encounters with `Event_Battle_Index`
  written over them, and nothing here walks the route.
- Event battle 8's and 9's later rounds (FLAELI, HEWN, FLARE SHOT) are not in any
  fixture; they are the damage lane's.
- The enemy-side agility cell is not in the log (`agi_bat` is the party's), so
  SHADOWBIND's effect on its own side is proven only through the party cells.
- SatMinion's WAITING, BARRIER, Tower's WARNING and the other latch readers are read or
  deferred, not observed (section 3).
- The animations' timing: no frame-level claim is made.
