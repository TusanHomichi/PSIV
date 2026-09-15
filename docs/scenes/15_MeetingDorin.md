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

The current 36-op transcription distinguishes a completed `$FF` reply from
a suspended `$F7` conversation. A completed reply restores the town music
and returns without setting `$36`. The punch branch moves Alys by character
identity, resumes all four suspended sections, and then sets `$36`. Dorin
stays in his chair. His final facing retains the retail repeated-Y-comparison
quirk.

The punch and startled temporary objects are still placeholders. Their art,
visibility keyframes, eight-pixel creep and backstep are not yet reproduced.
The movement and dialogue repairs do not constitute visual parity.

## Verification

`rust/psiv-runtime/tests/dorin_dialogue.rs` checks both the completed-reply
and four-resume branches, including flag timing, Alys movement and Dorin
facing. Native connected verification completed in `build/native-tonoe/route`,
including all three choices and four resumed sections. The earlier run in
`build/native-tonoe/dorin-incomplete` exposed the dropped fourth section and
must not be counted as a completed Tonoe route.
