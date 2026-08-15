# 07 — `Event_BasementContainers`

The party reaches the container room on basement level B2. The music drops to
`Mystery`, a line plays, a beat passes, and the cave music comes back.

| | |
|---|---|
| Event index | `$0C` (`EventPtrs[$0C]`) |
| Retail range | **`$06BC36` .. `$06BC69`** (52 bytes) |
| Entered from | trigger `$08` `RunEvent_BasementContainers`, map `$17` AcademyBasement_B2 |
| Condition | `EventFlag_BasementContainers ($0D)` **clear** |
| Runs on map | `$17` AcademyBasement_B2 (dialogue tree **33**) |

## Fork status: body deleted

`ps4.asm:145128`:

```asm
Event_BasementContainers:
	include "script/scenes/BasementContainers/event.asm"
	rts
```

Ungated include of an absent file.

## Retail disassembly

```
; === Event_BasementContainers @ 06BC36 .. 06BC6A (52 bytes) ===
  06BC36  13fc00abffff500a move.b #$AB, $FFFF500A.l  ; Sound_Index = MusicID_Mystery
  06BC3E  4eb90004204c     jsr    $4204C.l           ; VInt_Prepare  (let the music change land)
  06BC44  700f             moveq  #$F, d0            ; dialogue entry $0F
  06BC46  4eb90005ac66     jsr    $5AC66.l           ; Event_GetAndRunDialogue
  06BC4C  701d             moveq  #$1D, d0           ; 29 -> 30 iterations
  06BC4E  4eb90005a71e     jsr    $5A71E.l           ; DoMapUpdateLoop
  06BC54  13fc008affff500a move.b #$8A, $FFFF500A.l  ; Sound_Index = MusicID_InTheCave
  06BC5C  4eb90004204c     jsr    $4204C.l           ; VInt_Prepare
  06BC62  700d             moveq  #$D, d0            ; EventFlag_BasementContainers
  06BC64  4ef900057666     jmp    $57666.l           ; EventFlags_Set  (tail call)
```

## Transcription

```
PlaySound{MusicID_Mystery}          ; $AB
Wait{ticks: 1}                      ; bare VInt_Prepare
RunDialogue{tree: 33, entry: $0F}
Wait{ticks: 30}                     ; DoMapUpdateLoop, d0 = $1D
PlaySound{MusicID_InTheCave}        ; $8A
Wait{ticks: 1}
SetFlag{EventFlag_BasementContainers}
```

7 ops.

## Two different waits

Both `Wait` ops above are `DoMapUpdateLoop`-flavoured in effect but come from
different primitives, and the distinction is load-bearing:

- **`DoMapUpdateLoop` (`$5A71E`)**, `d0 = $1D`: a `dbra` loop that each
  iteration clears `Joypad_Held`, runs `$54938` (the map update) and then
  `VInt_Prepare`. Because it is `dbra`, the count is **`d0 + 1` = 30 frames**,
  not 29. Input is actively discarded for the duration.
- **A bare `VInt_Prepare` (`$4204C`)**: one frame, no map update. Used here only
  to let the sound driver see the new `Sound_Index` before the next thing
  happens.

There is a third wait primitive in this act, `VInt_PrepareLoop` (`$5A7AC`),
which loops `VInt_Prepare` *without* the map update. `Event_GameStart` uses it;
nothing else in the act does. An interpreter that collapses all three into one
`Wait` will be frame-accurate but will run map updates when retail does not.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 33, entry `$0F` | *"Wh..What's this...?!"* (23 bytes) |
| Flag set | `$0D` | `EventFlag_BasementContainers` — "Set when you arrive at the room with the containers" |
| Sound | `$AB` | `MusicID_Mystery` |
| Sound | `$8A` | `MusicID_InTheCave` — map `$17`'s own music, so this restores the normal track |
| RAM | `$FFFF500A` | `Sound_Index` |

## Open questions

1. **Whether the 30-frame beat is before or after the window closes.** The
   `DoMapUpdateLoop` is sequenced after `Event_GetAndRunDialogue` returns, and
   that returns only once the player dismisses the final page. So the half
   second of `Mystery` is *after* the text, with the window gone. Oracle claim:
   *`Sound_Index` is written `$8A` exactly 30 frames after the dialogue window
   finishes its close animation, and the player cannot move during those 30
   frames.*
2. **Does `Mystery` actually get to play?** The scene sets `$AB`, waits one
   frame, then opens a window; the text is dismissed at player pace, then 30
   frames, then `$8A`. So `Mystery` plays for however long the player takes to
   read, plus half a second. That reads correct, but confirm the sound driver
   does not treat a music change during a window specially.
