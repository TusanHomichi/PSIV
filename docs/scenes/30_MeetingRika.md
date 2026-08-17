# `Cutscene_MeetingRika`

- **Retail bytes:** `$0745DE..$074A7D` inclusive, 1,184 bytes.
- **Pointer:** `CutscenePtrs[$07]` at `$05A580`; scene event is `$8007`.
- **Entry map:** BioPlant B4 Part2 `$AC`, trigger `$1A`; the trigger requires
  `EventFlag_BioPlantEscape` (`$34`) clear and leader Y exactly `$1A0`.
- **Data:** `next_arc_followup.rs`, `MEETING_RIKA` (107 ops).

## Clone audit

The `grand_cross=0` cutscene body is present in the clone and matches the
retail pointer range. The Grand Cross branch includes
`script/scenes/MeetingRika/event.asm`, but that include is not retail byte
authority. `DialogueTree34` is loaded from `$1F7190`, the address embedded at
`$0749A2`.

## Retail transcription

The table is intentionally long. This scene is the first one in the chain
where collapsing choreography into “play a cutscene” would lose map state,
party state or the exact two-stage dialogue tree swap.

| Op | ROM offset | Retail primitive / literal | Scene op |
|---:|---|---|---|
| 0 | `$0745DE..$0745F1` | compare leader/NPC-0 X; skip or align | `BranchIfAligned(X)` |
| 1 | `$0745F2..$0745FB` | move NPC-0 to leader X, retaining its Y | `MoveActorToActorAxis(NPC-0, leader, X)` |
| 2 | `$0745FC..$074603` | face NPC-0 down | `Face(NPC-0, Down)` |
| 3 | `$074604..$074609` | `InitVRAMAndCRAM` | `InitVramAndCram` |
| 4 | `$07460A..$07460F` | `Pal_FadeIn` | `FadeIn` |
| 5 | `$074610..$074617` | render sprites in cutscenes ← 1 | `SetRenderSpritesInCutscene(true)` |
| 6 | `$074618..$07461D` | `Event_GetAndRunDialogue5`, entry 0 | cutscene dialogue entry `$00` |
| 7 | `$07461E..$074625` | `Sound_Index ← MusicID_DungeonArrange1 ($91)` | `PlaySound($91)` |
| 8 | `$074626..$07462B` | `Saved_Sound_Index ← $91` | `SetSavedMusic($91)` |
| 9 | `$07462C..$074631` | set `Map_Load_Flags` bit 3 | `SetMapLoadFlags(set: $08)` |
| 10 | `$074632..$074637` | `RefreshMap` on the current B4 Part2 map | `Presentation(RebuildSprites)` |
| 11 | `$074638..$074645` | construct/run Rika object `$C340`, id `$18`, art `$26A` | `ObjectAnimation(slot 1)` |
| 12 | `$074646..$074651` | load/build sprites; one VInt prep | `Presentation(RebuildSprites)` |
| 13 | `$074652..$074657` | bare `VInt_Prepare` | `WaitFrames(1)` |
| 14 | `$074658..$07465D` | `Pal_FadeIn` | `FadeIn` |
| 15 | `$07465E..$07466F` | move Rika object to `($210,$140)` | `MoveActorTo(NPC-1)` |
| 16 | `$07467C..$07467F` | clear step offset | `SetStepOffset(0)` |
| 17 | `$074670..$074689` | move secondary object to `($200,$140)` | `MoveActorTo(NPC-0)` |
| 18 | `$074686..$07468B` | restore step offset | `SetStepOffset(1)` |
| 19 | `$07468C..$07469D` | move party to `($200,$160)` | `MoveActorTo(leader)` |
| 20 | `$07469E..$0746A3` | `PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 21 | `$0746A4..$0746D3` | load map `$AD`, previous `$AC`, start `($3C,$4C)`, up, align 4; clear bit 3 | `LoadMap` |
| 22 | `$0746D4..$0746D9` | `Pal_FadeIn` | `FadeIn` |
| 23 | `$0746DA..$0746EB` | move party to `($1E0,$160)` | `MoveActorTo(leader)` |
| 24 | `$0746EC..$0746F5` | `DoMapUpdateLoop($1D)` = 30 ticks | `Wait(30)` |
| 25 | `$0746F6..$0746FB` | render sprites in cutscenes ← 0 | `SetRenderSpritesInCutscene(false)` |
| 26 | `$0746FC..$074701` | `Event_GetAndRunDialogue`, entry 1 | standard dialogue entry `$01` |
| 27 | `$074702..$074707` | `InitVRAMAndCRAM` | `InitVramAndCram` |
| 28 | `$074708..$07470D` | `Pal_FadeIn` | `FadeIn` |
| 29 | `$07470E..$074717` | `Panel_Create($33)` | `PanelCreate($33)` |
| 30 | `$074718..$07471D` | `DMAPlanes_VInt` | `DmaPlanes` |
| 31 | `$07471E..$074727` | `VInt_PrepareLoop($27)` = 40 frames | `WaitFrames(40)` |
| 32 | `$074728..$074731` | `Panel_Create($34)` | `PanelCreate($34)` |
| 33 | `$074732..$074737` | `DMAPlanes_VInt` | `DmaPlanes` |
| 34 | `$074738..$074741` | `VInt_PrepareLoop($13)` = 20 frames | `WaitFrames(20)` |
| 35 | `$07474A..$07474F` | render sprites in cutscenes ← 1 | `SetRenderSpritesInCutscene(true)` |
| 36 | `$074742..$074755` | `popdlg`; `Event_RunDialogue5` | `RunDialogueResume` |
| 37 | `$074756..$07475B` | set map-load bit 3 | `SetMapLoadFlags(set: $08)` |
| 38 | `$07475C..$074761` | `RefreshMap` | `Presentation(RebuildSprites)` |
| 39 | `$074762..$074767` | `Pal_FadeIn` | `FadeIn` |
| 40 | `$074768..$074779` | set camera-lock bit 2 in `Char_Move_Flags` | `SetFollowMode(bits: $04)` |
| 41 | `$07476C..$07477F` | move party to `($1F0,$240)` | `MoveActorTo(leader)` |
| 42 | `$074780..$074791` | move secondary object to `($1E0,$240)` | `MoveActorTo(NPC-0)` |
| 43 | `$074792..$074799` | clear step offset | `SetStepOffset(0)` |
| 44 | `$07479A..$0747A7` | move Rika object to `($1F0,$170)` | `MoveActorTo(NPC-1)` |
| 45 | `$0747A8..$0747AD` | restore step offset | `SetStepOffset(1)` |
| 46 | `$0747AE..$0747B7` | `DoMapUpdateLoop($1D)` = 30 ticks | `Wait(30)` |
| 47 | `$0747B8..$0747C3` | Rika object faces right (`d0=$08`) | `Face(NPC-1, Right)` |
| 48 | `$0747C4..$0747CD` | `DoMapUpdateLoop($09)` = 10 ticks | `Wait(10)` |
| 49 | `$0747CE..$0747D9` | Rika object faces up (`d0=$04`) | `Face(NPC-1, Up)` |
| 50 | `$0747DA..$0747E3` | `DoMapUpdateLoop($77)` = 120 ticks | `Wait(120)` |
| 51 | `$0747E4..$0747F5` | move Rika object to `($1F0,$240)` | `MoveActorTo(NPC-1)` |
| 52 | `$0747F6..$0747FD` | stop music | `PlaySound($FB)` |
| 53 | `$0747FE..$074807` | `DoMapUpdateLoop($1D)` = 30 ticks | `Wait(30)` |
| 54 | `$074808..$07480D` | sprites off | `SetRenderSpritesInCutscene(false)` |
| 55 | `$07480E..$074813` | `Event_GetAndRunDialogue`, entry 2 | `RunDialogue(entry $02)` |
| 56 | `$074814..$07481B` | `Sound_Index ← SpcSFXID_SpaceshipRadar ($F8)` | `PlaySound($F8)` |
| 57 | `$07481C..$074825` | `DoMapUpdateLoop($3B)` = 60 ticks | `Wait(60)` |
| 58 | `$074826..$074831` | palette offset `$18` ← `$000E,$000E` | `Presentation(SetPaletteWords)` |
| 59 | `$074832..$07483F` | `VInt_Prepare` loop, corrected `$3B+1=60` | `WaitFrames(60)` |
| 60 | `$074840..$074845` | `PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 61 | `$074846..$07484D` | stop special SFX (`$FD`) | `PlaySound($FD)` |
| 62 | `$07484E..$074853` | `Current_Party_Slot_5 ← CharID_Rika` | `JoinParty(slot 4, Rika)` |
| 63 | `$074854..$07487D` | construct `Character_5` from `$C340`, id `$18`, art `$55C`; `FieldObj_Rika` | `ObjectAnimation(slot 5)` |
| 64 | `$07487E..$074885` | `Event_AddMacro(5)` | `Presentation(AddMacro { slot: 5 })` |
| 65 | `$074886..$07488F` | clear `$C340` object | `DespawnNpc { npc_index: 1, count: 1 }` |
| 66 | `$074890..$074899` | `EventFlags_Set(EventFlag_BioPlantEscape)` | set event flag `$34` |
| 67 | `$07489A..$0748C9` | load Zema `$24`, previous Motavia `$00`, start `($3C,$14)`, down, align 0 | `LoadMap` |
| 68 | `$0748CA..$0748D1` | `Sound_Index ← MusicID_MotabiaTown ($84)` | `PlaySound($84)` |
| 69 | `$0748D2..$0748E1` | load `loc_1289FA` into tile `$4A5` | `LoadArt` |
| 70 | `$0748E8..$074919` | construct temporary Holt object `$C4C0`, id `$194`, art `$4A5`, at `($1F0,$A0)` | `ObjectAnimation(slot 7)` |
| 71 | `$07491A..$07491F` | `Pal_FadeIn` | `FadeIn` |
| 72 | `$074920..$07492F` | set Holt destination `($1F0,$F0)` | `Presentation(SetObjectDestination)` |
| 73 | `$074930..$074941` | clear camera-lock bit 2 | `SetFollowMode(bits: 0)` |
| 74 | `$074930..$074947` | move party to `($1E0,$F0)` | `MoveActorTo(leader)` |
| 75 | `$074948..$074951` | `DoMapUpdateLoop($3B)` = 60 ticks | `Wait(60)` |
| 76 | `$074952..$07496F` | get Rika; add `$10,$10` to her current position; move | `MoveActorOffset(Rika, $10, $10)` |
| 77 | `$074970..$074977` | Rika faces right (`d0=$08`) | `Face(Rika, Right)` |
| 78 | `$074978..$074981` | `DoMapUpdateLoop($13)` = 20 ticks | `Wait(20)` |
| 79 | `$074982..$074989` | Rika faces left (`d0=$0C`) | `Face(Rika, Left)` |
| 80 | `$07498A..$074993` | `DoMapUpdateLoop($13)` = 20 ticks | `Wait(20)` |
| 81 | `$074994..$07499B` | Rika faces down (`d0=0`) | `Face(Rika, Down)` |
| 82 | `$07499C..$0749A1` | `InitVRAMAndCRAM` | `InitVramAndCram` |
| 83 | `$0749A2..$0749AD` | `DialogueTreesToRAM(DialogueTree34 @ $1F7190)` | `SetDialogueTree($1F7190)` |
| 84 | `$0749AE..$0749B5` | `Sound_Index ← MusicID_Fal ($92)` | `PlaySound($92)` |
| 85 | `$0749B6..$0749BB` | `Pal_FadeIn` | `FadeIn` |
| 86 | `$0749BC..$0749C3` | render sprites in cutscenes ← 1 | `SetRenderSpritesInCutscene(true)` |
| 87 | `$0749C4..$0749C9` | `Event_GetAndRunDialogue5`, entry 3 | cutscene tree-34 entry `$03` |
| 88 | `$0749CA..$0749CF` | `InitVRAMAndCRAM` | `InitVramAndCram` |
| 89 | `$0749D0..$0749D5` | `Pal_FadeIn` | `FadeIn` |
| 90 | `$0749D6..$0749DD` | stop all sound | `PlaySound($FE)` |
| 91 | `$0749DE..$0749E3` | one bare `VInt_Prepare` | `WaitFrames(1)` |
| 92 | `$0749E4..$0749EB` | `Sound_Index ← MusicID_Explosion ($AD)` | `PlaySound($AD)` |
| 93 | `$0749EC..$0749F5` | `Panel_Create($3B)` | `PanelCreate($3B)` |
| 94 | `$0749F6..$0749FB` | `DMAPlanes_VInt` | `DmaPlanes` |
| 95 | `$0749FC..$074A05` | `VInt_PrepareLoop(9)` = 10 frames | `WaitFrames(10)` |
| 96 | `$074A06..$074A0F` | `Panel_Create($3C)` | `PanelCreate($3C)` |
| 97 | `$074A10..$074A15` | `DMAPlanes_VInt` | `DmaPlanes` |
| 98 | `$074A16..$074A1F` | `VInt_PrepareLoop($3B)` = 60 frames | `WaitFrames(60)` |
| 99 | `$074A20..$074A27` | render sprites in cutscenes ← 1 | `SetRenderSpritesInCutscene(true)` |
| 100 | `$074A28..$074A2D` | `Event_GetAndRunDialogue5`, entry 4 | cutscene tree-34 entry `$04` |
| 101 | `$074A2E..$074A35` | `EventFlags_Set(EventFlag_RikaJoined)` | set event flag `$35` |
| 102 | `$074A36..$074A65` | load Motavia `$00`, previous Zema `$24`, start `($C6,$A4)`, down, align 0 | `LoadMap` |
| 103 | `$074A66..$074A6D` | `Sound_Index ← MusicID_FieldMotabia ($8C)` | `PlaySound($8C)` |
| 104 | `$074A6E..$074A73` | `Saved_Sound_Index ← $8C` | `SetSavedMusic($8C)` |
| 105 | `$074A74..$074A79` | `Pal_FadeIn` | `FadeIn` |
| 106 | `$074A7A..$074A7D` | `moveq #1`; `rts` | `Return(1)` |

The two `SetFollowMode` records retain the raw camera-lock writes while the
runner executes the ordinary actor walks. Holt's `$C4C0` object, the Rika
construction and the panel/portrait work stay presentation records; they are
not misrepresented as stable map NPCs. The ordinary Rika object in BioPlant
and the final party slot are still asserted by the runtime recast.

## Verification

The headless arc starts the alarm after reaching the B4 room, then runs event
`$8007` with four pre-existing party slots so the retail slot-5 write has its
real precondition. It asserts temp flag `$08`, event flags `$34` and `$35`,
Rika in party slot 5, the final Motavia map `$00`, and no interpreter fault.
