# 05 — `Event_SuspicionOnPrincipal`

On the way out of the principal's office, Alys says the principal is hiding
something. Two ops.

| | |
|---|---|
| Event index | `$0F` (`EventPtrs[$0F]`) |
| Retail range | **`$06BEE8` .. `$06BEF7`** (16 bytes) |
| Entered from | trigger `$0A` `RunEvent_SuspicionOnPrincipal`, map `$13` PiataAcademy_F1 |
| Condition | `EventFlag_PrincipalMeeting ($09)` **set**, `EventFlag_PrincipalSuspicious ($0E)` **clear** |
| Runs on map | `$13` PiataAcademy_F1 (dialogue tree **1**) |

## Fork status: body deleted

`ps4.asm:145339`:

```asm
Event_SuspicionOnPrincipal:
	include "script/scenes/SuspicionOnPrincipal/event.asm"
	rts
```

Ungated include of an absent file. No fork version exists to diff against.

## Retail disassembly

```
; === Event_SuspicionOnPrincipal @ 06BEE8 .. 06BEF8 (16 bytes) ===
  06BEE8  706e           moveq    #$6E, d0          ; dialogue entry $6E
  06BEEA  4eb90005ac66   jsr      $5AC66.l          ; Event_GetAndRunDialogue
  06BEF0  700e           moveq    #$E, d0           ; EventFlag_PrincipalSuspicious
  06BEF2  4ef900057666   jmp      $57666.l          ; EventFlags_Set  (tail call)
```

Structurally identical to `Event_PiataChazAlone` — literal dialogue id, tail
call into the flag set.

## Transcription

```
RunDialogue{tree: 1, entry: $6E}
SetFlag{EventFlag_PrincipalSuspicious}
```

2 ops.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 1, entry `$6E` | *"Something smells fishy here."* (185 bytes) |
| Flag set | `$0E` | `EventFlag_PrincipalSuspicious` — "Set after Alys brings up the principal being suspicious" |
| Flag read | `$09` | `EventFlag_PrincipalMeeting` — by the trigger, not by this routine |

The scene runs on map `$13` (tree 1), and entries `$6D` and `$6E` are the last
two non-empty entries in tree 1 — a small pair set aside for these two scripted
beats, sitting after a run of 45 empty entries (`$40`–`$6C`). Worth knowing when
validating tree 1: the empties are real and `GetDialogueByID` counts them.

## Open questions

1. **When exactly it fires.** The trigger has no position test, so this should
   fire on the first rest frame after re-entering map `$13` from the principal's
   office. Oracle claim: *warping from `$14` back to `$13` after
   `Cutscene_PiataPrincipal` opens the window within 2 frames of the map load,
   with no input.* If instead it waits for the player to take a step, the
   trigger evaluation point differs from what `RunEvents` reads like and the
   whole trigger layer's timing model needs revisiting.
