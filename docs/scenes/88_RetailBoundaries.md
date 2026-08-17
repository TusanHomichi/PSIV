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
| `Cutscene_Ending` / `$8021` | `$078F3E..$07A811` | **Transcribed** | 367 typed ops; normal post-battle story panels, staff roll, credits renderer and final Start gate. See [89](89_Ending.md). |
| Musk Cats / `EventPtrs[$49]` | `$07069E..$0706BF` | **Precise why-not** | One interaction dialogue (`$1C`), two NPC dialogue ids `$1D`, and event flag `$90` (`MuskCats`); it is an NPC interaction side effect, not a fixed campaign scene. The pointer body is still recorded in [12](12_ArcTriggerCensus.md). |
| Musk Cat Elder / `EventPtrs[$4A]` | `$0706C0..$070701` | **Precise why-not** | Branches on all 40 inventory slots: full inventory shows dialogue `$2E`; otherwise dialogue `$2D`, inserts Silver Tusk and sets its flag. The branch depends on live inventory capacity. |
| Penguin Owner / `EventPtrs[$4B]` | `$070702..$070773` | **Precise why-not** | Computes the `$500` money gate, runs yes/no dialogue `$12`, inspects the retail dialogue terminator (`$35` on revision 1), then conditionally subtracts money, changes the NPC and sets the Penguin flag. Input and wallet state are part of the behavior. |
| Esper Guard permission / `EventPtrs[$4F]` | `$0709A2..$070A29` | **Precise why-not** | Reads Dark Force 2 and Carnivorous Trees flags to choose dialogue `$00/$01/$02`, changes two guard dialogue ids, and sets temp flag `EspMansionGuards`. Direct interaction, not a linear story edge. |
| Inner Sanctuary guard / `EventPtrs[$51]` | `$070A4E..$070A75` | **Precise why-not** | Faces the leader, runs dialogue `$35`, sets `InnerSanctGuard1`; it is a direct guard interaction. |
| Inner Sanctuary second guard / `EventPtrs[$52]` | `$070A76..$070A9D` | **Precise why-not** | Same direct interaction shape with dialogue `$36` and `InnerSanctGuard2`; no fixed campaign transition. |
| Raja Sick NPC interaction / `EventPtrs[$5B]` | `$070C30..$070C81` | **Precise why-not** | Faces the leader, tests Dark Force 2, then uses `UpdateRNGSeed` and `seed & 3` to select dialogue `$5D/$5E/$5F`, or `$60` after the gate. There is no deterministic dialogue result to encode as one scene. |
| Gyuna / `EventPtrs[$5C]` | `$070C82..$070CB3` | **Precise why-not** | Chooses dialogue `$31/$32` from `LandaleWhereabouts`, then inspects the selected dialogue's terminal byte (`$35` on USA revision 1) before setting the flag. The player dialogue path is the state transition. |
| Anger Tower Alys / `EventPtrs[$62]` | `$07148A..$071579` | **Precise why-not** | Dialogue `$07`, dynamic movement from table `$071572`, dialogue `$08`, then battle index `$18` and flag `$E1`; it owns live NPC/party positions and a battle handoff, not a fixed scene-only return. |
| Hunters Guild / `EventPtrs[$66]` | `$0718E6..$071E55` | **Precise why-not** | The retail body loops eight `HuntersGuildData` records (8 bytes each), branches on three quest flags plus availability/expiry flags, renders a job window, polls `Joypad_Pressed` for up/down/cancel/speak/camp, asks yes/no, and writes quest flags and money. A linear transcription would invent the cursor/input path. |
| Rune healing Chaz / `EventPtrs[$67]` | `$071E56..$071E65` | **Precise why-not** | It is only dialogue `$2B` followed by `Event_Recovery`, which mutates the live party's HP/TP/status. This is a direct NPC service, not a story scene or roster join. |
| Fifth-character selection / `EventPtrs[$68]` | `$071E66..$0721CB` | **Precise why-not** | Waits for all current party objects, derives a candidate from the leader's map position using `$071B0/$072182`, runs candidate-specific dialogue twice, reads yes/no, copies a 32-byte character record, removes/replaces a selected slot and sets `Current_Party_Slot_5`. The selected character and destination are input/position dependent. |
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
guild and fifth-character routines have a different shape from the transcribed
campaign scenes:

- **Guild:** eight quest records drive several branches before the job board
  even appears. The USA branch uses dialogue trees 26/27 for some ids, draws
  colored availability states, and then loops on cursor and yes/no input. A
  test that chooses “first job” is one path through a combinatorial state
  machine, not a transcription of the retail routine.
- **Fifth character:** the routine's candidate is selected from the current map
  coordinate and party object position. Confirmation can replace a selected
  seated character or fill slot 5, with character-specific dialogue and a
  character-record copy. There is no single roster edge to assert without
  choosing a player path.
- **Raja/Gyuna and guards:** these are direct interactions whose dialogue,
  flags, inventory, money or RNG are read at call time. Their exact branches
  are recorded above; flattening them into a scene would erase those inputs.
- **Rykros travel:** the cutscene is fixed, but the world-travel gate is a
  live loop. It is therefore split into the deterministic scene and the
  precise why-not gate rather than modeled as one invented “arrive at Rykros”
  op.

## Registry result

The typed registry now has 89 scenes: the former 86, plus Raja Sick `$8012`,
Rykros `$801B`, and Ending `$8021`. The full-campaign headless proof keeps a
single title-to-ending snapshot for the fixed story chain; this boundary file
records the adjacent player-controlled surfaces that proof deliberately does
not pretend to traverse.
