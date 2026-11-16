# `Cutscene_LashiecDefeated`

- **Retail bytes:** `$077A68..$077BD9` inclusive, 370 bytes.
- **Pointer:** `CutscenePtrs[$16]`; scene `$8016`.
- **Trigger:** `RunEventsJmpTbl[$47]`, Lashiec `$9B` set.
- **Data:** `dezo_campaign_late.rs`, `LASHIEC_DEFEATED` (40 ops).

## Clone audit

Retail `grand_cross=0` is the cutscene table body; the Grand Cross alternative
is excluded. ROM range: `$077A68..$077BDA` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-19 | `$077A68..$077B2D` | Init, In, panel `$107`, dialogue `$38`, Out, Init, explosions, panel `$10A`, In, resume | panels/dialogue/fades |
| 20-23 | `$077B2E..$077B67` | Out, add Eclipse Torch `$8E`, Init, world 1 | fade/inventory/world |
| 24-30 | `loc_64C1E`, called at `$077B68` | shared planet screen, In, 300-frame caption hold, Out | shared flight ops |
| 31-39 | `$077B6E..$077BD9` | Gumbious F1 `$162`, temple music, In, tree 22, dialogue `$39`, tree 38 | map/music/fade/dialogue |

## Fixed fades

The US range's direct calls are Init `$077A68`, In `$077A6E`, Out `$077AB4`,
Init `$077ABA`, In `$077AFC`, Out `$077B2E`, Init `$077B52`, In `$077BAC`.
The second Out is after Run5 and before the torch is returned. The caller's
world-1 write precedes `loc_64C1E`, whose own In `$064C8E` and Out `$064CFA`
belong between the third Init and the map load, just as for the Dark Force 1
aftermath. `flight::with_planet_screen` owns that shared transcription;
the image census inserts its calls at the caller's boundary without claiming
its bytes as Lashiec's direct range.

The runtime regression closes dialogue immediately on an isolated fixture,
checks the explosion's fade plus its independent 240-frame hold, the planet
screen's fades plus its independent 300-frame hold, and the map outcome.
These modeled intervals do not certify the whole retail scene clock: the
existing transcription still omits the second explosion's 20-frame wait,
the bare frame at `$077B14`, and two 60-frame map-update loops around the
item (`$077B36`, `$077B4C`), within
[#470](https://github.com/TusanHomichi/PSIV/issues/470)'s wait audit.
