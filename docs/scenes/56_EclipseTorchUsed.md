# `Event_EclipseTorchUsed`

- **Retail bytes:** `$07018A..$070481` inclusive, 760 bytes.
- **Pointer:** `EventPtrs[$47]`; event `$0047`.
- **Trigger:** `RunEventsJmpTbl[$36]`, Lashiec `$9B` set and Eclipse Torch
  `$9C` clear, position `x=$B80..$BB0`, `y<=$E0`.
- **Data:** `dezo_campaign.rs`, `ECLIPSE_TORCH_USED` (28 ops).

## Clone audit

The retail `grand_cross=0` EventPtrs body is used. The clone's Grand Cross
alternative is excluded (`ps4.options.asm:10`). ROM range:
`$07018A..$070482` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-7 | `$07018A..$0701FA` | mounted branch; stop SPC; rebuild sprites; dismount; wait | vehicle/presentation |
| 8-13 | `$0701FB..$07025E` | panel `$10D`, DMA, waits `120/40`, Deban, destroy panel | panel/SFX |
| 14-22 | `$07025F..$0703C8` | art `$1D712C` tile `$3AF`; nine object updates | art/object |
| 23-27 | `$0703C9..$070481` | tree `$2080BE`, dialogue `$2A`, set `$9C`, return | dialogue/flag |

