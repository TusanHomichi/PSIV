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

## Open fidelity questions (for the emulator oracle)

- Dialogue text speed: does retail draw characters progressively, at what
  rate? (Currently instant; `$F9` is an explicit pause, not a speed.)
- The scroll-arrow art: a hardware sprite, not yet extracted. Placeholder
  triangle at the pack's (264, 202) position.
- Talk range: Peter's memory says you could stand *beside* an NPC and they'd
  turn to face you; the transcribed `Interaction_ChkObjects` projects one
  cell ahead of the party's facing (±8px). Turn-to-face itself is implemented
  (the `$F3` keep-npc-facing code proves it was the default); the *range*
  question needs the oracle.
- Does an accept press during the window-open animation buffer or drop?
- Does the final dialogue page close on its own or require a press?
  (Currently requires a press.)

## Division of labor

Design and adjudication in the main loop; implementation lanes own disjoint
files and report for review. Same rules as the extraction waves: no lane
touches shared files, no git commands from lanes, the lead integrates,
verifies, and commits.
