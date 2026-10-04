# `Cutscene_InsideSpaceship`

- **Retail bytes:** `$07606C..$07607D` inclusive, 18 bytes, then the shared helper
  `loc_63BC4` (`ps4.asm:133499-133726`) and the flight routines it calls.
- **Pointer:** `CutscenePtrs[$0D]` at `$05A580`; scene event `$800D`.
- **Dispatch:** `RunEventsJmpTbl[$21/$23/$24/$25]`, see [Callers](#callers).
- **Data:** `rust/psiv-core/src/scenes/flight.rs`, `INSIDE_SPACESHIP_ROUTE` (38 ops,
  `INSIDE_SPACESHIP` in `post_zio_cutscenes.rs` is those ops). The menu window is
  the session's destination mode, `rust/psiv-runtime/src/session/destination.rs`.
- **Pack:** `ship_menu/` (`psiv_tools/ship_menu_pack.py`): the strings, the
  `WinGroup_Event` records, the palette and eight backgrounds.
- **Evidence:** oracle tape `oracle/tapes/35_ship_destination_menu.tape` and the
  certified pair `ship-menu` (`tools/certify.py`, `rmse=0.000000`).

## Clone audit

The retail `else` pointer at `ps4.asm:120777` selects the body at `ps4.asm:155427`:
`InitVRAMAndCRAM`, `Pal_FadeIn`, `jmp loc_63BC4`. The Grand Cross clone's hack-only
menu bodies are not treated as retail scene records. The text below is the
`revision>0` (English) branch: the strings are `loc_2AAA46`, `loc_2AAA60` and
`loc_2AAA7A` (`ps4.asm:341253-341267`), and Speak, not Start, cuts a flight short
(`:134228`, `:134315`).

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

The flight, in the order the scene runs it: takeoff map load, `Pal_FadeIn`, 419
(spaceports) or 228 iterations, the 60-frame tone loop (spaceports only),
`PalFadeOut`, 60 map updates, `InitVRAMAndCRAM`, `loc_64C1E`'s planet screen
(fade in, 300 frames, fade out), 60 map updates, the transit map load,
`Pal_FadeIn`, the camera pan `loc_5ABDC`, 257 iterations, `PalFadeOut`, the landing
map load, `rts` with `d0 = 0` (`FieldRoutine_Cutscene` reloads the map).
Each fade is 14 frames (the Godot shell's `SceneFadeIn`/`SceneFadeOut` use the
same 14).

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

## Oracle timeline

`oracle/tapes/35_ship_destination_menu.tape` (tape 28's schedule, the menu entered
by `--ram-patch`, Speak at frame 7401; the exact patches are `certify.py`'s
`TAPE_35`):

| From the press | Cartridge | Port |
|---|---|---|
| Cancel: scene returns | 55 | 55 |
| Zelan row: takeoff map load | 173 | 173 |
| Motavia row: takeoff map load | 179 | 179 |
| Mota Spaceport to Zelan Space load | 1,216 | 1,129 |
| Mota Spaceport to Zelan landing | 1,652 | 1,415 |
| Zelan to Zelan Space, Motavia, Mota Spaceport | 179, 946, 1,406 | 179, 883, 1,168 |

The menu's frames match; the flight is shorter by 237 frames (Mota Spaceport to
Zelan) and 238 (Zelan to the Mota Spaceport). The loops and fades are the
cartridge's; what is missing is what the scene model has no op for: the camera pan
`loc_5ABDC`, `RefreshMap`'s own frames and the typing of the planet's name by
`RunText2` (`:134666-134678`), which differ by map (87 and 63 frames on the first leg
of the two journeys, 150 and 175 on the second).

## Not modelled

- The windows' open animation (`Window_Draw`, `:139785-139836`: the cartridge reads
  no pad for the 12 frames before the loop starts, frames 7246 to 7257 on the
  oracle) and their close. The port accepts the pad on the first frame and holds a
  `Closing` phase for the measured 55 or 25 frames.
- Speak cutting a flight short (`:134228`, `:134315`).
- The camera pan and the planet name's typing, as above.
- `loc_64C1E`'s branch to `Cutscene_Rykros` (World 2, Rykros flag clear, Dark Force 3
  defeated, `:134644-134655`).
- The planet screen's own drawing during the flight: the pack carries the menu's
  picture only.
- `Message_Speed` options other than the default 2 (`:88668`).
