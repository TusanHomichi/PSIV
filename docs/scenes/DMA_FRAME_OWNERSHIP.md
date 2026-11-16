# Layout and animation DMA frame ownership

The US image's `DMAPlanes_VInt` (`$041678`), `DMAPlane_A_VInt`
(`$041682`) and `DMAPlane_B_VInt` (`$0416A6`) each end in
`VInt_Prepare`: one frame, no map or object updates inside the helper.
A static call in a loop can consume many frames. `DmaPlanes` is appropriate
only when no existing wait or mechanic already spends that frame.
`rust/psiv-core/src/scenes/dma_census/layout_animation.rs` checks these seven
callers against their image instructions and named native consumers.

| Caller | Uncovered DMA call | Repetitions | Native consumer / ownership |
| --- | --- | --- | --- |
| `Event_RuneFlaeli` | B at `$06D954` | 240, `move.w #$EF,d0` at `$06D92E`, DBF at `$06D964` back to `$06D932` | `WaitFrames` ops 23/27/29/31: 70 + 26 + 10 + 134. The `ObjectAnimation` op records 240 updates but does not block; the waits spend the frames. |
| `Event_MachineCenterAppearing` | B at `$06B536` | 369, `move.w #$170,d7` at `$06B510`, DBF at `$06B54A` back to `$06B514` | `Wait` ops 5/7: 308 + 61. The chunk write at op 6 is between those waits, after d7 = `$3D`'s DMA. |
| `Event_MeetingLeRoof` | Both at `$0705A2` | Once, after FG/BG refresh at `$070596`/`$07059C` | `ReloadMapChunks` op 18 is immediate and spends no DMA frame. Missing CPU refresh/DMA boundary: [#546](https://github.com/TusanHomichi/PSIV/issues/546). The earlier picture DMA is already op 8; its frame and palette-loop repairs remain intact. |
| `Event_EclipseTorchUsed` | B at `$07043C` | Once, after `Field_UpdateObjects` at `$070430` and BG refresh at `$070436` | `WaitFrames(1)` op 36 already spends the explicit DMA frame. No second wait. The preceding CPU refresh boundary belongs to #546; the missing chunk-definition copy and object pass remain [#674](https://github.com/TusanHomichi/PSIV/issues/674). The panel DMAs are already ops 10/25. |
| `Event_TylerGraveOpening` | B at `$06FCFE` | Once, after the stair write `$06FCF4` and BG refresh `$06FCF8` | `WriteMapChunks` op 6 is immediate. Missing CPU refresh/DMA boundary: #546. `DriftNpcs`' 64 updates are the following `DoMainUpdatesLoop`, not a consumer for this preceding DMA. |
| `Event_JuzaDefeated` | B at `$06FBB0` | Seven per row (DBF d0 at `$06FBB6` back to the call), eight rows at `$06FBC4`: 56 frames | Eight `WaitFrames(7)` ops 2/4/6/8/10/12/14/16, each after its chunk write/restore. The final flag follows the last delay. CPU BG refresh at `$06FBA6`, once per row, remains #546; the seven explicit waits per row are separate from its CPU cost. |
| `Event_WreckageEngine` | A at `$06DD3E` | Once per phase, eight phases (`moveq #7,d0` at `$06DD06`, DBF at `$06DD62`) | `WaitFrames(4)` ops 24/26/28/30/32/34/36/38 each spend this DMA plus the three bare `VInt_Prepare` calls at `$06DD44`/`$06DD4A`/`$06DD50`: 32 frames total. CPU FG refresh at `$06DD34` remains #546; adding a DMA op would make each phase five frames. |

## Refresh and update limits

Layout mutation, CPU plane composition and DMA/VInt synchronization are
separate operations. These classifications cover explicit synchronization
frames. They do not assign a fixed CPU cost to a chunk write, borrow time
from the next wait, or implement the shared refresh boundary (#546).
Rune's conditional `loc_6DBAC` tail-jumps to `RefreshPlaneBG` at `$06DBD2`,
inside its 240-pass loop; Machine Center's `loc_6B5C0` calls it at `$06B5D8`.
Their existing waits consume the loops' explicit DMA frames, without
establishing CPU refresh timing parity.

The [grave record](92_TylerGraveOpening.md#slab-animation-and-camera-timing)
distinguishes the refresh discrepancy from the event-return discrepancy:
the four one-frame pans, 64 drift updates and 60-frame hold remain unchanged.
The grave DMA is not charged to any of those phases.

Rune's waits run no native field-object pass; Machine Center's waits run
map updates without retail's object pass. These are existing update-policy
limits ([#470](https://github.com/TusanHomichi/PSIV/issues/470)), not evidence
that another synchronization frame is needed. Eclipse Torch's flash and
nine-object repairs are preserved. Wreckage and Juza's DMA delay phases run
neither map nor object updates.

The image guards pin call targets, loop counts/back edges and the Juza table's
row delays and writes. Runner tests check exact held-frame counts, Wreckage's
write every fourth frame and Juza's write every seventh frame. Negative
controls remove/add delay frames, insert a duplicate DMA, change update
policy, restore the old Juza collapse and alter a retail loop. These are
instruction and state checks, not a native-input or visual comparison.
