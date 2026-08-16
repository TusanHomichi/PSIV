# `Event_ZemaOldMan`

- **Retail bytes:** `$07313E..$073165` inclusive, 40 bytes.
- **Pointer:** `EventPtrs[$8B]` from `$05A2B4`.
- **Entry:** Zema tree 4 entry `$4C` (76).
- **Data:** `next_arc.rs`, `ZEMA_OLD_MAN_OPS`.

## Clone audit

The body is present in the clone's retail stream and has no Grand Cross scene
include. The 40-byte retail disassembly matches the clone's instructions and
operands exactly.

## Retail transcription

1. Read `Character_1`'s facing, flip its axis bit, and write the opposite
   facing to Zema NPC index 2 (`$C380`).
2. Run standard tree 4 dialogue entry `$6C`.
3. Set `EventFlag_ZemaOldMan` (`$B3`).

## Verification

The map-scoped headless scene test drives the dialogue and asserts the `$B3`
write plus `SceneEnded`.
