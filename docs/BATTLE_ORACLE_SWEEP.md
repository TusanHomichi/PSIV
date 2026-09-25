# Sweeping the Motavia overworld: every formation, and what the port does with it

What this ledger records: the first **sweep** of the oracle's forced-battle
harness - every formation the Motavia overworld and its vehicle tables can draw,
captured, extracted into a replay fixture, and replayed against `psiv-core` on
the cartridge's own rolls - what the sweep's coverage was, and every divergence
the one data-driven test
([`every_fixture_replays_as_recorded`](../rust/psiv-core/src/battle/replay/data.rs))
found, clustered by cause with a proposed fix scope. The capture tooling is
[`oracle/README.md`](../oracle/README.md), the forced captures' own ledger
[`BATTLE_ORACLE_FORCED.md`](BATTLE_ORACLE_FORCED.md), and the party battles'
[`BATTLE_ORACLE_REPLAY.md`](BATTLE_ORACLE_REPLAY.md).

Fixing these divergences is **not** this lane's work: this is the list, its
evidence and the scope each fix would need. The manifest
([`divergences.json`](../rust/psiv-core/src/battle/replay_fixtures/divergences.json))
is the same list in the form the test reads.

## 1. The list: 83 formations

The sweep's scope is the distinct formation ids in
`generated/formation_indexes.json`'s Motavia groups - **0-7** on foot (the
overworld's own groups: a map's `Battle_EnemyFormationIndexes` byte, or the
position grid's cell for the chunk the party stands on, `loc_7E4C`,
`ps4.asm:11841-11859`) and **8, 9, 10** (the three Motavia vehicle tables,
which `Battle_SetupEnemyData` picks by region when the map byte is `<= 1` and
`Vehicle_Index` is nonzero, `ps4.asm:11825-11839`). Group 13 is Dezolis's and is
not part of this sweep.

```
python3 - <<'PY'
import json
groups = json.load(open("generated/formation_indexes.json"))["groups"]
ids = {g["group"]: g["formation_ids"] for g in groups}
foot = sorted({i for group in range(0, 8) for i in ids[group]})
vehicle = sorted({i for group in (8, 9, 10) for i in ids[group]})
print(len(foot), len(vehicle), len(set(foot) | set(vehicle)))
PY
```

67 on foot and 16 in the vehicle tables, 83 in all and no overlap: the two sets
are disjoint in the pack as it stands. The sweep computes exactly that, and the
list is committed as [`oracle/sweep/motavia_formations.json`](../oracle/sweep/motavia_formations.json)
(`--list`, written by `oracle/sweep/plan.py`).

## 2. The method, and how to reproduce it

One command, from a checkout with the ROM linked in and the core built
(`./oracle/build_core.sh`):

```sh
python3 oracle/sweep.py --out build/lane-evidence/sweep --jobs 3 \
    --list oracle/sweep/motavia_formations.json
```

Per formation that runs `oracle/force_battle.py` with two options this sweep
exists to exercise, and then `oracle/battle_fixture.py` on the capture:

| option | what it does | where |
|---|---|---|
| `--durable` | patches each **living** party-side fighter's current and maximum HP to 999 at the frame the extractor reads the battle's start state from, so a level-1 tape party survives a strong formation's opening round | [`oracle/force/durable.py`](../oracle/force/durable.py) |
| `--max-rounds 5` | stops the capture at round 5's end (the frame before the next round's queue is built) and the fixture records `outcome.truncated` with the rounds it kept; a battle that ends inside the cap simply ends | [`oracle/force/capture.py`](../oracle/force/capture.py), [`oracle/fixture/assembly.py`](../oracle/fixture/assembly.py) |

Everything else is the tape's: the field prefix, the `attack` policy (one `C`
press every 16 frames), the party's other stats, the enemies and their rolls.

`--durable` is what makes the sweep possible at all. Tape 07's party is level 1
- Chaz 25 HP, Alys 53, Hahn 21 - and a Motavia formation whose enemies hit
harder than that would end the fight in round 1, showing only each enemy's
first action. The patch writes the two HP cells of the character record the
battle reads: the party-side fighter's `curr_hp`/`max_hp` **are**
`Chaz_Stats`/`Alys_Stats`/`Hahn_Stats` `+ $0E`/`+ $10`
(`oracle/ram_map.json`: `chaz_hp` `$FFFFF50E`, `chaz_maxhp` `$FFFFF510`, the
other two `$80` apart; measured in tape 07's own battle, where Hahn's word goes
21 -> 15 at f29872 mid-fight). `FillBattleStats` (`ps4.asm:11272-11290`)
refreshes each character's `mod` -> `battle` copies and touches no HP cell, so a
write placed on the frame the extractor samples the start state from
(`enemies_loaded`, the load's own last frame) is what the fixture and the battle
both see; the tool refuses a capture whose log does not read the patch back
there. A vehicle battle's party side is the vehicle (`loc_78EE`,
`ps4.asm:11408`), so the patch lands on `Vehicle_Stats`' two HP cells instead
(`vehicle_fighter_hp`/`vehicle_fighter_max_hp`, `$FFFF470E`/`$FFFF4710`, which
`loc_77AE` writes from `VehicleData` at `ps4.asm:11294-11295`).

The run is resumable (a formation whose fixture is on disk and whose recorded
hash still matches is skipped), a formation that fails is **recorded, not
fatal** - its stage and error are in the record - and at most three captures
run at once, which is this machine's memory cap (an emulator run plus a ~30MB
RAM log the extractor parses in Python).

`build/lane-evidence/sweep/sweep_motavia.json` is the run's record: per
formation the group, the selector, the capture's trace and log sha256, the
outcome, the rounds, the abilities observed, the fixture's own sha256, and the
two exact commands; plus a census and every failure. The fixtures are committed
under `rust/psiv-core/src/battle/replay_fixtures/sweep_motavia/`.

The replay side is unchanged: `every_fixture_replays_as_recorded` walks the
fixture directory (subdirectories included, a sweep's fixtures named by their
path - `sweep_motavia/formation_05`), builds each battle from the fixture's own
start state, feeds it the cartridge's rolls, and holds the timeline against the
log action by action. A fixture that does not replay exactly must have an entry
in `divergences.json` naming its first divergence, and the entry cannot outlive
its fix.

### Reproducing this ledger

```sh
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- --test-threads=1 every_fixture_replays_as_recorded
```

The manifest this ledger describes is generated, not typed: the Rust side prints
each diverging fixture's first divergence and
[`oracle/sweep/manifest.py`](../oracle/sweep/manifest.py) assigns it to a
cluster and writes `divergences.json`.

```sh
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- --ignored --nocapture dump_manifest_entries > build/lane-evidence/findings.txt
python3 -m oracle.sweep.manifest --dump build/lane-evidence/findings.txt \
    --manifest rust/psiv-core/src/battle/replay_fixtures/divergences.json
python3 -m oracle.sweep.manifest --dump build/lane-evidence/findings.txt --clusters
```

## 3. Coverage

| | |
|---|---|
| formations in the list | 83 |
| captured, with a fixture | 81 |
| failed (recorded, not fatal) | 2 |
| captures that hit the round cap | 36 |
| captures that ended in battle | 45 |
| rounds captured | 327 |

### Failures

| formation | stage | error |
|---|---|---|
| `0x27` | capture | force_battle: the probe run failed (exit 1): psiv_oracle: rng trace f24819: 1 call(s) from 6EA56A13 chain to B7526A13 but the frame ended at 6EA56A13 psiv_oracle: rng trace build/lane-evidence/sweep/f |
| `0x28` | capture | force_battle: the probe run failed (exit 1): psiv_oracle: rng trace f24819: 1 call(s) from 6EA56A13 chain to B7526A13 but the frame ended at 6EA56A13 psiv_oracle: rng trace build/lane-evidence/sweep/f |

### How each formation was reached

| selector | formations |
|---|---|
| grid | 51 |
| map | 14 |
| vehicle | 16 |

### Enemies seated (22 distinct)

| enemy | formations |
|---|---|
| 1 MonsterFly | 23 |
| 5 ForcedFly | 15 |
| 14 Locusta | 6 |
| 15 Fanbite | 7 |
| 16 Grasshound | 7 |
| 30 Crawler | 40 |
| 32 Caterpillr | 15 |
| 56 SandNewt | 21 |
| 67 Scorpirus | 6 |
| 80 SandWorm | 1 |
| 81 DesrtLeach | 1 |
| 82 Leviathan | 1 |
| 90 Depcen | 5 |
| 91 HewGilla | 5 |
| 92 Elmelew | 6 |
| 93 MiniWorm | 9 |
| 94 InfantWorm | 9 |
| 99 TechUser | 9 |
| 102 Speard | 6 |
| 147 Rappy | 9 |
| 148 BlueRappy | 5 |
| 150 InfantWorm2 | 9 |

### Enemy abilities the sweep saw run (12 of the inventory's 83)

| ability | status | formations | first seen |
|---|---|---|---|
| `$02` FlameBolt | implemented | 12 | 0x4A at f25122 |
| `$08` SpiralBld | implemented | 6 | 0x37 at f26955 |
| `$11` Poison | implemented | 15 | 0x38 at f26379 |
| `$37` SandStorm | implemented | 1 | 0x53 at f25295 |
| `$38` Earthquake | implemented | 1 | 0x3B at f26155 |
| `$39` Maelstrom | implemented | 1 | 0x54 at f24920 |
| `$3F` FlodBreath | implemented | 13 | 0x43 at f24885 |
| `$40` Wat | implemented | 14 | 0x2A at f25308 |
| `$44` Foi | implemented | 8 | 0x2A at f25019 |
| `$45` ? | unsupported (?) | 4 | 0x2A at f26539 |
| `$6D` RoundEyes | implemented | 9 | 0x45 at f24873 |
| `$6E` LovelEyes | implemented | 5 | 0x48 at f25026 |

72 of the inventory's 83 ability ids were not seen: `$03`, `$04`, `$05`, `$07`, `$09`, `$0B`, `$0E`, `$0F`, `$10`, `$13`, `$16`, `$17`, `$19`, `$1C`, `$1D`, `$1F`, `$20`, `$21`, `$22`, `$23`, `$24`, `$25`, `$26`, `$27`, `$28`, `$29`, `$2A`, `$2B`, `$2D`, `$2E`, `$2F`, `$30`, `$31`, `$32`, `$33`, `$34`, `$35`, `$36`, `$3C`, `$3D`, `$3E`, `$42`, `$43`, `$47`, `$48`, `$4A`, `$4B`, `$4C`, `$4D`, `$4E`, `$4F`, `$50`, `$51`, `$52`, `$54`, `$55`, `$56`, `$57`, `$58`, `$5A`, `$5D`, `$5E`, `$5F`, `$60`, `$61`, `$63`, `$64`, `$65`, `$69`, `$6A`, `$6C`, `$70`.

## 4. The divergences, clustered

52 of the 81 captured formations do not replay exactly. Each is one row below,
grouped by the **first** divergence `every_fixture_replays_as_recorded` finds
for it, and every entry in `divergences.json` names its cluster's anchor. The
cause column is what the evidence supports; where it does not settle one, the
cell says so and the fix scope is the measurement that would.

| cluster | cause as best supported | proposed fix scope |
|---|---|---|
| 1. a swing whose commanded enemy has fallen (10) | the cartridge moves `Current_Target_Index` off the commanded enemy between the command phase and the swing: measured, `Character_Command_Data` holds target 6 while `Current_Target_Index` reads 8 at the acting frame (probe: `build/lane-evidence/target-probe`), and `loc_B6A2` (`ps4.asm:17492`) then swings at the single slot that cell names. The port's `candidate_targets` (`rust/psiv-core/src/battle/action.rs`) falls back to `roster.first_living` - the lowest living id - when the order carries no target | port: the retarget rule itself, once its writer is identified; harness: log the party's command cells (`bcmd`) and `Current_Target_Index`, record the intended target per action and hand it to the replay as `Command::AttackTarget` |
| 2. a window the port narrows to one slot (13) | the port's single-target model: the log resolves two to four slots for one swing (a weapon or ability whose window covers more than the target), the port's `candidate_targets` returns one | port: the weapon's/attack's window rule; each fixture names the actor and the slots |
| 3. an ability that spends the turn (10) | the log shows the ability's byte moving with nothing behind it (`EnemyAbilityWasted`), the port resolves an effect - the abilities are `$45`, `$11` and `$02`, all *supported* carriers in `docs/ENEMY_ABILITIES.md`, so the arm taken differs (hypothesis: the object/timer dispatch) | port: `enemy_skill`/`enemy_damage`'s arm selection for the named ability and enemy |
| 4. the round's queue (7) | the turn order itself (`Battle_Turn_Order`, agility + jitter): the log's queue is in a different order | port: the queue build's jitter draws or the agility key; the fixtures name the round |
| 5. a verdict or a damage value (5) | one resolution's verdict (normal/critical) or damage word differs; `formation_3B`/`4F` are criticals the port lands stronger (248 -> 312, 71 -> 103) | port: `loc_B6A2`'s crit rule and `Battle_CalculateDamage`; per-fixture numbers are in the manifest |
| 6. a turn that never swings (4) | the log resolves zero targets for an actor the port gives no swing at all - an enemy whose turn the port spends otherwise (hypothesis: a status/ability arm) | port: the actor's turn arm; harness: the fixture's `kind` for that action |
| 7. a target the port leaves alone (2) | the log resolves a *party* member the port's swing never reaches (an all-party ability's slot) | port: the ability's target class |
| 8. a round's roll count (1) | one round draws 120 calls in the log and 119 in the port | port: the draw that is missing; the fixture is `formation_08` round 1 |



52 finding(s) in 8 cluster(s)

### A swing whose commanded enemy has fallen lands on another slot (`party-retarget`, 10 fixture(s))
- `sweep_motavia/formation_02` - f25669 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_04` - f26055 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_05` - f26151 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_0B` - f25661 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_0C` - f25512 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_17` - f26168 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_18` - f26280 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_19` - f26223 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_1B` - f26964 (targets): the log has targets [FighterId(8)], the port targets [FighterId(7)]
- `sweep_motavia/formation_1D` - f26667 (targets): the log has targets [FighterId(9)], the port targets [FighterId(7)]

### The port swings at one slot where the log's window covered several (`party-window`, 13 fixture(s))
- `sweep_motavia/formation_10` - f25693 (targets): the log has targets [FighterId(6), FighterId(7), FighterId(8), FighterId(9)], the port targets [FighterId(6)]
- `sweep_motavia/formation_16` - f25488 (targets): the log has targets [FighterId(6), FighterId(7)], the port targets [FighterId(6)]
- `sweep_motavia/formation_22` - f26271 (targets): the log has targets [FighterId(6), FighterId(7)], the port targets [FighterId(6)]
- `sweep_motavia/formation_23` - f26516 (targets): the log has targets [FighterId(6), FighterId(7), FighterId(8)], the port targets [FighterId(6)]
- `sweep_motavia/formation_2D` - f25766 (targets): the log has targets [FighterId(6), FighterId(7)], the port targets [FighterId(6)]
- `sweep_motavia/formation_2E` - f25927 (targets): the log has targets [FighterId(6), FighterId(7), FighterId(8)], the port targets [FighterId(6)]
- `sweep_motavia/formation_3C` - f25421 (targets): the log has targets [FighterId(6), FighterId(7)], the port targets [FighterId(6)]
- `sweep_motavia/formation_43` - f27819 (targets): the log has targets [FighterId(6), FighterId(7)], the port targets [FighterId(6)]
- `sweep_motavia/formation_44` - f27201 (targets): the log has targets [FighterId(6), FighterId(7), FighterId(8)], the port targets [FighterId(6)]
- `sweep_motavia/formation_45` - f25927 (targets): the log has targets [FighterId(6), FighterId(7)], the port targets [FighterId(6)]
- `sweep_motavia/formation_46` - f26189 (targets): the log has targets [FighterId(6), FighterId(7), FighterId(8)], the port targets [FighterId(6)]
- `sweep_motavia/formation_48` - f26564 (targets): the log has targets [FighterId(6), FighterId(7)], the port targets [FighterId(6)]
- `sweep_motavia/formation_49` - f26931 (targets): the log has targets [FighterId(6), FighterId(7), FighterId(8)], the port targets [FighterId(6)]

### An ability the port resolves as an effect spends the turn in the log (`ability-spends-the-turn`, 10 fixture(s))
- `sweep_motavia/formation_2A` - f26539 (not-wasted): the log has ability $45 spending the turn, the port an effect
- `sweep_motavia/formation_2B` - f27707 (not-wasted): the log has ability $45 spending the turn, the port an effect
- `sweep_motavia/formation_32` - f27115 (not-wasted): the log has ability $45 spending the turn, the port an effect
- `sweep_motavia/formation_34` - f27629 (not-wasted): the log has ability $45 spending the turn, the port an effect
- `sweep_motavia/formation_38` - f25019 (not-wasted): the log has ability $11 spending the turn, the port an effect
- `sweep_motavia/formation_39` - f25153 (not-wasted): the log has ability $11 spending the turn, the port an effect
- `sweep_motavia/formation_3A` - f24887 (not-wasted): the log has ability $11 spending the turn, the port an effect
- `sweep_motavia/formation_41` - f24902 (not-wasted): the log has ability $11 spending the turn, the port an effect
- `sweep_motavia/formation_42` - f25051 (not-wasted): the log has ability $11 spending the turn, the port an effect
- `sweep_motavia/formation_4B` - f25680 (not-wasted): the log has ability $02 spending the turn, the port an effect

### The round's queue is not the log's (`queue`, 7 fixture(s))
- `sweep_motavia/formation_13` - f25013 (queue): the log has round 1's queue [FighterId(1), FighterId(6), FighterId(7), FighterId(2), FighterId(3)], the port queue [FighterId(1), FighterId(6), FighterId(2), FighterId(3)]
- `sweep_motavia/formation_14` - f25013 (queue): the log has round 1's queue [FighterId(1), FighterId(6), FighterId(7), FighterId(8), FighterId(2), FighterId(3)], the port queue [FighterId(1), FighterId(6), FighterId(2), FighterId(3)]
- `sweep_motavia/formation_15` - f25013 (queue): the log has round 1's queue [FighterId(7), FighterId(1), FighterId(8), FighterId(6), FighterId(9), FighterId(2), FighterId(3)], the port queue [FighterId(1), FighterId(6), FighterId(2), FighterId(3)]
- `sweep_motavia/formation_25` - f25013 (queue): the log has round 1's queue [FighterId(8), FighterId(1), FighterId(6), FighterId(7), FighterId(2), FighterId(3)], the port queue [FighterId(1), FighterId(6), FighterId(7), FighterId(2), FighterId(3)]
- `sweep_motavia/formation_26` - f25013 (queue): the log has round 1's queue [FighterId(8), FighterId(1), FighterId(6), FighterId(7), FighterId(9), FighterId(2), FighterId(3)], the port queue [FighterId(1), FighterId(6), FighterId(7), FighterId(2), FighterId(3)]
- `sweep_motavia/formation_3E` - f24882 (queue): the log has round 1's queue [FighterId(9), FighterId(7), FighterId(6), FighterId(8)], the port queue [FighterId(7), FighterId(6), FighterId(8)]
- `sweep_motavia/formation_47` - f25013 (queue): the log has round 1's queue [FighterId(8), FighterId(7), FighterId(6), FighterId(9), FighterId(1), FighterId(3), FighterId(2)], the port queue [FighterId(8), FighterId(7), FighterId(6), FighterId(1), FighterId(3), FighterId(2)]

### A verdict or a damage value differs (`value`, 5 fixture(s))
- `sweep_motavia/formation_07` - f25542 (value): the log has FighterId(7): hit flag 00, damage Some(9), the port Normal with Some(8)
- `sweep_motavia/formation_36` - f26009 (value): the log has FighterId(6): hit flag 00, damage None, the port Normal with Some(1)
- `sweep_motavia/formation_37` - f25397 (value): the log has FighterId(6): hit flag FF, damage None, the port Normal with Some(1)
- `sweep_motavia/formation_3B` - f25003 (value): the log has FighterId(1): hit flag 01, damage Some(248), the port Critical with Some(312)
- `sweep_motavia/formation_4F` - f25240 (value): the log has FighterId(1): hit flag 01, damage Some(71), the port Critical with Some(103)

### The log resolves targets and the port's actor never swings (`no-swing`, 4 fixture(s))
- `sweep_motavia/formation_4C` - f26205 (no-swing): the log has actor FighterId(8) resolving 0 target(s), the port no swing
- `sweep_motavia/formation_4E` - f25586 (no-swing): the log has actor FighterId(6) resolving 0 target(s), the port no swing
- `sweep_motavia/formation_55` - f25250 (no-swing): the log has actor FighterId(6) resolving 0 target(s), the port no swing
- `sweep_motavia/formation_56` - f25496 (no-swing): the log has actor FighterId(6) resolving 0 target(s), the port no swing

### The log resolves a target the port leaves alone (`no-resolution`, 2 fixture(s))
- `sweep_motavia/formation_3F` - f25035 (no-resolution): the log has FighterId(7) resolving FighterId(3), the port no resolution
- `sweep_motavia/formation_40` - f25835 (no-resolution): the log has FighterId(7) resolving FighterId(3), the port no resolution

### A round draws a different number of rolls (`draws`, 1 fixture(s))
- `sweep_motavia/formation_08` - f25061 (draws): the log has 120 roll(s) in the round's frames, the port 119 roll(s) drawn

Each cluster's ledger anchor is the heading above (`#cluster-N-...`), and every entry in
`divergences.json` carries its own cluster's anchor as `ledger`, so the manifest and
this list cannot disagree: `oracle/sweep/manifest.py` refuses a finding that
belongs to no cluster or to more than one.

