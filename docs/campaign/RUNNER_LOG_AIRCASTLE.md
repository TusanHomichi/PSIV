# Runner log: the Air Castle to Dark Force 2 (C8, H47 to H50)

Lane C8 (2026-10-07): the route past `air-castle-arrival`. It restores the Xe-A-Thoul walk recorded
in [H38](RUNNER_LOG_ICEDIGGER.md#h38-the-air-castles-walk-meets-abilities-the-engine-does-not-run)
(without its `wait`), fights Xe-A-Thoul, goes back to Dezolis for arms and armour, trains in the
castle, opens the Eclipse Torch's chest (the Spector, Lashiec and `Cutscene_LashiecDefeated` in one
run of the field), sleeps at Jut, burns the carnivorous trees with the torch, climbs the Garuberk
Tower through its doors and eyes and defeats Dark Force 2. The milestone the brief named,
`Cutscene_DarkForce2Defeated` with control returned, is reached. The earlier arc is
[RUNNER_LOG_ICEDIGGER.md](RUNNER_LOG_ICEDIGGER.md); the index and current state are in the
[runner log](RUNNER_LOG.md).

## C8 runs

Base `d501aee` (`main`), release builds, `CARGO_BUILD_JOBS=2`, one heavy command at a time. Evidence
is under the ignored `build/` of the lane's worktree (`c8-base`, `c8-w*`, `c8-route`, `c8-route-b`);
the commands regenerate it. A full run takes about 45 s.

**The pack.** The Garuberk doors need their chunks in the tower maps' scene chunk atlas
([H50](#h50-the-garuberk-tower-doors-eyes-and-their-layouts)), which is a `psiv_tools` change, so the
route past the torch needs a rebuilt pack. The lane built it into the ignored `build/c8-pack`
(`python3 -m psiv_tools pack "Phantasy Star IV (USA).md" build/c8-pack`, 225 s, manifest SHA-256
`d46ae121…850e6c`) and never wrote the owner's `runtime-pack` (accepted c27 pack, manifest
`4964cc08…dbff7e`). Against the accepted pack it differs in exactly the seven tower map records,
their seven new `_patch.png` atlases (five with `_patch_indices.png`) and the manifest; every other
file of the 5,082 is byte-identical. With the accepted pack the route stops at the tower's first
door (`scene chunk atlas is absent`), which is the pack being stale, not a route fault: the
orchestrator rebuilds the pack at integration.

| Run | Command (from the worktree) | Result |
| --- | --- | --- |
| C8-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c8-base …` on the base's 50 chapters | exit 0, 3,852,511 frames, digest `29c81ae27664b15b`, tape SHA-256 `a66c5830…0131d`: the base's own numbers |
| C8-1 | with `air-castle-xe-athoul-room` restored (no `wait`) and the fight appended | the restored chapter completes (10,584 frames, 12 battles); event battle 14 is lost: [H47](#h47-xe-a-thoul-and-a-party-that-carried-a-shut-down-android) |
| C8-2 | with `Event_Recovery` transcribed and a recovery chapter | Xe-A-Thoul won in 5 rounds |
| C8-3 | the inner floors and the chest | the Spector falls in one round; Lashiec (event battle 16) wipes the party: [H48](#h48-lashiec-is-a-balance-loss) |
| C8-4, C8-5 | the boss policy taught every skill #88 made real | the 50-chapter prefix moves from `zio-fort-juza` (frame 1,074,986) and Lashiec still wins (2,339 HP left); reverted, recorded in H48 |
| C8-6 | `jut-outfit`, the original policy | Lashiec wins with 1,410 HP left |
| C8-8 | `air-castle-training` to level 35 | 423,358 frames, 212 battles; Lashiec wins with 385 HP left |
| C8-9 | training to level 37 | 554,799 frames, 283 battles; Lashiec falls in 10 rounds |
| C8-10 | `jut-inn` and `dezolis-eclipse-torch` | the torch scene leaves the party at Jut's door, (120,79): [H49](#h49-the-torch-scenes-dismount-left-the-party-where-it-boarded) |
| C8-12 | the tower on `build/c8-pack` | ten doors opened, ten doors entered, both eyes, Part7 reached |
| C8-13 | `garuberk-dark-force-2` | Dark Force 2 falls in 12 rounds; `Cutscene_DarkForce2Defeated` runs and the party stands on Dezolis (186,8) |
| C8-F1, F2 | see [the final runs](#the-final-runs) | |

**The 50-chapter prefix is untouched.** `psiv-campaign prefix-check build/c8-base/run.tape
build/c8-base/report.json <run>/run.tape <run>` (new in this lane, [CAMPAIGN_RUNNER.md](CAMPAIGN_RUNNER.md#the-runner))
reports every pad of the base's 3,852,511 frames and all 50 chapter saves identical for every run
from C8-1 on, except the reverted C8-4 and C8-5 experiments.

Chapter frames and what each ends on:

| Chapter | Frames | Battles | Ends |
| --- | ---: | ---: | --- |
| `air-castle-xe-athoul-room` | 10,584 | 12 | the Xe-A-Thoul room (31,51), `$9A` clear |
| `air-castle-recovery-tile` | 3,086 | 2 | `AirCastle_F1_Part9` (31,28), every status byte zero |
| `air-castle-xe-athoul` | 8,454 | 4 | the room's own tile (31,19), `$9A` set |
| `jut-outfit` | 12,416 | 9 | the Air Castle's landing (63,54) in the Jut gear |
| `air-castle-training` | 554,799 | 283 | `AirCastle_F1_Part9` (31,25); Chaz L48, Rika L52, Rune L44, Wren L43, Kyra L37 |
| `air-castle-inner` | 6,192 | 9 | `AirCastleInner_B1_Part3` (30,50) |
| `air-castle-lashiec` | 11,855 | 2 | `Gumbious_F1` (39,23) with the Eclipse Torch, `$A6` `$9B` |
| `jut-inn` | 894 | 0 | the Jut inn (28,36), every status byte zero |
| `dezolis-eclipse-torch` | 1,563 | 1 | Dezolis (186,15) on foot, `$9C` |
| `garuberk-tower-first-eye` | 5,086 | 7 | `GaruberkTower_Part5` (24,76), temp `$14` |
| `garuberk-tower-second-eye` | 4,754 | 9 | `GaruberkTower_Part7` (38,36), temp `$16` |
| `garuberk-dark-force-2` | 10,873 | 1 | Dezolis (186,8), `$9E` `$A1`, Kyra gone |

## The halts

| Halt | Class | What it was | Fix |
| --- | --- | --- | --- |
| H47 | allowlisted event (#83) | Xe-A-Thoul lost; Wren shut down since Dark Force 1 | `Event_Recovery` (`$21`) transcribed; the route takes the castle's recovery tiles |
| H48 | balance loss | Lashiec (event battle 16) | the Jut arms and armour, then training to level 37 in the castle |
| H49 | runtime | a scene's dismount left the party where it boarded | both dismounts stand the party on the machine |
| H50 | allowlisted events (#56, #83), pack, runtime | the Garuberk Tower's doors and eyes | six scenes, one new scene op, the door atlas, the layout-pair fix |

### H47: Xe-A-Thoul, and a party that carried a shut-down android

The restored walk completes from New Game with no `wait` (C8-1): lane A5's abilities and its
COMBINE correction hold on the stream the full route plays. Event battle 14 (three XeAThouls,
1,520 HP each, `$5C` THNDRBLAST while all three stand) was then lost in two rounds with Wren
giving no command. Her status byte was `$40`, `StatusAndroidDead`, at 384 of 384 HP: she fell at
Dark Force 1 (chapter 42) and the step regen androids get (`DoCharStatsUpdate`) refills HP without
touching the bit, as the cartridge does (`rust/psiv-core/src/field_status.rs`). Nothing on the route
since then repairs an android. That is a route gap, not a defect.

The cartridge's own answer sits on the way: `Event_Recovery` (`$21`), the recovery tile, on
`AirCastle_F1_Part9` (31..32,28..29) and behind Xe-A-Thoul in his room (31..32,18..19). It was
allowlisted under #83. Transcribed from `$06D37C..$06D467` ([102](../scenes/102_Recovery.md)): a
palette save, the recovery and Res SFX around six sixteen-frame flash loops, then `RecoverStats`,
which zeroes every status byte. The census shrinks by one (80); the flash is a new presentation
record (`PaletteToneFlash`). With the tile taken before the fight Xe-A-Thoul falls in 5 rounds
(Wren ends on 5 HP), and the route takes the room's own tile after it.

**Tests and negative controls:** `rust/psiv-core/tests/air_castle_scenes.rs`
`the_recovery_tile_restores_the_party_after_ninety_six_frames` (the SFX at frames 0 and 96, the six
flashes, every party record's HP, TP, skill uses and status, an out-of-party record untouched); with
`RecoverStats` replaced by a zero-frame wait it fails, no `RosterChanged`. The registry op-count and
the census (`event_census.rs`, `tests/test_event_coverage.py`) carry the scene.

### H48: Lashiec is a balance loss

Event battle 16 is one Lashiec (enemy 128), 6,383 HP in the cartridge capture
(`rust/psiv-core/src/battle/replay_fixtures/air_castle/lashiec.json`), acting once a round with
THNDHALBRT (attack 228 against defence, electric; Wren, an android, takes about 220) and ANOTHRGATE
(mental against magic defence, all party). A round costs the party 800 to 900 HP. The
`run_then_win` policy already uses his weakness to fire (Rune's and Kyra's NAFOI, 18 casts in the
winning fight). The trials, every one from New Game:

| Trial | Party at the chest | Lashiec when the party fell |
| --- | --- | --- |
| Kuran gear, full party (C8-3) | Chaz L43, Rika L46, Rune L39, Wren L38, Kyra L27 | 3,240 |
| the skill-aware policy below (C8-5) | the same | 2,339 |
| the Jut gear (C8-6) | the same | 1,410 |
| Jut gear, trained to 35 (C8-8) | Chaz L47, Rika L50, Rune L43, Wren L42, Kyra L35 | 385 |
| Jut gear, trained to 37 (C8-9 and the final runs) | Chaz L48, Rika L52, Rune L44, Wren L43, Kyra L37 | won in 10 rounds |

1. **Gear (`jut-outfit`).** The party wore Kuran-era arms and armour and carried 661,036 meseta. The
   chapter flies back to Dezolis from the castle's landing (`RunEvent_AirCstlEnterSpaceship`, y
   `$370`, cell row 56) and buys at Jut: LACO-SWORD for Chaz (attack 103 against 43), two LACO-CLAWs
   for Rika (51 each against 18), RFLC-MAIL for both (defence 53 and magic defence 14 against 49 and
   4) and RFLC-ROBE for Rune (58 and 20 against 49 and 8).
2. **Training (`air-castle-training`).** A patrol across `AirCastle_F1_Part9`'s recovery tile, so
   `RecoverStats` refills the party on every pass and no inn trip is needed, until every member is
   level 37: 554,799 frames and 283 battles, on both clocks. Kyra, who joined at 27, is the member it
   waits for. The Dezolis training before Dark Force 1 was 1,031,235 frames.
3. **Not done: the skill-aware boss policy.** `policy_boss.rs` still considers only Crosscut and
   Vortex among skills ("the two the engine runs", true before #88) and counts Crosscut twice, where
   the engine resolves one damage pass. Taught every supported damage skill with the engine's
   arithmetic (C8-4), it changes boss fights from Juza on, so the 50-chapter prefix moves, and it did
   not win Lashiec on its own (2,339 left, behind the gear-only trial). It was reverted to keep the
   prefix. The stale table is an open finding for the orchestrator: fixing it is right, and it costs a
   re-derivation of the whole route's evidence.

Dark Force 2 (event battle 17, 7,524 HP) then falls in 12 rounds at the same levels, with no further
training. No battle rule or stat changed.

### H49: the torch scene's dismount left the party where it boarded

`Event_EclipseTorchUsed` runs mounted (the route rides the Ice Digger into the trees' corridor), and
its mounted branch rebuilds every character object on `Character_1`, the machine's body, before it
clears `Vehicle_Index` (`loc_701DE`, `ps4.asm:149559-149577`); `Event_GettingOffVehicle` does the
same (`loc_6BF4E`, `:145362-145380`). The port's `set_vehicle_index(0)` dropped the machine and left
the party objects where the party had boarded, so the scene ended with the party at Jut's door
(120,79), 66 cells from the tower. Fixed as a class: a dismount from a mounted state stacks the party,
and a running scene's party actors (`SceneRunner::park_party`), on the machine's cell and facing
([VEHICLES.md](../field/VEHICLES.md#mount-and-dismount)). `Event_SavingKyra` in the prefix dismounts
the same way; the prefix is byte-identical with the change.

**Tests and negative controls:** `rust/psiv-runtime/src/field_entry_tests.rs`
`a_dismount_stands_the_party_on_the_machine` (the Land Rover driven off its boarding cell, then
dismounted: leader and followers on the machine's cell and facing); with the placement removed it
fails, the leader still on (114,177). The route's `dezolis-eclipse-torch` expects (186,15) on foot.

### H50: the Garuberk Tower, doors, eyes and their layouts

Every tower warp but the way out stands on a door, with a type-2 interaction area on its cell.
All six tower events were allowlisted (`$35`, `$36`, `$39`, `$3A` under #83; `$37`, `$38` under
#56), and the coverage census listed the doors as needing new ops. They did not: the door scenes are
the elevator's shape ([103](../scenes/103_GaruberkTowerDoors.md)). What the tower needed:

1. **Six scenes** (`rust/psiv-core/src/scenes/garuberk_events.rs`): the two door openings step their
   ROM tables at the leader (`WriteActorMapChunks` and a four-frame `Wait` a row), the two doors
   entered take the map's own transition, open the arrival door, walk the party out and shut it, and
   the two eyes set temp `$14` and `$16` ([104](../scenes/104_GaruberkTowerEyes.md)). The census
   shrinks by six (74).
2. **The closed-door guard as a rule.** Each opening returns at once unless the chunk under the
   leader is `$38`; the elevator's `$4F` test was an `if event == 0x13` in the interaction probe.
   Both are now one table, `CLOSED_DOOR_GUARDS` (`rust/psiv-runtime/src/field_triggers.rs`).
3. **The door atlas (pack).** A live chunk write needs the chunk's pixels and collision in the map's
   scene chunk atlas, and no tower map had one: `psiv_tools.map_patches.garuberk_door_chunks` reads the
   four door tables (guard bytes pinned) into every tower map with a door opening.
4. **The first eye's live layout swap.** `Event_GaruberkTwEyeAction1` ends by decompressing
   Part2's other layout into the live layout buffers. New scene op `ReplaceMapLayout`: the runtime
   installs the pack's variant decoded from the two sources and keeps the objects
   (`replace_scene_map_layout`). The eye animation is a new presentation record
   (`GaruberkEyeArtCycle`).
5. **The layout-pair fix (runtime defect).** A retail replacement pair decompresses both planes and
   one is the map's own; the effect walk refused that write as `layout_replace source "0x1C8F7A" has
   no variant`, so Part2 could not be built once `$14` was set
   ([MAP_EFFECTS.md](../field/MAP_EFFECTS.md#13-a-replacement-pair-whose-one-plane-is-the-base-c8-2026-10-07)).

The way up is a graph the layouts decide: Part2's two rooms meet only in the first eye's layout;
Part6's door to Part7 (50,23) shares a room with its door from Part5 (62,21) only in the second
eye's, and Part5's second eye is in a room reached from Part6. The route walks Tower, Part2 (eye),
Part3, Part4, Part5, Part6, Part5's eye room, Part6, Part5, Part6 (now on its variant), Part7.

**Tests and negative controls:** `air_castle_scenes.rs`
`the_door_openings_step_their_tables_around_the_leader`,
`a_door_entered_warps_opens_the_arrival_steps_out_and_shuts_it`,
`the_eyes_set_their_flags_and_the_first_replaces_the_layout`; `scene_garuberk_tests.rs` (the tower's
first door opened by Speak on the real map, the guard's silent return at an open door, the walk
through to Part2 with the arrival door shut; the first eye's live swap); `effects.rs`
`a_layout_pair_whose_one_plane_is_the_base_selects_the_variant`; `tests/test_garuberk_door_atlas.py`
(the fifteen door chunks, every scene write among them, every tower map carrying them; a moved guard
byte and a short table fail the build, and a pack map without `$3C` is named). The runtime and pack
tests skip on a pack that predates the door atlas. See [the negative controls](#negative-controls).

## Runner and route changes

- `expect` gains `items_held`, `items_absent` and `status_clear`: the brief asks each chapter's
  closing to name the inventory change (`ECLPSTORCH` after Lashiec), and the recovery chapters claim
  the status they cure. The validator checks the item names and refuses an item both held and absent.
- `prefix-check` (above), promoted from lane C7's scratch scripts on its second use.
- `inspect` prints each member's status byte, skills with their uses and techniques.
- Route authoring found on the way, none of them a game defect: the Dezo spaceport's boarding row is
  cell row 19 (`y = $120`), the Air Castle's row 56; the Ice Digger refuses Jut's town tile; a map load
  parks it at the spaceport, which the route must claim; one `fight_scripted` claims every event battle
  an objective's settle fought (the chest's chain fights two).

## The final runs

The brief's commands from the worktree, with the worktree's `runtime-pack` link pointed at
`build/c8-pack` (the pack integration will rebuild; the owner's pack untouched):

```
CARGO_BUILD_JOBS=2 cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c8-route --tape build/c8-route/run.tape --report build/c8-route/report.json
./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c8-route-b --tape build/c8-route-b/run.tape --report build/c8-route-b/report.json
./rust/target/release/psiv-campaign replay build/c8-route/run.tape
./rust/target/release/psiv-campaign prefix-check build/c8-base/run.tape build/c8-base/report.json build/c8-route/run.tape build/c8-route
```

| Run | Result |
| --- | --- |
| C8-F1 (`build/c8-route`) | exit 0 in 49.6 s, **62 chapters, 4,483,067 frames, digest `eefda814de149505`**, tape SHA-256 `62300cb1a6c7e27d194306b064952a8ed4a131f7fd31e97cd4102602d53d46a3` |
| C8-F2 (`build/c8-route-b`) | exit 0 in 47.5 s, the same frames, digest and tape SHA-256; `cmp` finds the tapes identical and all 62 chapter saves byte-identical |
| C8-R (`replay build/c8-route/run.tape`) | 4,483,067 frames, digest `eefda814de149505`, exit 0 |
| prefix check | 50 chapters, 3,852,511 frames: every pad and every chapter save the base's (C8-0), exit 0 |
| the accepted c27 pack (`--pack /home/peter/PSIV/build/accepted-c27-pack-37a02a2`) | exit 2 at `garuberk-tower-first-eye` objective 2 (`interact` at (34,20)): `scene chunk atlas is absent`, the stale pack |

The last chapter save is SHA-256 `a56fe128…a983f`; the pad SAVE `route/slot_1.sram` (no new `save`
objective) is the base run's own, `09f2f114…dca8`. Binary SHA-256 `909ebb10…3d2b`, route file
`70364c12…f984`. The new chapters add 630,556 frames: 554,799 of them training.

## Negative controls

Each break was made in the source, run against its guard and reverted; the guard passes again.

| Break | Guard | Result |
| --- | --- | --- |
| `RecoverStats` out of `Event_Recovery` | `air_castle_scenes` recovery test | fails: no `RosterChanged` |
| door opening 1's last row writes `$3B` under, not `$3C` | `the_door_openings_step_their_tables_around_the_leader` | fails at that row; the other three pass |
| the identical-plane write refused again | `effects` layout-pair test, the runtime eye test | both fail (`layout $1C8F7A is no layout variant of this map`); the door test passes |
| no party placement in `set_vehicle_index(0)` | `a_dismount_stands_the_party_on_the_machine` | fails: the leader still on the boarding cell |
| `$35`/`$36` out of `CLOSED_DOOR_GUARDS` | the runtime door test | fails: Speak at the open door starts the opening again |
| `ReplaceMapLayout` installs nothing | the runtime eye test | fails: no variant |
| a pad moved at frame 153 of a run's tape; a base chapter's hash altered | `prefix-check` | exit 2, naming frame 153 in `academy`, and chapter 7 `zema-training`'s save; unchanged inputs exit 0, missing ones 1 |
| the Lashiec chapter claims `items_absent: ["ECLPSTORCH"]` (a copy of the route) | `expect` | the run halts there, `the pack holds "ECLPSTORCH", expected none`, exit 2 |
| the recovery chapter stops at (31,27), above the tile (a copy) | `expect` `status_clear` | halts, `Wren has status 0x40, expected 0`, exit 2 |
| a guard byte moved, a table without its `$FF`, a pack map without `$3C` | `tests/test_garuberk_door_atlas.py` `NegativeControl` | each fails as the test expects |
| an unknown item, an item both held and absent | `validator.rs` `an_inventory_claim_names_a_real_item_once` | each rejected in its chapter |

## Not claimed

- **Native play.** Headless `Session`, pads only. No Godot replay of the new chapters and no
  capture of the castle, the tower or the eyes. The Godot-visible changes: two presentation records
  Godot logs and does not draw (`PaletteToneFlash`, `GaruberkEyeArtCycle`, in
  `rust/psiv-godot/src/cutscene/ops.rs`), the live layout swap drawn through the existing
  `MapRefreshed` reload, the door chunks drawn from the new atlas through the existing scene chunk
  blitter, and the party placed on the machine after a dismount.
- **The eyes' and the flash's pictures.** Their frames are timed; their palette and art writes are
  records only.
- **The Garuberk Tower's map updates.** `MapUpdate_GaruberkTowerPart4` (the eye art cycling outside
  the event) stays `unsupported` in the pack, and Part4's and Part5's position-gated temp flags
  (`$15`, `$17`) only feed it.
- **A skill-aware boss policy** (H48).
