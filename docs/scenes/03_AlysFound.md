# 03 — `Event_AlysFound`

Chaz finds Alys on the academy's first floor. She aligns with him, turns to
face him, they talk, and **Alys takes the lead** — the party order becomes
Alys first, Chaz second. This is the scene the scouting notes used as the
worked example, and it is the most fork-damaged one in the act.

| | |
|---|---|
| Event index | `$03` (`EventPtrs[$03]`) |
| Retail range | **`$06B2D8` .. `$06B3B9`** (226 bytes) |
| Entered from | trigger `$03` `RunEvent_FindingAlys`, map `$13` PiataAcademy_F1 |
| Condition | `EventFlag_AlysFound ($08)` **clear**, `curr_y_pos >= $F0`, `curr_x_pos == $260` |
| Runs on map | `$13` PiataAcademy_F1 (dialogue tree **1**) |

## Fork status: partly deleted, plus two behavioural edits

Unlike the other five, this scene's clone entry is *not* a bare stub — the fork
kept the retail prologue and epilogue and cut a hole in the middle. That makes
it the most dangerous file in the act to read casually, because it looks like
retail. Three divergences, all confirmed against cartridge bytes:

1. **The dialogue call is commented out and replaced by the include.** Retail
   `$6B324`–`$6B331`:
   ```asm
   lea     (Alys_Piata).w, a4
   move.b  dialogue_id(a4), d0        ; id comes from the NPC object, not a literal
   jsr     (Event_GetAndRunDialogue).l
   ```
   The clone has exactly these three lines commented out, followed by
   `include "script/scenes/AlysFound/event.asm"`.

2. **The flag set is commented out.** Retail ends
   `moveq #8,d0 / jmp (EventFlags_Set).l`, setting `EventFlag_AlysFound`.
   The clone comments this out with the note *"Testing out doing this in
   generated event asm"*. **The clone's version never sets flag `$08`** — the
   very flag `RunEvent_FindingAlys` tests, so on the fork the trigger would
   re-fire forever unless the generated file sets it.

3. **`Event_MoveCamera` was turned into a tail call.** The clone's last live
   line is `jmp (Event_MoveCamera).l` with the comment *"was jsr, but tail
   call"*. Retail is `jsr $5AAEE` followed by the flag set. Cosmetic on its own,
   but it is what made room for edit 2.

## Retail disassembly

```
; === Event_AlysFound @ 06B2D8 .. 06B3BA (226 bytes) ===
; --- align Alys with Chaz vertically ---
  06B2D8  49f8c4c0       lea.l    $C4C0.w, a4       ; Alys_Piata  (map NPC index 7)
  06B2DC  47f8c000       lea.l    $C000.w, a3       ; Character_1
  06B2E0  302b0034       move.w   $34(a3), d0       ; Chaz curr_y_pos
  06B2E4  b06c0034       cmp.w    $34(a4), d0       ; vs Alys curr_y_pos
  06B2E8  6700002e       beq.w    $6B318            ; already aligned -> skip
  06B2EC  7002           moveq    #$2, d0           ; movement command 2 = step DOWN
loc_6B2EE:                                          ; spin until the step completes
  06B2EE  49f8c4c0       lea.l    $C4C0.w, a4
  06B2F2  4eb9000467b4   jsr      $467B4.l          ; FieldObj_NPCAlysPiata (d0 = move cmd)
  06B2F8  4eb900044954   jsr      $44954.l          ; Field_LoadSprites
  06B2FE  4eb9000447fe   jsr      $447FE.l          ; Field_BuildSprites
  06B304  4eb90004204c   jsr      $4204C.l          ; VInt_Prepare
  06B30A  49f8c4c0       lea.l    $C4C0.w, a4
  06B30E  302c0028       move.w   $28(a4), d0       ; x_step_duration
  06B312  806c002a       or.w     $2A(a4), d0       ; | y_step_duration
  06B316  66d6           bne.b    $6B2EE            ; still moving -> loop
; --- face Chaz and talk ---
loc_6B318:
  06B318  49f8c4c0       lea.l    $C4C0.w, a4
  06B31C  7008           moveq    #$8, d0           ; FacingDir_Right
  06B31E  4eb90005a936   jsr      $5A936.l          ; Event_UpdateObjFacing
  06B324  49f8c4c0       lea.l    $C4C0.w, a4
  06B328  102c0014       move.b   $14(a4), d0       ; dialogue_id FROM THE NPC OBJECT
  06B32C  4eb90005ac66   jsr      $5AC66.l          ; Event_GetAndRunDialogue
; --- Alys takes slot 1, Chaz moves to slot 2 ---
  06B332  31fc0100f40a   move.w   #$0100, $F40A.w   ; Current_Party_Slots = (Alys<<8)|Chaz
  06B338  47f8c4c0       lea.l    $C4C0.w, a3
  06B33C  4253           clr.w    (a3)              ; despawn NPC Alys (clear obj id)
  06B33E  41f8c000       lea.l    $C000.w, a0       ; Character_1 (Chaz)
  06B342  43f8c040       lea.l    $C040.w, a1       ; Character_2
  06B346  3e3c001f       move.w   #$1F, d7
  06B34A  4e41           trap     #$1               ; copy 32 words = one char struct
  06B34C  49f8c040       lea.l    $C040.w, a4
  06B350  397c053c0016   move.w   #$53C, $16(a4)    ; Chaz art_tile in slot 2
; --- build field-Alys in slot 1 at NPC-Alys's position ---
  06B356  49f8c000       lea.l    $C000.w, a4       ; Character_1
  06B35A  47f8c4c0       lea.l    $C4C0.w, a3       ; Alys_Piata (still holds her position)
  06B35E  38bc0008       move.w   #$8, (a4)         ; obj id 8 = FieldObj_Alys
  06B362  397c000c0006   move.w   #$C, $6(a4)       ; facing_dir = LEFT
  06B368  397c05340016   move.w   #$534, $16(a4)    ; art_tile
  06B36E  396b00300030   move.w   $30(a3), $30(a4)  ; curr_x_pos
  06B374  396b00340034   move.w   $34(a3), $34(a4)  ; curr_y_pos
  06B37A  396b00300038   move.w   $30(a3), $38(a4)  ; dest_x_pos
  06B380  396b0034003a   move.w   $34(a3), $3A(a4)  ; dest_y_pos
  06B386  4eb90004672a   jsr      $4672A.l          ; FieldObj_Alys (init)
  06B38C  7001           moveq    #$1, d0           ; CharID_Alys
  06B38E  4eb900063b82   jsr      $63B82.l          ; Event_AddMacro (party join)
  06B394  41f8c4c0       lea.l    $C4C0.w, a0       ; clear NPC Alys's object slot
  06B398  3e3c000f       move.w   #$F, d7
  06B39C  4e40           trap     #$0               ; clear 16 longs = one object
; --- camera and flag ---
  06B39E  49f8c000       lea.l    $C000.w, a4
  06B3A2  302c0030       move.w   $30(a4), d0
  06B3A6  322c0034       move.w   $34(a4), d1
  06B3AA  7402           moveq    #$2, d2           ; camera speed 2
  06B3AC  4eb90005aaee   jsr      $5AAEE.l          ; Event_MoveCamera
  06B3B2  7008           moveq    #$8, d0           ; EventFlag_AlysFound
  06B3B4  4ef900057666   jmp      $57666.l          ; EventFlags_Set
```

## Transcription

```
BranchFlag{ if Chaz.curr_y_pos == NPCAlysPiata.curr_y_pos -> skip 1 }
MoveActor{who: NPCAlysPiata, cmd: StepDown, wait: until_idle}
Face{who: NPCAlysPiata, dir: Right}
RunDialogue{tree: 1, entry: NPCAlysPiata.dialogue_id}     ; = $29
SetPartySlots{(Alys << 8) | Chaz}                          ; word write to $F40A
CopyCharSlot{from: Character_1, to: Character_2}           ; trap #1, 32 words
SetArtTile{who: Character_2, tile: $53C}
PromoteNpcToChar{npc: NPCAlysPiata, char_id: Alys, slot: 1,
                 obj_id: 8, art_tile: $534, facing: Left,
                 position: from NPCAlysPiata}
JoinParty{char: Alys}                                      ; Event_AddMacro
DespawnNpc{who: NPCAlysPiata}                              ; clr.w + trap #0
MoveCamera{x: Chaz.curr_x_pos, y: Chaz.curr_y_pos, speed: 2}
SetFlag{EventFlag_AlysFound}
```

12 ops.

The `BranchFlag` at the top is a *position* comparison, not a flag test — the
only such branch in the act. If the interpreter's `BranchFlag` is strictly
flag-valued, this wants a sibling `BranchIf{cond}`; noting it rather than
silently widening `BranchFlag`.

## The alignment step is provably sufficient

The scene issues movement command 2 exactly **once** and then waits. Command 2
decodes (behaviour table for `d7 = 1` at `$4A266`, record `8*cmd`, record
`cmd 2 = 00 10 00 00 01 00 00 00`) as *one 16-pixel step down, ending facing
Down*. So the scene assumes Chaz is at most one cell below Alys.

That assumption holds, and statically:

- NPC-Alys is map `$13` NPC index 7, at cell (37, 16) = pixels **`($250, $F0)`**.
- The trigger requires `curr_x_pos == $260` exactly, i.e. cell 38.
- In the extracted collision grid for map `$13`, **column 38 is walkable only at
  rows 16 and 17** — rows 14/15 and 18–21 are type 8 (solid).
- Row 16 is `y = $F0` (Alys's row, `beq` taken, no movement); row 17 is
  `y = $100`, exactly one step.

So the single step always lands Alys on Chaz's row, and she ends to his left
(`$250` vs `$260`) facing Right — toward him. No edge case, no original bug.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 1, entry `$29` | read at runtime from `dialogue_id` of map `$13` NPC 7 (`= 41 = $29`): *"Chaz, where have you been?"* (129 bytes) |
| Flag set | `$08` | `EventFlag_AlysFound` |
| RAM | `$FFFFC4C0` | `Alys_Piata` — map `$13` NPC index 7 (`$C300 + 7*$40`) |
| RAM | `$FFFFC000` / `$FFFFC040` | `Character_1` / `Character_2` |
| RAM | `$FFFFF40A` | `Current_Party_Slots`, written as a **word**: `$0100` = slot1 Alys (`CharID_Alys = 1`), slot2 Chaz (`CharID_Chaz = 0`) |
| Object field | `$14` | `dialogue_id` (byte) |
| Object field | `$16` | `art_tile` — `$534` field-Alys, `$53C` Chaz-in-slot-2 |
| Object field | `$06` | `facing_dir` — `$C` = left |
| Object id | `8` | `FieldObj_Alys` |
| Routine | `$467B4` | `FieldObj_NPCAlysPiata` |
| Routine | `$4672A` | `FieldObj_Alys` |
| Routine | `$63B82` | `Event_AddMacro` |

**Alys leads the opening party** — confirmed from the cartridge, matching
Peter's memory and the scouting note.

## Open questions

1. **Camera speed 2 in pixels/frame.** `Event_MoveCamera` (`$5AAEE`) takes the
   target in pixels, subtracts `$98`/`$58` for the half-screen offset, masks to
   `$FFF`, and eases at `d2` per frame with a `$7FF` fast-path. Oracle claim:
   *after the dialogue closes, `Camera_X_Pos_FG` moves toward the target at 2
   pixels per frame and the whole camera move takes `|delta| / 2` frames.*
2. **Does the spin loop cost a visible frame when Chaz is already aligned?**
   Retail `beq`s past the loop entirely, so the answer should be no — but the
   sprite-rebuild calls inside the loop are also what animate Alys, so a
   renderer that hoists them will differ. Oracle claim: *entering the trigger
   from row 16 produces zero frames of Alys motion; from row 17, exactly 16
   frames of step animation.*
3. **`Event_AddMacro`'s side effects.** Not disassembled in this pass. It is the
   party-join primitive shared with `Event_MeetingHahn`; the follower/render
   half is core-lane's. Needs its own read before the interpreter can claim
   party joins are complete.
