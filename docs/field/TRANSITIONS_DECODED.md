# Transitions decoded from retail

This is the frame evidence and renderer contract for state changes in the
Godot field. The oracle PNGs are Genesis Plus GX output at 320x224; they are
the authority for timing and shape. The renderer uses quantized `ColorRect`
covers because the Godot presentation layer has no Genesis CRAM buffer to
rewrite. The cover levels are discrete, never a continuous alpha tween.

## Evidence captures

All captures below came from `oracle/tapes/07_first_battle.tape`, except the
game-start sequence from `oracle/tapes/01_newgame.tape`. Each pair is a
durable 320x224 PNG under `oracle/frames/transitions/`.

| transition | before | after / transition state |
| --- | --- | --- |
| doorway, map `$14 -> $13`, frame 13079 | [frame 13079](../../oracle/frames/transitions/doorway_before.png) | [frame 13159](../../oracle/frames/transitions/doorway_after.png) |
| battle entry, encounter fire, frame 24790 | [frame 24790](../../oracle/frames/transitions/battle_before.png) | [frame 24850](../../oracle/frames/transitions/battle_after.png) |
| cutscene start `$8001`, frame 8438 | [frame 8438](../../oracle/frames/transitions/scene_start_before.png) | [frame 8451](../../oracle/frames/transitions/scene_start_after.png) |
| cutscene end, frame 10670 | [frame 10670](../../oracle/frames/transitions/scene_end_before.png) | [frame 10686](../../oracle/frames/transitions/scene_end_after.png) |
| title hand-off, frame 879 | [frame 879](../../oracle/frames/transitions/game_start_before.png) | [frame 898](../../oracle/frames/transitions/game_start_after.png) |

The CSV state changes anchor the important boundaries:

- encounter setup starts at `game_mode_routine=0x14`, frame **24777**;
- the first battle-mode change is frame **24794**;
- the doorway map-index changes are frames **8001**, **13079**, **17545**,
  **18241**, and **23761**;
- the `$8001` scene is active from the principal sequence around frame
  **8437** through the map-load boundary at frame **10693**.

## Retail palette vocabulary

The fade ramps are CRAM steps, not linear compositing. The common field ramp
is visible as approximately:

```text
(238,206,139)  full palette
(205,170, 98)
(172,137, 65)
(139,101, 32)
( 98, 68,  0)
( 65, 32,  0)
( 32,  0,  0)
(  0,  0,  0)  black
```

Each changed level is held for two frames. `transitions.rs` represents the
seven increments as integer levels `0..=7`; the field renderer maps that to a
cover alpha only at the final presentation boundary.

## Doorway fade-through-black

`MapChanged` carries `WarpTrigger`. The renderer starts this transition only
for `WarpTrigger::MapChange`; ordinary-ground map changes still load directly.
That distinction is already authoritative in the runtime event and avoids
making every map edge look like a doorway.

At map-index change **N** (the 13079 crossing is the clearest sample):

| frames | retail observation |
| --- | --- |
| N | full old-map palette |
| N+1..N+12 | six CRAM-darkening levels, two frames each |
| N+13 | black |
| N+14..N+67 | black hold |
| N+68..N+79 | six reverse levels, two frames each |
| N+80 | full new-map palette |

That is **13 frames out, 54 black-hold frames, and 13 frames in**: 80 frame
ages from the map-change boundary. The renderer state is `Doorway`, with the
new map loaded underneath the cover before the fade begins.

## Battle-entry effect

The encounter is a **white flash and palette reveal**, not a swirl. Consecutive
frames around the encounter fire show:

- field art remains present through roughly **24779**;
- brightness ramps toward uniform white from **24780** through **24793**;
- the screen is a uniform `(238,238,238)` white hold at **24794..24821**,
  **28 frames**;
- the battle background and enemies appear while the white ramp reverses at
  **24822..24834**, in two-frame palette steps;
- battle art is settled at **24835**; the battle UI builds afterward (window at
  24845, enemy name at 24850, command window at 24855).

The renderer state is `BattleEntry`: **14 frames to white, 28 white-hold
frames, and a 14-frame white-to-battle reveal**. The final reveal is modeled
as a stepped white cover rather than pretending that Godot can reproduce the
cartridge's simultaneous tile upload and CRAM writes. The resulting shape is
the measured flash/hold/reveal, with no invented swirl.

## Scene start and end

The high-bit principal cutscene `$8001` is the clean full-screen scene sample.
At its start, the field palette changes in two-frame steps from **8439** to
black at **8451**; the dialogue window first appears over black at **8474**.
The renderer uses a **13-frame black ramp followed by a 23-frame black hold**
for `SceneStart` (37 frame ages from the scene hand-off). The cover sits below
the dialogue window so scene text can remain visible once it opens.

At scene end, the retail composition fades out through **10685**, reaches
black at **10686**, and holds through **10734**. The map load boundary is
frame **10693**. The reverse ramp starts at **10735**, advances every two
frames, and is full at **10747**; field mode resumes at **10748**. The renderer
therefore starts `SceneEnd` at the runtime `SceneEnded` boundary with **42
black-hold frames and a 13-frame reverse ramp**. This is the correct seam for
the Godot runtime: the pre-10693 fade is the retail scene's `InitVramAndCram`
work, which the runtime deliberately exposes only as the scene boundary.

The ordinary Alys interaction is different: it has the retail dialogue-window
open/close treatment (small window around 7175, full window by 7190, closing
around 7470) and no full-screen palette fade. That path remains owned by the
dialogue renderer; it is not mislabeled as a scene-wide fade here.

## Game start screen-wipe

The title hand-off sample shows the existing title window shrinking for
**eight frames, 880..887**, followed by a stepped title-image fade for
**ten frames, 888..897**; the screen is black at **898**. The Godot field does
not own the title image or its Plane A window, so the renderer preserves the
measured eight-frame centered-window geometry as a black-to-field startup
reveal. Its transition state remains alive for **18 frames**, matching the
full retail hand-off interval; the visual cover is gone after the eight-frame
window phase because there is no title plane to fade underneath it.

This is deliberately documented as a presentation-surface boundary, not
claimed as a literal title-screen recreation.

## Implementation

- `rust/psiv-godot/src/transitions.rs` contains the integer-frame state machine
  and tests for every measured ramp.
- `rust/psiv-godot/src/field_visuals.rs` owns the four cover rectangles and
  keeps them camera-relative; four pieces are needed for the centered startup
  wipe.
- `rust/psiv-godot/src/lib.rs` only routes `MapChanged`, encounter start,
  high-bit scene boundaries, and boot initialization into that module. No
  battle, dialogue, camp, or shop code is changed.
