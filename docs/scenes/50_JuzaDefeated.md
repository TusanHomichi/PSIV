# `Event_JuzaDefeated`

- **Retail bytes:** `$06FB5E..$06FBED` inclusive, 144 bytes.
- **Pointer:** `EventPtrs[$41]`; event `$0041`.
- **Trigger:** `RunEventsJmpTbl[$2F]`, Juza `$41` set and Juza Defeated `$48`
  clear.
- **Data:** `post_zio_cutscenes.rs`, `JUZA_DEFEATED` (3 ops).

## Clone audit

The body at `ps4.asm:149162` is the retail EventPtrs[$41] body selected by
the `grand_cross=0` table. Actual-ROM range: `$06FB5E..$06FBEE` exclusive end.
Grand Cross-only bodies and FortuneTeller records remain excluded.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$06FB5E..$06FB6F` | Door Opened SFX `$E2` | sound |
| 1 | `$06FB70..$06FBEC` | `GetMapLayoutOffset(12,9,BG)` then eight table rows at `$06FBC4` — delay, four chunk ids — alternating the closed chunk `$16` with `$23,$1A,$19,$1B`, each row followed by `RefreshPlaneBG` and `dbra 6` VBlanks | `RestoreMapChunks` |
| 2 | `$06FBEC..$06FBED` | set Juza Defeated `$48` | flag |

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

