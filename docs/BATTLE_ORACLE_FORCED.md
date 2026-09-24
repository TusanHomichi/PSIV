# Forcing a formation: capturing any battle on demand

What this ledger records: how `oracle/force_battle.py` makes a *chosen*
formation reachable in the battle oracle, the three captures it produced for
the enemy abilities `psiv-core` now implements, and what those captures do and
do not prove. The replay side of the same picture is
[`BATTLE_ORACLE_REPLAY.md`](BATTLE_ORACLE_REPLAY.md); the capture tooling is
[`oracle/README.md`](../oracle/README.md).

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
| `vehicle` | `Field_Map_Index` = Motavia, `Vehicle_Index` (`$FFFFF43C`) = 1, `Mota_Battle_BG_Index` (`$FFFFECEF`) | groups 8/9/10/13 exist only here |

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

## 2. The three captures

Each capture is a full run of `oracle/force_battle.py`: scout, probe, preview,
capture, verify. All three start from tape 07's own field prefix - frames
1..24794, i.e. everything up to and including the frame its encounter fires on -
then the attack policy; each was checked with
`python3 oracle/rng_trace.py check` (passed) and the capture was run twice with
the same tape path and output basenames, byte-identical both times (traces and
logs).

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
```

Each writes, in its output directory: `scout.json` (the base tape's own run,
cached and reused), `<stem>.full.tape` and `<stem>.tape` (the composed tape,
before and after the trim), `<stem>.patches.txt` (the `--ram-patch` list),
`probe/`, `preview/`, `capture/` and `verify/` (each a `psiv_oracle` run's log
and `--rng-trace`), and `report.json` - the battle window, the outcome, the
enemies and party at its end, the ability ids observed, the patches, and every
sha256. The tool prints the same summary and exits non-zero if any of its checks
fails: the probe's formation does not match the group table, a required ability
never fired, two runs of the capture differ, or `rng_trace.py check` fails.

A single capture is ~90 seconds of wall clock (five oracle runs over ~40k frames
of tape). The scout is cached in the output directory, so a re-run of the same
capture skips it, and `--scout` can point several captures at one cache.

The logs are pinned by sha256 above, and two measurements back the pins. First,
the traces are byte-identical to the ones the capture tool's own lane recorded
in a *different* worktree; the logs differ from those earlier pins only in the
`# tape=` line (a basename now) and in the seven columns the `vehicle` group
adds, which is what re-running the Helex capture's tape and patches with the
pre-change group set shows: 25,676 rows, every shared column identical. Second,
the tape's *directory* is not part of the capture: with the tape copied to
another directory and replayed from there, with the same patches, groups and
ROM spelling, the log comes back byte-identical to the capture's
(`b4ed383d91ea7ea7895ffbf559fd766be3d1317a80a9619b2be379442f3dd349`) and so does
the trace (`build/lane-evidence/group_compare/`).

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
| `$53` one Desrt Leach | `forced_53_desrtleach.json` | **not exact**: first divergence at f25026, the vehicle's turn |

The Desrt Leach capture is the worklist this machinery exists to produce. Its
first divergence, as `divergences.json` carries it:

* **frame** f25026, **round** 1, **kind** `no-swing`, **action** actor 1 (the
  vehicle), f25026-25169;
* the log has the vehicle swinging: `loc_B6A2`'s pass runs in **three separate
  frames** (f25059, f25060, f25072, one call each) and then the sixteen
  `Battle_CalculateDamage` draws at f25098 land 165 on the Desrt Leach;
* the port has `TurnSkipped { reason: Unarmed }` and no swing at all, so round 1
  drew 31 of the log's 50 rolls.

Best-supported cause, with the code involved:

* The port refuses the swing at `Character_Attack`'s weapon check
  (`ps4.asm:13002-13019`): with neither hand holding a weapon, the cartridge
  takes the `move.w #$FFFF, $32(a4)` arm and skips `jsr loc_B6A2`, and
  `rust/psiv-core/src/battle/engine.rs` models that as `Skipped::Unarmed`.
* The vehicle's fighter is not that path. `loc_78EE` (`ps4.asm:11408`) seats it
  from `Vehicle_Stats` with a **character/attack-route id of its own**, and the
  three-hit shape is `CharAttack_Seth`'s: `AttackMappings_Seth`
  (`ps4.asm:13759-13761`) holds three mappings - `OneKnife`, `OneKnife`,
  `TwoKnives` - and the close-range weapon it hands over to runs one
  `loc_B6A2` pass per mapping, which is exactly three passes in three frames.
  `rust/psiv-core/src/vehicle.rs`'s `battle_member` gives the vehicle
  `character = 0x0B + index` (12 for the Land Rover, index 1) and
  `rust/psiv-core/src/battle/action.rs`'s `takes_second_hit_pass` gives every
  character but Alys and Kyra one pass, so the port's vehicle has neither the
  route nor the pass count the log shows. The two extra rolls shift the rest of
  the round's stream, which is why the round's draw count is 31 against 50.
* Fixing it (the vehicle's character id, its attack route and its pass count)
  is out of this brief's scope. It is recorded, not repaired, in
  `replay_fixtures/divergences.json` and here.

The fixture's own reading of the vehicle battle is in its `vehicle` section:
`Vehicle_Index` 1 (the selector's patch), the fighter's HP at the battle's first
frame (`Vehicle_Stats + curr_hp`, `$FFFF470E`, logged as
`vehicle_fighter_hp`), and the saved record behind it
(`Saved_Vehicle_Stats`, `$FFFFFA80`), whose own current HP **equals** the
fighter's at that frame - which is what makes its maximum (740) the battle
copy's, and what the fixture records rather than assumes. The members' HP
columns are the field's: nothing loads them in a vehicle battle, so the fixture
carries no party at all and the party side's HP column *is*
`vehicle_fighter_hp`.

## 4. What this does and does not prove

Proved, for the three captures above: the formation in RAM is the one asked
for, the trace is the cartridge's own arithmetic (the checker re-derives every
roll, the seed chain and the per-frame anchor), the run is deterministic
byte-for-byte across two runs of the same tape and patches, and the enemy's
ability dispatch wrote the ability id the capture exists for.

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
  says how the fight is going. The Desrt Leach capture is that battle.
- **Anything after the battle.** The tape is trimmed 60 frames past the battle's
  last in-battle frame; the log's provenance after that is the game-over or
  field sequence and is not evidence of anything.
- **Formations nothing can reach.** A formation whose only groups no map byte,
  no grid cell and no vehicle table can produce is refused with the groups
  named, rather than approximated.
