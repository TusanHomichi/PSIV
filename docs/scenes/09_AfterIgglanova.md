# 09 — `Event_AfterIgglanova`

After the Igglanova dies, the party discusses what a plant monster was doing
in a university basement. This is the most elaborately staged scene in the act:
the dialogue **pauses mid-conversation twice** while the actors reposition.

| | |
|---|---|
| Event index | `$25` (`EventPtrs[$25]`) |
| Retail range | **`$06D6C8` .. `$06D783`** (188 bytes) |
| Entered from | trigger `$13` `RunEvent_AfterIgglanova`, map `$17` AcademyBasement_B2 |
| Condition | `EventFlag_AfterIgglanova ($0F)` **clear**, `EventFlag_Igglanova ($0B)` **set** |
| Runs on map | `$17` AcademyBasement_B2 (dialogue tree **33**) |

## Fork status: body deleted — but the clone kept a commented copy

`ps4.asm:147096`:

```asm
Event_AfterIgglanova:
	include "script/scenes/AfterIgglanova/event.asm"
	rts
```

Ungated include of an absent file. **However**, the clone preserved the retail
body immediately below as a block of `;`-commented lines
(`ps4.asm:147099`–`147138`). That commented block was compared against the
cartridge disassembly line by line and **matches exactly** — same order, same
operands, same primitives.

That is worth stating plainly: it is an independent cross-check on this
transcription, and it is where several primitive names below were confirmed
(`DoMapUpdateLoop`, `Event_GetCharacter`, `Event_MoveSingleObject`,
`Event_OverlapCharacters`, `Event_MoveCamera`, and the `popdlg` macro). It does
not make the clone trustworthy for this scene — the *live* code is still an
`include` of a file that does not exist — but the comment is good documentation.

## Retail disassembly

```
; === Event_AfterIgglanova @ 06D6C8 .. 06D784 (188 bytes) ===
  06D6C8  7012           moveq  #$12, d0            ; dialogue entry $12
  06D6CA  4eb90005ac66   jsr    $5AC66.l            ; Event_GetAndRunDialogue
  06D6D0  4238ece0       clr.b  $ECE0.w             ; FieldObj_Step_Offset = 0
; --- Chaz faces right, dialogue resumes ---
  06D6D4  7000           moveq  #$0, d0             ; CharID_Chaz
  06D6D6  4eb90005a6d6   jsr    $5A6D6.l            ; Event_GetCharacter -> a4
  06D6DC  303c0008       move.w #$8, d0             ; FacingDir_Right
  06D6E0  4eb90005a936   jsr    $5A936.l            ; Event_UpdateObjFacing
  06D6E6  70ff           moveq  #$FF, d0            ; \
  06D6E8  3038ecf0       move.w $ECF0.w, d0         ;  > popdlg: a0 = $FFFF____
  06D6EC  2040           movea.l d0, a0             ; /
  06D6EE  4eb90005ac6c   jsr    $5AC6C.l            ; Event_RunDialogue (resume)
; --- Chaz up, Alys up, Hahn down ---
  06D6F4  7000           moveq  #$0, d0             ; CharID_Chaz
  06D6F6  4eb90005a6d6   jsr    $5A6D6.l
  06D6FC  303c0004       move.w #$4, d0             ; FacingDir_Up
  06D700  4eb90005a936   jsr    $5A936.l
  06D706  7001           moveq  #$1, d0             ; CharID_Alys
  06D708  4eb90005a6d6   jsr    $5A6D6.l
  06D70E  303c0004       move.w #$4, d0             ; FacingDir_Up
  06D712  4eb90005a936   jsr    $5A936.l
  06D718  7002           moveq  #$2, d0             ; CharID_Hahn
  06D71A  4eb90005a6d6   jsr    $5A6D6.l            ; a4 stays = Hahn for the move below
  06D720  103c0000       move.b #$0, d0             ; FacingDir_Down
  06D724  4eb90005a936   jsr    $5A936.l
; --- beat, then Hahn walks to (256, 192) ---
  06D72A  701d           moveq  #$1D, d0            ; 30 frames
  06D72C  4eb90005a71e   jsr    $5A71E.l            ; DoMapUpdateLoop
  06D732  303c0100       move.w #$100, d0           ; dest_x_pos = 256
  06D736  323c00c0       move.w #$C0, d1            ; dest_y_pos = 192
  06D73A  4eb90005a9fc   jsr    $5A9FC.l            ; Event_MoveSingleObject (a4 = Hahn)
; --- dialogue resumes again, then cleanup ---
  06D740  70ff           moveq  #$FF, d0            ; \
  06D742  3038ecf0       move.w $ECF0.w, d0         ;  > popdlg
  06D746  2040           movea.l d0, a0             ; /
  06D748  4eb90005ac6c   jsr    $5AC6C.l            ; Event_RunDialogue (resume)
  06D74E  4eb90005a87a   jsr    $5A87A.l            ; Event_OverlapCharacters
  06D754  49f8c000       lea.l  $C000.w, a4         ; Character_1 (leader)
  06D758  302c0030       move.w $30(a4), d0
  06D75C  322c0034       move.w $34(a4), d1
  06D760  7401           moveq  #$1, d2             ; camera speed 1
  06D762  4eb90005aaee   jsr    $5AAEE.l            ; Event_MoveCamera
  06D768  49f8c000       lea.l  $C000.w, a4
  06D76C  303c0000       move.w #$0, d0             ; FacingDir_Down
  06D770  4eb90005a936   jsr    $5A936.l
  06D776  11fc0001ece0   move.b #$1, $ECE0.w        ; FieldObj_Step_Offset = 1
  06D77C  700f           moveq  #$F, d0             ; EventFlag_AfterIgglanova
  06D77E  4ef900057666   jmp    $57666.l            ; EventFlags_Set  (tail call)
```

## Transcription

```
RunDialogue{tree: 33, entry: $12}
SetStepOffset{0}
Face{who: char(Chaz), dir: Right}
RunDialogueResume{}
Face{who: char(Chaz), dir: Up}
Face{who: char(Alys), dir: Up}
Face{who: char(Hahn), dir: Down}
Wait{ticks: 30}
MoveActorTo{who: char(Hahn), x: 256, y: 192, mode: single_object}
RunDialogueResume{}
OverlapCharacters{}
MoveCamera{x: leader.curr_x_pos, y: leader.curr_y_pos, speed: 1}
Face{who: Character_1, dir: Down}
SetStepOffset{1}
SetFlag{EventFlag_AfterIgglanova}
```

15 ops.

## `RunDialogueResume` — the pause-and-restage mechanism

This is the vocabulary extension this scene forces, and it is not optional.

`RunText` (`$6A090`) writes its final `a0` — the position it stopped at inside
the decompressed tree buffer — into **`Saved_Dialogue_Addr` (`$FFFFECF0`)**, at
`$6A12C` and `$6AD82`, on *every* exit. `Saved_Dialogue_Addr` is a **word**,
because the buffer lives in the `$FFFFxxxx` scratch page; the scene
sign-extends it back to a full address with the idiom the clone calls `popdlg`:

```asm
moveq   #$FF, d0            ; d0 = $FFFFFFFF
move.w  (Saved_Dialogue_Addr).w, d0   ; d0 = $FFFF<offset>
movea.l d0, a0
```

Then `Event_RunDialogue` (`$5AC6C`) is entered **directly**, skipping
`GetDialogueByID` — because a0 is already positioned. The result is that tree 33
entry `$12` is one long conversation which retail interrupts twice to move
actors around, and the interpreter must model dialogue as a *resumable cursor*,
not as an atomic "show entry N".

Entry `$12` is 761 bytes — comfortably long enough for three chunks.

## `Event_GetCharacter` resolves character ids, not slots

`$5A6D6` takes a **`CharID`** in `d0`, looks up which party slot that character
occupies, multiplies by `$40` and returns the field object in `a4`. So the scene
says "Chaz", "Alys", "Hahn" and gets the right object regardless of party order —
which matters here, because after `Event_AlysFound` the leader is Alys, not Chaz.

`CharID_Chaz = 0`, `CharID_Alys = 1`, `CharID_Hahn = 2`.

Note the register discipline: the `Event_MoveSingleObject` at `$6D73A` uses the
`a4` left over from the **Hahn** lookup at `$6D71A`, six instructions earlier.
It is Hahn who walks to (256, 192), not the leader.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 33, entry `$12` | *"We did it!"* (761 bytes) — played in three chunks via two resumes |
| Flag set | `$0F` | `EventFlag_AfterIgglanova` — "Set after the Igglanova fight and the discussion about the monsters" |
| Flag read | `$0B`, `$0F` | by the trigger, not by this routine |
| RAM | `$FFFFECE0` | `FieldObj_Step_Offset` — cleared for the scene, restored to 1 at the end |
| RAM | `$FFFFECF0` | `Saved_Dialogue_Addr` (word, sign-extended) |
| RAM | `$FFFFC000` | `Character_1` — the party leader, i.e. Alys at this point |
| Routine | `$5A6D6` | `Event_GetCharacter` |
| Routine | `$5A9FC` | `Event_MoveSingleObject` |
| Routine | `$5A87A` | `Event_OverlapCharacters` |
| Routine | `$5A71E` | `DoMapUpdateLoop` — `d0 + 1` = 30 frames |
| Position | (256, 192) | `$100`, `$C0` in pixels = cell (16, 12) |

`FieldObj_Step_Offset` is cleared at the start and set to 1 at the end. It looks
like the follower-spacing value — with it at 0 the party members stand on top of
each other rather than trailing, which is what lets the scene pose them
individually and then `Event_OverlapCharacters` collapse them again. Confirm
with core-lane's follower work.

## Open questions

1. **Where the actors actually stand.** Every `Face` here is a pure facing
   change; only Hahn moves, and only to a fixed (256, 192). The party's
   positions at scene start are wherever the player left them when the battle
   fired. Oracle claim: *no party member's `curr_x_pos`/`curr_y_pos` changes
   during this scene except Hahn's, which ends at exactly `($100, $C0)`.* If
   Chaz and Alys also shift, something in `Event_OverlapCharacters` or the step
   offset is moving them and the transcription is incomplete.
2. **Exactly where in entry `$12` the two pauses fall.** Statically we know the
   scene resumes twice; we do not know which control code stops `RunText` mid
   entry. Oracle claim: *`Saved_Dialogue_Addr` takes exactly three distinct
   values during this scene, and the two intermediate ones fall inside entry
   `$12`'s byte range.* Logging that address per frame gives the answer directly
   and pins the chunk boundaries for the dialogue lane.
3. **`Event_OverlapCharacters` (`$5A87A`).** It computes the *last* party
   member's object (`party_count - 1`, `lsl #6`) into `a3` and clears
   `Joypad_Held`; the rest was not read. It presumably walks followers onto the
   leader's cell. Needs a full read before followers are considered done.
4. **Camera speed 1 here vs 2 in `Event_AlysFound`.** Both are
   `Event_MoveCamera` to the leader's position; only `d2` differs. Slower is
   presumably a softer settle after a fight. Oracle-checkable directly.
