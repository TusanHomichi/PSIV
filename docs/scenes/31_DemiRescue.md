# `Cutscene_DemiRescue`

- **Retail bytes:** `$074A7E..$074B71` inclusive, 244 bytes.
- **Pointer:** `CutscenePtrs[$08]` at `$05A580`; scene event is `$8008`.
- **Trigger:** Zio Fort F4 `$8B`, `RunEvent_SavingDemi` (`$1C`), leader Y
  exactly `$170`, with `EventFlag_Zio` (`$42`) clear.
- **Data:** `post_rika_cutscenes.rs`, `DEMI_RESCUE` (26 ops).

## Clone audit

The retail body is selected by the `grand_cross=0` scene build. The
`grand_cross=1` side is not used as behaviour authority. The cartridge pointer
range is the proof anchor; the body ends at `moveq #1; rts` `$074B70`.

## Retail transcription

| Op | ROM offset | Retail primitive / literal | Scene op |
|---:|---|---|---|
| 0 | `$074A7E` | `InitVRAMAndCRAM` | `InitVramAndCram` |
| 1 | `$074A84` | `Pal_FadeIn` | `FadeIn` |
| 2 | `$074A8A` | render-sprites byte ← 1 | `SetRenderSpritesInCutscene(true)` |
| 3 | `$074A8A..$074A97` | `Event_GetAndRunDialogue3`, entry `$42` | cutscene dialogue `$42` |
| 4-5 | `$074A98..$074AA3` | init/fade second panel phase | `InitVramAndCram`, `FadeIn` |
| 6-7 | `$074AA4..$074AB3` | `Panel_Create($40)`, DMA | `PanelCreate`, `DmaPlanes` |
| 8-9 | `$074AB4..$074AC5` | sword SFX `$F5`; VInt loop `$3B+1` | `PlaySound`, `WaitFrames(60)` |
| 10-13 | `$074AC6..$074ADE` | destroy panel twice, DMA after each | two `PanelDestroy`, two `DmaPlanes` |
| 14-17 | `$074AE2..$074AFC` | wait 10; create `$42`, DMA; wait 20 | panel/wait ops |
| 18-19 | `$074B0A..$074B1A` | enable sprites; resume saved dialogue | render + `RunDialogueResume` |
| 20 | `$074B1E..$074B46` | party loop: `curr_hp += $1C`, clamp to max, clear status | `RestorePartyHp(0x1C)` |
| 21 | `$074B48` | set `EventFlag_Zio` `$42` | `SetFlag($42)` |
| 22 | `$074B50` | save Red Alert `$A9` | `SetSavedMusic($A9)` |
| 23 | `$074B5A` | set map-load bits `$80` and `$08` | `SetMapLoadFlags($88)` |
| 24 | `$074B62` | battle index 4 and routine-exit bit | `StartBattle(4)` |
| 25 | `$074B6E` | `moveq #1; rts` | `Return(1)` |

The roster write is intentionally state-bearing. It clears status for each
occupied party record just as the cartridge loop does; the battle hand-off is
blocked until the runtime supplies its completion edge.

## Verification

The post-Rika headless arc starts this scene with four occupied party slots,
resolves event battle 4, and asserts Zio `$42` before entering Alys Wounded.
