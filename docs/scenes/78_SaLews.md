# `Event_SaLews`

- **Retail bytes:** `$07146A..$071489` inclusive, 32 bytes.
- **Pointer:** `EventPtrs[$61]`; event `$0061`.
- **Reachability:** Sa Lews battle dispatch in the post-tower retail path.
- **Data:** `dezo_endgame.rs`, `SA_LEWS` (4 ops).

## Clone audit

The `grand_cross=0` EventPtrs body is used; the Grand Cross body is excluded.
ROM range: `$07146A..$07148A` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-1 | `$07146A..$07147A` | dialogue entry `1`; set `$D2` | dialogue/flag |
| 2-3 | `$07147B..$071489` | event battle `$17`, return `1` | battle |

