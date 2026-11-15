# `Event_EclipseTorchUsed`

- **Retail bytes:** `$07018A..$070481` inclusive, 760 bytes.
- **Pointer:** `EventPtrs[$47]`; event `$0047`.
- **Trigger:** `RunEventsJmpTbl[$36]`, Lashiec `$9B` set and Eclipse Torch
  `$9C` clear, position `x=$B80..$BB0`, `y<=$E0`.
- **Data:** `dezo_campaign.rs`, `ECLIPSE_TORCH_USED` (30 ops).

## Clone audit

The retail `grand_cross=0` EventPtrs body is used. The clone's Grand Cross
alternative is excluded (`ps4.options.asm:10`). ROM range:
`$07018A..$070482` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-8 | `$07018A..$070225` | `beq.w loc_70226` (`$07018E`): on foot to op 9; mounted: save Dezolis music `$99`, stop SPC, a bare frame (`$0701A0`), play `$99` (`$0701A6`), rebuild sprites and a bare frame (`$0701B4`), dismount, `Field_UpdateObjects` and a frame (`$070220`) | vehicle/presentation/`WaitFrames` |
| 9-15 | `$070226..$07025E` | panel `$10D`, DMA, waits `120/40`, Deban, destroy panel | panel/SFX |
| 16-25 | `$07025F..$0703C8` | art `$1D712C` tile `$3AF`; nine object updates | art/object |
| 26-29 | `$0703C9..$070481` | tree `$2080BE`, dialogue `$2A`, set `$9C`, return | dialogue/flag |

The frame at `$070220` runs `Field_UpdateObjects`; the port's is a bare frame
(`docs/source-notes/README.md`, "Dezolis dismount frames").

