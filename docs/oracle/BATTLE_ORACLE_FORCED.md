# Forcing a formation: capturing any battle on demand

What this ledger records: how `python3 -m oracle.force` makes a *chosen*
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
   `python3 -m oracle.rng_trace check` requires a frame's first call to start
   from the seed the log holds for the frame before, so a patch applied at the
   draw's own frame fails the check that proves the trace is the cartridge's
   arithmetic.
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

`--party-script` gives per-round commands through the live cartridge menus.
The format is in [oracle/README.md](../../oracle/README.md#party-command-scripts),
and what a scripted capture guarantees is
[section 9](#9-party-command-scripts-what-a-capture-guarantees).
The two existing `--policy` inputs below are unchanged.

`attack` (the default) is tape 07's own fight input: one `C` press every 16
frames (4 held, 12 released). It takes COMD -> ATTACK -> the default target for
each member and then advances messages and rounds, and it is what both checked-in
battle tapes use, so "no menu drift" is inherited from them rather than assumed.
`defend` is tape 14's navigation - the per-character command menu is horizontal,
so four Right presses step ATTACK -> TECHNIQUE -> SKILL -> ITEM -> DEFEND.
Both are fixed frame patterns, so the inputs are frame-deterministic.

`defend` was run end to end once, as the acceptable fallback for a battle the
attack policy cannot finish: formation `$5E` again, `--repeats 60`. The
battle runs to f25760 - longer than the attack policy's f25616, as defending
should be - and the defender's `physical_prop` goes `514 -> 258` for the
surviving member at f25439, which is the same retail DEFEND clobber
`verify.sh`'s full lane measures for tape 14. FLAME BOLT still fires
(`e2=0x02` at f25269), `python3 -m oracle.rng_trace check` passes and the two
runs are byte-identical.

`--delay N` inserts N idle frames between the field prefix and the policy. The
encounter has already fired, so the formation stays forced, while every roll in
the fight shifts (`Main_Frame_Count` advances and the seed evolves frame by
frame): the knob for a battle whose enemy never rolls the ability a capture is
for. The probe and the seed patch are re-derived for each delay, because the
draw's frame moves with it. Formation `$5E` with `--delay 5` is the measurement: still two Helex, but the
draw's `hv` is `$6DFF` rather than `$7195`, so `K` is 26 rather than 16 and the
seed patch is `$0006` rather than `$001C`; the fight's own rolls differ (trace
`da65b77f…`, log `e7fdf0ca…`, the two FLAME BOLTs at f25008/f25130 instead of
f25003/f25125) and the battle ends at f25621 instead of f25616. None of the
three captures needed it - every ability fired on the first delay tried - but
the knob is measured rather than assumed.

A capture of the retarget rule's **mirror** loop (a single-target technique
ordered at an enemy that falls before it resolves) needs a scripted party that
has learned a single-target non-damaging technique; it is
[issue #37](https://github.com/TusanHomichi/PSIV/issues/37).

## 2. The four captures

Each capture is a full run of `python3 -m oracle.force`: scout, probe, preview,
capture, verify. All four start from tape 07's own field prefix - frames
1..24794, i.e. everything up to and including the frame its encounter fires on -
then the attack policy; each was checked with
`python3 -m oracle.rng_trace check` (passed) and the capture was run twice with
the same tape path and output basenames, byte-identical both times (traces and
logs). Three of them force a formation under the Land Rover; the fourth is the
same formation as the third, forced with `--vehicle 2` so the battle loads the
**Ice Digger's** record instead (section 5).

| | formation `$5E` | formation `$37` | formation `$53` |
|---|---|---|---|
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
(`oracle/build_core.sh`). The host needs no preparation: `oracle.force` asks
`oracle/host_binary.py` for a `psiv_oracle` compiled from `oracle/host/` as it
stands, and rebuilds it when it is not
([`oracle/README.md`](../../oracle/README.md#building-the-host-and-why-it-cannot-be-stale)):

```sh
python3 -m oracle.force --formation 0x5E \
    --out build/forced/helex --require-ability 2
python3 -m oracle.force --formation 0x37 \
    --out build/forced/fanbite --require-ability 8
python3 -m oracle.force --formation 0x53 \
    --out build/forced/desrtleach --require-ability 0x37
python3 -m oracle.force --formation 0x53 --vehicle 2 \
    --out build/forced/icedigger
```

Its runs log `core,battle,bhit,enemy,chars,rng,vehicle,bcmd`
(`oracle/force/runs.py`'s `GROUPS`) - the vehicle group because a vehicle
battle's party side is built from `Vehicle_Stats` and its saved record, which a
fixture's `vehicle` section reads, and `bcmd` (`Character_Command_Data`,
`constants:2005-2011`) because the command a member chose is the only record of
what a swing was *aimed* at: the cartridge moves `Current_Target_Index` off a
commanded enemy that has fallen (`ps4.asm:8345-8409`), so the cell is what tells
a swing that kept its aim from one the retarget scan re-aimed. A capture carries
`current_target`/`current_command` (`$FFFF4144`/`$FFFF4146`) and
`cmd0_target`..`cmd4_target` with it; the extractor records the command's target
per party action (`assembly.command_entry`) and no target at all for a capture
taken before the group existed.

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
an unreachable vehicle/group pair was asked for, or
`python3 -m oracle.rng_trace check` fails.

A single capture is ~90 seconds of wall clock (five oracle runs over ~40k frames
of tape). The scout is cached in the output directory, so a re-run of the same
capture skips it, and `--scout` can point several captures at one cache.

The logs are pinned by sha256 above. A log's bytes change with its column set
(a group gaining columns changes every later capture's log, not any pinned one),
and the trace's do not. Neither depends on where the inputs sit: the host records
`# rom=`, `# tape=` and `# rng-trace=` by file name
([log format](../../oracle/README.md#log-format)), so a capture replayed from
another directory with the same patches and groups is byte-identical, log and
trace.

## 3a. The captures' replay verdicts

Each capture is extracted into a fixture
(`rust/psiv-core/src/battle/replay_fixtures/forced_*.json`) and replayed by the
one data-driven test of [`BATTLE_ORACLE_REPLAY.md`](BATTLE_ORACLE_REPLAY.md),
which compares every action and every round's draw count against the log and
holds a fixture that does not match to its entry in
`replay_fixtures/divergences.json`:

```sh
python3 -m oracle.fixture --trace build/forced/helex/capture/forced_5E_attack_rolls.csv \
    --log build/forced/helex/capture/forced_5E_attack.csv \
    --tape "forced_5E_attack.tape (python3 -m oracle.force --formation 0x5E)" \
    --battle-first 24794 --battle-last 25616 \
    --out rust/psiv-core/src/battle/replay_fixtures/forced_5e_helex.json
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- --test-threads=1 every_fixture_replays_as_recorded
```

The four committed forced fixtures were extracted before the oracle entry
points became package modules, so the tape label each records in its own
`provenance` is the spelling of the time; everything else this recipe produces
is byte-identical to what they hold.

| capture | fixture | verdict |
|---|---|---|
| `$5E` two Helex | `forced_5e_helex.json` | **exact**: two rounds, 80 rolls, both FLAME BOLTs (`$02` at f25003 and f25125, and the re-rolled one at f25371) resolve damage for damage, and the party's defeat is the log's |
| `$37` two Fanbite | `forced_37_fanbite.json` | **exact**: SPIRAL BLD (`$08` at f24903) takes all three party slots for the damage the log shows, and the wipe is the log's |
| `$53` one Desrt Leach, Land Rover | `forced_53_desrtleach.json` | **exact**: six rounds, 285 rolls, every vehicle swing's three hit passes and sixteen damage draws, the Desrt Leach's death at f26459, and the log's 1500 exp and 1 meseta |
| `$53` one Desrt Leach, Ice Digger | `forced_53_icedigger.json` | **exact**: five rounds, 245 battle rolls, every swing's **two** hit passes and sixteen damage draws, and the Desrt Leach's death at f26253 — the one-pass-too-many this capture first recorded is fixed by `hit_passes` (section 5.1) |

The Desrt Leach capture first diverged at the vehicle's swing. What the
finding was, and the rule that closed it:

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
* The rule is `rust/psiv-core/src/battle/vehicle_attack.rs`, with
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
  154`; with the `atk >> 2` bonus it would be 204). Those two bytes are `$00`
  in the fixture, and `replay/compare.rs` compares every verdict strictly.

The fixture's own reading of the vehicle battle is in its `vehicle` section:
`Vehicle_Index` 1 (the selector's patch), the fighter's HP at the battle's first
frame (`Vehicle_Stats + curr_hp`, `$FFFF470E`, logged as
`vehicle_fighter_hp`), and the saved record behind it
(`Saved_Vehicle_Stats`, `$FFFFFA80`), whose own current HP **equals** the
fighter's at that frame - which is what makes its maximum (740) the battle
copy's, and what the fixture records rather than assumes: `max_hp` 740 and
`hp_matches_saved_record: true`, the claim
`rust/psiv-core/src/battle/replay/build.rs` asserts when it seats the vehicle. The
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
960. The second record's columns exist
(`vehicle_ice_hp`/`_max_hp`/`_skill_mask`/`_skill1_current`/`_skill1_max` at
`$FFFFFAA0`+0/2/4/6/7, `Ice_Digger_Stats` `constants:2413`), and
`oracle/fixture/vehicle.py` reads the record its `Vehicle_Index` names - index 1
through the first record's columns, index 2 through these, and an index with no
logged record (the Hydrofoil's 3) through the first record's, with a note saying
so. The committed Ice Digger capture predates those columns, so its fixture
still reads `max_hp: null` beside `hp: 960`; a re-capture of `$53 --vehicle 2`
fills it.

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
- **The Hydrofoil's swing.** The pass count is per vehicle (three for the Land
  Rover and the Hydrofoil, two for the Ice Digger, from each attack object's own
  `$1C` timer, section 5.1); the Hydrofoil's 12-frame wind-up is read from the
  disassembly, and no capture has been fought by the Hydrofoil.
- **A vehicle battle against the other vehicle tables.** The Dezolis table
  (group `$D`) and the capture a second table would prove are still open: its
  formations' enemies (ProtectBit, LwAddmer, Owltalon) would first need to be in
  the replay pack (`python3 -m oracle.sweep.replay_pack`), and the region rule for the Ice Digger says they are what a Dezolis
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
(trace
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
  150 land 116. The
  Land Rover's own capture is the mirror image: its round-1 draws sum to 52 and
  its 165 is what 200 produces there, while 250 would produce 214.
* **the element** is the *target's*, not the vehicle's - `loc_280A`
  (`ps4.asm:4016-4018`) reads `$32(a1)` of the thing being hit - so no second
  vehicle can move it. The Ice Digger's swing pins the same arithmetic: the
  Desrt Leach's energy byte 2 gives 212, factor 1 would give 92, and the
  target's defence 28 is the subtraction (3 would give 237).

The swing's pass count is where the two vehicles differ, below.

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

The rule is
`vehicle_attack.rs`'s `hit_passes(vehicle)`, 3 for the Land Rover and the
Hydrofoil (their objects are created with `#$C`, `ps4.asm:82945`/`83071`) and 2
for the Ice Digger (`clr.w $1C(a4)`, `ps4.asm:82996`), with
`VEHICLE_ATTACK_OBJECTS` carrying each object's id, routine, timer and both
citing lines. `resolve_vehicle_attack` derives the vehicle from the actor's
`character` (`Vehicle_Index + $B`) and draws that many passes; the last pass
still decides. Two tests now pin this capture's own swing - it replays to the
log's 212 and HP 828 on the rows of this fixture, and the pre-change uniform
three-pass rule is reproduced beside it at 214 (19 rolls against the log's 18) -
in `battle/vehicle_attack_passes_tests.rs`, and the fixture replays exactly.
Forcing `hit_passes` back to 3 fails the replay at:
`forced_53_icedigger: the port diverges at f25026 (round 1, value): the log has
FighterId(6): hit flag 00, damage Some(212), the port Normal with Some(214)`. The Hydrofoil's twelve-frame wind-up is still a
disassembly reading: no capture has been fought by it.

The sweep's own two options - `--durable` (the party-side HP patch,
[`oracle/force/durable.py`](../../oracle/force/durable.py)) and `--max-rounds N`
(stop at round N's end; the extractor records `outcome.truncated`) - and what a
whole region's captures found with them are
[`BATTLE_ORACLE_SWEEP.md`](BATTLE_ORACLE_SWEEP.md)'s.

## 6. The Motavia arc: FIREBREATH, ZAN, FORCEFLASH, CORRSION

This section is the cartridge evidence for the Zio-arc damage routes — `$21`
FIREBREATH, `$47` ZAN, `$56` FORCEFLASH, `$4D` CORRSION: one forced capture each,
extracted into a fixture and replayed by
[`every_fixture_replays_as_recorded`](../../rust/psiv-core/src/battle/replay/data.rs).

The tool, the selector logic and the checks are sections 1-3's, unchanged; what
these captures add is the option combination and the reason for it.

| option | why |
|---|---|
| `--require-ability <id>` | the capture is only worth taking if the enemy *used* the ability, and the tool refuses the run otherwise. A roll is one of eight, so a series of `--delay` values is taken until one lands on it: `--delay` shifts every roll in the fight, and the attempts that miss are discarded (the reports that carry
`require_ability_met: false` are what says so while the series runs). |
| `--max-rounds 1` (ZAN, FORCEFLASH, CORRSION) or `3` (FIREBREATH) | the cap is what keeps a fight's *other* actions inside abilities the port runs: 100 TechMaster's `$3E` GIRES and its `$46` SAR condition, 115 Greneris's `$28`/`$29`/`$2A`/`$2F`/`$57` and 106 Haunt's `$4C` EVIL EYE are none of these four, and one of them in a captured round is one fixture the replay cannot hold. FIREBREATH's carrier needs no cap for that (83 Ripper's list is five zeros and three `$21`) — its three rounds are the long capture. |
| no `--durable` | the tape's own party fights and keeps its own HP, so nothing in the fixture is the capture's invention. The party is level 1-7 and every one of these formations wipes it, which is why all four captures are **defeats**: the enemies act first (their agility is higher) and the log holds their whole round. A `--durable` run is refused for these formations anyway — the probe draws the group's own entry, whose enemy count can differ from the forced one's by a frame of load, and the tool then reads the patch one frame late (`--durable wrote 999 to alys_hp at f24818, but the capture reads 53 at f24817`). |

### 6.1 The four captures

```sh
python3 -m oracle.force --formation 219 --out <dir> --max-rounds 3 --require-ability 33
python3 -m oracle.force --formation 272 --out <dir> --max-rounds 1 --require-ability 71 --delay 19
python3 -m oracle.force --formation 281 --out <dir> --max-rounds 1 --require-ability 86 --delay 5
python3 -m oracle.force --formation 251 --out <dir> --max-rounds 1 --require-ability 77 --delay 53
```

| ability | carrier and formation | forced through | the ability's own action | fixture | replay |
|---|---|---|---|---|---|
| `$21` FIREBREATH | 83 Ripper ×2, formation 219 (`$DB`) | map 130 `Zio Fort` (`$082`, group 30; the map's own byte) | round 1 at f24985 (fighter 6) and round 3 at f25963 (fighter 7); 3 rounds, 255 battle rolls | `forced_db_ripper.json` | **exact** |
| `$47` ZAN | 100 TechMaster ×2, formation 272 (`$110`) | map 204 `Nurvus_Part2` (`$0CC`, group 40) | round 1 at f24864 (fighter 7), three party slots, 49 rolls | `forced_110_techmaster.json` | **exact** |
| `$56` FORCEFLASH | 115 Greneris, formation 281 (`$119`) | map 206 `Nurvus_B1` (`$0CE`, group 41) | round 1 at f24976 (fighter 6), three party slots, 49 rolls | `forced_119_greneris.json` | **exact** |
| `$4D` CORRSION | 106 Haunt ×2, formation 251 (`$FB`) | map 142 `LadeaTower_F2` (`$08E`, group 38) | round 1 at f25129 (fighter 7), two living party slots, 35 rolls | `forced_fb_haunt.json` | **exact** |

Every one of the four is a defeat of the tape's party: 100 TechMaster's `$44` FOI
follows ZAN in the same round, 83 Ripper's rounds 2 and 3 are the party's own
attacks and FIREBREATH's second use, and 106 Haunt's first action kills one
member so CORRSION's five-slot request resolves the two that are left — which is
the `AllParty` class's `Fighters_Hit_Flags` gate doing exactly what
[`ENEMY_DAMAGE_ROUTES.md`](../battle/ENEMY_DAMAGE_ROUTES.md) §3 says it does.

The extraction is section 3a's recipe with the cap and without the HP patch:

```sh
python3 -m oracle.fixture --trace <capture>/forced_110_attack_d19_rolls.csv \
    --log <capture>/forced_110_attack_d19.csv \
    --out rust/psiv-core/src/battle/replay_fixtures/forced_110_techmaster.json \
    --tape "forced_110_attack_d19.tape (python3 -m oracle.force --formation 272 --max-rounds 1 --require-ability 71 --delay 19)" \
    --battle-first 24794 --battle-last 25308 --max-rounds 1 --minified
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- --test-threads=1 every_fixture_replays_as_recorded
```

The replay harness reads the records those fixtures seat from
`replay_fixtures/motavia_pack.json`, which now covers the forced captures beside
the sweep (`oracle/sweep/replay_pack.py` walks the whole fixture directory);
`divergences.json` is empty, so every committed fixture — the two tape battles,
the four earlier forced captures, the four here, the three evidence-only
captures below and the sweep's 81 — replays exactly.

### 6.2 The three abilities that are not damage routes

`$4C` EVIL EYE, `$2A` RIMIT and `$2F` VOL are on issue #58's list too, and their
chains were read the same way: **none of them writes `move.w #$C` on any carrier
the survey covers**, so the damage pipeline is never reached and the effect
handler (`$07` SleepParalyze for the first two, `$02` Death for VOL) is the whole
turn. Their captures are evidence of that reading and nothing else — a fixture
for them would have to answer `EnemySkillUsed` for an ability whose handler this
port does not run, so none is committed:

| ability | carrier and formation | capture option | the ability's own action | what the log shows it doing |
|---|---|---|---|---|
| `$4C` EVIL EYE | 106 Haunt ×2, formation 251 (`$FB`) | `--formation 251 --max-rounds 2 --require-ability 76` | round 2 at f25403 (fighter 7) | 2 rolls, one slot, no damage word |
| `$2A` RIMIT | 115 Greneris, formation 281 (`$119`) | `--formation 281 --max-rounds 2 --require-ability 42` | round 2 at f25131 (fighter 6) | 4 rolls, three slots, no damage word |
| `$2F` VOL | 115 Greneris, formation 281 (`$119`) | `--formation 281 --max-rounds 2 --require-ability 47 --delay 17` | round 2 at f25324 (fighter 6) | 2 rolls, one slot, no damage word |

The three captures' round-1 enemy actions are plain attacks or the *other*
ability of the same carrier's list (115 Greneris's `$57` GELUN then RIMIT in one,
`$56` FORCEFLASH then VOL in the other), which is why they are not fixtures: a
round the port cannot run sits beside the one that proves the reading. Their
logs are the evidence the reading rests on, and they are the reason those
abilities' rows say `Not a damage route` rather than `single` or `all-party`:
the log of an EVIL EYE, RIMIT or VOL turn holds rolls and a slot and no damage
number at all.

### 6.3 What these captures do not prove

- **Juza's own battle.** 114 Juza is event battle 3 (the boss block of
  `generated/formations.json`), which no map's group reaches and this tool does
  not drive: his ZAN and FORCEFLASH arms are the same `EnemyAttack_Juza` arms
  100 TechMaster's and 115 Greneris's captures exercise, and the port's own
  ZAN row for 114 is the same object chain, but no fixture replays *his*
  battle. The campaign route's own run of it is the check for that.
- **A win, or a round the party lives through.** Every capture here ends in the
  party's defeat, so the fixtures prove the enemies' turns and the damage they
  deal, not what the party's commands did afterwards.
- **The abilities' own animations, sounds or timing.** `--max-rounds` cuts the
  capture at a round boundary and the fixture keeps the log's action frames; no
  frame-level animation claim is made here (that is
  `BATTLE_ANIMATIONS.md`'s).
- **The gated carriers.** 133 ProfoundDarkness1's `$21` and 140 Zio2's `$4D`
  sit behind `$FFFFEE87` and the `$FFFFEE98` phase counter. The port models
  both, and their captures run each event battle's own script:
  `replay_fixtures/endgame/profound_darkness*.json` and
  `replay_fixtures/zio_event06_psycho.json`.

## 7. The durable patch's frame

`--durable` writes the party's HP on the frame the extractor reads the battle's
start state from, and that frame is the forced formation's own: the
**probe** leaves the seed alone and so builds whatever formation the group's own
draw names, not the one being forced - and the load writes one enemy slot per
frame (`loc_7F2E`, `ps4.asm:11911`). Forcing a two-enemy formation out of a
three-enemy draw from the probe's frame would put the patch one frame after the
start state (TechMaster, `$110`: written at f24818, read at f24817). `oracle/force/durable.py`'s `forced_start_frame` derives the
forced formation's own frame (`probe_start - (probe_enemies - 1) + (forced_enemies
- 1)`), `oracle/force/phases.py` passes the formation to `probe_phase`, and the
`durable.verify` of the capture's start state is the guard. A formation whose probe draw happens to have the same
enemy count gets the frame it always did.

The Motavia arc's twelve captures
(`replay_fixtures/arc_motavia/`) were taken with it; their
recipe is `python3 -m oracle.sweep.arc`.

The durable patch's frame has a second case. The slot-per-frame
arithmetic is exact when a formation's enemies are different records, and one
frame late when some share a record: three GerotLux (`$11E`) read `enemy_count`
3 from f24821 with one slot filled and **all three** filled on f24822, not
f24823. The patch is therefore written on every frame from the
one after the draw to the estimate (`oracle/force/durable.py`'s
`first_frame`, the report's `durable.frames`): `FillBattleStats` is the only
writer of the party's HP cells and it runs before `Battle_SetupEnemyData`
(`ps4.asm:10005-10008`), so the repeated write changes nothing the first one
did not, and the estimate stays the *last* write (the existing run test pins
it).

## 8. Forcing an event battle: `--event`

`Battle_SetupEnemyData` reads `Event_Battle_Index` (`$FFFFECFC`, byte,
`ps4.constants.asm:2262`) first and, when it is not negative, takes the formation
from `Battle_BossFormationData` at once (`ps4.asm:11815-11819`): no group, no
grid cell, no vehicle table and no `UpdateRNGSeed2` call, so there is no entry
to steer and nothing to measure. The scenes write the same byte before they
start a battle (`ps4.asm:149140` `Event_DarkForce1`; the sabotage scene's index
8) and leave it set for the fight; the map loader resets it to `$FF`
(`ps4.asm:107595`, `121836`). `python3 -m oracle.force --event N` therefore
writes that one cell one frame after the encounter fires, exactly where a
group selector writes its cells, and nothing else.

What differs from `--formation`, and where the code says so:

* the formation is `generated/formations.json`'s `boss_formations` entry with
  `event_battle_index` N; `oracle/force/pack.py` numbers it `0x1000 + N` so it
  shares the regular formations' readers (`enemies_of`, `describe`) and no
  group lists it (`EVENT_BASE`);
* the cell has no column in the RAM log, so `oracle/force/pack.py`'s
  `EXTRA_CELLS` supplies its address next to `ram_map.json`'s fields;
* the probe **is** the forced battle: its log has to show exactly the boss
  formation's enemies (ids and HP) or the tool stops
  (`phases.event_probe`), and its own start frame is the durable patch's, with
  no estimate;
* the opening priority is the event rule, not the encounter's: `loc_B62A`
  draws its roll and then clears `Battle_Priority` for a non-negative
  `Event_Battle_Index` (`ps4.asm:17444-17447`), so the party opens unless the
  scripted-battle latch is raised (`battle/scripted_flag.rs`).

The first captures: event 8 (the Chaos Sorcerer of the sabotage scene), event
9 (Dark Force 1), `replay_fixtures/arc_zelan/event_08_d9.json` and
`event_09_d0.json`.

## 9. Party command scripts: what a capture guarantees

The format and menu rules have one owner:
[oracle/README.md](../../oracle/README.md#party-command-scripts).
`script.py` validates every written round against a hashed observation of the
base tape's actual party, TP, learned lists and inventory. `script_menu.py`
reads live cursor/list state and expands commands to joypad presses.
`script_capture.py` uses a read-only frame hook, freezes the input tape, then
the existing capture/verify path runs the stock self-building host.

The script never writes `Character_Command_Data`, `Current_Command`, target
cells, list scratch or menu cursors. Queue-build readback rejects another
command, record or target instead of patching it. The probe's input prefix is
preserved through its measured formation draw and selector restoration: pad
changes before that draw can change HV and select another formation.

Fixture extraction reads command bytes 1..5 and each selected id/target, plus
the selected equipment/inventory copy. No-RNG party turns are observed through
`loc_57C4` (`ps4.asm:8045-8055`), only outside menus and after the actor's `$EE`
status exclusion (`loc_576A`, 8033-8043). List scratch aliases the actor/command
words and must not create phantom turns. Inventory, party TP, enemy forms,
HP, status and battle MDEF are also checked at round end.

Party definitions in the replay's existing generated mirror are emitted by
`oracle.sweep.replay_pack` from the ignored, psiv_tools-decoded runtime pack.
The native runtime still loads that pack through `psiv-data`; no retail ability
record is transcribed into new Rust/Python source. Party ability ids are kept
separate from enemy ability ids when selecting mirror records.
