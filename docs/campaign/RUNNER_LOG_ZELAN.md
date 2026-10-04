# Runner log: the Zelan arc (C4, M23)

Run C4 and the halts H23 to H27 it found, then the M23 lane that built the
ship's destination menu (the Mota Spaceport to Zelan). Moved out of the
[runner log](RUNNER_LOG.md) on 2026-10-04 so that file stays under the size rule.
The Motavia arc is [RUNNER_LOG_MOTAVIA.md](RUNNER_LOG_MOTAVIA.md).

## C4 runs: the Mota Spaceport, Zelan and Wren

Base `3344f18`, release builds, `CARGO_BUILD_JOBS=2`, local `runtime-pack` (manifest
SHA-256 `7fe1e64a…16de`); evidence under the ignored `build/c1/` and `build/c4/` of the
lane's worktree. The brief: extend the route past Zio through the spaceport, Zelan, Wren,
the sabotage and the crash landing. It stops at the ship's destination menu (H23).

| Run | Command (from the worktree) | Result |
| --- | --- | --- |
| C4-0 | `run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-0` (29 chapters) | exit 0, 2,615,778 frames, digest `949c2abe3342e838` (the combined receipt's); checkpoint SHA-256 `4aeff0b1…ec62` |
| C4-1 | scratch chapter that boards, `--from-chapter mota-spaceport` (`build/c4/r1`) | halt `scene_fault` `WarpUnmapped (30,19)`, frame 240: H24 |
| C4-2 | the same after the runner fix (`build/c4/r2`) | the scene flies Motavia, Zelan Space, Zelan: Zelan `$18D` (31,46) after 929 frames, no menu: H23 |
| C4-3 | probe `zelan-wren` from C4-2's save (`build/c4/p1`) | exit 0, 6,507 frames: Wren L20 210/210 joins, `$70` set |
| C4-4 | probe `zelan-canceller` (`build/c4/p2`): chest 5, then 420 frames walking | halt `expect_failed`: `$72` clear after "CANCELLER is procured!": H25 |
| C4-5 | probe route from New Game (`build/c4/probe-full.json`: main, boarding restored, both probes) | exit 2 at `zelan-canceller` objective 4, 2,624,291 frames, digest `32ed271dcc5c620a`; `build/c4/probe-full/report.json` |
| C4-6 | probes from the Wren save (`build/c4/p3`): Zelan row 48; the F1 elevator before the chest | `budget_exhausted` in a Zelan, Motavia, Zelan loop (H23); `SceneMissing { event: 42 }` (H26) |
| C4-7, 8 | `run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-{a,b}` (30 chapters) | both exit 0, 2,616,234 frames (456 more than C4-0: the new chapter), digest `dccc4df3ce01bd22`, tape SHA-256 `b96ebf10…4d45`, `29-mota-spaceport` snapshot `314e6602…a698`, pad-SAVE `route/slot_1.sram` `4aeff0b1…` unchanged; `replay` reproduces the digest; runner `b5795f79…6c94` |

**Probe facts** for the lane that re-adds the chapters. Zelan `$18D` arrival (31,46);
its elevator door is `interact` from (30,18) up, `opens` (30..31,17), then `go_to_map
398 via_warp 0`. F1 `$18E`: `go_to` (31,14), `talk` npc 0 is Wren; chests 0 to 4 are
the Plasma Sword, Claw, Dagger, Field and Pulse Laser, chest 5 at (49,49) the Canceller
(chest flag `$0B`). Back down: `interact` from (30,50) up, `opens` (30..31,49), `via_warp
0`. Zelan row 48, cells (30..31,48), is the boarding and the sabotage row.

### H23: the ship's destination menu is missing

**What it is.** `Cutscene_InsideSpaceship` (`ps4.asm:155427`) jumps to `loc_63BC4`
(`:133499-133726`). Callers, all writing `$800D`: `RunEvent_EnterSpaceship` (`$21`,
`:115743`; the Mota and Dezo spaceports), `RunEvent_KuranEnterSpaceship` (`$23`, `:115787`;
Zelan row 48 and Kuran), `:115795`, `:115802`. `Cutscene_SpaceshipSabotage` embeds a copy
(`:155431-155470`, `:155590-155600`) with the one-row mask `$08`, Kuran (`:155741`). Inputs:
`World_Index` (the current world is left out) and the first set flag of `loc_63EF2`
(`:133728-133735`: `DarkForce3Defeated $F8`, `Lashiec $D8`, `AirCastleFound $DC`,
`DezoSpaceport $D8`, `AlysFound $90`, else `$88`), whose mask bits 7 to 2 are worlds 0 to 5:
Motavia, Dezolis, Rykros, Zelan, Kuran, Air Castle. A window lists the names
(`loc_2AAA7A`); `Window_Option_Index` moves under `Win_UpdateCursorUpDown` until Speak,
Camp or Cancel (`:133604-133615`). Speak or Camp: `SFXID_Selection`, the name, 60
frames, fade, **`World_Index` written from the row** (`:133677`), then the flight (takeoff
`loc_64568` from a spaceport, `loc_6488A` elsewhere; leg `loc_64B34` and landing `loc_64B5A`,
both indexed by the row). Cancel (`:133692`) reloads the map at its `loc_64B5A` start and
sets nothing. At the Mota Spaceport `AlysFound` picks `$90`, so the list is one row, Zelan;
at Zelan one row, Motavia; in the sabotage scene one row, Kuran.

**Port before M23.** None of it existed. `INSIDE_SPACESHIP`
(`rust/psiv-core/src/scenes/post_zio_cutscenes.rs:102-153`) hard-codes Mota Spaceport,
Motavia, Zelan Space, Zelan, with no list, cursor, Cancel or flag branch; `SPACESHIP_SABOTAGE`
(`:155-232`) keeps only the panels and sounds; the runtime holds just the saved word
(`rust/psiv-runtime/src/travel.rs:50-54`) and makes none of the cartridge's `World_Index`
writes (`ps4.asm:133677`, `:155599`, `:155847`, `:156396`, H27). The Session has no such
mode (`rust/psiv-runtime/src/session/`: title, shop, camp, battle, notices,
`menu_scene.rs`). The fixed flight also fires from Zelan's row 48 (C4-6), so a player there
can never choose Motavia. **Disposition: stopped at the foot of the boarding row, no
alternative** (it is the only way aboard); C4-2 passes the step only because the port
flies anyway, which is why the committed route does not. Smallest change, an orchestrator
lane: a scene op that hands the frame to a Session-owned list window (from the flag table
or the scene's mask; cursor, Speak or Camp, Cancel), writes `World_Index` and picks the
legs by row. The route then needs a Speak press at the spaceport and one in the sabotage.

**Fixed by M23** (below): the destination mode, the legs and `World_Index`, with the
route's `board` objective.

### H24: a landing that starts a scene reports its type-1 cell as unmapped

Runner, fixed: `driver.rs` `is_scene_fault` no longer halts on `WarpUnmapped` in a frame
that began a scene (unit tests, and `tests/spaceport.rs`, whose negative control fails
with the old fault). Port: `RunEvents` runs before `RunMapTransitions`
(`ps4.asm:116768-116773`), yet `rust/psiv-runtime/src/field_tick.rs:112` reports the cell
before `evaluate_triggers` (only elevator cells are special-cased, `:80-86`). Smallest
change: evaluate triggers first.

### H25: the Canceller flag is never set

`RunEvent_SpaceshipSabotage` (`ps4.asm:115826`, `rust/psiv-core/src/trigger_table.rs:195-197`)
needs `$72`, whose only writer is `MapUpdate_ZelanCanceller` (`:114507`, `MapUpdateJmpTbl`
`$2D` at `:112931`; it sets `$72` when `ChestFlag_Canceller`, `$0B`, is set). `Zelan_F1`'s
update list is `$0D,$15,$1B,$2D,$00` (`:266268`), run every field frame by
`RunMapUpdates` (`:112859`). The pack holds the map-data effects and event lists but not
the update list, and the runtime has no per-frame update that writes a flag: 420 frames
after the chest opens `$72` is clear (C4-4, C4-5), so the sabotage and the crash landing
behind it cannot be reached. **Disposition: no alternative** (the reads are `:115833`,
`:116378`). Smallest change: `psiv_tools` extracts each map's update list, `psiv-data`
loads it, `field_tick` runs the flag-writing entries (`$2D`) each frame. Everything past it
(sabotage, Chaos Sorcerer, `Cutscene_CrashLanding` with #67, Dezolis control) is unprobed.
**Fixed on main before C5** (the per-frame map updates, #41): the C5 route sets `$72` with
the chest and plays the rest ([C5 runs](RUNNER_LOG_DEZOLIS.md#c5-runs)).

### H26 and H27

**H26.** `Event_CancellerReminder` (`$2A`, `ps4.asm:147371`: dialogue entry `$0C`, then `$73`)
has no scene: `SceneMissing { event: 42 }` on F1 (30,50) with Wren aboard and the chest shut,
a #56-class gap; a player's order avoids it (chest first). **H27.** No flight writes
`World_Index` (H23), so `Runtime::world_index` stays `0` on Dezolis and the RYUKA and
TELEPIPE town lists (`travel.rs:100-109`) read the wrong world. **Fixed by M23**: the
menu writes the world on a confirm (`Runtime::answer_destination`), and four story
flights carry `SceneOp::SetWorldIndex` (`:156396`, `:156940`, `:157065`, `:157791`);
`ryuka_reads_the_world_the_flight_wrote` flies to Dezolis and reads RYUKA's list before
and after. The crash landing's write (`:155847`) is the Dezolis lane's.

## M23: the ship's destination menu and `World_Index` on every flight (2026-10-04)

Lane m23-shipmenu, base `bfa5c89` (`claude/campaign-17`: main `3344f18` plus the C4
route chapter). Release builds, `CARGO_BUILD_JOBS=2`. The pack is the local
`runtime-pack` as an overlay: every entry of the owner's pack linked, `manifest.json`
copied with the new `ship_menu` entry (SHA-256 `abe394ae…b87`, the owner's `7fe1e64a…16de`
plus that entry) and `ship_menu/` written by `psiv_tools/ship_menu_pack.py`. The
owner's pack is untouched; until it is rebuilt (`python3 -m psiv_tools pack`) the
menu has no names or typed message and the `ship-menu` pair cannot run.

What was built, and where it is cited: [Inside the spaceship](../scenes/41_InsideSpaceship.md)
(the cartridge's routine line by line, every caller, the tables, the oracle timeline and
what is not modelled). The menu is the session's destination mode
(`session/destination.rs`); the scene ops are `DestinationMenu`, `LoadFlightMap`,
`SetWorldIndex`, `SkipUnlessMap` and `SkipOps`; the route gains `board`.

| Run | Command | Result |
| --- | --- | --- |
| M23-1 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/m23-route --tape build/m23-route/run.tape --report build/m23-route/report.json` (release, 30 chapters) | exit 0, **2,617,668 frames**, digest `0bd25017e4696c02`, tape SHA-256 `eb8181dd…3835`; `mota-spaceport` 1,890 frames (1,434 more than C4's: the menu and the flight), its snapshot `038afc2a…0ab556`; the pad SAVE `route/slot_1.sram` is `4aeff0b1…ec62`, unchanged |
| M23-2 | `replay build/m23-route/run.tape` | digest `0bd25017e4696c02`, 2,617,668 frames |
| M23-3 | the same run into `build/m23-route-b` | exit 0, digest `0bd25017e4696c02`, 2,617,668 frames, tape byte-identical to M23-1 (`cmp`) |
| M23-4 | a scratch route (main plus the C4 probe chapters `zelan-wren` and `zelan-canceller`), `--from-chapter zelan-wren` from M23-1's Zelan snapshot (`build/m23-probe`) | Zelan row 48 to F1, Wren L20 210/210 joins (`$70`), chest 5 opens, then **halt `expect_failed`: `$72` clear**, frame 6,942: H25, as before; digest `35eca188f0aa28ba` |

The route now ends in Zelan `$18D` at (31,46) with `World_Index` 3, where it used to stop at
the foot of the boarding row. H25 is the next stop and is not touched here.

**Frames.** The menu's counts equal the oracle's: 55 from a Cancel press to the scene's
return, 173 (ZELAN) and 179 (MOTAVIA) from a confirm to the takeoff map load
(`the_answers_take_the_frames_the_cartridge_took`). The flight itself is 237 and 238 frames
shorter than the cartridge's on the two measured journeys (table in the scene doc): the
camera pan before the descent, `RefreshMap`'s frames and the planet name's typing have no
scene op. Every flight leg shifts the clock the field's RNG runs on by those frames.

**Certification.** `ship-menu` joins the pairs in `tools/certify.py`: the clone starts
`PSIV_DEBUG_EVENT=0x800D` (the Mota Spaceport, `AlysFound` set) and is shot at tick 70;
the oracle is frame 7293 of `oracle/tapes/35_ship_destination_menu.tape` with the
`TAPE_35` patches, hash-pinned (`af4aaf16…00b8`), regenerated when missing; the pair
reads `rmse=0.000000`. Its pixels recur on 23 frames of the cursor-hidden stretch (7283
to 7308) and on the clone's ticks that share its blink phase; the pair does not certify the visible-cursor half of the blink, the typed message or the
closing frames (the message was inspected against oracle frame 7520 by eye).

**Negative controls**, each run once on the finished tree (one source edit, the guarding
test, the file restored byte for byte):

| Break | Guard | Result |
| --- | --- | --- |
| Cancel's skip one op short | `flight` unit tests | 2 fail (`four_scenes_carry_the_menu_and_cancel_lands_on_the_return_leg`, `the_menu_blocks_until_answered_and_cancel_skips_the_flight`) |
| a confirm does not write `World_Index` | `session_destination` | 5 fail, among them `ryuka_reads_the_world_the_flight_wrote` and `zelans_boarding_row_offers_motavia_after_the_flight_there` |
| Speak beats Cancel pressed with it | `session_destination` | `cancel_wins_over_speak_pressed_with_it` fails |
| the flag table searched `AlysFound` first | `flight` unit tests | `the_flag_table_takes_the_first_set_flag_and_defaults_to_88` fails |
| the Cancel teardown 54 frames | `session_destination` | `the_answers_take_the_frames_the_cartridge_took` fails |
| a table with no row falls back to row 0 | `session_destination` | `a_flight_table_without_a_row_is_a_scene_fault` fails |
| the list keeps the current world | `session_destination` | 6 fail |
| the `ship-menu` capture at other ticks | `certify` | tick 60 and 76 `rmse=2.796263`, tick 84 `3.076501`; ticks 66 and 70 share a phase and read 0.000000 |
| a `board` to World_Index 9 or an unknown name | `psiv-campaign` `validator` | rejected with chapter and reason |
| `board` to a world the menu does not list, a step that opens no menu | `psiv-campaign` `spaceport` | halts `menu_entry_missing`, `unexpected_state` |

**Full certification** (`python3 tools/certify.py`, receipt `build/certify/20261004T044347Z-bfa5c89`):
12 of 12 pairs at `0.000000`; the eleven earlier pairs' capture and oracle SHA-256 are
byte-identical to `build/certify/20261004T023925Z-3344f18`; `ship-menu`'s capture is
`f0270647…2905`.

