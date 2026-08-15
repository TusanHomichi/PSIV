# PSIV runtime design

Decided 2026-08-14 (Peter + Fable design session). This is the reference for
the native runtime; changes to these decisions get recorded here, not left in
conversation.

## Goal

A one-for-one PSIV runtime on a modern stack: the retail cartridge is the
spec and the test oracle. Modernization is presentation only. Obvious original
bugs may be fixed (see SOURCE_NOTES discrepancies for the ledger); everything
else reproduces the cartridge, including its data quirks.

## Shape

Cargo workspace at `rust/`, three members, hard dependency direction
(each layer knows nothing about the ones after it):

- **`psiv-data`** — schema layer. Serde types over the runtime pack (below)
  and the `generated/` JSON. Loads, validates counts/id-ranges/cross-refs,
  exposes a typed `GameData`. No game logic.
- **`psiv-core`** — deterministic game core. Pure Rust, no I/O, no rendering,
  no engine types: `State + Input -> State + Effects`. Integer math only —
  the original is a 68000; floats are how you drift from the oracle. Field
  mode first, battle later. Fully headless and test-driven.
- **`psiv-godot`** — thin GDExtension bridge (`gdext`, Godot 4.x). Translates
  core state to the scene tree and input back. Owns presentation (scaling,
  transitions, enhancement packs). Zero game rules. Added once the Godot 4
  editor is installed locally.

## Data path

**Decision: the runtime consumes extractor output; the extractors stay in
Python.** Porting them to Rust re-proves 335 tests of format knowledge for no
gameplay gain, and the Python side is also the research bench. If one-binary
import UX is ever wanted, the escape hatch is porting only the three
decompressors (~150 lines each) against the existing byte-exact test vectors,
with Python remaining the reference implementation.

The `generated/` JSON is archaeology-flavored: provenance-heavy and
deliberately metadata-only for pixel-shaped data. The runtime instead eats a
**runtime pack**: `python -m psiv_tools pack <rom> runtime-pack/` emits a
lean, gitignored bundle shaped for `psiv-data`:

```
runtime-pack/
├── manifest.json          rom sha256, pack format version, map inventory
├── maps/<id>_<symbol>.json  per-map runtime record (below)
└── maps/<id>_<symbol>.png   composed visual render (BG+FG as the game draws)
```

Per-map JSON: dimensions (cells and pixels), the 4-bit collision grid as
rows, warps (source cell + XYRange rect, target map/position/facing), NPC
objects (id, symbol, position, facing, dialogue id), treasure chests, music,
the four per-map flags, dialogue tree binding. Same fail-closed hash
discipline as everything else: the pack records the ROM hash it came from and
`psiv-data` refuses a mismatched pack set.

The pack contains Sega-derived content and is never committed — same rule as
`generated/`.

## Fidelity spine (field mode)

- Logical position lives on the 16-pixel collision-cell grid, exactly as
  `GetChunkAndCollision` models it. Movement is cell-steps; a step carries
  animation progress that the renderer interpolates. Collision blocking set
  is exactly the original's: types 8 (solid), 9 (water), $A (sand), $B (ice),
  $C (shop). Type 1 (map change) does not block — the warp fires on entry,
  per the decoded semantics.
- NPC occupancy blocks movement independently of the terrain grid (the
  collision array is terrain-only) — implemented in FieldMap from the start
  and behaviorally confirmed by the oracle (walking into an NPC on walkable
  terrain stops with coll reading 00).
- Warps resolve through the extracted transition tables; target position and
  facing come from the cartridge data, not invention. There are two tables
  with distinct semantics (proven from `RunMapTransitions` +
  `FieldRoutine_Controls`, 2026-08-14): doorways (table 2) fire on landing on
  a type-1 cell only when the previous standing cell was not type-1, and
  normal-ground transitions (table 1) fire from ordinary standing cells via
  rect scan. The routine runs every frame but dispatches on a standing-cell
  value updated only at rest, so the effective model is per-landing — and
  `GameMode_LoadFieldMap` initializes the standing value to 1, which is the
  cartridge's own "placement never fires a doorway warp" anti-ping-pong rule;
  psiv-core reproduces both. Known microscopic deviation: on retail hardware
  a normal-ground transition can fire mid-step a few frames early as the
  pixel position enters its rect; psiv-core fires on landing. Same step, same
  destination.
- Encounters (later): port the game's own RNG (`UpdateRNGSeed2`) so rolls are
  identical, not merely plausible. Requires one more small extraction of the
  RNG constants/algorithm.
- The collision stride bug in `GetChunkAndCollision` (crossed branch, see
  SOURCE_NOTES) is fixed, not reproduced: the stride comes from the layout
  being read. This is an "obvious bug" fix under the fidelity policy, and it
  is unobservable on retail data anyway.

## Vertical slice (current target)

Walk + warp + NPCs: Chaz moves around Piata with real collision, doorways
warp into interiors (PiataItemShop etc.) and back, NPC objects appear at
their extracted placements. NPCs and Chaz render as placeholders until field
sprites are extracted (filed); the slice proves the data path, collision,
and the map graph, not art fidelity.

Out of scope for the original slice: dialogue windows, battle, sound.
(The overworld paged format and dialogue pack landed later the same day;
battle and sound remain.)

## Presentation: scaling and viewport

Integer scaling only, nearest-neighbor always — pixel art scales losslessly
at integer multiples and at nothing else. Viewport policy (Peter, 2026-08-14):
**wide view is the default.** PSIV's encounter model makes extra visible map
gameplay-neutral — encounters are invisible random rolls from per-map group
tables and bosses are fixed event triggers, so vision reveals nothing early.
An authentic 320×224 viewport (the Genesis's visible area, integer-scaled
with letterbox) becomes a settings toggle, mainly as insurance against the
one wide-view exposure that exists: scripted scenes can show actors
"offstage" at their marks. Both are renderer-only choices; the engine never
knows the viewport.

Planned fix for scene exposure (agreed 2026-08-14): cinema mode. When the
event system reports a scripted scene active, the renderer eases to the
authentic frame (zoom or letterbox) and eases back after — wide view for
play, tight frame for theater. Offstage actors are unexposed by construction
during the only moments that stage them, it reads as intentional
cinematography, and it needs no engine changes and no per-scene tuning. A
per-scene frame-hint override remains available as a fallback tier if some
scene still leaks.

## Testing

- `psiv-core`: unit tests on synthetic grids, plus golden tests driven by
  pack data ("at Piata's academy doorway, stepping up warps to map $11,
  MapID_PiataAcademy, at the coordinates the transition table stores").
- `psiv-data`: schema round-trip and validation tests, gated on pack
  presence (mirroring the Python suite's ROM-gated pattern).
- Later: the original running in an emulator becomes the behavior oracle for
  battle math and RNG streams — scripted input tapes, per-frame logging of
  named RAM addresses (the disassembly gives us the full RAM map), bit-exact
  comparison against psiv-core replaying the same inputs. Emulator choice is
  delegated (Peter, 2026-08-14): selection criteria are Lua/memory-watch
  quality, headless determinism, Linux support, and Genesis core accuracy.
  Likely pick: BizHawk with the Genesis Plus GX core (the TAS standard for
  scriptable determinism); final call when the oracle harness is built, at
  the start of battle work.

## Event engine (designed 2026-08-15, Peter + Fable)

Three kinds of data plus one machine, per docs/EVENT_ENGINE_SCOUT.md:

- **GameState** in psiv-core: the event-flag banks (**four** banks off $F100:
  event/extended, $F140, town — 256/256/256/128 addressable bits, MSB first —
  "~174" was the count of *named* constants, not the space), temp flags
  (never auto-cleared; scenes clear them in explicit pairs),
  `Current_Party_Slots`. Also the future save-file shape.

  **Corrected 2026-08-15**: this said five banks, splitting $F140 into a
  176-bit chest bank and an 80-bit `Temp_Event_Flags` bank at $F156. That
  split is the disassembly's, not the cartridge's — retail contains zero
  instructions addressing $F156, and every `TempEveFlags_*` call dispatches
  through the $F140 door with the id raw. **Retail temp flag N and chest
  flag N are the same bit.** $F140 is one 256-id space running to $F160.
  `Flag::chest(n)` and `Flag::temp(n)` both survive as constructors because
  both names appear in the cartridge's data, but they are aliases. Proof
  chain: the RETAIL FINDING entry dated 2026-08-15 in `SOURCE_NOTES.md`;
  discovery in `docs/MAP_EFFECTS.md` finding 5.

  One consequence is load-bearing rather than cosmetic:
  `TempEveFlag_BioPlantAlarm` is 8 and so is `ChestFlag_Alshline`, so trigger
  $0C (alarm, wants the bit clear) and trigger $18 (Finding Alshline, wants it
  set) are mutually exclusive on hardware. psiv-core reproduces that and pins
  it with a test rather than treating it as a defect. The bridge applies
  flag-gated extractions (layout patches, MapDataManager effects, NPC
  despawns) on flag changes by rebuilding the FieldMap.
- **Triggers**: the 128 `RunEventsJmpTbl` checks transcribed into a condition
  table (flags + position predicate -> event id); custom cases explicitly
  marked. Evaluated on landing, like transitions.
- **Scenes**: a `SceneOp` vocabulary interpreted deterministically in
  psiv-core (MoveActor, Face, RunDialogue, SetFlag, JoinParty, DespawnNpc,
  MoveCamera, Wait, flag/choice branches), effects out to the renderer. Each
  retail scene is a hand-transcribed SceneOp sequence, committed to the repo
  like all behavior code; dialogue text stays in the pack. **The 17
  fork-rewritten scenes are transcribed from retail bytes only** — the clone
  is adversarial there.
- **Renderer**: animates scene actors from effects, routes dialogue `$F6` and
  yes/no into the event queue, and turns on cinema mode (the scene-active
  signal is the hook the viewport section has been waiting for).

Decisions (Peter, 2026-08-15): the BizHawk oracle harness is built
**alongside** event work, not after — scenes are where static-data
verification runs out. v1 scope is the **opening act** (~6 scenes: intro,
Alys joins and leads, the principal's assignment, the basement quest,
leaving Piata), making the game playable as a story to the first dungeon.

## Fidelity questions: oracle verdicts (2026-08-15, oracle/README.md)

Answered by tapes against the running cartridge (Genesis Plus GX headless
harness; determinism proven byte-identical):

- **Text speed: one character per 3 frames** (20/second), measured off
  Win_Tile_Buffer writes. Implemented as a typewriter; a press mid-reveal
  completes the page (ASSUMPTION pending one more tape — the buffer-or-drop
  question below covers it).
- **Talk range: facing only.** The ±8px box one cell ahead is exact; a
  beside NPC at 16px off-axis cannot be talked to. The transcription stood;
  the beside-talk memory did not. (One targeted tape still owed for the
  staged empirical press; the code reading is unambiguous.)
- **Walk timing: exactly 8.00 frames/cell**, all four directions — third
  independent confirmation.
- **The final page does not auto-close** (300 frames observed); a press
  closes over 9 frames.
- **Window opens in 9 frames** — the pack's step_cells is per side; the
  renderer doubles it.
- **No turn-in-place toward walkable cells**: any such press commits a full
  step; turning without moving happens only against blocked cells. Matches
  the engine.
- **First control**: map $13, cell (48,18)+shift, Chaz alone — behavioral
  confirmation of the static extraction. The intro tours maps
  $11→$5E→$54→$00→$13 with Alys leading mid-scene.

Closed 2026-08-15 (tapes 11/13, oracle/README.md): the beside-press is
DISPROVEN on hardware — a press beside a non-faced NPC draws byte-identical
"nothing here" text to open ground (the staging must out-wait wandering
NPCs, and `Windows_Opened_Num` cannot support a negative test because
pressing at nothing opens a window too; `Text_Buffer` is the
discriminator). Text-speed mechanic SETTLED by the sweep + hold tapes
(13/15, correcting an earlier "consumed press" reading): retail is
HOLD-TO-ACCELERATE — one character per frame while Speak is held versus
one per three released — and a press landing in the first half of the
9-frame open animation is genuinely dropped (byte-identical to control).
So the renderer's swallow of early presses is retail-correct; the actual
missing feature is hold-to-fast-forward during the typewriter, and a
queued page-advance would be the wrong implementation. Filed for the
renderer.

Still open: the scroll-arrow art (hardware sprite, still a placeholder
triangle).

### Comparator verdicts (2026-08-15, psiv-replay vs tape 02)

- **Field movement is bit-exact**: 880 frames from first control, 36
  columns (positions in cartridge units, the $1000→0 duration ladder,
  facing, destinations, party slots, and all twelve flag-bank words),
  zero divergences across all four walk legs and every blocked press.
- **Neighbour-collision RAM lags on hardware**: the cartridge refreshes
  its four Tile_Collision mirror bytes ~2 frames after a landing; the
  engine computes them on the landing tick. Values agree; timing of the
  RAM mirror doesn't. Compared with `--skip coll_*` until modeled (if
  ever — nothing gameplay-visible reads the stale window).
- **NPC wander is the frontier**: from frame 7819 the engine's walk-left
  leg stops at cell (44,16) — an NPCType2's spawn cell — while the
  cartridge's copy of that NPC had wandered elsewhere and its walker
  passes through, stopping only at Alys (who stands still). Unmodeled
  movement, already on the backlog; the comparator turned it from a
  "nice to have" into a measured divergence with frame numbers.
- **Update (later 2026-08-15, wander built + seed-aligned replays)**:
  tape 02's scalar verdict is CLEAN over the full 1080 frames (after
  modeling the cartridge's one-frame collision-cache lag on landings);
  the per-object verdict is clean for 619 frames and then diverges at
  frame 7558 because the CAMERA is now the frontier — retail's camera
  scrolls against screen-space thresholds and trails the party (an
  object provably wakes 14 frames after the leader stops), while our
  placeholder is leader-centred. The visibility gate advances the
  shared RNG stream, so the camera is load-bearing for determinism.
  Honesty note: the comparator now reports compared-vs-unavailable
  columns explicitly; the follower group has never been verified
  against hardware and the output says so.

## RNG design (ratified by Peter 2026-08-15)

The oracle settled the facts first: there is ONE 32-bit seed
($FFFFEF0C) for the whole game, free-running as a per-frame counter
(1 tick/vblank in every mode; +1/frame in field mode from
GameMode_Field's unconditional opening call, which vanishes while a
window is up because window loops never return to the mode dispatcher;
wander and encounter draws are occasional extra consumers; never
advanced by steps — see docs/NPC_WANDER.md "per-frame tick structure"),
with two algorithms over it — the portable x41 LCG (UpdateRNGSeed) and
the battle mixer (UpdateRNGSeed2: ror the high word, return
HV_counter + frame_count - seed_word). Only the HV-counter term is
unportable.

Decision: the runtime keeps BOTH cartridge algorithms over the one
shared seed, ticks it on the cartridge's per-frame schedule, and
replaces the HV-counter read with a deterministic surrogate. Everything
else about battle rolls — the ror, the frame-count term, the masks, the
16-draw structure — is the cartridge's own code. Formula-exact,
structure-exact, distribution-faithful; stream-different from hardware
only through the missing beam position. Bit-exact battle replay against
the cartridge is explicitly out of scope; the oracle verifies battle
FORMULAS via forced seeds and end-of-turn RAM, not per-frame streams.
Field/encounter RNG stays fully bit-exact (it never reads the beam).

Surrogate VALIDATED (2026-08-15, oracle damage census x engine
simulation): 158 hardware damage samples across six matchups vs 20k
simulated rolls per cell - means agree within a few percent everywhere,
the surrogate's spread is statistically indistinguishable from ideal
iid draws (the seed's per-draw rotation supplies the variance; the
fixed residue does not collapse it), and the defence-exceeds-attack
cell reproduces the census's constant-1 floor as the expected ~99.5%
tail mass. The remaining sd gaps are exactly the census's included
criticals. oracle/logs/damage_census.csv is the ground truth.

## Battle bug policy (Peter, 2026-08-15)

"Fix obvious bugs still... this ain't 1994 and we can fix stuff that was
obviously meant to work another way." Battle-era ruling: bugs that are
plainly unintended get FIXED in the port (level-99 results-loop pointer
desync, the shield-element read, stat lag after level-up, Defend/armour
physical_prop clobber, Telepipe/Escapipe consumed in battle, Zio3's
out-of-table effect id rejected at load). Two of these are now MEASURED
on hardware (tape 10/14, oracle/README.md): the stat lag — Chaz levels
to 2, strength 8→9, and atk_pow stays 18 across 600 frames — and the
Defend clobber — defend rewrites physical_prop's high byte 2→1 for the
round while the fork-only physical_prop_save stays unwritten. Retail behavior stays documented
in SOURCE_NOTES per bug; quirks that read as design (cure spells also
restoring agility/dexterity, enemy paralysis clearing each turn) stay
faithful. Each fix gets its own ledger line at implementation time.

## Division of labor

Design and adjudication in the main loop; implementation lanes own disjoint
files and report for review. Same rules as the extraction waves: no lane
touches shared files, no git commands from lanes, the lead integrates,
verifies, and commits.

## Battle presentation (Peter, 2026-08-15)

No interim text-window battles. Encounters roll engine-side (logged as
EncounterRolled, no fight presented) until the REAL battle screen ships:
enemy bodies on the background layer, character battle sprites, battle
backgrounds, command menu, message window, damage numbers — the retail
screen, modernized like the field. Prerequisites, in flight: battle art
pack emission (sprite-lane), battle backgrounds into the pack,
characters/equipment pack files (overworld-lane), then the psiv-godot
battle scene consuming the engine's BattleEvent timeline.
