# 01 — `Event_GameStart`

The intro. Alys wakes Chaz at his house, they walk out through Aiedo, cross the
Motavia overworld, the AW 2284 prologue crawls over a title image, and the game
drops you on the academy's first floor in Piata.

At 1416 bytes this is the largest routine in the act — about as long as every
other opening-act scene put together.

| | |
|---|---|
| Event index | `$9F` (`EventPtrs[$9F]`) |
| Retail range | **`$073946` .. `$073ECD`** (1416 bytes) |
| — body | `$073946` .. `$073E85` (the linear scene, ending in the flag set) |
| — helpers | `$073E86` .. `$073ECD` (the two colour-ramp subroutines, `bsr`'d from phase 4) |
| Entered from | `Title_StartOption` (`$043788`) |
| Condition | none — it is the new-game entry point |
| Ends on map | `$13` PiataAcademy_F1 |

## How the title screen gets here

Cross-checked against the new-game extraction lane
(`psiv_tools/newgame.py`, `runtime-pack/game_start.json`); we agree on the
routine start (`$073946`) and on `rom_end = $073ECE`.

```
loc_4335C        $04335C   move.w #$11, (Field_Map_Index).w   -- title screen
loc_44414        $044414   new-game init: party, money, inventory, flags, stats
Title_StartOption $043788  move.w #$C,  (Game_Mode_Routine).w  -- "play event"
                           move.w #$9F, (Event_Index).w
                           jmp    (VInt_Prepare).l
Event_GameStart  $073946   this routine
```

`Title_StartOption` was disassembled here and matches: it sets the mode and the
event index and jumps, and it does **not** set `Game_Mode_Index`, so the field
mode enters the *event* path rather than a map-load path.

### On "the map the opening scene plays on"

`game_start.json` records `title_handoff.scene_map = $11 PiataAcademy` with the
note *"the map the opening scene plays on, not where the player starts."* The
first half of that is verified — `$04335C` really does write `$11` — but **the
scene does not play on map `$11`**, and the interpreter should not model it that
way. Two cartridge facts:

1. The scene's **first instruction** is `jsr InitVRAMAndCRAM` (`$5A658`), which
   fades the palette out and then clears the entire Plane A buffer
   (`lea $8000.w,a0 / move.w #$7FF,d7 / trap #0` = 2048 longwords = 8 KB).
   Whatever map `$11` had put on screen is gone before a single pixel of the
   scene is drawn. Entry `$36` — *"Chaz, we have work to do!"* — plays over
   black, which is what retail looks like.
2. Within 46 bytes of entry the scene writes `Field_Map_Index = $5E`
   (ChazHouse) and calls `RefreshMap`. It then goes `$54` Aiedo, `$00` Motavia,
   a title image, and finally `$13`. **`$11` is never among them.**

The accurate framing is that `$11` is the map *resident when the scene begins* —
a pre-scene default the title screen leaves behind — not a map the scene
renders. The distinction is load-bearing: map `$11` binds dialogue tree 1, and
an interpreter that honours "the scene plays on `$11`" would try to read the
intro's lines out of tree 1. Retail explicitly loads **tree 17** before every
one of its four dialogue calls, precisely because the resident binding is not
the one it wants.

(Whether map `$11`'s chunk/object data is actually resident, or only its index,
was not chased — nothing in the scene depends on it either way.)

## Fork status: body deleted

`ps4.asm:183572`:

```asm
Event_GameStart:
	jsr	(InitVRAMAndCRAM).l
	jsr	(Pal_FadeIn).l
	move.l	#Map_ShayHouse_Dialog, d0
	jsr	(DialogueTreesToRAM).l

	include "script/scenes/gamestart/event.asm"

	jsr	(PalFadeOut_ClrSpriteTbl).l
	...
```

The clone keeps a four-instruction prologue and a ~20-instruction epilogue and
`include`s away **everything in between** — which is the entire intro. The
include is ungated and `script/scenes/` does not exist.

The surviving **prologue** matches retail and was useful for naming primitives
(`InitVRAMAndCRAM = $5A658`, `Pal_FadeIn = $421D4`,
`DialogueTreesToRAM = $53F00`, `PalFadeOut_ClrSpriteTbl = $4223E`,
`VInt_PrepareLoop = $5A7AC`).

The surviving **epilogue does not** — it carries three Grand Cross edits, which
the fork helpfully annotates itself. Independently confirmed by
`psiv_tools/newgame.py`, which flags the same three:

| Clone | Retail | Effect |
|---|---|---|
| `move.w #$58, (Map_Start_X_Pos).w ; was 60` | **`$60`** | start column |
| `move.w #$22, (Map_Start_Y_Pos).w ; was 24` | **`$24`** | start row |
| `move.w #FacingDir_Right, (Map_Start_Facing_Dir).w ; was 0 (down)` | **`$0` (down)** | initial facing |

So the fork starts Chaz at `($2C0, $110)` facing right; retail starts him at
`($300, $120)` facing down. Everything below is cartridge-derived, including the
epilogue.

## The intro runs on dialogue tree 17 — the one tree the fork rewrote

`Event_GameStart` calls `DialogueTreesToRAM` with **`$1EBA90`** four separate
times. That address is **`DialogueTree17`**. Tree 17 is the tree
`SOURCE_NOTES.md` already records as *"wholesale rewritten by the fork (84
edits)"* on the data side — 66 entries in retail against the clone's 80.

So the intro is doubly fork-damaged: its code is deleted from `ps4.asm` *and*
its text is rewritten in `script/dialogue 17.asm`. Every line quoted below was
decoded from the cartridge through `psiv_tools.text`, never from the clone.

The clone calls the constant `Map_ShayHouse_Dialog`, which is a decent hint that
tree 17 is the Chaz-house/intro script rather than a map-bound talk tree. Map
`$5E` ChazHouse binds tree **10**, not 17, so the intro genuinely swaps the tree
out from under the map — this is one of only two scenes in the act that touch
`DialogueTreesToRAM` (the other is `Event_PiataGuardsReprimand`).

## Structure

The routine is four staged map sequences plus a two-page text crawl:

| Phase | Map | What happens |
|---|---|---|
| 1 | `$5E` ChazHouse | Alys wakes Chaz; party swap so Alys leads |
| 2 | `$54` Aiedo | walk out of town |
| 3 | `$00` Motavia | walk across the overworld |
| 4 | — (title screen) | AW 2284 prologue, two pages, on a loaded image |
| 5 | `$13` PiataAcademy_F1 | party reset to Chaz alone; control handed over |

## Retail disassembly

### Phase 0 — open on tree 17, entry `$36`

```
  073946  4eb90005a658   jsr    $5A658.l          ; InitVRAMAndCRAM
  07394C  4eb9000421d4   jsr    $421D4.l          ; Pal_FadeIn
  073952  203c001eba90   move.l #$1EBA90, d0      ; DialogueTree17
  073958  4eb900053f00   jsr    $53F00.l          ; DialogueTreesToRAM
  07395E  7036           moveq  #$36, d0
  073960  4eb90005ac66   jsr    $5AC66.l          ; Event_GetAndRunDialogue
  073966  4eb90004223e   jsr    $4223E.l          ; PalFadeOut_ClrSpriteTbl
```

### Phase 1 — ChazHouse (map `$5E`)

```
  07396C  13fc0084...    move.b #$84, $FFFF500A.l ; MusicID_MotabiaTown
  073974  31fc005eec28   move.w #$5E, $EC28.w     ; Field_Map_Index = ChazHouse
  07397A  31fc0054ec2a   move.w #$54, $EC2A.w     ; Field_Map_Index_2 = Aiedo
  073980  31fc0044ec48   move.w #$44, $EC48.w     ; Map_Start_X_Pos ($44*8 = $220)
  073986  31fc0038ec4a   move.w #$38, $EC4A.w     ; Map_Start_Y_Pos ($38*8 = $1C0)
  07398C  31fc0000ec44   move.w #$0,  $EC44.w     ; facing DOWN
  073992  31fc0000ec46   move.w #$0,  $EC46.w     ; align 0
  073998  08b80003ec4e   bclr.b #$3,  $EC4E.w     ; Map_Load_Flags bit 3
  07399E  4eb90005ae98   jsr    $5AE98.l          ; RefreshMap
; place both actors explicitly, overriding Map_Start_*
  0739A4  49f8c000       lea.l  $C000.w, a4       ; Character_1
  0739A8  397c02300030   move.w #$230, $30(a4)    ; curr_x_pos
  0739AE  397c01a00034   move.w #$1A0, $34(a4)    ; curr_y_pos
  0739B4  397c02300038   move.w #$230, $38(a4)    ; dest_x_pos
  0739BA  397c01a0003a   move.w #$1A0, $3A(a4)    ; dest_y_pos
  0739C0  49f8c040       lea.l  $C040.w, a4       ; Character_2
  0739C4  397c02a00030   move.w #$2A0, $30(a4)
  0739CA  397c02400034   move.w #$240, $34(a4)
  0739D0  397c02a00038   move.w #$2A0, $38(a4)
  0739D6  397c0240003a   move.w #$240, $3A(a4)
  0739DC  7000           moveq  #$0, d0
  0739DE  4eb90005a73c   jsr    $5A73C.l          ; DoMainUpdatesLoop, 1 iteration
  0739E4  4eb9000421d4   jsr    $421D4.l          ; Pal_FadeIn
; Alys walks over to Chaz
  0739EA  49f8c040       lea.l  $C040.w, a4       ; Character_2
  0739EE  397c02300038   move.w #$230, $38(a4)    ; dest_x_pos
  0739F4  397c0250003a   move.w #$250, $3A(a4)    ; dest_y_pos
  0739FA  49f8c000       lea.l  $C000.w, a4       ; Character_1
  0739FE  303c0230       move.w #$230, d0
  073A02  323c0240       move.w #$240, d1
  073A06  08f80001ecfe   bset.b #$1, $ECFE.w      ; Char_Move_Flags bit 1
  073A0C  08f80000ecfe   bset.b #$0, $ECFE.w      ; Char_Move_Flags bit 0 (follow leader)
  073A12  4eb90005aa84   jsr    $5AA84.l          ; Event_MoveCharacters -> ($230, $240)
  073A18  08b80001ecfe   bclr.b #$1, $ECFE.w
  073A1E  08b80000ecfe   bclr.b #$0, $ECFE.w
; party order: Alys leads, Chaz follows -- and swap the two objects to match
  073A24  21fc0100fffff40a move.l #$0100FFFF, $F40A.w  ; slots = Alys, Chaz, -, -
  073A2C  41f8c000       lea.l  $C000.w, a0       ; Character_1 -> scratch
  073A30  43f8e200       lea.l  $E200.w, a1       ; Nem_Code_Table used as scratch
  073A34  3e3c001f       move.w #$1F, d7
  073A38  4e41           trap   #$1
  073A3A  41f8c040       lea.l  $C040.w, a0       ; Character_2 -> Character_1
  073A3E  43f8c000       lea.l  $C000.w, a1
  073A42  3e3c001f       move.w #$1F, d7
  073A46  4e41           trap   #$1
  073A48  41f8e200       lea.l  $E200.w, a0       ; scratch -> Character_2
  073A4C  43f8c040       lea.l  $C040.w, a1
  073A50  3e3c001f       move.w #$1F, d7
  073A54  4e41           trap   #$1
  073A56  7027           moveq  #$27, d0
  073A58  4eb90005a71e   jsr    $5A71E.l          ; DoMapUpdateLoop, 40 frames
  073A5E  49f8c000       lea.l  $C000.w, a4
  073A62  7004           moveq  #$4, d0           ; FacingDir_Up
  073A64  4eb90005a936   jsr    $5A936.l
  073A6A  203c001eba90   move.l #$1EBA90, d0      ; DialogueTree17 again
  073A70  4eb900053f00   jsr    $53F00.l
  073A76  7037           moveq  #$37, d0
  073A78  4eb90005ac66   jsr    $5AC66.l          ; entry $37
  073A7E  49f8c000       lea.l  $C000.w, a4
  073A82  303c01f0       move.w #$1F0, d0
  073A86  323c0290       move.w #$290, d1
  073A8A  4eb90005aa84   jsr    $5AA84.l          ; walk out to ($1F0, $290)
  073A90  4eb90004223e   jsr    $4223E.l          ; PalFadeOut_ClrSpriteTbl
```

### Phase 2 — Aiedo (map `$54`)

```
  073A96  31fc0054ec28   move.w #$54, $EC28.w     ; Field_Map_Index = Aiedo
  073A9C  31fc005eec2a   move.w #$5E, $EC2A.w     ; from ChazHouse
  073AA2  31fc002cec48   move.w #$2C, $EC48.w     ; ($2C*8 = $160)
  073AA8  31fc0048ec4a   move.w #$48, $EC4A.w     ; ($48*8 = $240)
  073AAE  31fc0000ec44   move.w #$0,  $EC44.w
  073AB4  31fc0000ec46   move.w #$0,  $EC46.w
  073ABA  08b80003ec4e   bclr.b #$3,  $EC4E.w
  073AC0  4eb90005ae98   jsr    $5AE98.l          ; RefreshMap
  073AC6  4eb9000421d4   jsr    $421D4.l          ; Pal_FadeIn
  073ACC  ...            move to ($160, $260)     ; Event_MoveCharacters
  073ADE  ...            face RIGHT ($8)
  073AEA  ...            DialogueTree17, entry $38
  073AFE  ...            move to ($180, $300)     ; then, in order:
  073B10  ...            move to ($220, $3D0)
  073B22  ...            move to ($2B0, $3D0)
  073B34  ...            move to ($2B0, $420)
  073B46  ...            move to ($2D0, $420)
  073B58  ...            move to ($2D0, $440)
  073B6A  ...            move to ($300, $530)
  073B7C  13fc00fb...    move.b #$FB, $FFFF500A.l ; Sound_StopMusic
  073B84  4eb90004223e   jsr    $4223E.l          ; PalFadeOut_ClrSpriteTbl
```

Seven `Event_MoveCharacters` calls in a row — this is the walk out of Aiedo,
authored as a **waypoint list**, not a path. Each call sets `dest_*` and blocks
until arrival.

### Phase 3 — Motavia overworld (map `$00`)

```
  073B8A  13fc008c...    move.b #$8C, $FFFF500A.l ; MusicID_FieldMotabia
  073B92  31fc0000ec28   move.w #$0,  $EC28.w     ; Field_Map_Index = Motavia
  073B98  31fc0054ec2a   move.w #$54, $EC2A.w     ; from Aiedo
  073B9E  31fc004eec48   move.w #$4E, $EC48.w     ; ($4E*8 = $270)
  073BA4  31fc0074ec4a   move.w #$74, $EC4A.w     ; ($74*8 = $3A0)
  073BAA  31fc0008ec44   move.w #$8,  $EC44.w     ; facing RIGHT
  073BB0  31fc000cec46   move.w #$C,  $EC46.w     ; align $C
  073BB6  08b80003ec4e   bclr.b #$3,  $EC4E.w
  073BBC  4eb90005ae98   jsr    $5AE98.l          ; RefreshMap
  073BC2  4eb9000421d4   jsr    $421D4.l          ; Pal_FadeIn
  073BC8  203c001eba90   move.l #$1EBA90, d0      ; DialogueTree17
  073BCE  4eb900053f00   jsr    $53F00.l
  073BD4  7039           moveq  #$39, d0
  073BD6  4eb90005ac66   jsr    $5AC66.l          ; entry $39
  073BDC  ...            move to ($2B0, $370)
  073BEE  ...            bset #2, Char_Move_Flags ; then move to ($3B0, $370)
  073C06  ...            bclr #2, Char_Move_Flags
  073C0C  4eb90004223e   jsr    $4223E.l          ; PalFadeOut_ClrSpriteTbl
```

### Phase 4 — the AW 2284 prologue

```
; load a bespoke palette and reset the camera
  073C12  41f9001d2a3c   lea.l  $1D2A3C.l, a0     ; 32 longs = 64 CRAM words
  073C18  43f8fb00       lea.l  $FB00.w, a1       ; Palette_Table_Buffer
  073C1C  701f           moveq  #$1F, d0
loc_73C1E:
  073C1E  22d8           move.l (a0)+, (a1)+
  073C20  51c8fffc       dbra   d0, $73C1E
  073C24  11fc0001ef26   move.b #$1, $EF26.w      ; CRAM_Update_Flag
  073C2A  7000           moveq  #$0, d0
  073C2C  21c0ef90       move.l d0, $EF90.w       ; Camera_Y_Pos_FG (and X, as a long)
  073C30  21c0ef94       move.l d0, $EF94.w       ; Camera_X_Pos_FG
  073C34  31c0ef98       move.w d0, $EF98.w       ; Camera_Y_Pos_BG
  073C38  21c0ef9c       move.l d0, $EF9C.w       ; Camera_X_Pos_BG
; load the title image
  073C3C  4eb9000415dc   jsr    $415DC.l
  073C42  4eb900041678   jsr    $41678.l          ; DMAPlanes_VInt
  073C48  303c0010       move.w #$10, d0
  073C4C  41f9001cf1f2   lea.l  $1CF1F2.l, a0     ; art
  073C52  4eb900041c8a   jsr    $41C8A.l          ; (decompressor)
  073C58  4eb900041740   jsr    $41740.l
  073C5E  41f9001d25be   lea.l  $1D25BE.l, a0     ; plane map
  073C64  303c2010       move.w #$2010, d0
  073C68  43f9ffff0000   lea.l  $FFFF0000.l, a1
  073C6E  4eb900041a90   jsr    $41A90.l          ; (decompressor -> $FFFF0000)
  073C74  41f9ffff0000   lea.l  $FFFF0000.l, a0
  073C7A  43f89280       lea.l  $9280.w, a1       ; plane buffer
  073C7E  7228           moveq  #$28, d1          ; 40 columns
  073C80  7410           moveq  #$10, d2          ; 16 rows
  073C82  4eb900041cba   jsr    $41CBA.l          ; blit the map
  073C88  4eb9000416a6   jsr    $416A6.l
  073C8E  4eb9000421d4   jsr    $421D4.l          ; Pal_FadeIn
  073C94  303c003b       move.w #$3B, d0
  073C98  4eb90005a7ac   jsr    $5A7AC.l          ; VInt_PrepareLoop, 60 frames
  073C9E  31fc0666fb5e   move.w #$666, $FB5E.w    ; the text colour, at its dimmest
; page 1: four lines drawn into the plane buffer
  073CA4  103c003a       move.b #$3A, d0
  073CA8  4eb900059164   jsr    $59164.l          ; GetDialogueByID (NOT a window)
  073CAE  43f8840a       lea.l  $840A.w, a1       ; plane buffer row
  073CB2  363c4200       move.w #$4200, d3        ; VRAM address
  073CB6  7801           moveq  #$1, d4
  073CB8  4eb90006a9b0   jsr    $6A9B0.l          ; draw text to plane
  073CBE  4eb90004204c   jsr    $4204C.l
  ... same for $3B -> $858A/$4280, $3C -> $870A/$4300, $3D -> $888A/$4380 ...
  073D1E  4eb900041682   jsr    $41682.l          ; DMAPlane_A_VInt
  073D24  7013           moveq  #$13, d0
loc_73D26:
  073D26  6100015e       bsr.w  $73E86            ; IntroTextFadeUp
  073D2A  4eb90004204c   jsr    $4204C.l
  073D30  51c8fff4       dbra   d0, $73D26        ; 20 iterations
  073D34  303c0383       move.w #$383, d0
  073D38  4eb90005a7ac   jsr    $5A7AC.l          ; VInt_PrepareLoop, 900 frames
  073D3E  7013           moveq  #$13, d0
loc_73D40:
  073D40  61000162       bsr.w  $73EA4            ; IntroTextFadeDown
  073D44  4eb90004204c   jsr    $4204C.l
  073D4A  51c8fff4       dbra   d0, $73D40        ; 20 iterations
  073D4E  303c0059       move.w #$59, d0
  073D52  4eb90005a7ac   jsr    $5A7AC.l          ; VInt_PrepareLoop, 90 frames
; page 2: entries $3E..$41, identical shape
  ... $3E -> $840A/$4200, $3F -> $858A/$4280, $40 -> $870A/$4300, $41 -> $888A/$4380 ...
  073DD8  7013           moveq  #$13, d0          ; fade up, 20
  073DE8  303c0383       move.w #$383, d0         ; hold, 900
  073DF2  7013           moveq  #$13, d0          ; fade down, 20
  073E02  303c0077       move.w #$77, d0
  073E06  4eb90005a7ac   jsr    $5A7AC.l          ; VInt_PrepareLoop, 120 frames
  073E0C  4eb90004223e   jsr    $4223E.l          ; PalFadeOut_ClrSpriteTbl
  073E12  13fc00fb...    move.b #$FB, $FFFF500A.l ; Sound_StopMusic
  073E1A  303c0027       move.w #$27, d0
  073E1E  4eb90005a7ac   jsr    $5A7AC.l          ; VInt_PrepareLoop, 40 frames
```

The fade helpers are a colour ramp on a single CRAM entry, `$FFFFFB5E`, stepped
only every 4th frame:

```
loc_73E86:                                        ; IntroTextFadeUp
  073E86  1238ef1d       move.b $EF1D.w, d1       ; a frame counter
  073E8A  02410003       andi.w #$3, d1
  073E8E  6612           bne.b  $73EA2            ; only act 1 frame in 4
  073E90  41f8fb5e       lea.l  $FB5E.w, a0
  073E94  3210           move.w (a0), d1
  073E96  06410222       addi.w #$222, d1         ; brighten all three channels
  073E9A  0801000c       btst.b #$C, d1           ; overflow guard
  073E9E  6602           bne.b  $73EA2
  073EA0  3081           move.w d1, (a0)
loc_73EA2:
  073EA2  4e75           rts

loc_73EA4:                                        ; IntroTextFadeDown
  073EA4  1238ef1d       move.b $EF1D.w, d1
  073EA8  02410003       andi.w #$3, d1
  073EAC  6612           bne.b  $73EC0
  073EAE  41f8fb5e       lea.l  $FB5E.w, a0
  073EB2  3210           move.w (a0), d1
  073EB4  0c410666       cmpi.w #$666, d1         ; already at the floor?
  073EB8  6708           beq.b  $73EC2
  073EBA  04410222       subi.w #$222, d1
  073EBE  3081           move.w d1, (a0)
loc_73EC0:
  073EC0  4e75           rts
loc_73EC2:
  073EC2  4eb9000415e8   jsr    $415E8.l
  073EC8  4ef900041682   jmp    $41682.l          ; DMAPlane_A_VInt
```

### Phase 5 — hand over on PiataAcademy_F1

```
  073E24  21fc00fffffff40a move.l #$00FFFFFF, $F40A.w ; slots = Chaz, -, -, -
  073E2C  11fc00fff40e   move.b #$FF, $F40E.w     ; Current_Party_Slot_5 = empty
  073E32  31fc0013ec28   move.w #$13, $EC28.w     ; Field_Map_Index = PiataAcademy_F1
  073E38  31fcffffec2a   move.w #$FFFF, $EC2A.w   ; Field_Map_Index_2 = none
  073E3E  31fc0060ec48   move.w #$60, $EC48.w     ; ($60*8 = $300)
  073E44  31fc0024ec4a   move.w #$24, $EC4A.w     ; ($24*8 = $120)
  073E4A  31fc0000ec44   move.w #$0,  $EC44.w     ; facing DOWN
  073E50  31fc0000ec46   move.w #$0,  $EC46.w     ; align 0
  073E56  08b80003ec4e   bclr.b #$3,  $EC4E.w
  073E5C  4eb90005ae98   jsr    $5AE98.l          ; RefreshMap
  073E62  701d           moveq  #$1D, d0
  073E64  4eb90005a71e   jsr    $5A71E.l          ; DoMapUpdateLoop, 30 frames
  073E6A  13fc0084...    move.b #$84, $FFFF500A.l ; MusicID_MotabiaTown
  073E72  11fc0084ecec   move.b #$84, $ECEC.w     ; Saved_Sound_Index
  073E78  4eb9000421d4   jsr    $421D4.l          ; Pal_FadeIn
  073E7E  7007           moveq  #$7, d0           ; EventFlag_PiataFirstTime
  073E80  4ef900057666   jmp    $57666.l          ; EventFlags_Set  (tail call)
```

## Transcription

```
; phase 0
InitVRAMAndCRAM{};  FadeIn{}
SetDialogueTree{DialogueTree17 @ $1EBA90}
RunDialogue{tree: 17, entry: $36};  FadeOut{}
; phase 1 -- ChazHouse
PlaySound{MusicID_MotabiaTown}
LoadMap{map: $5E ChazHouse, prev: $54, start: ($44,$38), facing: Down, align: 0}
PlaceActor{Character_1, pos+dest: ($230, $1A0)}
PlaceActor{Character_2, pos+dest: ($2A0, $240)}
Wait{ticks: 1, mode: main_updates};  FadeIn{}
SetActorDest{Character_2, ($230, $250)}
SetFollowMode{set bits 0,1}
MoveActorTo{Character_1, ($230, $240), mode: characters}
SetFollowMode{clear bits 0,1}
SetPartySlots{$0100FFFF}                     ; long: Alys, Chaz, -, -
SwapCharSlots{Character_1, Character_2}      ; 3x trap #1 via $E200 scratch
Wait{ticks: 40}
Face{Character_1, Up}
SetDialogueTree{DialogueTree17};  RunDialogue{tree: 17, entry: $37}
MoveActorTo{Character_1, ($1F0, $290)};  FadeOut{}
; phase 2 -- Aiedo
LoadMap{map: $54 Aiedo, prev: $5E, start: ($2C,$48), facing: Down, align: 0}
FadeIn{};  MoveActorTo{Character_1, ($160, $260)};  Face{Character_1, Right}
SetDialogueTree{DialogueTree17};  RunDialogue{tree: 17, entry: $38}
MoveActorTo{($180,$300)} ... ($220,$3D0) ($2B0,$3D0) ($2B0,$420)
                             ($2D0,$420) ($2D0,$440) ($300,$530)
PlaySound{Sound_StopMusic};  FadeOut{}
; phase 3 -- Motavia
PlaySound{MusicID_FieldMotabia}
LoadMap{map: $00 Motavia, prev: $54, start: ($4E,$74), facing: Right, align: $C}
FadeIn{};  SetDialogueTree{DialogueTree17};  RunDialogue{tree: 17, entry: $39}
MoveActorTo{Character_1, ($2B0, $370)}
SetFollowMode{set bit 2};  MoveActorTo{Character_1, ($3B0, $370)}
SetFollowMode{clear bit 2};  FadeOut{}
; phase 4 -- prologue (x2, entries $3A-$3D then $3E-$41)
LoadPalette{$1D2A3C, 64 words};  SetCameraPos{0, 0}
LoadTitleImage{art: $1CF1F2, map: $1D25BE, 40x16 at plane $9280}
FadeIn{};  WaitFrames{60};  SetTextColour{$666}
[ DrawTextToPlane{entry, plane, vram} x4
  IntroTextFadeUp{} x20 ; WaitFrames{900} ; IntroTextFadeDown{} x20
  WaitFrames{90 | 120} ] x2
FadeOut{};  PlaySound{Sound_StopMusic};  WaitFrames{40}
; phase 5 -- hand over
SetPartySlots{$00FFFFFF};  SetPartySlot{5, empty}
LoadMap{map: $13 PiataAcademy_F1, prev: none, start: ($60,$24), facing: Down, align: 0}
Wait{ticks: 30}
PlaySound{MusicID_MotabiaTown};  SetSavedMusic{MusicID_MotabiaTown}
FadeIn{}
SetFlag{EventFlag_PiataFirstTime}
```

**~78 ops** (72 structural + the two prologue pages expanded).

## Where the act's opening state comes from

This is the routine the new-game-init lane's work has to agree with, and it
does — the epilogue below is decoded identically by `psiv_tools/newgame.py`
(`runtime-pack/game_start.json`, `first_control`), which is the citable source
for it. It ends by writing, in order:

- `Current_Party_Slots` (`$F40A`, long) = **`$00FFFFFF`** — Chaz in slot 1,
  slots 2–4 empty — plus `Current_Party_Slot_5` (`$F40E`) = `$FF`.
- `Field_Map_Index = $13`, `Field_Map_Index_2 = $FFFF` (no previous map).
- `Map_Start = ($60, $24)` in 8-pixel units, so Chaz is placed at
  **`curr = ($300, $120)` pixels** = collision cell (48, 18).
- `Saved_Sound_Index = MusicID_MotabiaTown`.
- `EventFlag_PiataFirstTime ($07)` set — and **nothing else**. In particular
  `EventFlag_PiataChazControl ($15)` is left clear, which is exactly what makes
  `RunEvent_PiataChazAlone` fire on the first rest frame.

Mid-intro the party is `$0100FFFF` (Alys leading, Chaz second) — the same order
`Event_AlysFound` restores later. The intro also physically swaps the two
character structs so the object order matches the slot order, using
`Nem_Code_Table` (`$FFFFE200`) as a 64-byte scratch buffer. That is a real
constraint on RAM layout if psiv-core models the object array literally.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 17, `$36` | *"Chaz, we have work to do! / Hurry up and get ready!"* (49 bytes) |
| Dialogue | tree 17, `$37` | *"This is your first job since you joined the Hunters Guild. …"* (207) |
| Dialogue | tree 17, `$38` | *"Where are we off to this time? / It's a bit far. / We're going to Motavia Academy in the town of Piata. …"* (265) |
| Dialogue | tree 17, `$39` | *"Since we're going to be there anyway, I'd sure like to tour the Academy. …"* (146) |
| Dialogue | tree 17, `$3A` | *"AW 2284. Monster attacks"* (24) |
| Dialogue | tree 17, `$3B` | *"have swelled the ranks of those"* (31) |
| Dialogue | tree 17, `$3C` | *"who call themselves 'Hunters.'"* (30) |
| Dialogue | tree 17, `$3D` | *"But as the attacks become ever"* (30) |
| Dialogue | tree 17, `$3E` | *"more frequent and powerful, an"* (30) |
| Dialogue | tree 17, `$3F` | *"elite few begin to wonder what"* (30) |
| Dialogue | tree 17, `$40` | *"is behind this outbreak... and"* (30) |
| Dialogue | tree 17, `$41` | *"when and how will it all end?"* (29) |
| Flag set | `$07` | `EventFlag_PiataFirstTime` |
| Maps | `$5E` / `$54` / `$00` / `$13` | ChazHouse / Aiedo / Motavia / PiataAcademy_F1 |
| Sound | `$84` / `$8C` / `$FB` | `MusicID_MotabiaTown` / `MusicID_FieldMotabia` / `Sound_StopMusic` |
| ROM | `$1EBA90` | `DialogueTree17` — the clone's `Map_ShayHouse_Dialog` |
| ROM | `$1D2A3C` | title-screen palette, 64 CRAM words |
| ROM | `$1CF1F2` / `$1D25BE` | title-screen art / plane map |
| RAM | `$FFFFE200` | `Nem_Code_Table`, reused as struct-swap scratch |
| RAM | `$FFFFFB5E` | the prologue text's CRAM entry, ramped `$666`..`$EEE` |
| RAM | `$FFFFEF1D` | a frame counter; the fade acts on `count & 3 == 0` |
| RAM | `$FFFFEF26` | `CRAM_Update_Flag` |
| RAM | `$FFFFEF90`–`$FFFFEF9F` | camera FG/BG X and Y |
| Routine | `$5A73C` | `DoMainUpdatesLoop` |
| Routine | `$5AA84` | `Event_MoveCharacters` |
| Routine | `$6A9B0` | draw a dialogue entry into a plane buffer (`a0` entry, `a1` buffer, `d3` VRAM, `d4` mode) |

The six wait loops in this family were disassembled and matched to the clone's
six labels by their bodies, in order:
`DoMapUpdateLoop $5A71E`, `DoMainUpdatesLoop $5A73C`,
`DoMapUpdate_DMAPlane_A_Loop $5A760`, `DoMapUpdate_DMAPlanes_Loop $5A77E`,
`DoDMAPlanesLoop $5A79C`, `VInt_PrepareLoop $5A7AC`. All are `dbra`, so every
count is **`d0 + 1`**.

## Argument: most of phase 4 is renderer work, not interpreter work

Phases 0–3 and 5 are ordinary scene material — map loads, waypoint walks,
dialogue, party writes — and belong in the `SceneOp` interpreter.

Phase 4 is a title-screen presentation: a palette blob, two compressed art
assets, a direct plane-buffer blit, and a hand-rolled CRAM ramp on one colour
entry. None of it touches game state; the only thing the engine needs to know is
"the prologue takes this long and then we continue". Recommend the interpreter
expose it as a single `PlayIntroPrologue{}` effect and let `psiv-godot` own the
whole sequence, rather than growing the `SceneOp` enum by six presentation-only
ops that no other scene will ever use. The exact frame counts (60, 20, 900, 20,
90 / 20, 900, 20, 120, 40) are recorded above so the renderer can be faithful.

## Open questions

1. **The prologue's total run time.** Statically: 60 + 2×(20 + 900 + 20) + 90 +
   120 + 40 = **2190 frames ≈ 36.5 s at 60 Hz**, and none of it is skippable —
   `VInt_PrepareLoop` clears `Joypad_Held` every iteration and never tests for a
   button. Oracle claim: *a fresh boot reaches `EventFlag_PiataFirstTime` set at
   a fixed frame count regardless of input.* If retail lets you skip the crawl,
   the skip is somewhere I did not find and the transcription is incomplete.
2. **Waypoint timing.** Each `Event_MoveCharacters` blocks until arrival, so the
   Aiedo walk's duration is a pure function of distance and step speed. Oracle
   claim: *the seven Aiedo waypoints take `sum(|Δx| + |Δy|) / step_speed`
   frames, with the party following at the standard spacing.*
3. **What `Char_Move_Flags` bit 2 does.** Set only for the last overworld
   waypoint and cleared straight after. Bits 0 (follow leader) and 1 (movement
   order) are documented in the clone; bit 2 is not. Oracle-checkable by watching
   the followers on that one leg.
4. **`Map_Start_Char_Align`.** `0` for three of the four loads, `$C` for the
   overworld. Consumed by `loc_535D4`. Not decoded.
5. **Whether the explicit `PlaceActor` writes defeat `Map_Start_*`.** Phase 1
   writes `Map_Start = ($44,$38)` and then immediately overwrites both
   characters' positions with different values. Either the map load has not run
   yet when the scene writes them, or the writes are dead. Oracle claim: *at the
   first `Pal_FadeIn` of phase 1, `Character_1.curr_x_pos` reads `$230`, not
   `$220`.*
