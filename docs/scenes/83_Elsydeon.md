# `Cutscene_Elsydeon`

- **Retail bytes:** `$078584..$078A7B` inclusive, 1272 bytes.
- **Pointer:** `CutscenePtrs[$1E]`; scene `$801E`.
- **Trigger:** the Elsydeon cave interaction after `$801D`; Reunion `$DA` is
  still clear.
- **Data:** `dezo_endgame.rs`, `ELSYDEON` (47 ops).

## Clone audit

Retail `grand_cross=0` selects this cutscene body. The Grand Cross alternative
is excluded. ROM range: `$078584..$078A7C` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-19 | `$078584..$0786D8` | panels `$129/$12B/$12C/$12E/$12F/$130`, waits, Age of Fables and the revision>0 `SFXID_Deban` at `$157483` | panel/sound/dialogue |
| 20-36 | `$0786D9..$0789A4` | panels `$139/$13A/$13B`, dialogue `$32`, palette/map presentation | presentation/dialogue |
| 37-41 | `$0789A5..$078A5D` | set Elsydeon `$D9`; put Elsydeon `$77` in Chaz equipment | flag/equipment |
| 42-46 | `$078A5E..$078A7B` | restore saved party slots; load Inner Sanctuary B1; return | transient party/map |


## Revision-conditional ops

`move.b #SFXID_Deban, (Sound_Index).l` at `$157483` sits under `if revision>0`
(this build is English, `revision = 1`) between the `$31` dialogue's wait and
the `$12B` panel. See the [revision audit](REVISION_AUDIT.md).
