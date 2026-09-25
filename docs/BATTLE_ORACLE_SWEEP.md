# Sweeping the Motavia overworld: every formation, and what the port does with it

What this ledger records: the first **sweep** of the oracle's forced-battle
harness - every formation the Motavia overworld and its vehicle tables can draw,
captured, extracted into a replay fixture, and replayed against `psiv-core` on
the cartridge's own rolls - what the sweep's coverage was, and every divergence
the one data-driven test
([`every_fixture_replays_as_recorded`](../rust/psiv-core/src/battle/replay/data.rs))
found, and what the triage that followed made of it: five of the eight clusters
were the harness misreading the cartridge, three are port rules, and §5 is every
fixture that had to be re-extracted because of the first. The capture tooling is
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

A sweep's captures are the expensive half, so the extraction is its own run:
every formation directory that holds a capture is extracted again, with the
numbers its own `report.json` recorded, into `--fixtures`.

```sh
python3 oracle/sweep.py --reextract build/lane-evidence/sweep --jobs 3
```

Everything a fixture records about a capture is path-free - the tape by its file
name, the log and trace headers with their path fields cut
(`oracle/fixture/logs.py`'s `provenance_lines`, and `oracle/host/provenance.h`
on the host's side) - so the same capture swept into another directory yields
the same fixture bytes and the same `log_sha256`
(`tests/test_oracle_provenance.py`).

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

## 4. The divergences, triaged

52 of the 81 captured formations did not replay exactly when this sweep was
written, in 8 clusters. Each cluster has since been settled against the raw
capture, the cartridge routine and the port's code, and the answers are not the
ones the first pass guessed:

* **Five of the eight clusters were the harness misreading what the cartridge
  did.** They are fixed - in the extractor, in the comparator and, for the
  provenance, in the host - with tests, and the fixtures were re-extracted from
  the sweep's own preserved captures (§4.2, §5).
* **Three were port rules** the cartridge has and `psiv-core` does not, and one
  cluster that looked like a fourth port rule turned out to be the harness too
  (§4.4).

18 findings remain, in 4 causes. The method per cluster was the same: read the
RAM log's own rows around the divergent action - not the fixture built from
them - read the fixture's record, run the port and read what it resolved, and
then read the routine the row points at in `reference/ps4disasm/ps4.asm`.

Every verdict below rests on those three sources and cites them: log rows by
frame, routines by `ps4.asm` line, the port by file. **Nothing is left as a
hypothesis** - the one question the sweep's own captures cannot answer at all
(the fighter a member's command actually named) is §4.3, which says what is
missing, what a capture with it looks like, and what a lane that fixes the rule
would have to re-take.

### 4.1 The verdicts

| cluster | findings | verdict | evidence | fix scope |
|---|---|---|---|---|
| 1. a swing whose commanded enemy has fallen | 10 | **port rule** | `formation_02` f25669: enemy slot 1 is down (-12 HP), slots 2-4 read 9/8/9 of 20, and the log's own flash pin `hit_07` (the byte `st -$1(a0,d6.w)` writes for slot 8, `ps4.asm:17528`) is `$00` - the swing went to slot **8**, the enemy with the largest `max_hp - curr_hp`. The port swings at slot **7**, `candidate_targets`' `roster.first_living` (`rust/psiv-core/src/battle/action.rs`). | the rule is `loc_5A98` -> `loc_5AE6` (`ps4.asm:8331-8410`): when the commanded target's slot is empty or the fighter is dead/paralyzed/absent, scan the enemy slots 6-9 for the largest HP deficit, break a tie with one `UpdateRNGSeed2` draw (`btst #0, d1`, `ps4.asm:8371`), and write the winner into `Current_Target_Index` (`move.w d3, (Current_Target_Index).l`, `ps4.asm:8409`) - which `loc_B6A2` then swings at. Port scope: model the scan, and read the command's own target (§4.3). No rule may be changed from the port's side alone: the scan fires only when the commanded target is already down. |
| 2. a window the port narrows to one slot | 13 | **harness artifact** | `formation_10` f25693: the pass's own verdicts are one resolved slot (`hit_05` = `$00`, everything else `$FF`), and the extra three "targets" came from f25808, where `battle_actor` reads **1541** and the bytes `00 00 03 06 04 ...` land in `$FFFF4150` - battle scratch the round's tail reuses. `03`/`06`/`04` are not bytes `loc_B6A2` writes. | fixed (§4.2 H3): the flags are read on the action's own pass frame. The port's one-slot swing was right; the "window" never existed. |
| 3. an ability that spends the turn | 10 | **split** | `$11` carries (5): `formation_38` f25019 writes the ability byte, presets the flags and resolves **FighterId(1)** - the pass's own coverage - then rolls the poison's chance once at f25082; `alys_status` never moves, so the arm ran and missed, and the port's `EnemySkillUsed` was right. `$45` carries (4): `formation_2A` f26539 rolls index 1, whose regular entry is `$40`, yet `e1_ability` reads **`$45`** and `e1_hp` rises 38 -> 75 - enemy 99's *conditional* ability, written by its AI instruction. `$02` (1): `formation_4B` f25680 is a window on a **dead** actor (see cluster 6). | harness for the `$11`/`$02` findings (fixed, §4.2 H4/H2); port for the `$45` four (§4.4 W2: the AI instruction block, `ps4.asm:19157-19168` and `21320-21338`). |
| 4. the round's queue | 7 | **harness artifact** | `formation_13` f24821: `enemy_count` reads 2 and **one** SandNewt record is written; f24822 holds both. The fixture started at f24821, so it seated one enemy where the cartridge fought two, and the port's queue (and every later round) was a different battle. The log's own queue at f25013 is `[1, 6, 7, 2, 3]` - five entries, not the port's four. | fixed (§4.2 H1): the start frame is the first frame whose intact records are all of `enemy_count`. The port's queue was right for the formation it was given. |
| 5. a verdict or a damage value | 5 | **split** | `formation_37` f25397 (2): Alys had just hit enemy 6 with verdict `$00`; Hahn's pass writes the same byte for the same slot, so *nothing changes*, and the old change-based target list lost the hit and kept a phantom `$FF` from the preset. The log's damage word was already 1, so the rewrite is invisible too. `formation_07` f25542 (1) and `formation_08` (round 1, 120 vs 119 rolls): the round draws **one** roll the port does not - the retarget scan's tiebreak, with the two slots' deficits tied at 13. `formation_3B` f25003 / `formation_4F` f25240 (2): a critical's damage, 248 vs 312 and 71 vs 103. | harness for `formation_36`/`37` (fixed, §4.2 H3/H5); port for `formation_07`/`08` (§4.4 W4) and for the criticals (§4.4 W3). |
| 6. a turn that never swings | 4 | **harness artifact** | `formation_4C` f26205, `formation_4E` f25586, `formation_55` f25250, `formation_56` f25496 (`formation_4B` f25680 is the same): the window holds **no call at all**, and the actor the field names is **dead** - `formation_55` f25250's slot reads HP -45 (`e1_hp` 65491) with status `$04` (`StatusDead`), and the round killed it before its turn came. `loc_576A` writes the queue entry into `$FFFF4142` and only then tests the fighter's status (`ps4.asm:8033-8043`), so the field keeps a skipped actor's id until the next queue build. | fixed (§4.2 H2): a window opens only where the action's own calls are. The port had no turn there because there was none. |
| 7. a target the port leaves alone | 2 | **harness artifact** | `formation_3F` f25035: the pass resolves **FighterId(3)**, the poison's own roll lands at f25099, and `hahn_status` goes 0 -> 1 in the same frame - the cartridge poisoned Hahn, and so did the port (`StatusInflicted`). The old finding came from the comparator, which demanded a `Resolved` swing event per resolved slot: a status arm resolves one and reports the *status*. | fixed (§4.2 H5): the ability path walks the slots the log shows **damage** on, and checks the log's status movements against the port's own status events - a weaker claim than "a swing resolved this slot", and one the log can support. |
| 8. a round's roll count | 1 | **port rule** | `formation_08` round 1: the log's frames hold 120 calls, the port draws 119. The missing one is at f25374 (Hahn's turn): two calls where his single-target swing rolls once - the retarget scan's tiebreak (deficits tied), which the port never makes because it never scans. | port: the same scan as cluster 1 (§4.4 W1/W4). |

### 4.2 Harness artifacts, and what they read now

Every fix below is in a committed file with its own test, and the affected
fixtures were re-extracted from the sweep's preserved captures
(`python3 oracle/sweep.py --reextract build/lane-evidence/sweep`; §5). The
captures themselves were not re-taken - nothing here is about the emulator.

**H1. The start state was a half-written formation.**
`oracle/fixture/observations.py`'s `enemies_loaded` took the first frame an
enemy record was intact. The load writes one slot per frame, so for 7 captures
that frame held only the first enemy: `formation_13` f24821 (`enemy_count` = 2,
one record) against f24822 (both). The start frame is now the first frame whose
occupied slots are all of `enemy_count`, and a log where the two never agree is
refused with the counts it saw. Tests:
`test_oracle_battle_fixture.Segmentation`. Clusters 4 and the queue half of
nothing else; 7 fixtures.

**H2. A window could open on a turn nothing drew.**
`action_windows` opened a window whenever the actor field named a fighter - but
`loc_576A` writes the queue entry *before* it tests the fighter's status
(`ps4.asm:8033-8043`), so a fighter that fell before its turn arrived leaves its
id in the field for the rest of the round with no call, no flag write and no
damage behind it. A window now opens only on a frame the trace drew in. Tests:
`test_oracle_battle_fixture.Segmentation`,
`test_oracle_battle_fixture_forced.ActionWindows`. Clusters 6 (4 fixtures) and
`formation_4B`.

**H3. The hit flags were read by change, at the wrong frame.**
`Fighters_Hit_Flags` is battle scratch: `loc_B6A2` presets all nine bytes to
`$FF` and then writes the acting window (`ps4.asm:17493-17498`, `17528`), and
the round's tail reuses the same addresses when it runs out of actors
(`formation_10` f25808). The extractor now reads the nine bytes on the action's
own **pass frame** (`observations.pass_frame`): the last frame that drew and
whose nine bytes are all verdicts, falling back to the action's first roll
frame. Reading *state* rather than *change* is what keeps a verdict that repeats
the previous action's (`formation_37`), and the plausibility filter is what
keeps the scratch out (`03`, `06`, `04` are not verdicts). Tests:
`test_oracle_battle_fixture.Observations`,
`test_oracle_battle_fixture_forced.ActionEffects`. Cluster 2 (13 fixtures),
`formation_36`/`37`, and the coverage half of cluster 7.

**H4. An ability's effect was only looked for on the opposing side.**
`enemies.kind_of` read the ability's id and what moved on the *other* side, so
an arm that touched its own side - `formation_2A`'s TechUser casts RES on
itself, HP 38 -> 75 - or set a status byte looked like a spent turn. The
extractor now records what the action moved on **every** seated fighter
(`observations.action_effects`: HP, the status byte with the two death bits
masked out, and the battle stat cells), and calls the turn an ability when
anything moved. The comparator in turn accepts either arm for a `wasted`
reading, because the log genuinely cannot tell one whose arm ran and found
nothing to do from one whose arm does not exist - and still refuses damage the
log does not show. Tests:
`test_oracle_battle_fixture_forced.ActionEffects`,
`test_oracle_battle_fixture_forced.EnemyAbilities`. Cluster 3's `$11` findings
(5 fixtures).

**H5. The comparator asked for a swing a status arm does not make.**
An ability's pass resolves every slot in its *range*, whether or not the effect
reaches them, so using the pass's coverage as "the slots the port must have
resolved" demanded a `Resolved` event that a status arm never emits. The
ability path now walks the slots the log shows **damage** on, and checks the
log's status movements (`action.effect.status`) against the port's own
`StatusInflicted` - a new `status` divergence, whose negative control is
recorded in this lane's evidence (flipping `formation_3F`'s recorded status to
another fighter makes the test fail at f25035 naming `FighterId(1) at status
01`). A hit whose damage word did not move is no longer compared as `None`
either: the word was rewritten to the value it already held
(`formation_37`), and the target's `hp_after` is what pins the number. Tests:
`test_oracle_battle_fixture_forced.ActionEffects`; the comparator itself is
exercised by the data-driven test. Clusters 7 (2 fixtures) and `formation_36`.

**H6. A fixture named directories.**
The fixture recorded `--tape` as it was given - a sweep's own working path - and
stored the capture's header lines verbatim, `# rom=` included, so the same
capture swept into another directory produced different bytes and a different
`log_sha256`. The host now names **every** input by its file alone
(`oracle/host/psiv_oracle.c`, `provenance.h`), and the extractor records the
tape's name and stores the header with its path fields cut
(`oracle/fixture/logs.py`'s `provenance_lines`). Tests:
`tests/test_oracle_provenance.py` extracts one capture twice, into two
directories and with two tape spellings, and insists the bytes are equal;
`tests/test_oracle_force_battle_provenance.py` pins the host's two lines.

### 4.3 The command cells a retarget needs (a capture-side gap)

Cluster 1's rule cannot be *checked* without one more observation: the fighter
the command phase sent the member after. `Character_Command_Data` holds it
(`constants:2005-2011`, `cmd0_target`..`cmd4_target` in `oracle/ram_map.json`),
the turn engine copies it into `Current_Target_Index` (`ps4.asm:8051-8053`) and
then moves it (`loc_5AE6`), and the sweep's captures do not carry the group:

* `bcmd` is now in the group list every capture logs
  (`oracle/force/runs.py`'s `GROUPS`), and `current_target` /
  `current_command` (`$FFFF4144`/`$FFFF4146`, `constants:2017-2018`) are new
  cells in the map, so a capture taken after this lane can say what the swing
  was *aimed at* as well as what it hit;
* the extractor records it: `assembly.command_entry` writes
  `{"id": 2, "command": "attack", "target": 6}`, and `-1` where the command was
  whole-side (`ps4.asm:8464`), for every capture that carries the cells.

The sweep's 81 fixtures cannot show it - their captures were taken before the
group existed - so a lane that fixes the retarget rule should re-capture the ten
affected formations (`python3 oracle/sweep.py --only 0x02,...`) or take the
test's evidence from `tests/test_oracle_battle_fixture.Commands`.

A capture taken with the group, of cluster 1's own anchor fixtures, is what the
rule looks like in the cells: `python3 oracle/sweep.py --only 0x02` (this lane
ran it) writes a `formation_02` capture whose `current_target` reads `0006` on
Chaz's turn at f25665 - his own `cmd1_target`, so no retarget - and **`0008`**
on Hahn's at f25669 while `cmd2_target` reads `0006`: the enemy slots read
-12/9/8/9 of 20, the deficits 11/**12**/11, and the scan has moved the swing to
slot 8. Later in the same capture, Alys's `current_target` reads `$FFFF` (-1) on
her round-2 turn at f25950: the whole-side window the Boomerang's command opens
(`ps4.asm:8464`, and the reason her swings cover four slots).

### 4.4 The port rules that remain (the worklist)

Four causes, 18 findings, each one entry per fixture in
`rust/psiv-core/src/battle/replay_fixtures/divergences.json` under the heading
below. Every one is a *rule*: the evidence is the log's, the cartridge's routine
is cited, and the port's own code is where the fix goes.

**W1. A swing whose commanded enemy has fallen lands on another slot**
(`party-retarget`, 10 fixtures: `formation_02`, `04`, `05`, `0B`, `0C`, `17`,
`18`, `19`, `1B`, `1D`).

The cartridge re-aims the swing at the enemy with the **largest**
`max_hp - curr_hp`, scanning slots 6-9, tie broken by one RNG draw
(`loc_5A98` -> `loc_5AE6` -> `loc_5B88`, `ps4.asm:8331-8410`); a member whose
commanded enemy is alive keeps it (the same routine returns early,
`ps4.asm:8356-8361`). `formation_02` f25669 is the shape: the deficits are
11/12/11 for slots 7/8/9, the log's flag write is on slot 8, and the port's
`candidate_targets` (`action.rs`) falls back to the lowest living id, 7.

Fix scope: implement the scan in `candidate_targets`/`resolve_attack`'s caller,
drawing the tiebreak through the same `Rolls` stream the cartridge does, and
read the commanded target from the fixture's `commands` (§4.3). Closing the
entries is the acceptance.

**W2. The enemy's AI instruction, not the ability roll, picks the ability**
(`enemy-ai-conditional`, 4 fixtures: `formation_2A`, `2B`, `32`, `34`).

`Enemy_Attack` rolls a regular ability and writes it
(`move.b $58(a3,d0.w), ability+1(a4)`, `ps4.asm:19153`), and then runs the
enemy's own AI instruction bytes (`ps4.asm:19157-19168`): a nonzero byte at
`$50(a3)` selects an `EnemyAIInstructionsOffs` arm (`ps4.asm:19364`), and the
arm may overwrite the ability with the **conditional** one
(`move.b $3(a0), ability+1(a4)`). Enemy 99 TechUser's bytes are `15` (`$0F` =
`EnemyAI_HalfHPOrLower_AllEnemies`, `ps4.asm:21320-21338`), and its conditional
ability is `$45` RES (`generated/enemies.json`'s `ai`): when the actor is at or
below half HP - `formation_2A` f26539: 38 of 80 - the arm writes RES and the
turn heals the caster. The port rolls among the *regular* abilities only
(`engine.roll_enemy_ability` -> `ai::choose_ability`), so it runs `$40` WAT
instead, on a slot whose log shows a heal.

Fix scope: model the instruction block (the condition ids at `$50(a3)`, the
dispatch table, and the arms the sweep's carriers need) and the RES arm's own
effect; `ai.rs` is where the ability choice lives, `enemy_skill.rs` where an
arm's effect goes.

**W3. A critical reads the attack power's low byte, not the word**
(`critical-bonus`, 2 fixtures: `formation_3B`, `4F`).

`Enemy_DamageCharacter` (`ps4.asm:3789-3790`) - and the party's own path
(`ps4.asm:3970-3971`) - computes the bonus as `move.b d1, d4 / lsr.w #2, d4`:
the attack power's **low byte**, quartered. `formation_3B` f25003: attack 279,
defence 18, element 2, the 16 damage draws summing to 48 - the cartridge's 248
is exactly `bonus = (279 & $FF) >> 2 = 5`, and the port's 312 is
`bonus = 279 >> 2 = 69`. `formation_4F` f25240 is the same 32-point gap with
the Land Rover's defence 80 and element 1. The port's
`action::critical_bonus(attack) = attack >> 2`.

Fix scope: `>> 2` on the low byte, in `critical_bonus` and wherever the same
bonus is computed for party swings.

**W4. The retarget scan's tiebreak draw is missing from the round**
(`retarget-tiebreak`, 2 fixtures: `formation_07`, `08`).

The scan draws `UpdateRNGSeed2` once whenever two slots' HP deficits are equal
(`ps4.asm:8364-8374`, `btst #0, d1`). `formation_08`'s round 1 draws 120 calls
to the port's 119, and the missing one is where Hahn's single-target swing rolls
twice (f25374); `formation_07` f25542 draws it in the same place, so the port's
hit roll is the *tiebreak* value and its damage comes out 8 where the log has 9.
It is the same rule as W1, seen from the RNG stream's side: fixing W1 without
the draw would leave the counts wrong.

## 5. Every re-extracted fixture

All 81 captured formations were re-extracted from the sweep's own preserved
captures after the harness fixes above
(`python3 oracle/sweep.py --reextract build/lane-evidence/sweep`, and the
fixture's `log_sha256` still matches the hash the sweep's record pins for each
capture). The first divergence each one has now, against the one it had in the
sweep's own manifest:

* **34 are now exact** - every cluster 2, 4, 6 and 7 fixture, plus the three
  `value`/`not-wasted` ones the reading itself had produced.
* **4 moved** - the `$45` fixtures, whose finding is now the ability the
  cartridge ran rather than "an effect where the log shows a spent turn".
* **14 are the same finding at the same frame** - the ten retarget fixtures, the
  two criticals, and the two tiebreak ones. Their entries' own text changed
  (the action now reports the slots the pass really resolved), which is why the
  manifest was regenerated rather than edited.

| fixture | first divergence before | after | |
|---|---|---|---|
| `formation_2A` | `not-wasted` at f26539 | `ability` at f26539 | moved |
| `formation_2B` | `not-wasted` at f27707 | `ability` at f27707 | moved |
| `formation_32` | `not-wasted` at f27115 | `ability` at f27115 | moved |
| `formation_34` | `not-wasted` at f27629 | `ability` at f27629 | moved |
| `formation_02` | `targets` at f25669 | `targets` at f25669 | same |
| `formation_04` | `targets` at f26055 | `targets` at f26055 | same |
| `formation_05` | `targets` at f26151 | `targets` at f26151 | same |
| `formation_07` | `value` at f25542 | `value` at f25542 | same |
| `formation_08` | `draws` at f25061 | `draws` at f25061 | same |
| `formation_0B` | `targets` at f25661 | `targets` at f25661 | same |
| `formation_0C` | `targets` at f25512 | `targets` at f25512 | same |
| `formation_17` | `targets` at f26168 | `targets` at f26168 | same |
| `formation_18` | `targets` at f26280 | `targets` at f26280 | same |
| `formation_19` | `targets` at f26223 | `targets` at f26223 | same |
| `formation_1B` | `targets` at f26964 | `targets` at f26964 | same |
| `formation_1D` | `targets` at f26667 | `targets` at f26667 | same |
| `formation_3B` | `value` at f25003 | `value` at f25003 | same |
| `formation_4F` | `value` at f25240 | `value` at f25240 | same |
| `formation_10` | `targets` at f25693 | **now exact** | now exact |
| `formation_13` | `queue` at f25013 | **now exact** | now exact |
| `formation_14` | `queue` at f25013 | **now exact** | now exact |
| `formation_15` | `queue` at f25013 | **now exact** | now exact |
| `formation_16` | `targets` at f25488 | **now exact** | now exact |
| `formation_22` | `targets` at f26271 | **now exact** | now exact |
| `formation_23` | `targets` at f26516 | **now exact** | now exact |
| `formation_25` | `queue` at f25013 | **now exact** | now exact |
| `formation_26` | `queue` at f25013 | **now exact** | now exact |
| `formation_2D` | `targets` at f25766 | **now exact** | now exact |
| `formation_2E` | `targets` at f25927 | **now exact** | now exact |
| `formation_36` | `value` at f26009 | **now exact** | now exact |
| `formation_37` | `value` at f25397 | **now exact** | now exact |
| `formation_38` | `not-wasted` at f25019 | **now exact** | now exact |
| `formation_39` | `not-wasted` at f25153 | **now exact** | now exact |
| `formation_3A` | `not-wasted` at f24887 | **now exact** | now exact |
| `formation_3C` | `targets` at f25421 | **now exact** | now exact |
| `formation_3E` | `queue` at f24882 | **now exact** | now exact |
| `formation_3F` | `no-resolution` at f25035 | **now exact** | now exact |
| `formation_40` | `no-resolution` at f25835 | **now exact** | now exact |
| `formation_41` | `not-wasted` at f24902 | **now exact** | now exact |
| `formation_42` | `not-wasted` at f25051 | **now exact** | now exact |
| `formation_43` | `targets` at f27819 | **now exact** | now exact |
| `formation_44` | `targets` at f27201 | **now exact** | now exact |
| `formation_45` | `targets` at f25927 | **now exact** | now exact |
| `formation_46` | `targets` at f26189 | **now exact** | now exact |
| `formation_47` | `queue` at f25013 | **now exact** | now exact |
| `formation_48` | `targets` at f26564 | **now exact** | now exact |
| `formation_49` | `targets` at f26931 | **now exact** | now exact |
| `formation_4B` | `not-wasted` at f25680 | **now exact** | now exact |
| `formation_4C` | `no-swing` at f26205 | **now exact** | now exact |
| `formation_4E` | `no-swing` at f25586 | **now exact** | now exact |
| `formation_55` | `no-swing` at f25250 | **now exact** | now exact |
| `formation_56` | `no-swing` at f25496 | **now exact** | now exact |
| `formation_00` | **now exact** | **now exact** | was exact |
| `formation_01` | **now exact** | **now exact** | was exact |
| `formation_03` | **now exact** | **now exact** | was exact |
| `formation_06` | **now exact** | **now exact** | was exact |
| `formation_09` | **now exact** | **now exact** | was exact |
| `formation_0A` | **now exact** | **now exact** | was exact |
| `formation_0D` | **now exact** | **now exact** | was exact |
| `formation_0E` | **now exact** | **now exact** | was exact |
| `formation_0F` | **now exact** | **now exact** | was exact |
| `formation_11` | **now exact** | **now exact** | was exact |
| `formation_12` | **now exact** | **now exact** | was exact |
| `formation_1A` | **now exact** | **now exact** | was exact |
| `formation_1C` | **now exact** | **now exact** | was exact |
| `formation_1E` | **now exact** | **now exact** | was exact |
| `formation_1F` | **now exact** | **now exact** | was exact |
| `formation_20` | **now exact** | **now exact** | was exact |
| `formation_21` | **now exact** | **now exact** | was exact |
| `formation_24` | **now exact** | **now exact** | was exact |
| `formation_3D` | **now exact** | **now exact** | was exact |
| `formation_4A` | **now exact** | **now exact** | was exact |
| `formation_4D` | **now exact** | **now exact** | was exact |
| `formation_50` | **now exact** | **now exact** | was exact |
| `formation_51` | **now exact** | **now exact** | was exact |
| `formation_52` | **now exact** | **now exact** | was exact |
| `formation_53` | **now exact** | **now exact** | was exact |
| `formation_54` | **now exact** | **now exact** | was exact |
| `formation_57` | **now exact** | **now exact** | was exact |
| `formation_58` | **now exact** | **now exact** | was exact |
| `formation_59` | **now exact** | **now exact** | was exact |
