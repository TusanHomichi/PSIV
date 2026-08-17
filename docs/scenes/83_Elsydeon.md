# `Cutscene_Elsydeon`

- **Retail bytes:** `$078584..$078A7B` inclusive, 1272 bytes.
- **Pointer:** `CutscenePtrs[$1E]`; scene `$801E`.
- **Trigger:** the Elsydeon cave interaction after `$801D`; Reunion `$DA` is
  still clear.
- **Data:** `dezo_endgame.rs`, `ELSYDEON` (46 ops).

## Clone audit

Retail `grand_cross=0` selects this cutscene body. The Grand Cross alternative
is excluded. ROM range: `$078584..$078A7C` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-18 | `$078584..$0786D8` | panels `$129/$12B/$12C/$12E/$12F/$130`, waits and Age of Fables | panel/dialogue |
| 19-35 | `$0786D9..$0789A4` | panels `$139/$13A/$13B`, dialogue `$32`, palette/map presentation | presentation/dialogue |
| 36-40 | `$0789A5..$078A5D` | set Elsydeon `$D9`; put Elsydeon `$77` in Chaz equipment | flag/equipment |
| 41-45 | `$078A5E..$078A7B` | restore saved party slots; load Inner Sanctuary B1; return | transient party/map |

