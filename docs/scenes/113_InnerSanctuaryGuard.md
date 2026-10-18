# `Event_InnerSanctGuard` and `Event_InnerSanctGuardBeforeElsydeon`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0051` `Event_InnerSanctGuard` | `$070A4E..$070A75` inclusive, 40 bytes | `EventPtrs[$51]` at `$05A2B4 + $144` | `esper_events.rs`, `INNER_SANCT_GUARD` (3 ops) |
| `$0052` `Event_InnerSanctGuardBeforeElsydeon` | `$070A76..$070A9D` inclusive, 40 bytes | `EventPtrs[$52]` at `$05A2B4 + $148` | `esper_events.rs`, `INNER_SANCT_GUARD_BEFORE_ELSYDEON` (3 ops) |

- **Trigger:** dialogue `$F6`, `DialogueTree20` (the Inner Sanctuary's old man). Entry `$24` is
  `$FA $A2 $01` then `$F6 $00 $51`: while `EventFlag_InnerSanctGuard1` (`$A2`) is clear it fires
  `$51`, and once it is set the chain skips to entry `$25`. Entry `$26`'s chain is `$FA $D9 +3`,
  `$FA $A3 +2`, `$FA $D7 +1`: with Elsydeon (`$D9`) and `$A3` clear and `$D7` set it lands on entry
  `$27`, `$F6 $00 $52`. (`$FA`'s operand is a forward delta: [00](00_triggers.md).)

## Clone audit

Both bodies were read from the US image through their `EventPtrs` slots, and every instruction
word matches `ps4.asm:150114-150123` and `:150124-150133`. Neither has a `revision` conditional or
a `DialogueTreesToRAM` call, so the line is an entry of the map's own tree.

## Retail transcription

The two routines differ only in their literals.

| Op | ROM offsets (`$51` / `$52`) | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$070A4E..$070A63` / `$070A76..$070A8B` | `lea (Field_Obj_Secondary).w, a4` (`$C300`, map object 0) / `lea (Character_1).w, a3` / `move.w $6(a3), d0` / `bchg #2, d0` / `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| 1 | `$070A64..$070A6B` / `$070A8C..$070A93` | `moveq #$35, d0` / `moveq #$36, d0`, `jsr Event_GetAndRunDialogue` (`$5AC66`) | entry `$35` / `$36`, standard window |
| 2 | `$070A6C..$070A75` / `$070A94..$070A9D` | `move.b #$A2, d0` / `move.b #$A3, d0`, `jmp EventFlags_Set` (`$57666`) | `SetFlag` event `$A2` / `$A3` |

The flags are `EventFlag_InnerSanctGuard1` and `EventFlag_InnerSanctGuard2`
(`ps4.constants.asm:1601-1602`). They gate only the old man's own tree entries, so neither event
moves the story: these were "precise why-not" rows in [88](88_RetailBoundaries.md) until the census
(#83) asked for every fired event to have a scene, and the bodies are plain enough to transcribe
whole.

## Tests

`rust/psiv-core/tests/late_dezolis_scenes.rs`,
`the_inner_sanctuary_old_man_turns_speaks_and_sets_his_flag`: for each leader facing, object 0
turns the opposite way before the line, the line is `$35` / `$36`, and the only flag written is
`$A2` / `$A3`.
