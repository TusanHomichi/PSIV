# `Event_CarnivorousTrees`

- **Retail bytes:** `$070774..$070855` inclusive, 226 bytes.
- **Pointer:** `EventPtrs[$4C]`; event `$004C`.
- **Trigger:** custom arm of `RunEventsJmpTbl[$35]`, Raja Sick `$94` clear.
- **Data:** `dezo_campaign.rs`, `CARNIVOROUS_TREES` (14 ops).

## Clone audit

The retail pointer at `EventPtrs[$4C]` is the `grand_cross=0` body. The clone
build is Grand Cross (`ps4.options.asm:10`), and its alternative body is not
authority. ROM range: `$070774..$070856` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-1 | `$070774..$070781` | save Dezolis music `$99`; `beq.w loc_70826` (`$07077E`): on foot to op 11, past the dismount and dialogue | vehicle |
| 2-8 | `$070782..$070811` | mounted: stop SFX, a frame of map updates (`$07078C`), Red Alert, rebuild sprites and a bare frame (`$0707A0`), dismount, `Field_UpdateObjects` and a frame (`$07080C`) | presentation/`WaitFrames`/vehicle |
| 9-10 | `$070812..$070825` | dialogue tree `$2080BE`, entry `$34` | dialogue |
| 11 | `$070826..$070847` | `loc_70826`: every party object's Y set to the leader's plus `$20` | `PlacePartyY { dy: 0x20 }` |
| 12-13 | `$070848..$070855` | event battle `$0A`, return `1` | battle/return |

The frame at `$07080C` runs `Field_UpdateObjects`; the port's is a bare frame
(`docs/source-notes/README.md`, "Dezolis dismount frames").

