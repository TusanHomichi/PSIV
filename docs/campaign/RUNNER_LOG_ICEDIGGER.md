# Runner log: the Ice Digger arc, Zelan to the Air Castle (C7, H36 to H44)

Lane c7-icedigger (2026-10-04): the route past `Cutscene_DarkForce1Defeated`. It carries the
party from Zelan F1 with the Ice Digger through Meese (Raja falls sick), the carnivorous trees
(Kyra joins), the Esper Mansion (Rune is Lutz), the Gumbious Temple (the Eclipse Torch is stolen)
and the flight to the Air Castle, and stops in the Xe-A-Thoul room, one step short of the first
fixed battle the engine cannot play. The earlier arc is [RUNNER_LOG_KURAN.md](RUNNER_LOG_KURAN.md);
the index and current state are in the [runner log](RUNNER_LOG.md).

**The milestone the brief named was not reached, and could not be.** Event `$40`/`$41` (Juza) was
played in the Zio Fort long before this arc, so the next boss scene the gates open is
`Cutscene_DarkForce2Defeated` (`$8017`). Between here and it stand three fixed battles whose
enemies use abilities the engine does not run ([H44](#h44-the-xe-a-thoul-fight-needs-gizan-and-thndrblast)),
all in #88's lane.

## C7 runs

Base `20f22e6` (`claude/campaign-23`: main plus the X86 oracle lane), release builds,
`CARGO_BUILD_JOBS=2`, one heavy command at a time. The pack is the owner's `runtime-pack`
(manifest SHA-256 `02bb29ad…11f2`), read through a symlink and never written; no `psiv_tools`
change, so no pack rebuild. Evidence is under the ignored `build/` of the lane's worktree
(`c7-base`, `c7-route`, `c7-route-b`, `c7-w`, `s/` for the scratch drivers); the commands
regenerate it. A "snapshot clock" run (`--from-chapter`) reloads the previous chapter's snapshot
and restarts the frame clock and RNG: it is the same game on other dice, and only a full run is
evidence for the route.

| Run | Command (from the worktree) | Result |
| --- | --- | --- |
| C7-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c7-base …` on the base's 43 chapters | exit 0, 3,700,582 frames, digest `fbe9fef7015aba2e`, tape SHA-256 `969ba6dc…4996`, the `42-…` snapshot `f7b74e4f…34c2`: the base's own numbers |
| C7-1 | scratch chapters from `42-kuran-dark-force-1`: the elevator, Zelan, `board` to Dezolis, `use_item` ICE-DIGGER | halt `unexpected_state`, "the camp is not open": H36 |
| C7-2 | `go_to (186,13)` on the trees' corridor | the walk fired the scene, the scene left the party on (186,15), the plan went up again: 14 trees battles, `lost_battle`: H37 |
| C7-3 | `talk` the Esper door guard | halt `scene_fault`, "dialogue event 0x4f has no transcribed scene": H39; then the guards walked off the map's edge and the planner saw the door shut: H40 |
| C7-4 | `talk` the Gumbious monk (dialogue 36) | halt `scene_fault`, "dialogue event 0x46 has no transcribed scene": H41 |
| C7-5 | `go_to_map 212`, the Dezo spaceport | halt `unexpected_state`, "the ship's destination menu is open and no objective asked": H42 |
| C7-6 | `go_to_map 388` through the Air Castle, three first tries | halts `unsupported_ability` (58 and 45) in random battles: H38; with a pause first, `WarpUnmapped (34,43)`: H43 |
| C7-F1, F2 | `run rust/psiv-campaign/routes/main.json --save-dir build/c7-route{,-b} --tape …/run.tape --report …/report.json` (51 chapters, twice from New Game) | both exit 0, **3,754,623 frames**, digest `641d58ecd6645729`, tape SHA-256 `5aa1f9d2499cdffa7e7a438dab5b9704f3f22cad516b472357e0cac1db4206a8` (`cmp` says the two tapes are identical; the two `50-…` snapshots are `ae614f13…1b`); the pad SAVE `route/slot_1.sram` is `4aeff0b1…ec62`, unchanged; the route file is `9c73aa08…0a7f`, the binary `e786300b…db25` |
| C7-R | `psiv-campaign replay build/c7-route/run.tape` | digest `641d58ecd6645729`, 3,754,623 frames |
| C7-P | the probe: `step_onto (31,35)` of the Xe-A-Thoul room from the last snapshot, then `fight_scripted` | the scene runs, event battle 14 starts, halt `unsupported_ability` 92 (`$5C` THNDRBLAST), fighter 8, round 1: H44 |

**The 43-chapter prefix is untouched.** The final tape's first 3,700,582 pads are the base's
(`build/s/tapeprefix.py`: every frame compared, identical), and the 43 chapter snapshots are
byte-identical to the base's (`build/s/prefix.py`). The bespoke-pattern fix (H40) and the planner and
runner changes therefore moved no NPC and no step on any map those chapters cross.

Chapter frames and what each ends on (SHA-256 of its snapshot, abbreviated):

| Chapter | Frames | Battles | Ends |
| --- | ---: | ---: | --- |
| `dezolis-ice-digger` | 2,443 | 0 | Dezolis (10,73), Ice Digger mounted, Raja still in the party; `2f3f7d9c…` |
| `meese-raja-sick` | 4,599 | 0 | Meese Clinic F1 (26,29), four in the party, `$94`; `b77962bf…` |
| `dezolis-saving-kyra` | 7,689 | 1 | Dezolis (186,15), mounted, Kyra in slot 5, `$95` `$A0`; `ad4ab33c…` |
| `esper-mansion` | 2,972 | 1 | the Courtyard (31,16), `$96`; `aa2ae0f2…` |
| `esper-inner-sanctuary` | 9,452 | 0 | Inner Sanctuary B1 (30,37), `$97`; `7107ed72…` |
| `gumbious-torch-stolen` | 9,615 | 1 | Gumbious F1 (39,23), `$98`; `185e35eb…` |
| `air-castle-arrival` | 5,976 | 3 | the Air Castle (63,54), `World_Index` 5, `$98 $99 $9F`; `06feecfa…` |
| `air-castle-xe-athoul-room` | 11,295 | 16 | the Xe-A-Thoul room (31,51), `$9A` clear; `ae614f13…` |

## The halts

| Halt | Class | What it was | Fix |
| --- | --- | --- | --- |
| H36 | runner | `use_item` of a boarding item | `camping.rs` waits the boarding scene out |
| H37 | runner objective | a trigger on plain ground | `step_onto` takes any trigger cell |
| H38 | #88, random battles | the Air Castle's encounters use abilities the engine does not run | none here; the pause that re-rolls them, recorded |
| H39 | scene, class #83 | `Event_EsperGuardPermission` (`$4F`) | scene, `SetNpcDialogue` op, census entry |
| H40 | runtime, planner | the Esper door guards never leave the door | latched patterns, per-guard table, vacated cells, `wait`, `talk` `opens` |
| H41 | scene, class #83 | `Event_EclipseTorchStolen` (`$46`) | one scene, a census entry |
| H42 | runner | a scene opens the ship's menu on arrival | `board` without a step, `settle_at` |
| H43 | runner | a doorway row whose warp is one row on | the unmapped report is no fault beside a warp |
| H44 | #88, fixed battle | the Xe-A-Thoul fight | none: the route stops at its door |

### H36: `use_item` assumed the camp stays open

`ItemAction_IceDigger` (`ps4.asm:123436-123448`) accepts on plain ground, destroys the open windows
(`loc_5B7E4`, `:122596-122600`) and writes `Event_Index` `$0A`; the field then runs
`Event_BoardingIceDigger` with no menu over it. The controller read the camp page after the
Speak press and halted on a camp that was already gone. `use_item` now returns the scene's
wait when the press closed the menu on a started scene. **Tests and negative controls:**
`tests/esper_arc.rs` `the_ice_digger_boards_from_the_item_menu_on_plain_ground_only` (Vehicle_Index 2 on plain
ground, 0 on the spaceport door tile where the action is refused); with the branch switched off it
fails `the camp is not open`.

### H37: a `go_to` onto a trigger cell fires the trigger again

`RunEvent_CarnivorousTrees` (`$35`, `ps4.asm:115977`) fires at x `$BA0..$BB0`, y `<= $C0`: the
corridor (186..187,12..13) under the Garuberk Tower's warp. With Raja sick it is
`Event_SavingKyra` (`$4D`), then `Cutscene_MeetingKyra` leaves the party on (186,15) with Kyra and
the machine; a `go_to (186,13)` planned again from there and fired `Event_CarnivorousTrees`
(entry 52, "It won't work", event battle 10) fourteen times until the party fell. `step_onto`,
which the Landale row introduced for a warp footprint, now also takes any cell a walk reaches
(`Flood::onto_plan` falls back to the plain walk) and halts when the party stands on the cell and
no scene ran. **Tests and negative controls:** `esper_arc.rs`
`step_onto_a_plain_trigger_cell_fires_the_trees_arm_the_flags_pick` (`$94` set: `$95` and battle 10;
`$94` clear: battle 10 and no flag; (186,16): "no scene ran"), `kuran_arc.rs` and
`validator.rs` `step_onto_names_a_reachable_cell_on_the_map_the_party_is_on` (a wall is
rejected); with the fallback removed the first fails `Unreachable`.

### H38: random battles on the Air Castle's walk use abilities the engine does not run

Not fixed: `battle/**` is the p88 lane's. The walk from the landing to the Xe-A-Thoul room crosses
`AirCastle_Part2`, `Part3`, `Part6`, `F1` and `F1_Part2` (52 foot groups each; the sweep below) and
meets 3 to 17 random battles. The policy runs from them, and an enemy that acts before the
party's RUN resolves (round 1, 110 frames after the battle begins) can use one of these:

| Ability | Carrier | Run |
| --- | --- | --- |
| `$3A` COMBINE (unsupported) | BladeRight (84), formation 373 or 374 | scratch run 1: halt, fighter 8 (formation 374) |
| `$2D` DEBAN (unsupported) | FrostSaber (71) | scratch run 2 (`attack_all`): halt, fighter 7 |
| `$31` GRA (unsupported) | DimensWorm (73), formation 372 on `$173` | the full route, first try: halt, fighter 7, round 1 |
| `$3A` COMBINE | formation 373 on `$173` | the full route with a 1-frame pause: halt, fighter 6, round 1 |

`python3 -m oracle.sweep.route_abilities --map-pattern 'AirCastle_Part3$|AirCastle_Part6$|AirCastle_F1_Part2$|AirCastle_F1$|AirCastle_Part7$|AirCastle_Part2$'`
lists, for the maps reached: `$2C` AIRSLASH, `$2D` DEBAN, `$30` DISTORTION, `$31` GRA, `$3A`
COMBINE and `$4E` DTHSPELL unsupported, `$4C` EVIL EYE partial, FIREBREATH, GIWAT and CORRSION
implemented. A player meets the same abilities; the engine just halts on them.

The full route's first try halted this way, so `air-castle-xe-athoul-room` opens with
`{"do": "wait", "frames": 7}`: a pause on the landing that re-rolls the walk's encounters. With 1
frame it halts (COMBINE), with 7 and 13 it completes (16 and 17 battles). **This is the route's
dice, not the game's, and it is fragile in the way the C6 log notes for the elevator chapter: any
earlier chapter's frame count changes it. It should go when #88 lands those abilities.** The
orchestrator may prefer to end the route at `air-castle-arrival` instead; nothing after the pause
depends on it.

### H39: `Event_EsperGuardPermission` and `SetNpcDialogue`

The first guard at the Esper Mansion's door is a `$F6 $4F` line (tree 21 entry 51), allowlisted
under #83. Transcribed from `$0709A2..$0709CD` ([99](../scenes/99_EsperGuardPermission.md); every
instruction word matches `ps4.asm:150070-150113`): both guards turn to the leader, Dark Force 2 or
the trees pick entry 2 or 1 (else entry 0 and the event ends), both guards' dialogue ids are
rewritten (`$14(a4)` and `$54(a4)` off `Field_Obj_Secondary`) and temp flag `$1A` is set. That needed
**one new scene op**, `SetNpcDialogue` (`SceneOp`, `SceneEffect::NpcDialogueSet`, the runtime's
dialogue overrides), the first scene write of a field object's dialogue id.
`Event_PersistentEsperGuards` (`$45`, `$06FE1C..$06FE55`, [100](../scenes/100_PersistentEsperGuards.md))
is the inner pair's twin: tree 20's entry `$2E`, both ids `$23`, `$96`. The census allowlist shrank
by two (#83: 19 to 17) and `ALLOWLIST_CEILING` went 84 to 82. **Tests and negative controls:**
`rust/psiv-core/tests/late_dezolis_scenes.rs` (the three flag arms, the facings, the inner guards),
`esper_arc.rs` `the_door_guards_step_aside_for_the_party_that_fought_the_trees` (`npc_dialogue_id`
reads 1; with the write changed to `+100` it reads 101 and fails); the registry's op-count and
tree-load censuses carry both scenes.

### H40: the Esper door guards walk away forever, and the planner never sees them go

Two port defects and two route gaps, found in that order.

1. **Looping tables.** The three guard routines end their `$FF`-terminated tables differently from
   the looping NPCs: they store the index of the `$FF` itself (`move.w #2 / #3, $1C(a4)`,
   `loc_4A0A4`, `loc_4A0F4`, `loc_4A022`, ROM `$04A0A4..`) and idle for good, where `loc_49FCE` clears the
   cursor. The port's `next_pattern` looped them: both guards walked left until the leash stopped
   them, eight cells and more. The tables also differ per guard (`03 02 FF` at `$04A0EC` for the first
   guard, `04 02 FF` at `$04A0F0` when the object before it has the guard's id word,
   `cmpi.w #$8268 / #$826C, -$40(a4)`), where the port gave both the first. `bespoke.rs` has a
   `LATCH` terminator beside `IDLE`; `bridge.rs` picks `PATTERN_ESPER_GUARD_SECOND` for a guard
   that follows another. The Musk cat guard's table `03 03 02 FF` (`loc_4A05A`, `$04A05A`) latches the same
   way and is fixed with it ([NPC_WANDER](../field/NPC_WANDER.md)). **No other NPC changes:**
   `next_pattern` is shared by the looping tables, the Type-11 sequences and these three, and only
   these three now contain `LATCH`; the 43-chapter prefix of the route is byte-identical.
2. **The planner reads the pack, not the game.** The warp graph is built from the records, where a
   guard stands on its spawn cell for ever. `Driver::first_leg` now opens the walkable spawn cells of
   objects the game has moved off (`vacated_spawn_cells`) before it plans the chain.
3. **The validator does not know a conversation clears a door.** `talk` takes `opens`, as `interact`
   does, and a new `wait` objective (1 to 600 frames) lets the guards finish walking before the
   next walk is planned around them.
4. **The inner door is entered from the north.** The mansion hall's two corridors meet the courtyard
   door (47..48,20) from above; the west warp (5) lands in a corridor with no way to it, so the route
   names `via_warp 6`.

**Tests and negative controls:** `bespoke_tests.rs`
`the_esper_door_guards_step_aside_once_and_stay` (left once, right once, then 6,000 more frames
without a move), `the_musk_cat_guard_steps_aside_twice_and_stays`,
`a_looping_pattern_restarts_on_its_terminator` (the latch is the guards' alone); with `LATCH`
turned back into `IDLE` the first two fail (the guard ends at x 0). `esper_arc.rs` walks the door
(planner fix off: `Unreachable`, "3 positions explored"; the other half of the test, with no trees
fought, keeps the door shut and the flag clear), and `validator.rs`
`the_esper_chapter_rejects_a_door_the_talk_does_not_open` and
`wait_is_bounded_and_a_scene_menu_board_validates`.

### H41: `Event_EclipseTorchStolen`

The Gumbious monk (object 6 at (39,22), dialogue 36) answers with entry 36, whose preamble is
`$FA $98 +1 / $FA $97 +17`: with `$97` set and `$98` clear it jumps 17 entries on, to entry 53,
`$F6 $46`. The body is 820 bytes (`$06FE56..$070189`, [101](../scenes/101_EclipseTorchStolen.md)): tree 37's
entry `$32` and three resumes, ten teleporting figures (typed `CreateFieldObject` and
`SetObjectDestination` records; their step constants are not modelled, as for the other Dezolis
presentation scenes), the torch's two objects cleared, the camera returned, music changes and
`$98`. Tree loads: two. Allowlist #83 17 to 16, ceiling 81. **Tests:**
`late_dezolis_scenes.rs` (the three resumes and the despawn order, `$98` as the last write, the
facings); the registry census counts 58 ops and two tree loads.

### H42: a scene opens the ship's menu when the party arrives

`RunEvent_FindingAirCastle` (`$42`) fires on the Dezo spaceport's first frame once `$98` is set and
`$99` clear, and `Cutscene_FindingAirCastle` ends in the inlined spaceship menu. `go_to_map` ended
with the menu open as a halt ("no objective asked"). `Driver::settle_at(stop_at_choice, stop_at_menu)`
lets `go_to_map` stop there as it does for a prompt, and `board` without a `step` waits for a menu a
scene opens (`board_from_scene`), pages the scene's dialogue with Speak and flies. The Air Castle
is the last row of the `$DC` mask, `World_Index` 5. **Tests and negative controls:**
`esper_arc.rs` `a_scene_opens_the_air_castle_menu_and_board_answers_it` (without `$98` the arrival
opens nothing and `board_from_scene` halts `unexpected_state`; with `settle_at` switched off the
arrival halts `the ship's destination menu is open`).

### H43: a doorway row whose warp is one row on

`AirCastle_F1` (`$181`) warp 0 is a normal-ground record on row 44 with the doorway row above it
(33..34,43) in collision type 1 and no map-change entry. On the type-1 cell `RunMapTransitions`
reads the map-change table, finds nothing, and the walk goes on to row 44, which fires. The runtime
reported `WarpUnmapped (34,43)` and the runner's guard (H24: an unmapped type-1 cell halts unless the
frame began a scene) stopped a walk that was doing what the cartridge's data asks. The guard stays
for a cell no warp borders; one beside a warp footprint is no fault (`is_scene_fault` takes a third
argument, `Driver::is_doorway_row`). The same row type appears at `AirCastle_Part6`'s (34,51).
**Tests and negative controls:** `driver.rs` `an_unmapped_doorway_row_beside_its_warp_is_no_fault` and the
two existing H24 tests; `esper_arc.rs` `a_doorway_row_above_its_warp_is_walked_through` (with the
argument forced to false it halts `SceneFault WarpUnmapped`).

### H44: the Xe-A-Thoul fight needs GIZAN and THNDRBLAST

`RunEvent_FindXeAThoul` (`$44`, `Event_XeAThoulBeforeBattle` `$59`) fires inside the room (x `$1D0..$220`,
y `<= $220`; the probe steps onto (31,35)), runs dialogue entry 54 and starts event battle 14
(enemy 123, three XeAThouls). The route stops at its door, in `air-castle-xe-athoul-room`.

| Probe | Result |
| --- | --- |
| C7-P, event battle 14, round 1 (Chaz TECH, Rika ATTACK, Rune TECH, Kyra TECH; Wren has no command) | `unsupported_ability` 92 (`$5C` THNDRBLAST), fighter 8, 598 frames after the battle began |

The sweep for the room and the other two fixed battles on the way to `$8017`
(`python3 -m oracle.sweep.route_abilities --map-pattern 'AirCastleXeAThoulRoom|AirCastleInner_B1_Part3' --scene-doc docs/scenes/70_XeAThoulBeforeBattle.md`, and `72_…`, `57_…`):

| Event battle | Enemy | Unsupported abilities |
| --- | --- | --- |
| 14 (Xe-A-Thoul) | 123 XeAThoul | `$35` GIZAN, `$5C` THNDRBLAST (conditional) |
| 16 (Lashiec) | 128 Lashiec | `$5F` THNDHALBRT, `$60` POSESSION, `$61` ANOTHRGATE, `$62` REINFORCE (conditional) |
| 17 (Dark Force 2) | 131 DarkForce2 | `$64` SHDWBREATH, `$65` LIGHTSHOWR; `$4C` EVIL EYE partial |

A fixed battle cannot be run from. Everything after this point belongs to #88 and the ability
lanes; the route resumes at `Event_XeAThoulBeforeBattle` when they land.

Unrelated to the walk: the scene docs 69 to 74 named `dezo_campaign.rs` for scenes that live in
`dezo_campaign_late.rs`, which made `--scene-doc` fail with a `KeyError` on them; they are
corrected. `route_abilities.py` also raised `KeyError: 'groups_available'` on a pattern that matched a
map outside the encounter table (`'^AirCastle'` reaches `AirCastleSpace`, mode `outside_table`);
such a map draws nothing now (see the widenings below).

The arc's own scope for the next enemy-ability lane is a named stretch,
`python3 -m oracle.sweep.route_abilities --stretch dezolis-air-castle`: the route's chapters
`dezolis-ice-digger` to `air-castle-xe-athoul-room`, the Air Castle walk's maps, and event battles
10 (the trees) and 14 (Xe-A-Thoul). Its **unsupported** abilities: `$2C` AIRSLASH, `$2D` DEBAN,
`$30` DISTORTION, `$31` GRA, `$35` GIZAN, `$3A` COMBINE, `$4E` DTHSPELL, `$5C` THNDRBLAST;
**partial**: `$22` RAY BREATH (the Ice Digger's group on Dezolis) and `$4C` EVIL EYE.

## Balance and levels

No balance loss: no battle of the arc was lost to the party's strength (the one loss, H37, was a
trigger fired fourteen times). The party crosses the arc at Chaz L43, Rika L46, Rune L39, Wren L38
and Kyra L27 (her level when she joins, `Cutscene_MeetingKyra`), with the HP the camp cure after
each battle leaves (`recovery.rs`); the Air Castle's poison floors (`FieldPoisonFlash`) cost HP
the cure covers. No chapter trains, and no battle rule or stat changed.

## Negative controls (the checks that changed)

| Break | Guard | Result |
| --- | --- | --- |
| the boarding branch of `use_item` off | `esper_arc.rs` the Ice Digger test | fails, "the camp is not open" |
| the plain-cell fallback of `onto_plan` off | the trees test | fails, `Unreachable` |
| `vacated_spawn_cells` off | the door-guard test | fails, `Unreachable`, "3 positions explored" |
| `dialogue_id + 100` in the runtime handler | the door-guard test | fails, `Some(101)` against `Some(1)` |
| `settle_at(true, false)` in `go_to_map` | the Air Castle menu test | fails, "destination menu is open" |
| `is_doorway_row` forced false | the doorway test | fails, `SceneFault WarpUnmapped (34,43)` |
| `LATCH` back to `IDLE` | the two guard tests | fail, the guard ends at x 0 |
| a wall for `step_onto`, a 601-frame `wait`, a shut `opens` | `validator.rs` | each rejected in its chapter |

## Checks

See the receipt of the lane's commit; the commands, counts and exit codes are recorded there and
in the integration ledger.

## Widenings of the write set (approved by the orchestrator, 2026-10-04)

Three files outside the brief's list changed, each because the brief's own scope needed it:
`rust/psiv-core/src/scene_types.rs` and `rust/psiv-core/src/scene_runner/ops.rs` (the new
`SetNpcDialogue` op, H39), `rust/psiv-core/src/bespoke.rs` (the latched guard patterns, H40) and
with it `rust/psiv-core/src/lib.rs` (one re-export, `PATTERN_ESPER_GUARD_SECOND`) and
`rust/psiv-runtime/src/bridge.rs` (the per-guard table, inside `psiv-runtime/src/**`).
Also outside the list and touched: `tests/event_census.py` and `rust/psiv-core/tests/event_census.rs`
(the allowlist, under `tests/**` and `rust/psiv-core/tests/**`).

A fourth, approved after the lane's first full test run failed on it: `oracle/sweep/route_abilities.py`.
The `zelan-kuran` stretch ran to the route's *last* chapter, so the eight new chapters widened what
`docs/battle/ENEMY_ABILITIES_ROUTE.md` derives from; `Stretch` now names its `last_chapter`
(`kuran-dark-force-1`) and the doc block is byte-identical. The same change makes a map outside the
encounter table draw nothing (the `KeyError`), lets a stretch name its scenes' event battles
(`extra_battles`) and adds the `dezolis-air-castle` stretch above. Tests:
`tests/test_route_abilities.py` (an appended chapter does not widen the stretch, with a stretch that
has no named end as the control; the C7 stretch's chapters and battles; an `outside_table` map and an
unknown mode as the control). P88 owns the rest of `oracle/sweep/**`.

## Not claimed

- **The Air Castle walk's pause.** `wait 7` is a re-roll of random battles whose abilities the engine
  does not run ([H38](#h38-random-battles-on-the-air-castles-walk-use-abilities-the-engine-does-not-run)).
- **The thieves' choreography.** Ten object records, their step constants and the sprite art are not
  modelled; the scene's frames, dialogue, despawn and flag are.
- **Native play.** Headless `Session`, pads only. No Godot capture of Meese, the trees, the
  Esper Mansion, the Gumbious Temple or the Air Castle, and no tape replay in Godot. The Godot-visible
  changes are `SceneOp::SetNpcDialogue` / `SceneEffect::NpcDialogueSet` (Godot ignores the effect; it reads
  `Runtime::npc_dialogue_id`), the guard routes (`bespoke.rs`: guards now stop one cell aside) and the
  ten scene objects of `$46`.
- **Event battle 10's meaning.** The carnivorous trees' battle (enemy and rewards: 1 or 0
  experience) is won; its rows are in the report, and nothing was compared with the cartridge.
