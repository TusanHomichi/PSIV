# `Event_CarnivorousTrees`

- **Retail bytes:** `$070774..$070855` inclusive, 226 bytes.
- **Pointer:** `EventPtrs[$4C]`; event `$004C`.
- **Trigger:** custom arm of `RunEventsJmpTbl[$35]`, Raja Sick `$94` clear.
- **Data:** `dezo_campaign.rs`, `CARNIVOROUS_TREES` (13 ops).

## Clone audit

The retail pointer at `EventPtrs[$4C]` is the `grand_cross=0` body. The clone
build is Grand Cross (`ps4.options.asm:10`), and its alternative body is not
authority. ROM range: `$070774..$070856` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-7 | `$070774..$0707F9` | save Dezolis music; mounted/on-foot branch; stop SFX and dismount | vehicle/presentation |
| 8-10 | `$0707FA..$07082F` | tree dialogue tree `$2080BE`, entry `$34`, move down `#$20` | dialogue/actor |
| 11-12 | `$070830..$070855` | event battle `$0A`, return `1` | battle/return |

