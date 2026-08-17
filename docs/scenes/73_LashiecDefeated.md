# `Cutscene_LashiecDefeated`

- **Retail bytes:** `$077A68..$077BD9` inclusive, 370 bytes.
- **Pointer:** `CutscenePtrs[$16]`; scene `$8016`.
- **Trigger:** `RunEventsJmpTbl[$47]`, Lashiec `$9B` set.
- **Data:** `dezo_campaign.rs`, `LASHIEC_DEFEATED` (27 ops).

## Clone audit

Retail `grand_cross=0` is the cutscene table body; the Grand Cross alternative
is excluded. ROM range: `$077A68..$077BDA` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-5 | `$077A68..$077B0A` | panels `$107/$10A`, dialogue `$38`, explosion and waits | panel/dialogue/SFX |
| 6-16 | `$077B0B..$077B75` | field music, resume, add Eclipse Torch `$8E`, load Gumbious F1 `$162` | inventory/map |
| 17-26 | `$077B76..$077BD9` | temple music/tree `$1F471C`, dialogue `$39`, tree `$209B36` | music/dialogue |

