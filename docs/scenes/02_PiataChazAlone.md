# 02 — `Event_PiataChazAlone`

Chaz, alone on the academy's first floor at the very start of play, notes that
he has lost Alys. This is the scene that hands the player control.

| | |
|---|---|
| Event index | `$A0` (`EventPtrs[$A0]`) |
| Retail range | **`$073ECE` .. `$073EDF`** (18 bytes) |
| Entered from | trigger `$7C` `RunEvent_PiataChazAlone`, map `$13` PiataAcademy_F1 |
| Condition | `EventFlag_PiataChazControl ($15)` **clear** |
| Runs on map | `$13` PiataAcademy_F1 (dialogue tree **1**) |

## Fork status: body deleted

`ps4.asm:183657` is

```asm
Event_PiataChazAlone:
	include "script/scenes/PiataChazAlone/event.asm"
	rts
```

ungated, and `script/scenes/` does not exist in the clone. There is no fork
version to diff — the clone contributes the entry label and nothing else.
Everything below is from cartridge bytes.

## Retail disassembly

```
; === Event_PiataChazAlone @ 073ECE .. 073EE0 (18 bytes) ===
  073ECE  706d           moveq    #$6D, d0          ; dialogue entry $6D
  073ED0  4eb90005ac66   jsr      $5AC66.l          ; Event_GetAndRunDialogue
  073ED6  303c0015       move.w   #$15, d0          ; EventFlag_PiataChazControl
  073EDA  4ef900057666   jmp      $57666.l          ; EventFlags_Set  (tail call)
```

Note the tail call: the routine ends in `jmp`, not `jsr`+`rts`, so `EventFlags_Set`
returns directly to `FieldRoutine_Cutscene`'s caller path. Behaviourally
identical to set-then-return; worth matching only if you are byte-counting.

## Transcription

```
RunDialogue{tree: 1, entry: $6D}
SetFlag{EventFlag_PiataChazControl}
```

2 ops.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 1, entry `$6D` | *"Oops! I wandered around and now / I've gotten separated from Alys. / I've got to find her..."* (91 bytes, opens with portrait `$F4 $01`) |
| Flag set | `$15` | `EventFlag_PiataChazControl` — "Set when you gain control of Chaz in Piata at the start of the game" |
| RAM | `$FFFF500A` | *(not touched — no sound in this scene)* |

The dialogue id is a **literal** here, unlike `Event_AlysFound`, which reads its
id from the NPC object. Tree 1 is the map's bound tree, so no
`SetDialogueTree` is needed.

## Open questions

None material. The scene is four instructions and fully determined statically.

One oracle claim worth taping down anyway, because it fixes when control
actually arrives:

> Starting a new game and letting the intro run to completion, event `$A0`
> fires with no player input, the window shows tree 1 entry `$6D`, and
> `EventFlag_PiataChazControl` reads set the frame after the window closes.
> Chaz has not moved from the `Map_Start = ($60, $24)` placement
> (`curr = ($300, $120)` pixels) at any point.
