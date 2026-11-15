# `Cutscene_Reunion`

- **Retail bytes:** `$078A7C..$078D2F` inclusive, 692 bytes.
- **Pointer:** `CutscenePtrs[$1F]`; scene `$801F`.
- **Trigger:** `RunEventsJmpTbl[$50]`, Elsydeon `$D9` set and Reunion `$DA`
  clear.
- **Data:** `dezo_endgame.rs`, `REUNION` (38 ops).

## Clone audit

The retail `grand_cross=0` cutscene table selects this body; the hack-only body
is excluded. ROM range: `$078A7C..$078D30` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-12 | `$078A7C..$078B5D` | move/object destinations, panel `$13D`, dialogue and takeoff | actor/presentation/dialogue |
| 13-21 | `$078B5E..$078C06` | load Mota Spaceport, Machine Center music, move Chaz, panels `$13F/$13E` | map/presentation |
| 22-24 | `$078C07..$078C32` | dialogue `1`, set Reunion `$DA`, reload spaceport | dialogue/flag/map |
| 25-36 | `$078C33..$078D2F` | configure Hahn/Gryz/Demi/Raja/Kyra equipment; return | roster equipment |

