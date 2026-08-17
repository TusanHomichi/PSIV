# `Cutscene_Ending`

- **Retail bytes:** `$078F3E..$07A811` inclusive, 6,356 bytes of executable
  routine, helpers and inline credits data.
- **Padding:** `$07A812..$07AFFF`; the next aligned panel record is at
  `$07B000`.
- **Pointer:** `CutscenePtrs[$21]` at `$05A580`; scene event `$8021`.
- **Dispatch:** `RunEventsJmpTbl[$54]` (`RunEvent_Ending`), after event flag
  `$E8` is set by the Profound Darkness battle handoff.
- **Data:** `retail_endgame.rs`, `ENDING` (367 typed ops).

All ranges in this record are inclusive. The cartridge is the USA revision
(`revision=1`) and the disassembly's retail branch is `grand_cross=0`. The ROM
used here is `Phantasy Star IV (USA).md`, SHA-256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`.

## Entry branch and normal post-battle path

The ending is not the `$8020` battle request with a longer return value. It is
a separate presentation routine. The first bytes make that boundary explicit:

| ROM offset | Retail operation | Typed record |
|---|---|---|
| `$078F3E` | `DialogueTreesToRAM(#$1FC920)`; load dialogue tree 42 | `SetDialogueTree(0x001F_C920)` |
| `$078F4A..$078F50` | On USA/EU, test held Menu (`ButtonCamp`); branch to `$079D0C` when held | documented shortcut; not taken by the normal scene runner |
| `$078F54` | play `Music_Explosion` (`$AD`) | `PlaySound($AD)` |
| `$078F5C` | `InitVRAMAndCRAM` | `InitVramAndCram` |
| `$078F62` | `Pal_FadeIn` | `FadeIn` |
| `$078F68..$078F90` | fetch dialogue entry `$09`, create the window, run `RunText3` | `RunDialogue(9, EndingIntro)` |
| `$078F96` | `DMAPlane_A_VInt` | `DmaPlanes` |
| `$078F9C..$078FA8` | `Pal_IncreaseTone` / VInt loop, raw `#$3B` | `WaitFrames(60)` plus the presentation fade |
| `$078FAE..$078FCC` | clear panel counter; destroy the optional portrait window and the text window | `WindowDestroy` |
| `$078FD6` | `Map_LoadChunks` from the current map chunk pointer | `ReloadMapChunks` |
| `$078FDC..$078FE2` | raw `#$77` `VInt_PrepareLoop` | `WaitFrames(120)` |
| `$078FE6..$078FF4` | initialise VRAM/CRAM, play `Music_ThePromisingFuture1` (`$AF`), fade | init/music/fade |

The Menu shortcut is real retail behaviour. It skips the normal panel/dialogue
sequence and enters the credits renderer at `$079D0C`. The typed normal path
does not pretend that a held-menu sample exists: the current runtime exposes
the final Start gate, but not a separate “held Menu at scene entry” input. The
shortcut and its exact target remain recorded here rather than silently
claiming parity.

## Panel stream: one retail create op per record

The source addresses below are the literal `move.w #panel_id,d0` that feeds
`Panel_Create`; the call itself is four bytes later. Every create is followed by
the recorded `DMAPlanes_VInt` unless the next row says otherwise. The panel
pack grew from 178 to 228 records in this wave: these are the 50 ending-only
IDs, including the easy-to-miss `$17E` final story panel.

| Order | Literal-load offset | Panel ID |
|---:|---|---:|
| 1 | `$078FFA` | `$147` |
| 2 | `$079010` | `$148` |
| 3 | `$07902A` | `$149` |
| 4 | `$079044` | `$14A` |
| 5 | `$07905E` | `$14B` |
| 6 | `$07909A` | `$14C` |
| 7 | `$0790BA` | `$14D` |
| 8 | `$0790D4` | `$14E` |
| 9 | `$0790EE` | `$14F` |
| 10 | `$079120` | `$150` |
| 11 | `$07914E` | `$187` |
| 12 | `$079188` | `$151` |
| 13 | `$0791FA` | `$182` |
| 14 | `$079250` | `$152` |
| 15 | `$079292` | `$153` |
| 16 | `$0792AC` | `$154` |
| 17 | `$0792C6` | `$155` |
| 18 | `$0792FE` | `$156` |
| 19 | `$079348` | `$157` |
| 20 | `$079362` | `$158` |
| 21 | `$079390` | `$159` |
| 22 | `$0793AA` | `$15A` |
| 23 | `$07942E` | `$188` |
| 24 | `$079492` | `$15D` |
| 25 | `$0794CA` | `$15F` |
| 26 | `$07951A` | `$160` |
| 27 | `$079534` | `$161` |
| 28 | `$07954E` | `$162` |
| 29 | `$079568` | `$163` |
| 30 | `$0795AE` | `$164` |
| 31 | `$0795C8` | `$189` |
| 32 | `$0795E2` | `$165` |
| 33 | `$079624` | `$166` |
| 34 | `$07963E` | `$167` |
| 35 | `$07966C` | `$168` |
| 36 | `$0796A8` | `$169` |
| 37 | `$0796FC` | `$16A` |
| 38 | `$079746` | `$16B` |
| 39 | `$079760` | `$16C` |
| 40 | `$07977A` | `$16D` |
| 41 | `$0797F4` | `$16E` |
| 42 | `$07980E` | `$16F` |
| 43 | `$079832` | `$170` |
| 44 | `$079884` | `$171` |
| 45 | `$0798FE` | `$173` |
| 46 | `$07997E` | `$175` |
| 47 | `$0799FE` | `$177` |
| 48 | `$079A7E` | `$179` |
| 49 | `$079B04` | `$17C` |
| 50 | `$079B84` | `$17E` |

The destroy calls are stack-pop operations (`Panel_Destroy` with no literal
ID), not a guessed panel identity. Their call offsets are:

```
$079176 $0791A6 $0791BC $0791D2 $0791E8
$07926E $079274 $07927A $079280
$07931C $079322 $079328 $07932E
$079404 $07940A $079410 $079416 $07941C
$07946A $079480
$0794FC $079502 $079508
$07959A $0795A0 $0795A6
$079600 $079606 $07960C $079612
$07968A $079690 $079696
$079738 $07973E
$0797B6 $0797CC $0797E2
$0798C8 $0798DE
$079948 $07995E
$0799C8 $0799DE
$079A48 $079A5E
$079AC8 $079ACE $079AE4
$079B4E $079B64
```

The source's “destroy all” runs are therefore represented as repeated
`PanelDestroyLast` records, preserving the retail stack and the intervening
DMA/wait order. The pre-panel window destroys at `$078FC0` and `$078FCC` are
different: they are `Window_Destroy`, not panel operations.

## Every explicit wait

`VInt_PrepareLoop` is a `dbra` loop. The typed count is the corrected frame
count (`raw d0 + 1`), not the immediate copied from the source. The following
is the complete wait census for the normal path; the bare VInt and DMA loop at
`$079866..$07986E` are called out separately because they have no immediate
wait word.

| ROM offset | Raw `d0` | Typed frames | ROM offset | Raw `d0` | Typed frames |
|---|---:|---:|---|---:|---:|
| `$078FDC` | `$77` | 120 | `$07900A` | `$EF` | 240 |
| `$079024` | `$81` | 130 | `$07903E` | `$31` | 50 |
| `$079058` | `$31` | 50 | `$079072` | `$27` | 40 |
| `$079094` | `$77` | 120 | `$0790B4` | `$77` | 120 |
| `$0790CE` | `$77` | 120 | `$0790E8` | `$3B` | 60 |
| `$079102` | `$13` | 20 | `$07911A` | `$3B` | 60 |
| `$079134` | `$1D` | 30 | `$079182` | `$13` | 20 |
| `$07919C` | `$EF` | 240 | `$0791B2` | `$09` | 10 |
| `$0791C8` | `$09` | 10 | `$0791DE` | `$09` | 10 |
| `$0791F4` | `$B3` | 180 | `$07920E` | `$13` | 20 |
| `$07922C` | `$77` | 120 | `$07924A` | `$13` | 20 |
| `$079264` | `$B3` | 180 | `$07928C` | `$1D` | 30 |
| `$0792A6` | `$59` | 90 | `$0792C0` | `$C7` | 200 |
| `$0792DA` | `$B3` | 180 | `$0792F8` | `$09` | 10 |
| `$079312` | `$EF` | 240 | `$07933A` | `$3B` | 60 |
| `$07935C` | `$59` | 90 | `$07938A` | `$1D` | 30 |
| `$0793A4` | `$59` | 90 | `$0793BE` | `$13` | 20 |
| `$0793DC` | `$3B` | 60 | `$0793FA` | `$77` | 120 |
| `$079428` | `$3B` | 60 | `$079442` | `$13` | 20 |
| `$079460` | `$1D` | 30 | `$079476` | `$09` | 10 |
| `$07948C` | `$3B` | 60 | `$0794A6` | `$3B` | 60 |
| `$0794C4` | `$3B` | 60 | `$0794DE` | `$3B` | 60 |
| `$079514` | `$3B` | 60 | `$07952E` | `$1D` | 30 |
| `$079548` | `$1D` | 30 | `$079562` | `$77` | 120 |
| `$079590` | `$9F` | 160 | `$0795C2` | `$1D` | 30 |
| `$0795DC` | `$77` | 120 | `$0795F6` | `$77` | 120 |
| `$07961E` | `$3B` | 60 | `$079638` | `$1D` | 30 |
| `$079666` | `$3B` | 60 | `$079680` | `$77` | 120 |
| `$0796A2` | `$3B` | 60 | `$0796BC` | `$77` | 120 |
| `$0796F6` | `$1D` | 30 | `$079710` | `$13` | 20 |
| `$07972E` | `$9F` | 160 | `$07975A` | `$1D` | 30 |
| `$079774` | `$1D` | 30 | `$07978E` | `$3B` | 60 |
| `$0797AC` | `$77` | 120 | `$0797C2` | `$09` | 10 |
| `$0797D8` | `$09` | 10 | `$0797EE` | `$1D` | 30 |
| `$079808` | `$13` | 20 | `$079822` | `$12B` | 300 |
| `$07985C` | `$63` | 100 | `$079866` | bare VInt | 1 |
| `$07986C` | `$3B` DMA loop | 60 | `$0798B2` | `$1D` | 30 |
| `$0798D4` | `$1D` | 30 | `$0798EA` | `$3B` | 60 |
| `$079932` | `$1D` | 30 | `$079954` | `$1D` | 30 |
| `$07996A` | `$3B` | 60 | `$0799B2` | `$1D` | 30 |
| `$0799D4` | `$1D` | 30 | `$0799EA` | `$3B` | 60 |
| `$079A32` | `$1D` | 30 | `$079A54` | `$1D` | 30 |
| `$079A6A` | `$3B` | 60 | `$079AB2` | `$1D` | 30 |
| `$079ADA` | `$1D` | 30 | `$079AF0` | `$3B` | 60 |
| `$079B38` | `$1D` | 30 | `$079B5A` | `$1D` | 30 |
| `$079B70` | `$3B` | 60 | `$079CAE` | `$B3` | 180 |
| `$079CC0` | `$B3` | 180 | `$079CFC` | `$3B` | 60 |

## Dialogue, palette and staff-roll transitions

| ROM offset | Retail bytes | Typed record |
|---|---|---|
| `$07907C..$07908A` | entry `$0A`, `Event_GetAndRunDialogue4` | `RunDialogue(10, Ending)` |
| `$07910C..$079114` | entry `$0B`, `Event_GetAndRunDialogue4` | `RunDialogue(11, Ending)` |
| `$07913E..$0791E8` | repeated `popdlg` / `Event_RunDialogue4` resumes between panels | `RunDialogueResumeWithWindow(Ending)` |
| `$079218..$07924E` | two more saved-dialogue resumes around panel `$182` | same typed resume |
| `$0792E4..$0797E2` | saved dialogue resumes interleaved with the story-panel stream | same typed resume |
| `$0798A4..$0798AC` | entry `$0C`, `Event_GetAndRunDialogue4` | `RunDialogue(12, Ending)` |
| `$079C16..$079C88` | staff entries `$12`, `$13`, `$14`, `$15`; `RunText2` to `$FFFF8406`, `$8586`, `$8706`, `$8886`; DMA target `$041682` | four `DrawTextToPlane` records |
| `$079CA6..$079CEE` | decrement `Palette_Table_Buffer+$5E` by `$222` every 8 frames, wait 180, switch to `Music_StaffRoll` (`$AE`), wait 180, increment until bit `$C` | `EndingStaffRollTransition` |
| `$079CFA..$079D02` | final 60-frame plane-update loop before credits | transition `final_hold: 60` |

The final story panel `$17E` uses palette mode 5 while its dialogue resumes;
the source then switches to mode `$0F`, stops music (`$FE`), raises the palette
for 60 frames, clears the palette scratch and both planes, and draws the four
staff entries. Those are separate typed operations, not an inferred “fade to
credits.”

## Credits renderer (`$079D0C`)

The shortcut and the normal path meet here. The routine is a real tile/plane
renderer, not a bitmap placeholder:

| ROM offset / data | Retail operation | Typed record |
|---|---|---|
| `$079D0C` | clear `TextCounter`, credit counters and FG camera; set BG camera Y `$FF20` | `EndingCreditsAssets` prelude |
| `$1DE278` -> VRAM `$2000` | Kos-decompress small planet | asset field |
| `$1DEA48` -> VRAM `$4000` | Kos-decompress large planet | asset field |
| `$07A26C` / `$2A3350` -> tile `$07C0` | read the credit-font tile command table and upload `ArtNem_CreditFont` | asset field |
| `$1DF09C` -> `$FFFF1000` | Enigma mapping, 0x100 words | stage 1 source |
| `$1DF17C` -> `$FFFF1380` | Enigma mapping, `$80` words at the retail offset | stage support data |
| `$1DF294` -> `$FFFF1700` | Enigma mapping, `$C0` words | stage support data |
| `$1DF3E0` -> `$FFFF1A80` and `$FFFF1E00` | Enigma mapping for Plane B and its companion | stage support data |
| `$1DF4C2` -> `$FFFF2000` | Enigma mapping, `$2200` bytes | stages 2 and 3 source |
| `$079E70..$079F14` | stage 1: `TextCounter` terminal `$380`, BG step `$2000`, source `$FFFF1000`, Plane B, command table `$07A522` | `EndingCreditsStage(1)` |
| `$079F26..$079FCE` | stage 2: terminal `$400`, FG step `$2000`, BG step `$1000`, source `$FFFF2000`, Plane A, commands `$07A522` | `EndingCreditsStage(2)` |
| `$079FAE..$07A086` | stage 3: terminal `$480`, same camera steps/source/plane, commands `$07A548` | `EndingCreditsStage(3)` |
| `$07A0A6..$07A0FA` | fourteen palette steps: eight-word primary table `$07A274`, then two-word late table `$07A2A0` | `EndingCreditsPaletteRamp` |
| `$07A0FC..$07A102` | palette increase-tone loop, raw `$3B` | `PaletteIncreaseTone(60)` |

The typed stage records retain the terminal counters, camera increments, RAM
source and command-table addresses. They do not claim that a generic sprite
scroller is equivalent to the retail plane map loops.

## Final Termi field and the actual end

| ROM offset | Retail bytes | Typed record |
|---|---|---|
| `$07A13A..$07A15E` | load map `$07B`, previous map `$FFFF`, start `$36,$28`, facing down, align 0; clear map-load bit 3; `RefreshMap` | `LoadMap` |
| `$07A164..$07A1A6` | clear `Character_1` `$FFFFC000` (0x50 longwords), secondary `$FFFFC300` (0x70), scratch `$FFFFC540` (0x10); rebuild Plane A/B, width `$18`, height `$1C`, flags `$8002` | `EndingFinaleFieldPrep` |
| `$07A1A6` | copy 64 words `$FFFFFB00` -> `$FFFFFB80` | `CopyRamWords` |
| `$07A1C4` | fill 64 words at `$FFFFFB00` with `$0EEE` | `FillRamWords` |
| `$07A1E0..$07A20E` | upload `ArtNem_Fin` `$1DEEB8` to tile `$580`; Enigma map `$1DF52C` -> VRAM `$C580`; palette `$1DF59A` -> Palette 2 line 3 | `EndingFinale` |
| `$07A218..$07A21E` | raw `$27` lightning loop, corrected 40 frames | `EndingFinale.lightning_frames` |
| `$07A228..$07A25A` | update field objects and wait for `ButtonStart` | `WaitForStart` |
| `$07A25A` | write `Game_Cleared_Flag` at `$200035` to 1, set mode `$03`, run palette variable fade out, jump main loop | `MarkGameCleared`, final fade, `End` |

The runtime therefore proves both edges: it exposes `WaitForStart` and only
then latches `GameCleared`. The headless test releases that gate explicitly,
asserts `$E8`, Termi `$07B`, the `$17E` panel and `game_cleared()`, and never
pretends that the credits are gameplay dialogue.

## Census delta

| Surface | Before this wave | After this wave | Delta |
|---|---:|---:|---:|
| Registered typed scenes | 86 | 89 | +3 (`$8012`, `$801B`, `$8021`) |
| Presentation panel records | 178 | 228 | +50 ending panels |
| Raja Sick transcription | boundary placeholder | 54 ops | promoted |
| Rykros transcription | boundary placeholder | 27 ops | promoted |
| Ending transcription | boundary placeholder | 367 ops | promoted |

The remaining direct Raja/Gyuna, Rykros travel gate, guild, Alys tower and
fifth-character controls are audited in [88](88_RetailBoundaries.md). Their
input/roster state machines are not silently flattened into this linear
presentation record.
