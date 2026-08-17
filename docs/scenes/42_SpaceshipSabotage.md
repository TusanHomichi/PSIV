# `Cutscene_SpaceshipSabotage`

- **Retail bytes:** `$07607E..$076589` inclusive, 1,292 bytes.
- **Pointer:** `CutscenePtrs[$0E]` at `$05A580`; scene event `$800E`.
- **Trigger:** Zelan `$18D`, `RunEventsJmpTbl[$28]`, Wren `$70` and Canceller
  `$72` set, Chaos Sorcerer `$71` clear, leader Y `$2F0`.
- **Data:** `post_zio_cutscenes.rs`, `SPACESHIP_SABOTAGE` (40 ops).

## Clone audit

`ps4.asm:120778` selects the retail pointer in the `grand_cross=0` table, and
the body begins at `ps4.asm:155431`. The table body is not the Grand Cross
hack's alternate story. The actual-ROM pointer pair is `$07607E..$07658A`
(exclusive end).

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-3 | `$07607E..$0760C1` | init/fade; spaceship art; clear temporary object memory | init/assets |
| 4-9 | `$0760C2..$0761CE` | build destination list from `loc_63EF2`; radar SFX; menu windows/cursors | typed presentation/window records |
| 10-15 | `$0761F6..$0762C8` | selected-name window; stop radar SFX; `60` update ticks; close/fade/reload | selection/wait/reload |
| 16-17 | `$0762F6..$076354` | Zelan route loads Zelan Space at `($43,$3C)` from Zelan | `LoadMap` |
| 18-26 | `$0763CA..$0764A4` | panels `$7A/$7C`; waits `90/60`; dialogue entry `2`; alert panels | panels/waits/dialogue |
| 27-33 | `$0764A5..$0764E9` | resume dialogue; set Chaos Sorcerer `$71`; saved Red Alert; map flags `$88` | resume/flags/music |
| 34-39 | `$0764EA..$076589` | event battle index `8`; routine-exit return | `StartBattle(8)`, return |

The source has a cancel branch at `loc_764F8` that restores the current
spaceport. The scene record follows the selected Zelan branch used by the
headless arc; cancel and the other world choices remain documented helper
branches, not silently collapsed into a universal Zelan fact.

## Verification

The arc test observes `SceneBattleStarted { index: 8 }`, asserts `$71` before
the battle, resolves it, and confirms the runtime is at Zelan Space `$18C`.

