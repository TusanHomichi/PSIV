# `Event_MeetingDorin`

- **Retail bytes:** `$06EEBE..$06F1A3` inclusive, 742 bytes.
- **Pointer:** `EventPtrs[$32]` from `$05A2B4`; plain event `$0032`.
- **Entry:** tree 7 entry `$17` (23), before `RunEvent_Dorin` (`$14`).
- **Data:** `next_arc.rs`, `MEETING_DORIN_OPS`.

## Clone audit

The retail body is in the clone's `grand_cross=0` branch. The Grand Cross
branch does not provide an equivalent retail routine. The 742-byte retail
range was byte-diffed at the instruction level against the surviving source;
the fork-sensitive values are the temporary objects at `$C480/$C540/$C4C0`,
not map NPCs. Those are retained as `ObjectAnimation` records rather than
silently dropped.

## Retail transcription

1. Stop music; run the corrected ten-tick map-update wait; play
   `Music_JijyNoRag` (`$A6`).
2. Face the secondary object (Dorin, `$C300`) opposite the leader and run
   tree 7 dialogue entry `$18`.
3. Load art `$4A5` and stage the temporary Alys punch object (`$C480`), anger
   lines (`$C540`) and struck-Dorin object (`$C4C0`). Move Alys to
   `($200,$1E0)`, face left, step once, then run the punch/update loop. The
   punch keyframe plays `SFX_Foi` (`$BE`); the `$82`, `$3C` and `$50` timer
   branches toggle the struck and anger objects.
4. Resume dialogue; face Dorin right; resume dialogue again; clear the two
   temporary effects and run the startled-Dorin wind-up loop.
5. Resume the final dialogue, restore Alys, step her back from the chair, set
   `EventFlag_Dorin` (`$36`) and return.

The native data keeps the keyframe work literal and executable as a bounded
presentation record. The map-facing/movement part uses ordinary scene actor
ops, so it cannot mutate a private fake actor and disappear from the field
state.

## Verification

The runtime test drives `$0032` to completion and asserts `$36`. The arc then
fires the `$14` census condition and runs `Cutscene_Dorin`.
