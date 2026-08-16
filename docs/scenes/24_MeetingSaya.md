# `Event_MeetingSaya`

- **Retail bytes:** `$06BC6A..$06BE77` inclusive, 526 bytes.
- **Pointer:** `EventPtrs[$0D]` from `$05A2B4`; trigger `$16` writes event
  `$000D`.
- **Map:** KrupKindergarten `$3A`, the route's optional Saya branch.
- **Data:** `next_arc.rs`, `MEETING_SAYA_OPS`.

## Clone audit

This is one of the seven ungated scene includes. `script/scenes/MeetingSaya`
does not exist in the Grand Cross clone, so its branch is an absent include
and `rts`. The 526 retail bytes were disassembled directly; the clone supplied
only names and struct offsets. Temporary objects `$C480` and `$C4C0` are not
map NPCs and were not misidentified as stable map indices.

## Retail transcription

1. Run tree 5 dialogue entry `$0E`.
2. Copy Hahn's field object into the temporary `$C480` object and hide the
   party copy; move it to `($190,$1F0)`. Face Saya's field object left, move
   the party group to `($190,$210)`, and resume dialogue.
3. Face Saya and temporary Hahn down; load the sparkle object `$1A0`, play
   the `Res` SFX (`$CC`), and run the embrace/update loop. Resume dialogue,
   rotate the two actors through the retail left/right beats, and run the
   healing/presentation waits.
4. Restore Hahn's party object, clear the temporary object, and set
   `EventFlag_Saya` (`$12`).

The native scene keeps the temporary object and keyframe work as
`ObjectAnimation` records while executing the ordinary map-NPC walk and
facing through the same runner used by the main arc.

## Verification

The runtime scene suite starts on KrupKindergarten, closes the dialogue
windows, waits through the presentation, and asserts the return edge and
flag `$12`.
