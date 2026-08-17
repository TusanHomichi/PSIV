# `Event_DeVars`

- **Retail bytes:** `$07144A..$071469` inclusive, 32 bytes.
- **Pointer:** `EventPtrs[$60]`; event `$0060`.
- **Reachability:** DeVars battle dispatch in the post-tower retail path.
- **Data:** `dezo_endgame.rs`, `DE_VARS` (4 ops).

## Clone audit

`EventPtrs[$60]` is the retail `grand_cross=0` body. The Grand Cross body is
excluded. ROM range: `$07144A..$07146A` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-1 | `$07144A..$07145A` | dialogue entry `3`; set `$D4` | dialogue/flag |
| 2-3 | `$07145B..$071469` | event battle `$16`, return `1` | battle |

