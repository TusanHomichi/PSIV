# `Cutscene_SpaceshipSabotage`

- **Retail bytes:** `$07607E..$076589` inclusive, 1,292 bytes.
- **Pointer:** `CutscenePtrs[$0E]` at `$05A580`; scene event `$800E`.
- **Trigger:** Zelan `$18D`, `RunEventsJmpTbl[$28]`, Wren `$70` and Canceller
  `$72` set, Chaos Sorcerer `$71` clear, leader Y `$2F0`.
- **Data:** `post_zio_cutscenes.rs`, `SPACESHIP_SABOTAGE` (36 ops).

## Clone audit

`ps4.asm:120778` selects the retail pointer in the `grand_cross=0` table, and
the body begins at `ps4.asm:155431`. The table body is not the Grand Cross
hack's alternate story. The actual-ROM pointer pair is `$07607E..$07658A`
(exclusive end).

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-3 | `$07607E..$0760C1` | init/fade; spaceship art; clear temporary object memory | init/assets |
| 4-5 | `$0760C2..$0761F5` | radar SFX; the menu: list from the literal mask `loc_76586` (`$08`, Kuran alone), windows, cursor, loop | `PlaySound`, `DestinationMenu { Fixed($08) }` (the session's destination mode) |
| 6-10 | `$0761F6..$0762F5` | selection sound and the typed name, the 60-tick wait (the menu's own), windows closed, `Map_LoadChunks`, music stopped; `World_Index` written from the row (`:155599`, the session writes it on the confirm) | `PanelDestroyAll`, `FadeOut`, `ReloadMapChunks`, `PlaySound` |
| 11 | `$0762F6..$076354` | takeoff by `loc_64B02` from Zelan: Zelan Space at `($43,$3C)`, `Field_Map_Index_2 = $FFFF` | `LoadMap` |
| 12-20 | `$0763CA..$0764A4` | panels `$7A/$7C`; waits `90/60`; dialogue entry `2`; alert panels | panels/waits/dialogue |
| 21-27 | `$0764A5..$0764E9` | resume dialogue; set Chaos Sorcerer `$71`; saved Red Alert; map flags `$88` | resume/flags/music |
| 28-33 | `$0764EA..$076589` | event battle index `8`; routine-exit return | `StartBattle(8)`, return |

The cancel branch at `loc_764F8` (`ps4.asm:155546`) lands on `loc_64B5A`'s row for the
current map and returns `0`: ops 34 and 35 (`LoadFlightMap { Return }`, `Return`),
reached by `DestinationMenu`'s `cancel_skip`. The menu is the shared one
([Inside the spaceship](41_InsideSpaceship.md)); its mask `$08` lists Kuran alone,
and the confirm writes `World_Index = 4` before the takeoff.

## Verification

`rust/psiv-core/src/scenes/flight.rs` tests that Cancel lands on the return leg and that
a confirm reaches the scene's own takeoff load. The arc test observes `SceneBattleStarted { index: 8 }`, asserts `$71` before
the battle, resolves it, and confirms the runtime is at Zelan Space `$18C`.

