# `Cutscene_LutzRevelation`

- **Retail bytes:** `$077896..$077A2D` inclusive, 408 bytes.
- **Pointer:** `CutscenePtrs[$14]`; scene `$8014`.
- **Trigger:** `RunEventsJmpTbl[$39]`, Revelation `$97` clear at the Lutz
  position rectangle.
- **Data:** `dezo_campaign.rs`, `LUTZ_REVELATION` (30 ops).

## Clone audit

The retail `grand_cross=0` CutscenePtrs body is authoritative; the Grand Cross
include is excluded. ROM range: `$077896..$077A2E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-6 | `$077896..$07791F` | dialogue `$2A`, move Chaz, stairs SFX, load Inner Sanctuary B1 | dialogue/map |
| 7-18 | `$077920..$0779A9` | Kyra movement/facing choreography and waits | actor/wait |
| 19-25 | `$0779AA..$0779F0` | dialogue `$2F`, panel `$9E`, resume; set `$97` | dialogue/flag |
| 26-29 | `$0779F1..$077A2D` | reload Inner Sanctuary B1 with alignment `8`, return | map |

