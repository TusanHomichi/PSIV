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

## Current implementation limits

The connected native route completes the party replacement and flag `$30`,
and CONTINUE restores the resulting Gryz party. This proves progression only.
The current transcription incorrectly treats map NPC 6 as the temporary Rune
object at `$C480`; the original initializes a new `$190` object from Rune's
actual party position. It also uses a fixed party slot and camera destination.
The original replaces Rune in his current slot, seeds Gryz from NPC 2, waits
for Rune's Y destination, settles the leader, and pans to the leader's current
position. These staging differences remain to be repaired. The existing
headless NPC landing assertion verifies the current implementation, not the
original scene. Do not call it comparator or visual fidelity proof.
