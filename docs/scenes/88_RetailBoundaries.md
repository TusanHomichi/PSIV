# Retail boundary census

This is the honest boundary record for retail surfaces adjacent to the fixed
story dispatch chain. It is not a junk drawer of unimplemented scenes: each
row is either promoted to the typed registry because its retail body is a
deterministic scene-shaped sequence, or has a precise reason it remains an
interaction/state-machine boundary.

## Authority and range convention

The reference source is a Grand Cross clone, so `reference/ps4.options.asm:10`
sets `grand_cross = 1`. The retail `grand_cross=0` branches were read against
the USA ROM, not treated as executable proof from the clone. Ranges below are
inclusive bytes. Pointer-table entries remain the authority for the start;
the next pointer or the routine's terminal instruction supplies the end.

ROM: `Phantasy Star IV (USA).md`, SHA-256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`.

## Disposition table

| Surface | Retail bytes | Disposition | Evidence / exact reason |
|---|---|---|---|
| `Cutscene_RajaSick` / `$8012` | `$07734C..$077787` | **Transcribed** | 54 typed ops in `retail_endgame.rs`; fixed post-arrival presentation and map handoff. |
| `Cutscene_Rykros` / `$801B` | `$07818E..$078345` | **Transcribed** | 27 typed ops; panels `$18B/$18F/$190`, palette cycle and flag `$D0`. |
| `Cutscene_Ending` / `$8021` | `$078F3E..$07A811` | **Transcribed** | 370 typed ops; normal post-battle story panels, staff roll, credits renderer and final Start gate. See [89](89_Ending.md). |
| Musk Cats / `EventPtrs[$49]` | `$07069E..$0706BF` | **Transcribed** | [180](180_LateArcInteractions.md#event_muskcatsguarding). |
| Musk Cat Elder / `EventPtrs[$4A]` | `$0706C0..$070701` | **Transcribed** | [180](180_LateArcInteractions.md#event_muskcatelder). |
| Penguin Owner / `EventPtrs[$4B]` | `$070702..$070773` | **Transcribed** | [180](180_LateArcInteractions.md#event_penguinowner). |
| Esper Guard permission / `EventPtrs[$4F]` | `$0709A2..$070A29` | **Precise why-not** | Reads Dark Force 2 and Carnivorous Trees flags to choose dialogue `$00/$01/$02`, changes two guard dialogue ids, and sets temp flag `EspMansionGuards`. Direct interaction, not a linear story edge. |
| Inner Sanctuary guard / `EventPtrs[$51]` | `$070A4E..$070A75` | **Transcribed** | Faces the leader, runs dialogue `$35`, sets `InnerSanctGuard1`: three typed ops. See [113](113_InnerSanctuaryGuard.md). |
| Inner Sanctuary second guard / `EventPtrs[$52]` | `$070A76..$070A9D` | **Transcribed** | The same shape with dialogue `$36` and `InnerSanctGuard2`. See [113](113_InnerSanctuaryGuard.md). |
| Raja Sick NPC interaction / `EventPtrs[$5B]` | `$070C30..$070C81` | **Transcribed** | [180](180_LateArcInteractions.md#event_rajasick). |
| Gyuna / `EventPtrs[$5C]` | `$070C82..$070CB3` | **Precise why-not** | Chooses dialogue `$31/$32` from `LandaleWhereabouts`, then inspects the selected dialogue's terminal byte (`$35` on USA revision 1) before setting the flag. The player dialogue path is the state transition. |
| Anger Tower Alys / `EventPtrs[$62]` | `$07148A..$071579` | **Transcribed** | Dialogue `$07`, the facing-table steps of object 0 and the leader (`StepObjectAlongFacing`), retained dialogue `$08`, flag `$E4`, the leader set at `$1F0,$1F0`, battle `$18`. See [114](114_AngerTowerAlys.md). |
| Hunters Guild / `EventPtrs[$66]` | `$0718E6..$071E55` | **Transcribed** | [124](124_HuntersGuild.md): the routine's branches as ops, its list window a session mode. |
| Rune healing Chaz / `EventPtrs[$67]` | `$071E56..$071E65` | **Transcribed** | [180](180_LateArcInteractions.md#event_runehealingchaz). |
| Fifth-character selection / `EventPtrs[$68]` | `$071E66..$0721CB` | **Transcribed** | [181](181_PickingFifthCharacter.md): the routine's legs as ops on its table row. |
| Rykros arrival gate / `loc_64C1E` | `$064C1E..$064CFA` | **Precise why-not** | Tests `World_Index == 2`, Rykros flag clear and Dark Force 3 defeated; otherwise it runs the ordinary space-travel loop, which polls ButtonSpeak/Start, updates objects, palette and camera, and can exit early. The deterministic `Cutscene_Rykros` body is transcribed; this global travel gate is not flattened. |

The old `$8021` “to EOF” row is deliberately gone. The executable ending stops
at `$07A811`; `$07A812..$07AFFF` is padding and `$07B000` begins aligned panel
records. That distinction is checked by the ending documentation and pack
extractor.

## Transcribed boundary: Raja Sick

`Cutscene_RajaSick` is the fixed cinematic after the player reaches the Raja
arrival state. The retail body:

1. faces the leader opposite the C400 object and runs dialogue `$59`;
2. cycles Raja through eight facings, ten map-update ticks each;
3. loads art `$1D78CA` at tile `$37F`, stages temporary object `$FFFFC4C0`
   (`$374`, same art), plays `$E1`, waits 20, plays music `$9F`, waits 20;
4. runs the party-arrangement helper against Raja and companions Chaz, Rune,
   Rika and Wren;
5. runs dialogue `$62`, fades out, sets event flag `$94`, removes Raja, clears
   Raja stats byte `$16`, and loads map `$134` from `$133` at `$34,$38`;
6. places the four remaining party objects at the retail coordinates, resumes
   dialogue `$63` after the stair/alert temporary-object sequence, faces the
   four named characters down, resumes the saved dialogue, and returns 1.

The typed contract keeps the temporary object and arrangement helper as
literal presentation records. It does not invent sprite rendering or battle
behavior. `retail_boundaries.rs` proves flag `$94`, map `$134`, the four-member
roster and the arrangement record.

## Transcribed boundary: Rykros arrival/tower

The global gate at `$064C1E` is separate from `Cutscene_Rykros`:

```text
World_Index != 2                         -> ordinary space travel
EventFlag_Rykros already set             -> ordinary space travel
EventFlag_DarkForce3Defeated is clear    -> ordinary space travel
otherwise                                -> jump Cutscene_Rykros ($801B)
```

The scene itself is deterministic: initialise/fade, create panels `$18B` and
`$18F`, select dialogue tree 39, run entry `$16` through the Rykros window,
create `$190`, copy the palette buffer, raise the palette for 30 frames, play
barrier sound `$E6`, resume dialogue, destroy the last panel, run the six-delay
palette table at `$0782DA` using delays `7,5,7,7,7,7`, run the final dialogue
through window 5, set `$D0`, return. The typed record and headless test cover
that body; the travel loop remains an input boundary.

## Why the interactive surfaces stay boundaries

The fixed scene vocabulary is intentionally not a fake UI automation API. The
guild routine has a different shape from the transcribed campaign scenes, as the
fifth-character routine does, and each keeps its branches as a typed op family
([124](124_HuntersGuild.md), [181](181_PickingFifthCharacter.md)):

- **Guild:** eight quest records drive several branches before the job board
  even appears. The USA branch uses dialogue trees 26/27 for some ids, draws
  colored availability states, and then loops on cursor and yes/no input. A
  test that chooses “first job” is one path through a combinatorial state
  machine, not a transcription of the retail routine.
- **Gyuna and guards:** these are direct interactions whose dialogue and flags
  are read at call time. Their exact branches are recorded above. Raja's talk
  and the musk cats' are of the same shape, and are
  [transcribed](180_LateArcInteractions.md) with the branches kept as ops.
- **Rykros travel:** the cutscene is fixed, but the world-travel gate is a
  live loop. It is therefore split into the deterministic scene and the
  precise why-not gate rather than modeled as one invented “arrive at Rykros”
  op.

## The registry

The typed registry has 89 scenes, including Raja Sick `$8012`, Rykros `$801B`,
and Ending `$8021`. The full-campaign headless proof keeps a
single title-to-ending snapshot for the fixed story chain; this boundary file
records the adjacent player-controlled surfaces that proof deliberately does
not pretend to traverse.
