# `Cutscene_PsycoWand`

- **Retail bytes:** `$075200..$075A11` inclusive, 2,066 bytes.
- **Pointer:** `CutscenePtrs[$0A]` at `$05A580`; scene event is `$800A`.
- **Trigger:** Ladea Tower F5 `$91`, `RunEvent_PsycoWandFound` (`$1E`):
  Psycho Wand chest set and After Alys Death 2 `$67` clear.
- **Data:** `post_rika_cutscenes.rs`, `PSYCO_WAND` (107 ops).

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
| 5-10 | `$07524A..$07534E` | wait `$3B+1=60`; Her Last Breath `$A4`; stage Gryz/Rika/Demi/Rune/Chaz; follow bit; dialogue `$2E` | wait/sound/presentation/dialogue |
| 11-21 | `$075358..$0753B0` | palette/sprite reset, init/fade, panel `$56`, DMA, wait `$59+1=90`, panel `$57`, DMA, wait `$13+1=20` | typed panel sequence |
| 22-27 | `$0753B6..$07542E` | resume raw dialogue; destroy windows/chunks; variable fade; panel reset | resume/presentation/fade |
| 28-36 | `$075434..$075484` | init/fade, wait 60, panel `$5A`, DMA, wait 40, panel `$5B`, DMA, wait 10, resume | panel sequence + resume |
| 37-45 | `$0754C6..$0754FA` | panels `$5D/$5E`, waits 40/120; fade/destroy/chunks; map wait 90 | panel/fade/wait records |
| 46-61 | `$07553E..$0755CC` | panels `$60..$64`, DMA after each; waits 60/40/80/40/300 | panel sequence |
| 62-70 | `$0755D8..$075638` | fade/reset; panels `$65/$66/$67`, waits 60/90/60; resume | panel sequence + resume |
| 71-79 | `$07563E..$0756F6` | destroy three active panels, DMA, wait 90; panels `$69/$6A`, waits 60/20; resume | panel sequence + resume |
| 80-86 | `$075700..$0757CA` | panel `$6C`, wait 120, fade, stop music `$FB`, destroy all, `RecoverStats`, wait 60 | final panel/wait/state |
| 87-91 | `$0757D0..$075840` | load Krup `$39`, previous Motavia `$00`, start `($28,$1C)`, face left, align `$10`; patch map chunks | `LoadMap`, reload chunks |
| 92-96 | `$075840..$0758E4` | load art `$2B8`; stage temporary objects; Motabia Village `$83`; fade | art/object/sound/fade |
| 97-103 | `$0758F0..$0759A8` | load current dialogue tree; resume dialogue; temporary movements and map-update `$3B+1=60` | presentation/dialogue/wait |
| 104-106 | `$0759F2..$075A10` | set After Alys Death `$63` and `$67`; load tree 5; return 1 | flags/tree/return |

`RecoverStats` now writes the occupied roster records (HP, TP and status) and
also emits the normal typed roster effects. The temporary Krup objects remain
presentation records; they are not map NPCs and do not alter the census.

## Verification

The arc test enters this scene after the Psycho Wand battle, asserts both
Alys-death flags and map `$39`, and checks that the later Zio scene starts with
the recovered roster.
