# `Cutscene_MeetingRune`

- **Retail bytes:** `$073F9C..$074033` inclusive, 152 bytes.
- **Pointer:** `CutscenePtrs[$03]` from `$05A580`; scene event `$8003`.
- **Entry:** tree 7 entry `$02` on the Tonoe route.
- **Data:** `next_arc.rs`, `MEETING_RUNE_OPS`.

## Clone audit

This body is present in the clone's surviving retail stream. The retail
instruction sequence and the clone's `grand_cross=0` source match across the
152-byte range; there is no Grand Cross replacement body to trust instead.
The `C400` address is Tonoe map NPC index 4, not a character slot guessed from
the dialogue text.

## Retail transcription

1. Face NPC `$C400` opposite `Character_1`.
2. Initialise VRAM/CRAM; play `Music_Thray` (`$94`); fade in.
3. Enable cutscene sprites and run tree 7 entry `3` with dialogue window 5.
4. Write Rune (`CharID $03`) into party slot 4 (native slot 1), construct the
   Rune field object at the NPC's position with art tile `$54C`, and run its
   field routine once.
5. Clear the temporary NPC object (`trap #0`, one `$40`-byte object), add Rune
   to the macro party, and set `EventFlag_RuneJoined` (`$11`).
6. Stop music, clear saved music, set the previous-map word to `$FFFF`, and
   return `d0 = 0`.

The interpreter expresses the object construction as `PromoteNpcToChar`,
followed by the explicit despawn and party-macro edge. It does not invent a
map transition: retail leaves the current field map in place and the caller
reloads according to the zero return.

## Verification

The per-scene headless test runs the dialogue and party mutation to
`SceneEnded`; the arc test then carries Rune into the Dorin conversation.
