# `Event_JuzaDefeated`

- **Retail bytes:** `$06FB5E..$06FBED` inclusive, 144 bytes.
- **Pointer:** `EventPtrs[$41]`; event `$0041`.
- **Trigger:** `RunEventsJmpTbl[$2F]`, Juza `$41` set and Juza Defeated `$48`
  clear.
- **Data:** `post_zio_cutscenes.rs`, `JUZA_DEFEATED` (18 ops).

## Clone audit

The body at `ps4.asm:149162` is the retail EventPtrs[$41] body selected by
the `grand_cross=0` table. Actual-ROM range: `$06FB5E..$06FBEE` exclusive end.
Grand Cross-only bodies and FortuneTeller records remain excluded.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$06FB5E..$06FB65` | Door Opened SFX `$E2` | sound |
| 1/3/5/7/9/11/13/15 | `$06FB74..$06FBAC`; table `$06FBC4..$06FBEC` | eight alternating closed/open chunk rows at BG `(12,9),(13,9),(12,10),(13,10)`; `RefreshPlaneBG` at `$06FBA6` | closed `WriteMapChunks`, open `RestoreMapChunks` |
| 2/4/6/8/10/12/14/16 | `$06FBB0..$06FBBA` | `DMAPlane_B_VInt`, DBF d0: each row's delay 6 means seven frames, 56 total | `WaitFrames(7)`, no updates |
| 17 | `$06FBBC..$06FBC3` | set Juza Defeated `$48` after the last delay | flag |

**The tiles are the mechanism, not decoration.** `MapDataManager` runs at map
load only, so the event cannot rely on the flag to open the stairway: it writes
the four chunks through the layout itself and collision follows. The row the
eight-row table ends on is the map's *own* layout at those chunks
(`(12,9)=$23`, `(13,9)=$1A`, `(12,10)=$19`, `(13,10)=$1B`), so the transcription
uses `RestoreMapChunks`, which verifies those literal ids and drops the map
effect's closed-chunk overlay — the same shape as
[Event_RuneFlaeli](17_RuneFlaeli.md)'s rock. See the
[live-layout census](LIVE_LAYOUT_WRITES.md); the flag is the story edge, the
chunk restore is the walkable one. The next dispatch in the same retail trigger
table is Rune Flaeli `$30`, not a hidden Grand Cross FortuneTeller scene.

The [DMA frame classification](DMA_FRAME_OWNERSHIP.md) separates the repaired
explicit seven-frame delays from the CPU refresh boundary at `$06FBA6`, which
remains [#546](https://github.com/TusanHomichi/PSIV/issues/546). The census
reads each row's delay and chunk ids from the local US image; runner coverage
checks the eight writes at frames 0, 7, …, 49 and the flag at frame 56.
