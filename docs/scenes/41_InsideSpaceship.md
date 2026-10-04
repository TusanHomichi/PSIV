# `Cutscene_InsideSpaceship`

- **Retail bytes:** `$07606C..$07607D` inclusive, 18 bytes, then the shared helper
  `loc_63BC4` (`ps4.asm:133499-133726`) and the flight routines it calls.
- **Pointer:** `CutscenePtrs[$0D]` at `$05A580`; scene event `$800D`.
- **Dispatch:** `RunEventsJmpTbl[$21/$23/$24/$25]`, see [Callers](#callers).
- **Data:** `rust/psiv-core/src/scenes/flight.rs`, `INSIDE_SPACESHIP_ROUTE` (48 ops,
  `INSIDE_SPACESHIP` in `post_zio_cutscenes.rs` is those ops). The menu window is
  the session's destination mode, `rust/psiv-runtime/src/session/destination.rs`.
- **Pack:** `ship_menu/` (`psiv_tools/ship_menu_pack.py`): the strings, the
  `WinGroup_Event` records, the palette and eight backgrounds.
- **Evidence:** oracle tape `oracle/tapes/35_ship_destination_menu.tape` and the
  certified pair `ship-menu` (`tools/certify.py`). Its previous image pass was
  `rmse=0.000000`; the current candidate requires an orchestrator X11 run.

## Clone audit

The retail `else` pointer at `ps4.asm:120777` selects the body at `ps4.asm:155427`:
`InitVRAMAndCRAM`, `Pal_FadeIn`, `jmp loc_63BC4`. The Grand Cross clone's hack-only
menu bodies are not treated as retail scene records. The text below is the
`revision>0` (English) branch: the strings are `loc_2AAA46`, `loc_2AAA60` and
`loc_2AAA7A` (`ps4.asm:341253-341267`), and Speak, not Start, cuts a flight short
(`:134265-134272`, `:134349-134357`; Start interrupts the pan at `:121572-121573`).

## The menu, `loc_63BC4`

| Step | Source | What it does |
|---|---|---|
| list | `:133499-133535` | `$FFFFED42` takes the mask of the first set flag of `loc_63EF2` (`:133728-133735`), else `loc_63EFE`'s `$88`; each set bit from 7 to 2 names a world 0 to 5 unless it is `World_Index`; the rows go to `$FFFFED43`, the count to `$FFFFED40` |
| screen | `loc_63F00`, `loc_63F2C` (`:133745-133911`) | two Nemesis blobs to tiles `$100` and `$1EB`, two Enigma pictures (Plane A and B), the 32-word palette `loc_1DE158`, and a cover over every marker the mask hides |
| windows | `:133537-133602` | radar SFX (`SpcSFXID_SpaceshipRadar`), `Window_Create(5)` with "Where do you want to go?", `Window_Create(rows + 5)` with a `WinTiles_CursorBox` per row and the names three cells in |
| loop | `:133604-133615`, `loc_64144` | until Cancel, Speak or Camp: `Win_UpdateCursorUpDown` (`:141702`), the palette re-upload with the cursor row's marker lit on frames where `Main_Frame_Count` bit 3 is set, `DMAPlane_A_VInt` |
| Cancel | `loc_63E5E` (`:133692-133726`) | windows destroyed, fade out, `Map_LoadChunks`, `Field_Map_Index_2 = $FFFF`, the map reloaded at `loc_64B5A`'s row for the current map, `Window_Option_Index` cleared, `moveq #0, d0 / rts` |
| Speak, Camp | `:133616-133690` | `SFXID_Selection`, the prompt window cleared and typed over with "NAME will be" and "the destination." (three frames a character, one while a button is held), `Sound_StopSpcSFX`, a 60-frame wait, the windows destroyed, a fade out, **`World_Index` written from the row (`:133677`)**, then the takeoff: `loc_64568` when the map is the Mota or Dezo Spaceport (`:133669-133674`), else `loc_6488A` |

The cursor is `FieldObj_RedCursor` (`:141817`): visible 16 frames, then hidden 26
(`timer` `$19` at start and after every move, reloaded `$F`, plus `$A` when it goes
hidden). Speak or Camp stamps `$C6E8` over the row's box (`loc_69A32`,
`:141805`), which is the pattern the visible cursor draws too.

The geometry is `WinGroup_Event` (`:141150`, ROM `$0693A0`): window 5 is
`(7,21)` 26 by 5 cells, window `rows + 5` is `(26,9)` 11 cells wide and `2 * rows + 1`
high; the prompt sits one cell inside window 5, the boxes one cell inside the list
and the names at its x + 3. The pack's `ship_menu.json` carries the records.

## Flights

Three tables, each six rows (`ps4.asm:134540-134610`, `scenes/flight.rs`):

| Table | Keyed by | Row |
|---|---|---|
| `loc_64B02` | the map the party is on | the takeoff map (Motavia, Dezolis, Rykros, Zelan Space, Kuran Space, Air Castle Space) and its start |
| `loc_64B34` | the chosen world | the map the ship crosses and its start |
| `loc_64B5A` | the chosen world, or the current map for Cancel | the landing: Mota Spaceport, Dezo Spaceport, Le Roof Room, Zelan, Kuran, Air Castle |

### Frame clock and return boundary (#77)

All durations below include the initiating frame and exclude the next step's
first frame. The reference is **US retail**, ROM SHA-256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`.
`oracle/ship_flight.py` runs tape 35 through `oracle/host_binary.py`'s host,
with frame-end CPU registers/return stacks (`--cpu-trace`) and RAM. For the
return leg it changes only the origin map/world of the frame-7000 fixture;
the tape's Speak remains frame **7401**. Source line citations here refer to
`reference/ps4disasm/ps4.asm`; retail bytes, not fork build-address comments,
resolve the caption pointer and mode.

| Ordered step | Source | Mota→Zelan frames | Zelan→Mota frames |
|---|---|---:|---:|
| confirm message, tail wait, window teardown/fade/map setup | `loc_63D46`, `:133616-133690`, `:133778-133833`; default `Message_Speed=2`, `:88668` | 173 (87 glyph waits + 60 tail + 26 entry/close) | 179 (93 + 60 + 26) |
| takeoff `RefreshMap` | `:134183-134191` / `:134392-134400`; body `:121767-121909` | 42 | 18 |
| ship art / sprite DMA | `loc_641A2`, `loc_64230`, `:134192-134194` / `:134401-134403` | 2 + 1 | 2 + 1 |
| takeoff `Pal_FadeIn` | `:134195` / `:134404`; body `:85897-85950` | 16 | 16 |
| takeoff map-update loop | `loc_645E6`, `:134199-134273` / `loc_64908`, `:134408-134453` | 419 | 228 |
| tone increase | `loc_646DA`, `:134274-134292` | 60 | 0 |
| fade out, build sprites/VInt, map-update wait | `loc_6472C`, `:134293-134302` / `:134454-134463` | 14 + 1 + 60 | 14 + 1 + 60 |
| `InitVRAMAndCRAM` | `:134303` / `:134464`; body `:120854-120877` | 15 | 13 |
| planet background | `loc_643B8`, `:134059-134117`, called `:134654` | 18 | 18 |
| planet sprite/setup | `loc_644AE`, `:134118-134166`, called `:134655` | 2 | 4 |
| complete planet caption / text DMA setup | `loc_64C4A`, `:134661-134678`; `RunText2`, `:143694-143734` | 3 CPU frames, **0 per-character waits** | 2 CPU frames, **0 per-character waits** |
| planet fade in | `:134678`; `Pal_FadeIn`, `:85897-85950` | 16 | 16 |
| planet hold | `:134679-134711`, counter `$12B` | 300 | 300 |
| planet fade out / map-update wait | `:134713-134714`, `:134305-134306` / `:134466-134467` | 14 + 60 | 14 + 60 |
| transit `RefreshMap` | `:134307-134320` / `:134468-134481`; body `:121767-121909` | 17 | 41 |
| ship art / sprite DMA | `loc_641A2`, `loc_64332`, `:134321-134322` / `:134482-134483` | 2 + 1 | 2 + 1 |
| transit fade in | `:134323` / `:134484`; `Pal_FadeIn`, `:85897-85950` | 16 | 16 |
| Y camera pan | `loc_5ABDC`, `:121550-121600`, called `:134324-134332` / `:134485-134493` | 128 | 128 |
| sprite placement/VInt | `:134333-134337` / `:134494-134498` | 1 | 1 |
| descent map-update loop, terminal pass included | `loc_64800`, `:134338-134358` / `:134499-134519` | 257 | 257 |
| fade out | `loc_6483A`, `:134359-134360` / `:134520-134521` | 14 | 14 |
| landing `RefreshMap` | `:134361-134375` / `:134522-134536`; body `:121767-121909` | 33 | 24 |
| cutscene return VInt + normal field-reload preparation | `FieldRoutine_Cutscene`, `:120738-120764`; `GameMode_LoadFieldMap`, `:107505-107635` | 1 + 35 | 1 + 25 |
| ordinary field fade in | `GameMode_LoadFieldMap`, `Pal_FadeIn` | 16 | 16 |
| place-name window to field control | `FieldRoutine_PlaceName`, `:136552-136646`, timer `$78` plus draw/teardown DMA | 124 | 124 |
| **confirm to field control** | first `Game_Mode=$0C`, `Game_Mode_Routine=0` after landing | **1,861** | **1,596** |

`RefreshMap`, VRAM initialization and caption generation are CPU/decompression/
DMA work; the disassembly does not give them a constant VInt count. The table's
map-specific spans are measured from the fresh trace's PC/return stack, not
inferred from glyph count. Only these two legs' setup spans are measured.
Other destinations, other message speeds, interrupted flights and the Rykros
branch are not frame-exact claims.

**Fade correction:** flight fade-in spends 16 frames, including display enable
and the terminal palette pass. Fade-out spends 14. The old claim that all fades
spend 14 was wrong (`Pal_FadeIn`/`VDP_EnableDisplay`, `:85897-85950`).

**Pan:** `loc_5ABDC` subtracts home offsets `$98/$58`, masks to twelve bits and
uses the shortest wrapped route. This call selects only Y, speed 2, with the
subject at `start_y*8+$100`; from RefreshMap's initial camera that is 256 pixels,
or 128 frames (`:121550-121600`, `:134324-134332`). Runtime camera glide owns the
state and clock; Godot reads that camera.

**Caption:** US ROM `$064C62` (pointer operand `$064C64`) reads `SpaceTravel_PlaceNamePtrs` at `$2AAADE`;
`$064C78` is `moveq #1,d4`. `RunText2`'s `tst.w d4 / bne loc_6A9CE`
(`:143728-143733`) bypasses the per-glyph DMA/wait path. The final `$FF` queues
DMA (`loc_6AA10` → `loc_6AC98`); it does not create a glyph wait. The pack carries
the six complete, space-preserving dialogue-font lines, not menu-name
substitutes. The palette entry stays zero until counter `$6D`, after 191 of
300 planet frames, then becomes `$EEE` (`:134700-134711`).

**Control:** landing-map write is not control. `FieldRoutine_Cutscene` returns
through `GameMode_LoadFieldMap` (`$08`), the field fade and routine `$1C`'s
place-name window; control begins in field mode `$0C`, routine zero. The
measurement rejects a tape ending at the map write/reload/window. This accounts
for the extra 209/190 frames beyond the old landing-write boundary. The VInt
advances field RNG on the frame clock (`:617`); presentation/setup waits spend
that same runtime clock, not a Godot timer.

### Opening window and ignored input (#78)

`Window_Draw` (`:139748-139836`, `loc_68690`) grows two columns per pass;
`Window_Render_Mode` bit 1 skips alternate DMA uploads. Tape 35's prompt/list
opening is frames **7246..7257**, with the first pad-reading loop at **7258**
(`loc_63D20`, `:133604-133615`). Its 12 no-input frames are:

| Frames | Visible uploaded columns / work |
|---|---|
| 7246..7252 | prompt 2, 6, 10, 14, 18, 22, 26 columns (7 uploads) |
| 7253 | prompt text DMA (1) |
| 7254..7256 | list 2, 6, 10 columns (3 uploads) |
| 7257 | final odd list column (11 total), text/cursor DMA (1) |

Session exposes `Opening`, uploaded column counts and prompt readiness for
Godot. It consumes pad edges throughout all 12 frames, including the last;
a held Speak is not queued for Choosing. Cursor updates start only afterward.
Close timing remains the measured 55-frame Cancel or 25-frame confirm teardown;
the close window shape animation is still not modelled.

The first `moveq #2,d0` pass uploads; the next bit-2 toggle skips its DMA.
The trace's `cpu_d0` at `DMAPlane_A_VInt` confirms the widths above. On the odd
11-column list, the last Window_Draw pass skips DMA, so the text upload carries
that final column. The regression uses this measured width sequence, not the
implementation's growth formula.

## Callers

Every caller reaches the same menu and leaves the same way: `Event_Index = $800D`
starts the cutscene, and every exit is `d0 = 0`, which reloads the field.

| Caller | Source | Where | Menu |
|---|---|---|---|
| `RunEvent_EnterSpaceship` (`$21`) | `:115743` | Mota Spaceport `$0BF`, Dezo Spaceport `$0D4`: leader X `$1E0..$1F0`, Y `$120` | the flag table; long takeoff |
| `RunEvent_KuranEnterSpaceship` (`$23`) | `:115787` | Zelan `$18D` and Kuran `$190`: Y `$2F0` (row 48) | the flag table; short takeoff |
| `RunEvent_AirCstlEnterSpaceship` (`$24`) | `:115795` | Air Castle `$171`: Y `$370` | the flag table |
| `RunEvent_SilenceTmEnterSpaceship` (`$25`) | `:115802` | Le Roof Room `$0F0`: Y `$120` | the flag table |
| `Cutscene_LeRoofAgain`, `Cutscene_FindingAirCastle` | `:155427` inlined (`jmp`) | end of their own bodies | the flag table; the route is inlined, relative skips |
| `Cutscene_SpaceshipSabotage` | `:155431-155560` | Zelan, after the Canceller | its own copy, mask `$08` (Kuran alone); Cancel (`:155546`) lands on Zelan's `loc_64B5A` row |

Stories that fly without the menu write `World_Index` and run `loc_64C1E`:
`Cutscene_DarkForce1Defeated` (`:156396`, 3), `Cutscene_LashiecDefeated` (`:156940`, 1),
`Cutscene_GumbiousBishop` (`:157065`, 0), `Cutscene_Reunion` (`:157791`, 0) and
`Cutscene_CrashLanding` (`:155847`, 1). The first four carry `SetWorldIndex` now; the
crash landing's write is the Dezolis lane's (`docs/scenes/43_CrashLanding.md`).
The Landale writes none.

## Port

- Mask and rows: `destination_mask_byte`, `destination_worlds` (`flight.rs`).
- The menu: `SceneOp::DestinationMenu` blocks the scene; the runtime hands it to
  the session as a `ScenePresentation` event, which opens the destination mode.
  The answer reaches the scene through `Runtime::answer_destination`, which writes
  `World_Index` for a confirm (`travel.rs`). A Cancel skips `cancel_skip` ops, so
  an inlined copy needs no absolute index.
- The legs: `SceneOp::LoadFlightMap { leg }`; `scene_runtime.rs` resolves the table
  by the current map and `World_Index`. A key a table has no row for is a scene
  fault (`SceneFault::NoFlightTarget`), not a no-op.
- `World_Index` is the high byte of the saved word; `Runtime::town_destinations`
  reads it for RYUKA and TELEPIPE (`travel.rs`, `ps4.asm:128945-128975`).
- The cartridge's other `World_Index` readers: `Interaction_DisplayDialogue` picks the
  dialogue tree by it (`ps4.asm:118304-118315`, `WorldDialogueTreePtrs`), the
  RYUKA/TELEPIPE builder picks the town table and, on a confirm, the overworld map
  (`:128954-128970`, `:129410-129418`), and a field save copies it (`:124184`). The
  port reads it for the town lists only; its dialogue trees come from each map's own
  binding.
- Sound: radar on entry, `SFXID_MovingCursor` on a move, `SFXID_Selection` on a
  confirm, `Sound_StopSpcSFX` when the message ends.

## Oracle timeline and port parity

Offsets are from tape 35's Speak at frame 7401. Both legs use ordinary
`Session::frame` pads after a saved-state origin fixture; no test calls a
runtime mutator to advance the flight. The return oracle leg changes the
fixture origin only. These isolated fixtures prove the interaction/timing
path, not a connected campaign route or full-scene visual parity.

| Boundary | Oracle offset | Port offset |
|---|---:|---:|
| Cancel return (existing test) | 55 | 55 |
| Mota→Zelan takeoff map | 173 | 173 |
| Mota→Zelan transit map | 1,216 | 1,216 |
| Mota→Zelan landing map | 1,652 | 1,652 |
| Mota→Zelan field control | **1,861** (frame 9262) | **1,861** |
| Zelan→Mota takeoff map | 179 | 179 |
| Zelan→Mota transit map | 946 | 946 |
| Zelan→Mota landing map | 1,406 | 1,406 |
| Zelan→Mota field control | **1,596** (frame 8997) | **1,596** |

The old port's landing offsets were 1,415 and 1,168 (237/238 short). Those are
retained as historical measurements only. Current tests assert all map edges,
actual control return, the full flight's clock cost, packed caption delivery and
post-control overlay removal. The correction-ladder rung is **compile-time full-route concatenation**
for inlined callers, plus regression tests through the owning runtime, with a repository-owned oracle probe; no shell
sleep or timing workaround.

`ship-menu` retains clone tick **70**, oracle frame **7293** and SHA-256
`af4aaf169ea283c1784dad98822f02adcaa64684ccd3a5d037596df58f3300b8`.
The runtime's tick-30 event fixture verifies at tick 70: Choosing, both windows
fully uploaded, cursor hidden and marker blink off. No tick changed. The lane
cannot run X11; the orchestrator must rerun `python3 tools/certify.py --pair
ship-menu`. A state-pin pass is not an image-parity pass.

## Pack and verification receipt

The full pack is built inside this worktree with the owner's 184 battle-art
frames linked individually for reading. No local input symlink is staged or
modified. Flight captions and landing-window geometry (`WinGroup_PlaceName`,
US `$0695F8`, `:141402-141429`) are decoded by `psiv_tools` and loaded by
`psiv-data`. Older packs expose missing records rather than invented captions.
Local-input tests select `PSIV_RUNTIME_PACK`, otherwise the repo pack, and
announce an absent input/record.

Raw evidence is under `build/x77-evidence/`: oracle CSVs/host logs/receipt,
pack comparison, focused runs, deliberate negative controls and gate logs.

### Pack comparison

`python3 -m psiv_tools pack "Phantasy Star IV (USA).md" build/x77-pack` exited
0. Compared with the owner pack, **5,079 files are byte-identical**; only
`manifest.json` and `ship_menu/ship_menu.json` changed. Manifest SHA-256:

- owner: `b6d9dd8c4d9ad6b75c9b85801ff372b812bbf8b04db3f6f7420c38ab916683bc`
- worktree: `e31033598b348400d59d0ee849ee511f6d915e2b63eb2663490c6d055aaa74dd`

See `build/x77-evidence/pack-comparison.json`, `inputs.json` and
`retail-caption-call.json`. The oracle host fingerprint is
`24043c4c2a06eae76519c06186f0c8448a692e78aa6bd6f5bc717b641d6581e2`;
`python3 -m oracle.ship_flight` exited 0. Its receipt includes both trace hashes
and exact host invocations; `opening-trace.json` records the window DMA widths.

### Checks and negative controls

Rust tests use `CARGO_BUILD_JOBS=2`, `PSIV_RUNTIME_PACK="$PWD/build/x77-pack"`
and `--test-threads=1`. Heavy runs are sequential. The focused command
`cargo test --manifest-path rust/Cargo.toml -p psiv-runtime --test
session_destination -- --test-threads=1 --nocapture` exited 0: **13 passed**,
no failures, ignores or filters, including both field-control counts, the
full flight clock cost, opening input loss, uploaded widths and the tick-70
view pin.

| Command (worktree pack env as above for Rust) | Exit / result |
|---|---|
| `cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1 --skip boarding_opens_the_menu_and_the_flight_lands_in_zelan` | 0; 1,342 passed, 0 failed, 3 ignored, 1 filtered; 40 harness/doc-test summaries |
| `cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings` | 0; no warnings |
| `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` | 0 |
| `PYTHONPATH=. python3 -m unittest tests.test_certify` | 0; 19 tests, no failures/errors/skips |
| `PYTHONPATH=. python3 -m unittest tests.test_certify tests.test_ship_flight` | 0; 22 tests, no failures/errors/skips |
| `PYTHONPATH=. python3 -m unittest discover -s tests` | 0; 1,261 tests, no failures/errors/skips |
| `python3 tools/size_guard.py` | 0; no file over limit |
| `python3 tools/check_docs.py` | 0; no problems |
| `git diff --check` | 0 |

The three ignores are the two explicitly long campaign-route tests and
`dump_manifest_entries` (a harvesting tool). The filtered test is named above;
this is **not** a pass of the required unfiltered workspace command. The final
Rust rerun follows the measured-width correction; all non-doc inputs remained
frozen. Python inputs did not change after the full Python pass.

Deliberate negative controls:

| Mutation / invalid input | Result | Evidence |
|---|---|---|
| flight fade-in 16 → 14 | exact-frame test exited 101, 1 failed; first leg returned at 1,853 instead of 1,861 | `negative-flight-fade.log` |
| Opening dispatches to `choose` | pad test exited 101, 1 failed; phase became Confirmed during opening | `negative-opening-input.log` |
| earlier four-column-first formula against measured widths | width/pad test exited 101, 1 failed; first width 4 instead of 2 | `negative-opening-columns.log` |
| clone tick 70 → 64 in memory, oracle pin unchanged | Python certification metadata tests exited 1, 1 failed of 19 | `negative-certify-pin.log` |
| dangling caption pointer / missing FF; waiting caption / wrong glyph count / invalid window; measurement without field control; truncated shared route | repository tests reject each input | `python-focused.log`, workspace logs |

All mutations were restored before the passing runs. Repair attempts are
preserved: private-clock access compile error; wrong nested carrier import;
stale 38-op carrier lists (2 core failures); wrong tick-82 expectation
(1 failure); width-test loop integer inference compile error; initial fmt
differences. These are repaired, not hidden passes. Full commands, exits and
counts are in `build/x77-evidence/check-results.json`.

### Release route and acceptance blocker

`CARGO_BUILD_JOBS=2 cargo build --release --manifest-path rust/Cargo.toml -p
psiv-campaign` exited 0. The run used the worktree pack explicitly:

```sh
PSIV_PACK="$PWD/build/x77-pack" PSIV_SAVE_DIR="$PWD/build/x77-route" ./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/x77-route --tape build/x77-route/run.tape --report build/x77-route/report.json
```

It exited **2**: **29/30 chapters completed**, then the final `mota-spaceport`
chapter, objective 3 (`board`), exhausted its 20,000-frame budget on map `$0BF`,
cell `(30,19)`, still in destination mode. No flight occurred, so post-flight
route balance and a completed-route RNG digest are **unverified**. The halted
run digest is `0d7ae08759f2b6a7`, with 2,612 battles and 29 chapter saves. Raw
report/tape are `build/x77-route/report.json` and `build/x77-route/run.tape`;
hashes are in `build/x77-evidence/route-summary.json`.

`Driver::board` (`rust/psiv-campaign/src/ship.rs:63`) treats the first view as
ready, releases the step for one frame, then confirms while Opening still
ignores input. It subsequently sends only neutral frames. Its unbounded unit
test hangs for the same reason. This file belongs to the campaign lane, outside
the write set: **no driver or route edit was made**. The required unfiltered
workspace test was interrupted (exit 130) at
`boarding_opens_the_menu_and_the_flight_lands_in_zelan`; this acceptance item
has not passed.

**Next action:** the campaign lane must wait for
`DestinationPhase::Choosing` before cursor motion/confirm, then rerun the exact
unfiltered workspace command and release route. The orchestrator must also
rerun the fixed-frame `ship-menu` X11 pair. This candidate is self-reviewed;
timing/view-state, extraction and route evidence are labelled separately.

## Not modelled

- The windows' close shape animation; Closing still spends its measured interval.
- Speak cutting a flight short (`:134265-134272`, `:134349-134357`; Start interrupts the pan at `:121572-121573`).
- `loc_64C1E`'s branch to `Cutscene_Rykros` (World 2, Rykros flag clear, Dark Force 3
  defeated, `:134644-134655`).
- The planet screen's own drawing during the flight: the pack carries the menu's
  picture only.
- `Message_Speed` options other than the default 2 (`:88668`).
