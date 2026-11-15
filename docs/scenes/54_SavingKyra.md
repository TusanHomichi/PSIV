# `Event_SavingKyra`

- **Retail bytes:** `$070856..$070975` inclusive, 288 bytes.
- **Pointer:** `EventPtrs[$4D]`; event `$004D`.
- **Trigger:** custom arm of `RunEventsJmpTbl[$35]`, Raja Sick `$94` set and
  Saving Kyra `$95` clear.
- **Data:** `dezo_campaign.rs`, `SAVING_KYRA` (21 ops).

## Clone audit

`EventPtrs[$4D]` is read from the retail `grand_cross=0` table. The clone's
Grand Cross branch is excluded; `ps4.options.asm:10` proves it is not a retail
body. ROM range: `$070856..$070976` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$070856..$07085D` | `beq.w loc_70908` (`$07085A`): on foot to op 11, past the dismount and dialogue `$34` | vehicle |
| 1-8 | `$07085E..$0708F3` | mounted: save Dezolis music `$99`, stop SFX, a frame of map updates (`$07086E`), stop music `$FB`, rebuild sprites and a bare frame (`$070882`), dismount, `Field_UpdateObjects` and a frame (`$0708EE`) | presentation/`WaitFrames`/vehicle |
| 9-10 | `$0708F4..$070907` | dialogue tree `$2080BE`, entry `$34` | dialogue |
| 11-16 | `$070908..$07093B` | `loc_70908`: red alert, a frame of map updates (`$070912`), panel `$98`, DMA, dialogue entry `$26` | presentation/dialogue |
| 17 | `$07093C..$07095D` | every party object's Y set to the leader's plus `$20` | `PlacePartyY { dy: 0x20 }` |
| 18-20 | `$07095E..$070975` | set Saving Kyra `$95`, event battle `$0A`, return `1` | flag/battle |

The frame at `$0708EE` runs `Field_UpdateObjects`; the port's is a bare frame
(`docs/source-notes/README.md`, "Dezolis dismount frames").

The dialogue call is `Event_GetAndRunDialogue2` (`$5ACDC`), the retained window
(`retained(0x26)`): see [the Dialogue2 audit](DIALOGUE_CALLERS.md#the-dialogue2-audit), which also
resolves the party move.
