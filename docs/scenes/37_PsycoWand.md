# `Cutscene_PsycoWand`

- **Retail bytes:** `$075200..$075A11` inclusive, 2,066 bytes.
- **Pointer:** `CutscenePtrs[$0A]` at `$05A580`; scene event is `$800A`.
- **Trigger:** Ladea Tower F5 `$91`, `RunEvent_PsycoWandFound` (`$1E`):
  Psycho Wand chest set and After Alys Death 2 `$67` clear.
- **Data:** `post_rika_cutscenes.rs`, `PSYCO_WAND` (110 ops).

## Clone audit

This is the retail `grand_cross=0` cutscene body. The Grand Cross build's
scene include is not evidence for the panel sequence. The final map write at
`$0757D0..$0757FA` proves the often-missed `Map_Start_Char_Align = $10`.

## Retail transcription

The code record keeps every panel creation, DMA and corrected VInt count. The
long raw-dialogue waits are represented by the existing `RunDialogueResume`
edge; the renderer still receives the panel operations in order.

| Op(s) | ROM offset | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-4 | `$075200..$075244` | init/fade, cutscene dialogue 5, load Krup Inn F1 `$3F` from Krup `$3E`, start `($46,$44)`, face left | init/dialogue/map |
| 5-11 | `$07524A..$07533C` | wait `$3B+1=60`; Her Last Breath `$A4`; stage Gryz/Rika/Demi/Rune/Chaz; follow bit; load tree 6 (revision>0); dialogue `$2E` | wait/sound/presentation/tree/dialogue |
| 12-22 | `$075358..$0753B0` | palette/sprite reset, init/fade, panel `$56`, DMA, wait `$59+1=90`, panel `$57`, DMA, wait `$13+1=20` | typed panel sequence |
| 23-28 | `$0753B6..$07542E` | resume raw dialogue; destroy windows/chunks; variable fade; panel reset | resume/presentation/fade |
| 29-37 | `$075434..$075484` | init/fade, wait 60, panel `$5A`, DMA, wait 40, panel `$5B`, DMA, wait 10, resume | panel sequence + resume |
| 38-46 | `$0754C6..$0754FA` | panels `$5D/$5E`, waits 40/120; fade/destroy/chunks; map wait 90 | panel/fade/wait records |
| 47-62 | `$07553E..$0755CC` | panels `$60..$64`, DMA after each; waits 60/40/80/40/300 | panel sequence |
| 63-71 | `$0755D8..$075638` | fade/reset; panels `$65/$66/$67`, waits 60/90/60; resume | panel sequence + resume |
| 72-80 | `$07563E..$0756F6` | destroy three active panels, DMA, wait 90; panels `$69/$6A`, waits 60/20; resume | panel sequence + resume |
| 81-87 | `$075700..$0757CA` | panel `$6C`, wait 120, fade, stop music `$FB`, destroy all, `RecoverStats`, wait 60 | final panel/wait/state |
| 88-93 | `$0757D0..$075840` | load Krup `$39`, previous Motavia `$00`, start `($28,$1C)`, face left, align `$10`; `GetMapLayoutOffset` writes FG (9,5)←`$8F` and BG (9,6)←`$90`, then `RefreshPlaneBG` | `LoadMap`, `WriteMapChunks`, reload chunks |
| 93-97 | `$075840..$0758E4` | load art `$2B8`; stage temporary objects; Motabia Village `$83`; fade | art/object/sound/fade |
| 98-105 | `$0758EA..$0759A8` | temporary movements and map-update `$3B+1=60`; load tree 6 again (revision>0); resume dialogue | presentation/tree/dialogue/wait |
| 106-108 | `$0759F2..$075A10` | set After Alys Death `$63` and `$67`; load tree 5; return 1 | flags/tree/return |

`RecoverStats` now writes the occupied roster records (HP, TP and status) and
also emits the normal typed roster effects. The temporary Krup objects remain
presentation records; they are not map NPCs and do not alter the census.

## Revision-conditional ops

The two `move.l #DialogueTree6, d0 / jsr (DialogueTreesToRAM).l` calls before
dialogue `$2E` and before the late `popdlg` resume (`ps4.asm:154633`,
`:154938`), and the tree-5 restore at `:155002`, all sit under `if revision>0`;
this build is English (`revision = 1`), so all three run. The first two were
missed until H17. See the [revision audit](REVISION_AUDIT.md).

## Verification

The arc test enters this scene after the Psycho Wand battle, asserts both
Alys-death flags and map `$39`, and checks that the later Zio scene starts with
the recovered roster.
