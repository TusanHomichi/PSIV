# `Cutscene_ProfHolt`

- **Retail bytes:** `$073F22..$073F9B` inclusive, 122 bytes.
- **Pointer:** `CutscenePtrs[$02]` from `$05A580`; scene event `$8002`.
- **Entry:** tree 3 entry `$6A` (`106`), after the Piata gate.
- **Data:** `rust/psiv-core/src/scenes/next_arc.rs`, `PROF_HOLT_OPS`.

## Clone audit

The body survives in the clone's main `ps4.asm` stream rather than an absent
scene include. Retail disassembly and the surviving `grand_cross=0` source
agree instruction-for-instruction across the pointer range. The Grand Cross
fork does not supply a replacement body. The retail bytes, not the clone's
labels, decide the map ids and start words below.

## Retail transcription

1. `InitVRAMAndCRAM`.
2. `Pal_FadeIn`.
3. Play `Music_Mystery` (`$AB`).
4. Set `Render_Sprites_In_Cutscenes = 1`.
5. `Event_GetAndRunDialogue5`, tree 3 entry `$66`.
6. Stop music (`$FB`).
7. Write `Field_Map_Index = Zema ($24)` and
   `Field_Map_Index_2 = BirthValley ($2B)`.
8. Write `Map_Start = ($3C,$14)` in 8-pixel units, facing down, alignment 0,
   then `RefreshMap`.
9. Fade in and start `Music_TerribleSight` (`$97`); save the same track.
10. Add 500 meseta.
11. Set `EventFlag_HoltPetrified` (`$10`).
12. Return `d0 = 1`.

`LoadMap` is blocking in the native interpreter: the runtime rebuilds Zema and
recasts the scene before the return edge can run. That barrier is required for
the following tree-4 and Zema-object scenes; the old runner advanced through a
map request with the old cast still attached.

## Verification

`next_arc.rs` drives this event from a Birth Valley runtime and asserts the
scene returns on Zema with flag `$10` set. The arc test uses that exact edge as
its first post-Piata state.
