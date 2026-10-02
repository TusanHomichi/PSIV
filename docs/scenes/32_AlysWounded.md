# `Cutscene_AlysWounded`

- **Retail bytes:** `$074B72..$0751FF` inclusive, 1,678 bytes.
- **Pointer:** `CutscenePtrs[$09]` at `$05A580`; scene event is `$8009`.
- **Trigger:** Zio Fort F4 `$8B`, `RunEvent_AlysWounded` (`$1D`), after
  `EventFlag_Zio` (`$42`) is set and before `EventFlag_DemiJoined` (`$47`).
- **Data:** `post_rika_cutscenes.rs`, `ALYS_WOUNDED` (57 ops).

## Clone audit

This is a retail-body transcription from the `grand_cross=0` branch. The
Grand Cross scene include is not used to fill the range. The source's first
dialogue is the raw `GetDialogueByID/RunText` path at `$074B86`; the runner
records that as a cutscene dialogue edge and resumes it with the existing
typed dialogue effect.

## Retail transcription

| Op(s) | ROM offset | Retail bytes / behaviour | Scene record |
|---:|---|---|---|
| 0-3 | `$074B72..$074BB0` | init, fade, sprites on, raw entry `$43` | init/fade/render/dialogue |
| 4-8 | `$074BC8..$074C0E` | teleport SFX, variable fade, panel/window teardown, chunk reload, `DoMapUpdateLoop($3B)` | presentation + `Wait(60)` |
| 9-15 | `$074C14..$074C4A` | init/fade, panel `$4A`, DMA, VInt `$45+1=70`, resume dialogue | panel/wait/dialogue ops |
| 16-18 | `$074C4E..$074C62` | teleport SFX, `DoMapUpdateLoop($45+1=70)`, test Saya `$12` | sound/wait/`BranchFlag` |
| 19 | `$074C6C..$074C96` | first branch loads Krup Inn F1 `$3F`, previous Motavia `$00`, start `($4C,$6C)`, up, align 4 | `LoadMap` |
| 20-21 | `$074C9C..$074CF0` | Motabia Village `$83`; construct healing object `$8194`, art `$2B8` | sound + `ObjectAnimation` |
| 22-25 | `$074D5A..$074E40` | load tree 6 (revision>0), dialogue `$2C`, two `popdlg` continuations | tree load + dialogue + resumes |
| 26 | `$074EC4` | set Saya `$12` | `SetFlag($12)` |
| 27-33 | `$074ECA..$074F1E` | Pain `$A5`; remove Hahn `$02`, clear status; remove Alys `$01`, clear status; seat Demi `$06` in slot 4; add macro | state ops |
| 34-36 | `$074F24..$075026` | load Krup Inn F1 `$3F`, previous Krup `$3E`, start `($46,$44)`, right; fade; camera to staged party | `LoadMap`, `FadeIn`, `MoveCamera` |
| 37-48 | `$075044..$07519E` | load tree 6 again (revision>0), dialogue `$2D` plus ten `popdlg` continuations, with the source's facing choreography between them | tree load + dialogue + ten `RunDialogueResume` |
| 49-53 | `$0751A8..$0751E2` | move party to `($210,$210)` then `($210,$220)`, restore follow/camera state | follow + leader moves/camera |
| 54-56 | `$0751E8..$0751FE` | set Demi Joined `$47`, load tree 5, return 1 | flag/tree/return |

The remove operations compact the party slots in source order. Starting from
`[Chaz, Alys, Hahn, Gryz, Rika]`, the common tail therefore produces
`[Chaz, Gryz, Rika, Demi]`. The Saya branch is real control flow: if `$12` is
already set, the ROM enters the common tail at `$074ECA` and does not replay
the inn/healing section.

## Revision-conditional ops

The two `move.l #DialogueTree6, d0 / jsr (DialogueTreesToRAM).l` calls before
dialogues `$2C` and `$2D` (`ps4.asm:154305`, `:154465`), and the tree-5 restore
at `:154558`, all sit under `if revision>0`; this build is English
(`revision = 1`), so all three run. The first two were missed until H17 — the
Saya branch then read Krup Inn F1's bound tree 5, whose entries 44 to 46 are
empty, and the resume after dialogue `$2C` had no cursor. See the
[revision audit](REVISION_AUDIT.md).

## Verification

The headless chain runs both cutscenes with `$12` clear, asserts the map recast
and party `[Chaz, Gryz, Rika, Demi]`, then continues to the Land Rover event.
