# `Cutscene_Dorin`

- **Retail bytes:** `$074034..$0741E5` inclusive, 434 bytes.
- **Pointer:** `CutscenePtrs[$04]` from `$05A580`; scene event `$8004`.
- **Trigger:** map TonoeGryzHouse `$43`, trigger `$14`, after flag `$36`.
- **Data:** `next_arc.rs`, `DORIN_OPS`.

## Clone audit

The retail implementation is the `grand_cross=0` side of the clone's
conditional body. The Grand Cross side is a different/absent scene include;
it is not a retail oracle. The retail disassembly was compared against the
surviving branch over all 434 bytes before transcription.

## Retail transcription

1. Initialise VRAM/CRAM, fade in, enable cutscene sprites, and run cutscene
   dialogue entry `$25`.
2. Set `Map_Start = ($3C,$3C)` and set `Map_Load_Flags` bit 3; refresh the
   current Tonoe Gryz House map.
3. Find Rune's party slot, stage the `$C480` invisible Rune sequence, remove
   Rune from macros, and clear Rune's transient animation state.
4. Replace Rune with Gryz. Build Gryz from NPC `$C380` (map NPC index 2),
   choose the party-order art tile from `$534`, run the Gryz field routine,
   clear the NPC object, add Gryz to macros, rebuild sprites and fade in.
5. Wait corrected `$13 + 1 = 20` map-update ticks; play `Music_TonoeDePon`
   (`$81`). Set the temporary Rune destination to `($1F0,$300)` and the
   Dorin secondary-object destination to `($1E0,$300)`. Run both objects until
   their destinations and then settle `Character_1`.
6. Move the camera at speed 2, set `EventFlag_GryzJoined` (`$30`) and return
   `d0 = 1`.

The core transcription keeps NPC 6 as the retail invisible Rune slot and NPC 0
as the secondary Dorin slot. The runtime now consumes both
`ActorMoveStarted` and `ActorArrived` for those NPCs and writes their landing
cells into `FieldMap`.

## Verification

The headless test asserts NPC 6 lands at cell `(31,49)` and NPC 0 at
`(30,49)`, then checks flag `$30`. This is the comparator-facing proof for the
long-flagged movement gap.
