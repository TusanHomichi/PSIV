# `Event_SavingKyra`

- **Retail bytes:** `$070856..$070975` inclusive, 288 bytes.
- **Pointer:** `EventPtrs[$4D]`; event `$004D`.
- **Trigger:** custom arm of `RunEventsJmpTbl[$35]`, Raja Sick `$94` set and
  Saving Kyra `$95` clear.
- **Data:** `dezo_campaign.rs`, `SAVING_KYRA` (19 ops).

## Clone audit

`EventPtrs[$4D]` is read from the retail `grand_cross=0` table. The clone's
Grand Cross branch is excluded; `ps4.options.asm:10` proves it is not a retail
body. ROM range: `$070856..$070976` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-8 | `$070856..$0708E7` | vehicle branch, music/SFX stop, rebuild sprites, tree dialogue `$34` | vehicle/dialogue |
| 9-14 | `$0708E8..$07094E` | red alert, panel `$98`, DMA, dialogue entry `$26`, move down `#$20` | presentation/actor |
| 15-18 | `$07094F..$070975` | set Saving Kyra `$95`, event battle `$0A`, return `1` | flag/battle |

