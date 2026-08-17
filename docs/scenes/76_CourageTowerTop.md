# `Event_CourageTowerTop`

- **Retail bytes:** `$071102..$071449` inclusive, 840 bytes.
- **Pointer:** `EventPtrs[$5F]`; event `$005F`.
- **Trigger:** `RunEventsJmpTbl[$4A]`, Courage Tower top `$E3` clear.
- **Data:** `dezo_endgame.rs`, `COURAGE_TOWER_TOP` (18 ops).

## Clone audit

Retail `grand_cross=0` selects this body; the clone's Grand Cross body is
excluded. ROM range: `$071102..$07144A` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-7 | `$071102..$07125C` | face up, camera, four `$2BC` object animations, Foi | actor/camera/object |
| 8-16 | `$07125D..$0713F9` | four `$2B4` animations, Recovery/Laser/Killed SFX, waits | object/SFX |
| 17 | `$0713FA..$071449` | set Courage Tower top `$E3` | flag |

