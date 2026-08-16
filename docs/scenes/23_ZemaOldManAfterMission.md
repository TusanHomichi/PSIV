# `Event_ZemaOldManAfterMission`

- **Retail bytes:** `$073166..$07318D` inclusive, 40 bytes.
- **Pointer:** `EventPtrs[$8C]` from `$05A2B4`.
- **Entry:** Zema tree 4 entry `$4E` (78).
- **Data:** `next_arc.rs`, `ZEMA_OLD_MAN_AFTER_MISSION_OPS`.

## Clone audit

The routine is an unconditional retail body in the clone. Its 40 bytes were
checked instruction-for-instruction against the clone source; there is no
Grand Cross replacement to use as an authority.

## Retail transcription

1. Face Zema NPC index 2 opposite the leader.
2. Run standard tree 4 dialogue entry `$6D`.
3. Set `EventFlag_OldManZemaAfterDaughter` (`$B7`).

This is registered with the arc because it is a live tree-4 Zema control, even
though the daughter mission is not on the linear Birth Valley route.

## Verification

The per-scene runtime test drives the event on Zema to its return edge and
checks the `$B7` flag.
