# `Event_PersistentEsperGuards`

- **Retail bytes:** `$06FE1C..$06FE55` inclusive, 58 bytes.
- **Pointer:** `EventPtrs[$45]` at `$05A2B4`; event `$0045`.
- **Trigger:** Dialogue `$F6 $00 $45`, entry 52 of `DialogueTree21` (and `DialogueTree20`): the two
  `InnerEsperGuards` objects of `EspMansionNorth` `$16A` (objects 3 and 4, dialogue 52).
- **Data:** `esper_events.rs`, `PERSISTENT_ESPER_GUARDS` (6 ops).

## Clone audit

Read from the US image through its `EventPtrs` slot; the instruction words match
`ps4.asm:149341-149361`. Both `if revision>0` blocks of the clone are in the retail bytes
(`move.l #DialogueTree20, d0 / jsr DialogueTreesToRAM` at `$06FE1C` and the same with
`DialogueTree21` at `$06FE40`), which is why the tree-load census counts two for this scene.

## Retail transcription

`$FFFFC3C0` is object 3 of the map and `$14(a0)` its dialogue id; `$54(a0)` reaches
`$FFFFC414`, object 4's.

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FE1C..$06FE27` | `move.l #DialogueTree20 ($1EDCA0), d0` / `jsr DialogueTreesToRAM` | `SetDialogueTree` |
| 1 | `$06FE28..$06FE2F` | `moveq #$2E, d0` / `jsr Event_GetAndRunDialogue` | `standard(0x2E)` |
| 2-3 | `$06FE30..$06FE3F` | `lea ($FFFFC3C0).w, a0` / `move.b #$23, $14(a0)` / `move.b #$23, $54(a0)` | `SetNpcDialogue` 3 and 4 to `$23` |
| 4 | `$06FE40..$06FE4B` | `move.l #DialogueTree21 ($1EE9B0), d0` / `jsr DialogueTreesToRAM` | `SetDialogueTree` |
| 5 | `$06FE4C..$06FE55` | `move.b #$96, d0` / `jmp EventFlags_Set` (Inner Sanctuary) | `SetFlag` `$96` |

Tree 20 entry 46 is the scene's one dialogue: Kyra is stopped, Rune speaks to the guards and
the guards give way ("Hey! Oh, it's...it's you! Forgive my rudeness! Please enter! Rune...?").
`$96` is what `FieldObj_InnerEsperGuards` tests to step aside (`ps4.asm:96583`).
