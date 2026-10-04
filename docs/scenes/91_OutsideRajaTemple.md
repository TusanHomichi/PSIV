# `Event_OutsideRajaTemple`

- **Retail bytes:** `$06FC76..$06FC93` inclusive, 30 bytes.
- **Pointer:** `EventPtrs[$43]` at `$05A2B4`; event `$0043`.
- **Trigger:** Dezolis `$001`, `RunEventsJmpTbl[$31]` (`RunEvent_OutsideRajaTemple`,
  the `$0043` write is at `$056D50`), Snowstorm `$80` clear. The routine has no
  position test: it fires on the first field frame after every arrival, which
  from Raja Temple (`$14C`, one warp) is the only way out. Runner log H28.
- **Data:** `dezolis_route.rs`, `OUTSIDE_RAJA_TEMPLE` (3 ops).

## Clone audit

`ps4.asm:149239-149244` is the body and `ps4.asm:120641` the retail pointer
slot; the Grand Cross build adds nothing here. The bytes were read from the US
image, and the pointer slot (`EventPtrs[$43]` is `$06FC76`, the next slot
`$06FC94`) bounds the range.

## Retail transcription

`203C 001E99F0 / 4EB9 00053F00 / 7006 / 4EB9 0005AC66 / 103C 0080 / 4EF9 00057666`.

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FC76..$06FC81` | `move.l #$1E99F0, d0` / `jsr DialogueTreesToRAM` | `SetDialogueTree` `DialogueTree14` |
| 1 | `$06FC82..$06FC89` | `moveq #6, d0` / `jsr Event_GetAndRunDialogue` (`$5AC66`, the standard window) | `RunDialogue` entry 6 |
| 2 | `$06FC8A..$06FC93` | `move.b #$80, d0` / `jmp EventFlags_Set` (tail call, so the routine ends here) | `SetFlag` `$80` |

Entry 6 of `DialogueTree14` is the snowstorm exchange ("Wow! What a heavy snow
storm!"); the text carries `pause_music` and `resume_music` actions around its
`delay 89`, which the dialogue system owns
([DIALOGUE_ACTIONS](DIALOGUE_ACTIONS.md)). The scene names its own tree, so the
`World_Index` tree selection of [#79](https://github.com/TusanHomichi/PSIV/issues/79)
is not in play.

Setting `$80` is what stops the trigger: with it set the next field frame's
`RunEvent_OutsideRajaTemple` takes `RunEvent_NoEvent`.
