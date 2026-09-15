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
   The caster object spawns `$218` (Flaeli) and `$BD` (TechCast) on
   update 70. The fire object enables the debris counter 26 updates later;
   `SFX_Megid` (`$C0`) is gated on counter **value** `$3B`, not frame 59.
   At value `$32`, `loc_6DBAC` writes BG chunks `(15,14)=$0C,$0D` and
   `(15,15)=$14,$15`, then refreshes the plane. This opens both the picture
   and collision before the final story flag.
4. Show Rune, delete the temporary object, wait 10 frames, face Rune down,
   release the camera lock and resume dialogue.
5. Move the leader to `($1F0,$220)` and then `($1F0,$210)`, restoring the
   follow/camera bits and set `EventFlag_TonoePathOpen` (`$13`).

The current 45-op transcription stages all four named characters and keeps
240 explicit animation updates. `RestoreMapChunks` validates the four base
chunk ids and removes only those map overlays; it does not rerun the
load-time flag walker. Party slot and character references resolve to the
same actor, and the runtime carries the cast's positions into field control.

The pack now includes Rune, Flaeli, dust and rock art. Rune and Flaeli use
their own animation ages and sequences; Rune's ordinary sprite is hidden
while its temporary replacement is present. The flame palette comes from
the event's line-3 upload. The debris emitter/trajectories and a frame-aligned
oracle comparison remain pending; source-derived timing is not that proof.

## Verification

The scene test starts on ValleyMazeOutside, closes its dialogue windows and
waits through the animation, checks the rock becomes walkable while flag
`$13` is still clear, and verifies Rune's separate staging, the leader's final
cell and the open layout after reloading. The larger arc fixtures now visit
this map while Rune is still in the party, before Dorin replaces him.
