# Forcing a formation: capturing any battle on demand

What this ledger records: how `oracle/force_battle.py` makes a *chosen*
formation reachable in the battle oracle, the four captures it produced (three
for the enemy abilities `psiv-core` now implements, one for a second vehicle
record), and what those captures do and do not prove. The replay side of the
same picture is [`BATTLE_ORACLE_REPLAY.md`](BATTLE_ORACLE_REPLAY.md); the
capture tooling is [`oracle/README.md`](../../oracle/README.md).

The oracle could already replay a captured battle exactly, but only battles an
existing tape happens to reach: tape 07 meets two Zoran Bults because the
cartridge rolled them. A formation an implemented ability belongs to (a Helex,
a Fanbite, a Desrt Leach) is not something a tape can walk to on request. This
tool forces it.

## 1. Where the cartridge picks a formation

`RunRandomBattles` (`ps4.asm:116840`) sets `Game_Mode_Routine` to `$14`
(`FieldRoutine_Battle`) when `RNG_Seed & $1F` is zero on a completed step.
`FieldRoutine_Battle` (`ps4.asm:117926`) fades the screen for fifteen frames
and hands over to the battle mode, whose load reaches `Battle_SetupEnemyData`
(`ps4.asm:11813`) a few frames later. That routine makes the whole choice in
one pass, inside a single frame - so there is **no frame boundary between the
choice and the enemy build**, and nothing that holds the chosen formation id in
RAM for a patch to land on:

```
    move.b  (a0,d1.w), d0      ; a0 = Battle_EnemyFormationIndexes, d1 = Field_Map_Index
    cmpi.w  #1, d0
    bgt.s   loc_7E8A           ; byte > 1: d0 IS the group index
    tst.w   (Vehicle_Index).w
    beq.s   loc_7E4C           ; on foot: the position grid's cell for the chunk
    ...                        ; vehicle: group 13 (Dezolis) / 9, $A, 8 (Motavia)
loc_7E8A:
    lea     (Battle_FormationIndexes).l, a0     ; 68 groups x 32 formation ids
    jsr     (KosDecomp).l                       ; the group table -> RAM_Start
    lsl.w   #6, d0 / adda.w d0, a0              ; this group's 64 bytes
    jsr     (UpdateRNGSeed2).l                  ; THE ROLL
    andi.w  #$1F, d0 / add.w d0, d0
    move.w  (a0,d0.w), d0                       ; formation id = table[roll & 31]
    ; id < $80 -> Battle_FormationData1, < $100 -> block 2, ... ($80 steps)
```

Two inputs, then, and the tool forces both:

| input | where it comes from | how it is forced |
|---|---|---|
| the **group** | the map's `Battle_EnemyFormationIndexes` byte, the position grid's cell, or a vehicle table | `--ram-patch` of those RAM cells one frame after the encounter fires |
| the **entry** (0-31) | `UpdateRNGSeed2`'s roll, masked to five bits | `--ram-patch` of `RNG_Seed`'s high word, one frame before the draw |

### 1.1 The group

`generated/encounters.json` says which groups a map can draw
(`Battle_EnemyFormationIndexes`) and which group each position-grid cell holds
(`Battle_MotaFormationGroupIndexes`); `generated/formation_indexes.json` is the
68 x 32 table of formation ids per group. The tool picks the least invasive
selector that reaches a group holding the wanted formation:

| selector | cells written (one frame after the encounter) | notes |
|---|---|---|
| `map` | `Field_Map_Index` (`$FFFFEC28`) | only for the groups some map's byte carries; one patch |
| `grid` | `Field_Map_Index` = the world map, `Character_1`'s `curr_x_pos`/`curr_y_pos` (`$FFFFC030`/`$FFFFC034`) = a cell whose byte is the group | the chunk `(y>>6)*64 + (x>>6)` selects the group (`ps4.asm:11850`) |
| `vehicle` | `Field_Map_Index` = the table's own region, `Vehicle_Index` (`$FFFFF43C`) = the `--vehicle` record (the region's default without it), `Mota_Battle_BG_Index` (`$FFFFECEF`) | groups 8/9/10/13 exist only here; `--vehicle` is section 5 |

The selector cells are written back from the scout run's own values one frame
after the formation is drawn, so only the load itself runs on fixture RAM. The
**vehicle index is the deliberate exception**: `loc_78EE` (`ps4.asm:11408`)
builds the party-side fighter from `Vehicle_Stats` and the battle's UI reads
`Vehicle_Index` on every frame, so it stays set for the whole run - and a battle
forced through a vehicle table is a vehicle battle, the vehicle fighting alone.

### 1.2 The entry

`UpdateRNGSeed2` (`ps4.asm:86097`) is

```
    move.w  $8(a5), d0             ; the VDP HV counter
    add.w   (Main_Frame_Count).w, d0
    sub.w   (RNG_Seed).w, d0       ; d0 = the roll:  (hv + frame_count - seed_high)
    ror     (RNG_Seed).w
```

so with `K = (hv + Main_Frame_Count) & 31` the entry the draw lands on is
`(K - seed_high) & 31`, and patching `RNG_Seed`'s high word (`$FFFFEF0C`,
two bytes) to `K - entry` puts it on any of the group's 32 entries. The tool
measures `K` from a **probe run**: the draw is the first `UpdateRNGSeed2` call
of its frame (the load calls nothing else), and the run's `--rng-trace` carries
`hv` and `frame_count` for it.

Three details the tool insists on, because each was found the hard way:

1. **The patch lands one frame *before* the draw**, not on it.
   `oracle/rng_trace.py check` requires a frame's first call to start from the
   seed the log holds for the frame before, so a patch applied at the draw's own
   frame fails the check that proves the trace is the cartridge's arithmetic.
   Patching one frame earlier is sound only because nothing advances the seed
   during the load - the VBlank's `UpdateRNGSeed` and every `UpdateRNGSeed2` are
   quiet, which is also why `Main_Frame_Count` stands still there. The tool
   verifies that from the probe's own log (the seed at `f(draw-1)` must be the
   seed the trace's roll started from, and `f(draw-1)` must hold no call of its
   own) before it writes anything.
2. **The probe must not outlive the fight.** The probe only needs the draw, so
   it stops at 200 policy blocks. An untrimmed run that outlives a defeat walks
   into the game-over sequence, where a new game re-initializes
   `Main_Frame_Count` and the host rightly refuses to close the trace.
3. **The probe's model check is part of the run.** The formation the probe's
   own log shows must be exactly the group table's entry the roll named
   (enemy ids *and* HP per slot); if not, the tool stops rather than patching a
   seed against a model that does not explain what happened.

### 1.3 The policy

`attack` (the default) is tape 07's own fight input: one `C` press every 16
frames (4 held, 12 released). It takes COMD -> ATTACK -> the default target for
each member and then advances messages and rounds, and it is what both checked-in
battle tapes use, so "no menu drift" is inherited from them rather than assumed.
`defend` is tape 14's navigation - the per-character command menu is horizontal,
so four Right presses step ATTACK -> TECHNIQUE -> SKILL -> ITEM -> DEFEND.
Both are fixed frame patterns, so the inputs are frame-deterministic.

`defend` was run end to end once, as the acceptable fallback for a battle the
attack policy cannot finish: formation `$5E` again, `--repeats 60`
(`build/lane-evidence/f94_defend/`, trace `7c4798e3…`, log `5a728739…`). The
battle runs to f25760 - longer than the attack policy's f25616, as defending
should be - and the defender's `physical_prop` goes `514 -> 258` for the
surviving member at f25439, which is the same retail DEFEND clobber
`verify.sh`'s full lane measures for tape 14. FLAME BOLT still fires
(`e2=0x02` at f25269), `rng_trace.py check` passes and the two runs are
byte-identical.

`--delay N` inserts N idle frames between the field prefix and the policy. The
encounter has already fired, so the formation stays forced, while every roll in
the fight shifts (`Main_Frame_Count` advances and the seed evolves frame by
frame): the knob for a battle whose enemy never rolls the ability a capture is
for. The probe and the seed patch are re-derived for each delay, because the
draw's frame moves with it. Formation `$5E` with `--delay 5`
(`build/lane-evidence/f94_d5/`) is the measurement: still two Helex, but the
draw's `hv` is `$6DFF` rather than `$7195`, so `K` is 26 rather than 16 and the
seed patch is `$0006` rather than `$001C`; the fight's own rolls differ (trace
`da65b77f…`, log `e7fdf0ca…`, the two FLAME BOLTs at f25008/f25130 instead of
f25003/f25125) and the battle ends at f25621 instead of f25616. None of the
three captures needed it - every ability fired on the first delay tried - but
the knob is measured rather than assumed.

## 2. The four captures

Each capture is a full run of `oracle/force_battle.py`: scout, probe, preview,
capture, verify. All four start from tape 07's own field prefix - frames
1..24794, i.e. everything up to and including the frame its encounter fires on -
then the attack policy; each was checked with
`python3 oracle/rng_trace.py check` (passed) and the capture was run twice with
the same tape path and output basenames, byte-identical both times (traces and
logs). Three of them force a formation under the Land Rover; the fourth is the
same formation as the third, forced with `--vehicle 2` so the battle loads the
**Ice Digger's** record instead (section 5).

| | formation `$5E` | formation `$37` | formation `$53` |
|---|---|---|---|
| capture | `build/lane-evidence/f94` | `build/lane-evidence/f37` | `build/lane-evidence/f53` |
| formation | 2 Helex (id 0, hp 90) | 2 Fanbite (id 15, hp 261) | 1 Desrt Leach (id 81, hp 1040) |
| group | 44 | 4 | 8 |
| selector | `map 351` (Hangar), one cell | `grid`: world map 0, cell (36,4) | `vehicle`: table 8 |
| entries in the group | 20-31 | 2, 3 | 24-31 |
| forced entry | 20 | 2 | 24 |
| probe: draw frame | f24820 | f24833 | f24831 |
| probe: roll, entry drawn | `$606B` & 31 = 11 -> formation 284 (one Gerot Lux) | `$E256` & 31 = 22 -> formation 64 (Fanbite + 2 Caterpillr) | `$1C44` & 31 = 4 -> formation 76 (4 ForcedFly) |
| probe: `hv`, `frame_count`, `K` | `$7195`, 23931, 16 | `$F380`, 23931, 27 | `$2D6E`, 23931, 9 |
| seed patch (`f(draw-1)`) | `$001C` at f24819 | `$0019` at f24832 | `$0011` at f24830 |
| battle window | f24794-25616 | f24794-25166 | f24794-26872 |
| outcome | defeat (Alys, Chaz, Hahn all at 0 HP) | defeat (all three at 0 HP) | **victory**: the Desrt Leach at 0 HP, the vehicle at 378 HP, +1500 exp, +1 meseta |
| ability id used | `$02` FLAME BOLT - enemy 2 at f25003, enemy 1 at f25125 | `$08` SPIRAL BLD - enemy 2 at f24903 | `$37` SAND STORM - enemy 1 at f25839 |
| trace sha256 | `b1e96472d9282c9172fea59f6e93d92209189047f90887e9049ad861fac86abe` | `e74f0d6645295ec4ec1296ba2b6687262ba547595c96db70d5b325cbdcc09257` | `aa133d61288c886b34af14572a4ef9080fc59e47bd20bbce02a011daf06a214b` |
| log sha256 | `b4ed383d91ea7ea7895ffbf559fd766be3d1317a80a9619b2be379442f3dd349` | `86c6e5e7e41a324d2c4a68a720c78e4dc9de9f1609ee46331a521172655b1a30` | `33f19744e8fd485c625a81b302c8a979af567463ce46c731a028b98afe13a0c3` |

"the captured battle is that formation" is the tool's own check, not a reading
of the report: the ids *and* the HP per enemy slot must equal
`generated/formations.json`'s record for the forced id. The ids alone are not
enough - Helex's enemy id is `0`, which is also what an empty slot reads - so
the HP columns disambiguate the two.

The abilities come from `eN_ability`, added to `oracle/ram_map.json` for this
work: `Enemy_Attack` writes the id it rolled into `ability+1` of the acting
fighter object (`move.b $58(a3,d0.w), ability+1(a4)`, `ps4.asm:19153`), and
enemy slot N is `Obj_Fighters + (N+4)*$40` (`Fighter_Enemy_1` = `$FFFF4540`,
`ps4.asm:315652`), so `$FFFF4565`/`$45A5`/`$45E5`/`$4625` are the four ids as
they are being executed. `$00` means no ability was written and the enemy made
its basic attack, which is what most of a Fanbite's or Desrt Leach's actions
are: Fanbite's eight regular slots are six zeros and two `$08`s, the Desrt
Leach's five zeros and three `$37`s, and only Helex's eight are all `$02`. A
capture that never rolls the interesting slot is a capture of basic attacks -
hence `--delay`, and hence `--require-ability`, which turns "the ability fired"
into the run's own exit status.

## 3. Reproducing one of them

From a checkout with the ROM linked in and the core built
(`oracle/build_core.sh`), and working from a lane whose `psiv_oracle` was built
by `oracle/verify.sh`:

```sh
python3 oracle/force_battle.py --formation 0x5E \
    --out build/forced/helex --require-ability 2
python3 oracle/force_battle.py --formation 0x37 \
    --out build/forced/fanbite --require-ability 8
python3 oracle/force_battle.py --formation 0x53 \
    --out build/forced/desrtleach --require-ability 0x37
python3 oracle/force_battle.py --formation 0x53 --vehicle 2 \
    --out build/forced/icedigger
```

Each writes, in its output directory: `scout.json` (the base tape's own run,
cached and reused), `<stem>.full.tape` and `<stem>.tape` (the composed tape,
before and after the trim), `<stem>.patches.txt` (the `--ram-patch` list),
`probe/`, `preview/`, `capture/` and `verify/` (each a `psiv_oracle` run's log
and `--rng-trace`), and `report.json` - the battle window, the outcome, the
enemies and party at its end, the ability ids observed, the patches, the vehicle
it was told to force (`--vehicle`), and every sha256. The stem names the vehicle
too when one was asked for (`forced_53_attack_v2.tape`), so two captures of one
formation cannot overwrite each other. The tool prints the same summary and
exits non-zero if any of its checks fails: the probe's formation does not match
the group table, a required ability never fired, two runs of the capture differ,
an unreachable vehicle/group pair was asked for, or `rng_trace.py check` fails.

A single capture is ~90 seconds of wall clock (five oracle runs over ~40k frames
of tape). The scout is cached in the output directory, so a re-run of the same
capture skips it, and `--scout` can point several captures at one cache.

The logs are pinned by sha256 above, and two measurements back the pins. First,
the traces are byte-identical to the ones the capture tool's own lane recorded
in a *different* worktree; the logs differ from those earlier pins only in the
`# tape=` line (a basename now) and in the seven columns the `vehicle` group
adds, which is what re-running the Helex capture's tape and patches with the
pre-change group set shows: 25,676 rows, every shared column identical. (Later
on 2026-09-24 the group grew by five more - the Ice Digger's saved record, section
5 - so a re-run against *those* pins now also carries
`vehicle_ice_hp`/`_max_hp`/`_skill_mask`/`_skill1_current`/`_skill1_max`, which
is a change to the log's column set and not to any pinned capture's log.) Second,
the tape's *directory* is not part of the capture: with the tape copied to
another directory and replayed from there, with the same patches, groups and
ROM spelling, the log comes back byte-identical to the capture's
(`b4ed383d91ea7ea7895ffbf559fd766be3d1317a80a9619b2be379442f3dd349`) and so does
the trace (`build/lane-evidence/group_compare/`).

**The ROM's spelling is part of the log, and only the log.** `# rom=` records
the path the host was given, so the log's sha256 is worktree-specific while the
trace's is not. Measured on the `$53` capture re-run in a later lane
(`build/lane-evidence/desrtleach_recapture/`): the trace came back byte-identical
to its pin (`aa133d61…`), the log differed in that one line
(`<lane>/Phantasy Star IV (USA).md` rather than the pinning lane's), and
substituting the pinning lane's ROM path into the re-run's log reproduces
`33f19744…` exactly. The tool's own two-run byte comparison is unaffected - both
runs share the path - and `oracle/rng_trace.py check` reads the log's rows, not
its header.

## 3a. The captures' replay verdicts

Each capture is extracted into a fixture
(`rust/psiv-core/src/battle/replay_fixtures/forced_*.json`) and replayed by the
one data-driven test of [`BATTLE_ORACLE_REPLAY.md`](BATTLE_ORACLE_REPLAY.md),
which compares every action and every round's draw count against the log and
holds a fixture that does not match to its entry in
`replay_fixtures/divergences.json`:

```sh
python3 oracle/battle_fixture.py --trace build/forced/helex/capture/forced_5E_attack_rolls.csv \
    --log build/forced/helex/capture/forced_5E_attack.csv \
    --tape "forced_5E_attack.tape (oracle/force_battle.py --formation 0x5E)" \
    --battle-first 24794 --battle-last 25616 \
    --out rust/psiv-core/src/battle/replay_fixtures/forced_5e_helex.json
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- --test-threads=1 every_fixture_replays_as_recorded
```

| capture | fixture | verdict |
|---|---|---|
| `$5E` two Helex | `forced_5e_helex.json` | **exact**: two rounds, 80 rolls, both FLAME BOLTs (`$02` at f25003 and f25125, and the re-rolled one at f25371) resolve damage for damage, and the party's defeat is the log's |
| `$37` two Fanbite | `forced_37_fanbite.json` | **exact**: SPIRAL BLD (`$08` at f24903) takes all three party slots for the damage the log shows, and the wipe is the log's |
| `$53` one Desrt Leach, Land Rover | `forced_53_desrtleach.json` | **exact** (2026-09-24): six rounds, 285 rolls, every vehicle swing's three hit passes and sixteen damage draws, the Desrt Leach's death at f26459, and the log's 1500 exp and 1 meseta |
| `$53` one Desrt Leach, Ice Digger | `forced_53_icedigger.json` | **exact** (later on 2026-09-24): five rounds, 245 battle rolls, every swing's **two** hit passes and sixteen damage draws, and the Desrt Leach's death at f26253 — the one-pass-too-many this capture first recorded is fixed by `hit_passes` (section 5.1) |

`replay_fixtures/divergences.json` carried one entry per recorded divergence.
It was **empty** as of 2026-09-24, when the Desrt Leach capture closed the
vehicle-rule worklist; the Ice Digger's capture (section 5) reopened it with a
finding of its own, and **later the same day a per-vehicle pass count closed it
again** — the manifest is empty once more. Section 5.1 says what the finding was
and what fixed it. What the old entry carried, and what closed it:

* **frame** f25026, **round** 1, **kind** `no-swing`, **action** actor 1 (the
  vehicle), f25026-25169;
* the log has the vehicle swinging: `loc_B6A2`'s pass runs in **three separate
  frames** (f25059, f25060, f25072, one call each) and then the sixteen
  `Battle_CalculateDamage` draws at f25098 land 165 on the Desrt Leach;
* the port has `TurnSkipped { reason: Unarmed }` and no swing at all, so round 1
  drew 31 of the log's 50 rolls.

What the port did, and what the cartridge does instead:

* The port refused the swing at `Character_Attack`'s weapon check
  (`ps4.asm:13002-13019`): with neither hand holding a weapon, the cartridge
  takes the `move.w #$FFFF, $32(a4)` arm and skips `jsr loc_B6A2`, and
  `rust/psiv-core/src/battle/engine.rs` models that as `Skipped::Unarmed`. The
  port's vehicle had no equipment - `rust/psiv-core/src/vehicle.rs`'s
  `battle_member` leaves `equipment` zero - and so skipped. The vehicle's
  fighter is not that path at all: `loc_5B8E` (`ps4.asm:8410-8416`) sends its
  command-6 Attack to `loc_AF9C` (`ps4.asm:16810`), a routine of its own whose
  three `loc_B6A2` passes (states 4, 5 and 5 again, `ps4.asm:16957-16979`,
  `14964`, and the attack object's `move.w #5, $32(a0)` at `ps4.asm:82975`)
  are exactly the three frames the log holds. `Character_Attack`, its weapon
  check and its `Character_AttackActionOffs` dispatch are never reached for a
  vehicle; the two extra rolls shifted the rest of the round's stream, which is
  why the round's draw count was 31 against 50.
* The correction lane's first cut read the three passes as `CharAttack_Seth`'s
  three close-range mappings (`ps4.asm:13759-13761`) instead. That was wrong:
  the mappings belong to fighter routine 6's *character* dispatch, which a
  vehicle never reaches, and the three frames come from the attack object the
  vehicle's own routine loads. `loc_9848` (`ps4.asm:14964`) is the shared
  second pass both readings share, and the third is the object's hand-back.
* Fixed on 2026-09-24 in `rust/psiv-core/src/battle/vehicle_attack.rs`, with
  `resolve_attack` dispatching to it. The rule, its citations, the six rounds'
  arithmetic and the tests are in
  [`source-notes/battle-party.md`](../source-notes/battle-party.md#the-vehicles-own-attack-command-6-three-hit-passes-2026-09-24).
* The fixture's per-target `hit` byte is the one the swing's **last** pass
  wrote: `loc_B6A2` presets all nine `Fighters_Hit_Flags` to `$FF` before every
  pass (`ps4.asm:17493-17498`), so an earlier pass's verdicts are overwritten,
  and `oracle/fixture/observations.py`'s `pass_frame` reads the byte on the
  last frame the flags moved on that also drew - the action's own last
  `loc_B6A2` pass, an ability's auto-hit pass included. Rounds 3 and 6 of this
  capture are why it matters: their first pass came back critical (`$01`) and
  their third normal, and their damage - 154 and 189 - is what only a normal
  hit's arithmetic produces (`((45+8)*200)>>6 + 200 = 365`, `365*2>>2 - 28 =
  154`; with the `atk >> 2` bonus it would be 204). The fixture was re-extracted
  from this capture on 2026-09-24, so those two bytes are `$00` and
  `replay/compare.rs` compares every verdict strictly again - no
  frame-dependent relaxation.

The fixture's own reading of the vehicle battle is in its `vehicle` section:
`Vehicle_Index` 1 (the selector's patch), the fighter's HP at the battle's first
frame (`Vehicle_Stats + curr_hp`, `$FFFF470E`, logged as
`vehicle_fighter_hp`), and the saved record behind it
(`Saved_Vehicle_Stats`, `$FFFFFA80`), whose own current HP **equals** the
fighter's at that frame - which is what makes its maximum (740) the battle
copy's, and what the fixture records rather than assumes. The 2026-09-24
re-extraction is what put that on the record: the committed file predated
`oracle/fixture/vehicle.py`'s `matches` reading, so its `max_hp` was `null` and
it carried no `hp_matches_saved_record`; the regenerated file has `740` and
`true` - the claim `rust/psiv-core/src/battle/replay/build.rs` asserts when it
seats the vehicle - while every other vehicle cell is byte-identical. The
members' HP columns are the field's: nothing loads them in a vehicle battle, so
the fixture carries no party at all and the party side's HP column *is*
`vehicle_fighter_hp`.

The Ice Digger's fixture is the same reading on the second record, with one
cell it cannot fill: the log's `vehicle_land_*` columns are
`Saved_Vehicle_Stats`'s **first** record (`$FFFFFA80`, the Land Rover's - 740
HP, mask 3), while the battle's fighter was built from `VehicleData[2]`, so
`oracle/fixture/vehicle.py`'s `matches` test fails on purpose and the fixture
carries `hp_matches_saved_record: false` with `max_hp: null` beside
`hp: 960`. That is the extractor declining to call the saved Land Rover record
the Ice Digger's maximum, which is exactly what the two records being different
means; `rust/psiv-core/src/battle/replay/build.rs` only asserts the equality
when the fixture claims it, so the replay seats the fighter on the log's own
960. The Ice Digger is the record that would want a *second* `Saved_Vehicle_Stats`
column set in `oracle/ram_map.json` (`$FFFFFAA0`), and that file is outside this
lane's write set - the reading above is what it can say without it.

**Added later on 2026-09-24:** the second record's columns now exist
(`vehicle_ice_hp`/`_max_hp`/`_skill_mask`/`_skill1_current`/`_skill1_max` at
`$FFFFFAA0`+0/2/4/6/7, `Ice_Digger_Stats` `constants:2413`), and
`oracle/fixture/vehicle.py` reads the record its `Vehicle_Index` names - index 1
through the columns the fixtures already carry, index 2 through these, and an
index with no logged record (the Hydrofoil's 3) through the first record's, with
a note saying so. Nothing in the two committed fixtures changes: no capture has
logged the new columns, so the Ice Digger's still reads `max_hp: null` beside
`hp: 960`, and re-reading its log finds all five cells absent rather than wrong.
Filling it needs a re-capture of `$53 --vehicle 2`, which is out of this lane's
scope.

## 4. What this does and does not prove

Proved, for the four captures above: the formation in RAM is the one asked
for, the trace is the cartridge's own arithmetic (the checker re-derives every
roll, the seed chain and the per-frame anchor), the run is deterministic
byte-for-byte across two runs of the same tape and patches, the enemy's ability
dispatch wrote the ability id the capture exists for, and - for the `$53` pair -
the battle's party-side fighter is `VehicleData`'s record for the
`Vehicle_Index` the capture forced, on two of the three records: its HP
(740/960) and its swing's damage (165 from the 200 byte, 212 from the 250).

Not proved, and not claimed:

- **That the party can win.** The party is whatever the base tape has: the
  prefix comes from tape 07, so Alys (level 7), Chaz and Hahn at the levels and
  equipment that tape reaches. Two Helex or two Fanbite wipe it in one or two
  rounds - that is the retail outcome for this party at this point in the game,
  and the captures are useful precisely because the enemies act. The one victory
  among the three is the vehicle's, not the members': in that battle they never
  act. A capture of a fight the members survive needs a base tape with a
  stronger party.
- **That the encounter is natural.** Forcing is a fixture: `--ram-patch` writes
  retail RAM, exactly as the re-roll probes in `BATTLE_ORACLE_REPLAY.md` do, and
  those runs are experiments rather than natural-route evidence. The forced
  battle is the formation's own data and the cartridge's own engine from the
  draw onward, but no player could walk to it here.
- **The background.** `Field_Map_Index` drives `Battle_SetupBackground`
  (`ps4.asm:10124`), so a capture forced through a map byte shows that map's
  battle background; a grid or vehicle capture shows Motavia's, from
  `Mota_Battle_BG_Index`. The tile you see is not the room the party is in.
- **The party side of a vehicle battle.** The vehicle is the only party-side
  fighter (`loc_78EE`, `ps4.asm:11408`): the members' HP columns never move,
  and `vehicle_fighter_hp` (`Vehicle_Stats + curr_hp`, `$FFFF470E`) is what
  says how the fight is going. The two Desrt Leach captures are those battles.
- **How many passes each vehicle's swing draws.** The Ice Digger's capture says
  two where the rule says three (section 5.1): that is a recorded divergence in
  `replay_fixtures/divergences.json`, not a verified rule, and the Hydrofoil's
  12-frame wind-up is read from the disassembly rather than measured - no
  capture has been fought by the Hydrofoil. **Corrected later on 2026-09-24:**
  the rule *is* per vehicle now (three for the Land Rover and the Hydrofoil, two
  for the Ice Digger, from each attack object's own `$1C` timer), the entry is
  gone and the manifest is empty; the Hydrofoil's wind-up is still a
  disassembly reading rather than a measurement.
- **A vehicle battle against the other vehicle tables.** The Dezolis table
  (group `$D`) and the capture a second table would prove are still open: its
  formations' enemies (ProtectBit, LwAddmer, Owltalon) are not in
  `rust/psiv-core/src/battle/replay/pack.rs`, which is outside this lane's write
  set, and the region rule for the Ice Digger says they are what a Dezolis
  vehicle battle would build.
- **Anything after the battle.** The tape is trimmed 60 frames past the battle's
  last in-battle frame; the log's provenance after that is the game-over or
  field sequence and is not evidence of anything.
- **Formations nothing can reach.** A formation whose only groups no map byte,
  no grid cell and no vehicle table can produce is refused with the groups
  named, rather than approximated.

## 5. `--vehicle`: the second record, and the Ice Digger's capture

The vehicle rule was proven on one record - the Land Rover of the `$53` capture.
`--vehicle N` chooses which of `VehicleData`'s records the forced battle loads,
and the fourth capture is the same formation fought by the **Ice Digger**
(`build/forced/icedigger/`, trace
`ae50eaaeee6383f4cb0e7ddde9f702ae5fe9da1ef08d63a033ad7f999f6ff288`, log
`5226afa4697307aa240a9fd047ab5a82a5975399e7648944463594116df6e2e9`). Two things
about that choice are the cartridge's, and the tool checks both rather than
assuming them (`oracle/force/selectors.py`).

**Which values exist.** `Vehicle_Index` selects a record in `VehicleData`
(`ps4.asm:321152-321174`) and there are three: `loc_77AE` (`ps4.asm:11295-11307`)
indexes the table by `Vehicle_Index - 1` with a hard-coded 26-byte stride and
**no bounds check**, so `4` and up read past the table's last record, and `0` is
not a vehicle at all - `FillBattleStats` takes its on-foot arm for it
(`ps4.asm:11273-11274`). `--vehicle 0` or `--vehicle 4` is refused with that.

**Which of them a region's tables can seat.** The group is the *region's*: `$D`
for `Field_Map_Index != 0`, else `9`/`$A`/`8` by `Mota_Battle_BG_Index`
(`ps4.asm:11825-11839`). A vehicle reaches a battle only by being mounted, and
outside Motavia nothing mounts one: `ItemAction_LandRover` /
`ItemAction_IceDigger` / `ItemAction_HydroFoil` (`ps4.asm:123419-123465`)
require `Field_Map_Index & $FFF0 == 0` **and** the bit
`VehicleBoardingFlags[Field_Map_Index & $F]` sets, and that table
(`ps4.asm:117189-117197`) is `$07` only for the low nibble `0` - Motavia - with
`$04` for `9` and `$07` for `$A`/`$B`, which are `ErrorTrap` maps
(`ps4.asm:184035-184036`), and `$00` for every other low nibble, Dezolis's `1`
and Rykros's `2` included. Every other write of a nonzero selector in this
disassembly is the three boarding events those items run (`ps4.asm:145007`,
`145066`, `145125`), the Motavia cutscene that hands over the Land Rover
(`ps4.asm:147452`, `147483`) and the Dezolis one that hands over the Ice Digger
(`Cutscene_DarkForce1Defeated`, `ps4.asm:156378`, writing `MapID_Dezolis` at
`156773` and `VehicleID_IceDigger` at `156790`). So the Motavia tables
seat all three machines and the Dezolis table seats the Ice Digger, and
`--vehicle 1` (or `3`) on a group-`$D` formation is refused with that reason.
Without `--vehicle` the table's own region decides: the Land Rover on Motavia
(what the three earlier captures were taken with) and the Ice Digger on Dezolis.
A `--vehicle` on a formation that sits in no vehicle table is refused too - the
four tables' formations (`generated/formation_indexes.json` groups 8, 9, 10, 13)
sit in no other group.

| | `$53` Land Rover | `$53` Ice Digger |
|---|---|---|
| command | `--formation 0x53` | `--formation 0x53 --vehicle 2` |
| selector | vehicle table 8, entry 24 | the same (`Vehicle_Index` 2) |
| probe: draw, entry drawn | f24831, `$1C44` & 31 = 4 -> formation 76 | f24831, `$1C44` & 31 = 4 -> formation 76 |
| seed patch (`f(draw-1)`) | `$0011` at f24830 | `$0011` at f24830 |
| `Vehicle_Index` patch | `24795:FFFFF43C:0001` | `24795:FFFFF43C:0002` |
| the fighter's HP at the battle's first frame | **740** (`VehicleData[1]`'s `$02E4`) | **960** (`VehicleData[2]`'s `$03C0`) |
| battle window | f24794-26872 | f24794-26616 |
| rounds, vehicle swings | 6, 6 | 5, 5 |
| round 1's swing | 3 hit passes (f25059, f25060, f25072) + 16 draws at f25098 summing 52 -> 165 | 2 hit passes (f25058, f25059) + 16 draws at f25075 summing 51 -> 212 |
| outcome | victory, the Leach at 0 HP, +1500 exp, +1 meseta | victory, +1500 exp, +1 meseta |
| ability id used | `$37` SAND STORM at f25839 | `$37` SAND STORM at f25416 |
| trace sha256 | `aa133d61288c886b34af14572a4ef9080fc59e47bd20bbce02a011daf06a214b` | `ae50eaaeee6383f4cb0e7ddde9f702ae5fe9da1ef08d63a033ad7f999f6ff288` |
| log sha256 | `33f19744e8fd485c625a81b302c8a979af567463ce46c731a028b98afe13a0c3` | `5226afa4697307aa240a9fd047ab5a82a5975399e7648944463594116df6e2e9` |

The Ice Digger capture's log pin carries this lane's `# rom=` line (section 3);
its trace pin does not.

The two captures share a draw, a seed patch and a formation, so what differs
between them is the vehicle record alone, and the log's own numbers say which
record the battle loaded:

* **the HP** - `vehicle_fighter_hp` (`Vehicle_Stats + curr_hp`, `$FFFF470E`) at
  the battle's first frame is 960, `VehicleData[2]`'s word, where the Land Rover
  capture's is 740, `VehicleData[1]`'s. This is `loc_77AE`'s reload
  (`ps4.asm:11295-11404`, reached from `GameMode_LoadBattle`'s `FillBattleStats`
  call, `ps4.asm:10005`) measured on a second record: the field's own
  `Saved_Vehicle_Stats` cells never move (they are the first record's - see
  section 3a), and the battle's fighter is the static word.
* **the attack byte** - the swing's damage is the second record's byte and not
  the first's. Round 1's sixteen draws at f25075 sum to 51 and the log holds
  **212**: `((51+8)*250)>>6 + 250 = 480`, `480*2>>2 - 28 = 212` for the Ice
  Digger's 250, where the Land Rover's 200 would land 164 and the Hydrofoil's
  150 land 116 (`build/lane-evidence/icedigger/damage_arithmetic.log`). The
  Land Rover's own capture is the mirror image: its round-1 draws sum to 52 and
  its 165 is what 200 produces there, while 250 would produce 214.
* **the element** is the *target's*, not the vehicle's - `loc_280A`
  (`ps4.asm:4016-4018`) reads `$32(a1)` of the thing being hit - so no second
  vehicle can move it. The Ice Digger's swing pins the same arithmetic: the
  Desrt Leach's energy byte 2 gives 212, factor 1 would give 92, and the
  target's defence 28 is the subtraction (3 would give 237).

The replay of this fixture does **not** come back exact, and that is the
finding below rather than a rule change: `rust/psiv-core/src/battle/vehicle_attack.rs`
is outside this lane's write set, so the divergence is recorded in
`replay_fixtures/divergences.json` with its numbers.

### 5.1 The Ice Digger's two-pass swing

Round 1's swing at f25026 - actor 1, the Ice Digger - is where the port and the
log part company, and it is the **pass count** that differs:

* the log's frames hold **two** `loc_B6A2` calls for the swing, one each at
  f25058 and f25059, then the sixteen `Battle_CalculateDamage` draws at f25075,
  whose 51 sum lands 212 on the Desrt Leach (round 1 spends 49 rolls);
* the port draws **three** passes, so its sixteen draws start one roll late and
  pick up round 2's first roll (`$3E43` & 7 = 3) as their last: the sum is 52
  and the damage 214 (round 1 spends 50). That is the recorded divergence,
  `kind` `value`, at its first frame.

What the cartridge does, and why the two vehicles differ. The vehicle's swing is
`loc_AF9C` (`ps4.asm:16810`), whose state 4 creates the attack object and jumps
into `loc_B6A2` (pass 1); its state 5, `loc_9848` (`ps4.asm:14964`), runs
`loc_B6A2` (pass 2) and then advances the vehicle's `action_routine`
(`addq.w #1, action_routine(a4)`, `ps4.asm:14978`); and the third `loc_B6A2`
comes from the object handing the routine **back** to 5
(`move.w #5, $32(a0)`), which re-enters state 5. That hand-back is on a timer:

* `BattleObj_LandRoverAtk` (`$644`) is created with `move.w #$C, $1C(a4)`
  (`ps4.asm:82945`) and `BattleObj_HydrofoilAtk` (`$64C`) with the same
  (`ps4.asm:83071`), so their first state (`ps4.asm:82970-82979`,
  `83104-83113`) waits twelve frames and then writes `move.w #5, $32(a0)`
  (`ps4.asm:82975`, `83109`) - **after** state 4's own `addq.w #1, $32(a4)`
  (`ps4.asm:16961`, the same frame the object was created) has already moved the
  vehicle from 4 to 5 and state 5 has moved it on to 6. The write therefore
  re-enters state 5, and that is the third pass: the Land Rover's f25072, which
  is the creation frame (f25059) plus thirteen countdown frames.
* `BattleObj_IceDiggerAtk` (`$648`) is created with `clr.w $1C(a4)`
  (`ps4.asm:82996`), so its first state (`ps4.asm:83043-83052`) is already
  expired in the frame the object is created and writes `move.w #5, $32(a0)`
  (`ps4.asm:83048`) while the vehicle is entering 5 - the write lands on the
  value it already holds, state 5 runs exactly once, and the swing has **two**
  passes.

The objects' own timelines confirm both counts, and they are in the logs: the
damage frame is the hand-back frame plus the object's state-1 timer, `$19` for
the Land Rover (f25072 + 26 = f25098) and `$F` for the Ice Digger (f25059 + 16 =
f25075), which is where each capture's sixteen draws sit.

So `rust/psiv-core/src/battle/vehicle_attack.rs`'s `VEHICLE_HIT_PASSES = 3` is
the Land Rover's and the Hydrofoil's pass count, not every vehicle's: the Ice
Digger's record runs state 5 once, and a port that models the pass count per
vehicle would need 2 for it. Changing that rule is a `psiv-core` change and is
what this capture's manifest entry exists to record - the fixed rule, its
citations and its tests are
[`source-notes/battle-party.md`](../source-notes/battle-party.md#the-vehicles-own-attack-command-6-three-hit-passes-2026-09-24)'s,
and they were not changed here.

**Done, later on 2026-09-24.** `VEHICLE_HIT_PASSES` is gone: the rule is
`vehicle_attack.rs`'s `hit_passes(vehicle)`, 3 for the Land Rover and the
Hydrofoil (their objects are created with `#$C`, `ps4.asm:82945`/`83071`) and 2
for the Ice Digger (`clr.w $1C(a4)`, `ps4.asm:82996`), with
`VEHICLE_ATTACK_OBJECTS` carrying each object's id, routine, timer and both
citing lines. `resolve_vehicle_attack` derives the vehicle from the actor's
`character` (`Vehicle_Index + $B`) and draws that many passes; the last pass
still decides. Two tests now pin this capture's own swing - it replays to the
log's 212 and HP 828 on the rows of this fixture, and the pre-change uniform
three-pass rule is reproduced beside it at 214 (19 rolls against the log's 18) -
in `battle/vehicle_attack_passes_tests.rs`. The `forced_53_icedigger` entry is
**gone** from `replay_fixtures/divergences.json`, which is empty again, and
`every_fixture_replays_as_recorded` passes. Forcing `hit_passes` back to 3 fails
it at exactly the entry's finding:
`forced_53_icedigger: the port diverges at f25026 (round 1, value): the log has
FighterId(6): hit flag 00, damage Some(212), the port Normal with Some(214)`
(`build/lane-evidence/10-negative-control-uniform-three.log`; the restored run is
`11-replay-after-restore.log`). The Hydrofoil's twelve-frame wind-up is still a
disassembly reading: no capture has been fought by it.

The sweep's own two options - `--durable` (the party-side HP patch,
[`oracle/force/durable.py`](../../oracle/force/durable.py)) and `--max-rounds N`
(stop at round N's end; the extractor records `outcome.truncated`) - and what a
whole region's captures found with them are
[`BATTLE_ORACLE_SWEEP.md`](BATTLE_ORACLE_SWEEP.md)'s.
