# `Event_RuneFlaeli`

- **Retail bytes:** `$06D7C2..$06DBD7` inclusive, 1046 bytes.
- **Pointer:** `EventPtrs[$27]` from `$05A2B4`.
- **Trigger:** ValleyMazeOutside `$D8`, trigger `$30`, Rune joined and path
  flag `$13` clear.
- **Data:** `next_arc.rs`, `RUNE_FLAELI_OPS`.

## Clone audit

The retail source is the `grand_cross=0` branch; the `grand_cross=1` branch
includes the absent `script/scenes/RuneFlaeli/event.asm`. The 1046 retail bytes
were disassembled directly and compared with the surviving retail branch.
The four art uploads and the `$ECF2/$ECF3` frame choreography are precisely
the kind of fork surface that a generic “Rune talks, then flag” rewrite would
lose, so they remain explicit ops.

## Retail transcription

1. Decompress art to tiles `$179`, `$191`, `$1FB` and `$20C`; copy the palette
   line at `loc_1DE0B8` to `Palette_Line_4`; clear `$ECF2`.
2. Run tree 7 entry `$26`. Stage Chaz at `($1F0,$260)`, Alys at
   `($200,$260)`, Hahn at `($200,$270)`, and Rune at `($1F0,$240)` with
   follow/camera locks. Wait `$3B + 1 = 60` ticks, clear follow, face the
   first three actors up, wait 30 ticks, face Rune up and wait 60.
3. Resume dialogue, hide Rune, create the temporary `$214` Flaeli object at
   Rune's position with art `$179`, and run its 240-frame keyframe table.
   `SFX_Megid` (`$C0`) fires at frame `$3B`.
4. Show Rune, delete the temporary object, wait 10 frames, face Rune down,
   release the camera lock and resume dialogue.
5. Move the leader to `($1F0,$220)` and then `($1F0,$210)`, restoring the
   follow/camera bits and set `EventFlag_TonoePathOpen` (`$13`).

The headless representation records the four uploads and the keyframe table
as `ObjectAnimation`; all ordinary leader movement uses `MoveActorTo`, so the
scene still exercises the same movement/arrival machinery used by the field.

## Verification

The scene test starts on ValleyMazeOutside, closes its dialogue windows and
waits through the animation, then asserts completion and flag `$13`. The arc
test uses that flag as the road-open beat before the Alshline sequence.
