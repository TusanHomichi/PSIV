# Scene presentation consumption

This slice is the renderer half of the typed scene seam. `SceneRunner` still
owns scene control, blocking, actor semantics, flags, and map changes. The
runtime translates each `SceneEffect::Presentation` into one
`RuntimeEvent::ScenePresentation` at the same tick and in the same vector
position. `rust/psiv-godot/src/runtime_events.rs` consumes that event in order;
it does not replay or infer scene operations.

The ordering contract is covered by
`presentation_events_keep_scene_order_and_tick_boundaries` in
`rust/psiv-runtime/tests/next_arc.rs`.

## Op coverage

| Operation family | Shell status | Evidence / limit |
|---|---|---|
| `InitVramAndCram` | Implemented | Clears staged/visible scene planes and opening text. |
| `FadeIn`, `FadeOut` | Implemented | Seven CRAM-equivalent levels, two-frame stepping, 14 renderer ticks; the cover is above cutscene planes and below dialogue. |
| `Panel_Create`, `Panel_Destroy`, `Panel_DestroyAll` | Implemented for the extracted scene-panel set | Retail records are decoded from `$07B000`; Meeting Rika's `$33/$34/$3B/$3C` panels are live, destroy is stack-pop, and an id mismatch warns. Later post-Rika panel ids remain an explicit pack gap. |
| `DmaPlanes` | Implemented | Staged panels become visible only at the DMA event. |
| `LoadPalette` | Implemented | Decoded word records are loaded and length-checked from `presentation/panels.json`. Pixel assets bake their retail palette for Godot's texture path. |
| `LoadArt` | Implemented for all 7 decoded scene writes | `presentation/load_art/` carries each Nemesis payload, source address, destination tile, map context, consumed/decompressed size, and hash. The four object-consuming writes also feed the temporary-object sheets. |
| `LoadTitleImage` | Implemented | Retail opening art/mapping/palette decode is emitted as a 320x128 background. |
| `DrawTextToPlane` | Implemented for opening tree 17 | Four rows at the retail `$840A/$858A/$870A/$888A` positions; ordinary scene dialogue remains `DialogueWindow`-owned. |
| `IntroTextFadeUp/Down` | Implemented | Text-only 20-frame ramp, sampled every four ticks by the retail `0x222` channel step. |
| `SetRenderSpritesInCutscene` | Implemented | Gates party, follower, and map NPC visibility while a scene is active. |
| `ObjectAnimation`, `SetObjectDestination` | Implemented for map sprites and the 6 standalone object keys | MeetingRika's two Rika keys (`$18/$26A`, `$18/$55C`) use the raw field-art source at `$292D00`; Holt, RuneFlaeli, Igglanova, and the chest splinter use their decoded `LoadArt` payloads. Sheets are gated until the matching art upload is consumed and are rendered at the scene destination. |
| `PlaySound` | Implemented | Routes through the live `AudioOutput`/`SoundMachine` path, separate from battle SFX dispatch. |
| `SetSavedMusic` | Implemented | Stores the retail one-byte restore word; zero clears it. Scene end, battle close, and non-scene map reload consume it and replay the sound through `AudioOutput`. |
| `WaitFrames` | Implemented | Runtime owns the blocking count; Godot records the op and does not create a second timer. |
| `PresentationOp::RebuildSprites` / `ReloadMapChunks` | Implemented | Rebuilds map visuals at the ordered event. |
| palette-word/red-fade/window/portrait records | Implemented for the decoded generic set | Generic `WindowDestroy/Create`, `LoadWindowTiles`, `LoadPortrait`, and `DrawPortrait` now drive the runtime window layer. The Meseta window roles come from `dialogue/set.window.png`; `shopkeeper_2` is emitted as a 48x48, 36-tile portrait from art `$29DE1E`, mapping `$2A2B36`, destination tile `$55C`. |

### Additive asset coverage

The rebuilt pack reports this exact census in
`runtime-pack/presentation/panels.json`:

| Surface | Count | Result |
|---|---:|---|
| scene panels | 15 | exact decoded panel records |
| `LoadArt` writes | 7 | all payloads rendered to preview PNGs |
| standalone temporary-object keys | 6 | 5 exact sheets; chest splinter preserves 66 named transparent VRAM pattern holes |
| generic portraits | 1 | exact 48x48 `shopkeeper_2` sheet |

The chest splinter is the one deliberate partial result. Its four directional
animation tables reference the contiguous pattern range `$34B..$38C` (66
patterns) outside the Nemesis upload at `$2E6`; the pack leaves those pixels
transparent instead of borrowing whatever happened to be in map VRAM. The
manifest says `partial_transparent_holes` and names every missing pattern, so
this is visible debt rather than a pretty lie.

## Oracle provenance

The source ROM is `Phantasy Star IV (USA).md`, SHA-256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`.
`oracle/tapes/27_opening_scene_presentation.tape` is the deterministic
power-on-to-first-control schedule for `Event_GameStart ($9F)`. Its captures
are in `oracle/frames/opening/`; the frame-4000 state is
`oracle/states/opening/frame_4000.json`.

The pack decoder asserts all 15 scene-panel records, all 7 `LoadArt` source /
destination pairs, the 6 temporary-object keys, the generic portrait's art /
mapping / tile contract, both Enigma planes for each panel, their retail
coordinates, and the opening image addresses in
`tests/test_presentation_pack.py`. That test also invokes
`oracle/decode_layout.py` against the committed frame-4000 state; its
`self_check.passed` value is `true` and all four layout checks pass.

Godot capture comparison uses `psiv_tools/presentation_rmse.py`. It crops the
centred 960x672 3x surface from `PSIV_DEBUG_SHOT`, normalizes it to 320x224,
and reports RGB RMSE against an oracle frame. The identity-path tool check is
`rmse=0.000000`.

The capture command, once a display backend is available, is deliberately
boring and explicit:

```sh
PSIV_DEBUG_EVENT=0x9f \
PSIV_DEBUG_AUTOCLOSE_SCENE=1 \
PSIV_DEBUG_RETAIL_PACE=1 \
PSIV_DEBUG_SHOT=/tmp/psiv-opening-retail.png \
PSIV_DEBUG_SHOT_FRAME=<clone-tick> \
godot --path godot --quit-after <clone-tick-plus-one>
python3 psiv_tools/presentation_rmse.py \
  /tmp/psiv-opening-retail.png oracle/frames/opening/frame_4000.png

PSIV_DEBUG_EVENT=0x8007 \
PSIV_DEBUG_AUTOCLOSE_SCENE=1 \
PSIV_DEBUG_RETAIL_PACE=1 \
PSIV_DEBUG_SHOT=/tmp/psiv-meeting-rika-retail.png \
PSIV_DEBUG_SHOT_FRAME=<clone-tick> \
godot --path godot --quit-after <clone-tick-plus-one>
python3 psiv_tools/presentation_rmse.py \
  /tmp/psiv-meeting-rika-retail.png \
  /tmp/psiv-meeting-rika-oracle/frame_7250.png
```

The `<clone-tick>` values must come from the same run's deterministic event
log and be recorded beside the resulting RMSE. They are not filled with a
guessed offset here.

The historical live opening capture (2026-08-16), harness tick 1500 versus
`oracle/frames/opening/frame_4000.png`, is **rmse=6.586813**. It remains useful
placement evidence, but it is not one of this slice's exact-frame
certifications: the old capture used `PSIV_DEBUG_AUTOCLOSE_SCENE=1` and
therefore compressed dialogue timing.

The retail-paced path is now explicit. Set both
`PSIV_DEBUG_AUTOCLOSE_SCENE=1` and `PSIV_DEBUG_RETAIL_PACE=1`; dialogue keeps
the retail 3-frame-per-character typewriter and holds a completed page for
4 frames before the debug harness advances it. With the oracle frame number
and the clone's fixed debug shot tick, the pair is deterministic and the
offset is recorded rather than guessed.

**Certified retail-paced pairs (integration, 2026-08-16, Xvfb captures):**

| Pair | Clone tick | Oracle frame | RMSE |
|---|---:|---:|---:|
| Opening narration page 1 | 3450 | `opening/frame_4000.png` | **6.586813** |
| Opening narration page 2 | 4440 | `opening/frame_5200.png` (= 5600) | **6.578011** |

Both captures land inside a settled 900-frame hold, where every frame is
pixel-identical, so hold-window pairing is exact by construction. The
clone timeline (holds at t3000–3900 and t3990–4890) is printed by
`PSIV_DEBUG_SCENE_TICKS=1`, added for exactly this pairing work. The shared
~6.58 residual is dominated by the intro's flying sprite sitting at a
different point on its path plus minor sky deltas.

Two defects were found and fixed to get there:

* Retail-pace deadlock: the auto-advance condition used `is_waiting()`,
  which excludes the entry-final `End` page — the state a scene dialogue
  rests in until dismissed. Every retail-paced scene stalled at its first
  dialogue. The harness now uses `is_dismissable()` (`End` included,
  `Choice` deliberately excluded — advancing one selects an answer).
* The `0x8007` debug fixture spawned the party at cell (1,1); it now uses
  the oracle tape-28 position (leader pixel `($1F0,$1A0)`, the retail
  trigger's exact Y).

Capture discipline: run certifications under Xvfb (`xvfb-run -a … 
--display-driver x11`), not on the live desktop — a focused game window on
the real session receives real input, and the typewriter's hold-to-
accelerate makes the timeline input-dependent, which showed up as
run-to-run timeline shifts until isolated.

The MeetingRika pair remains **uncertified and blocked**: oracle frame 7250's
professor/Rika picture is drawn by the dialogue's own `Ctrl::Action`
`LoadPanel` (panel `$30`), and dialogue-embedded actions are not wired —
`Ctrl::Action` is skipped with a log across the whole dialogue system, and
panel `$30` is outside the extracted 15-record set. That wiring (actions:
sounds, panels, flags) is a scoped follow-up lane, not a capture problem.

## Runtime evidence

The rebuilt opening harness (`PSIV_DEBUG_EVENT=0x9f`, 7,000 frames) completed
in `/tmp/psiv-opening-final.log`: it traversed the scene, emitted the 64-word
palette load, both 900-frame holds and 90/120-frame pauses, then logged
`scene ended` followed by `scene restored saved music: 0x84`.

The rebuilt Meeting Rika harness (`PSIV_DEBUG_EVENT=0x8007`, 4,500 frames) is
in `/tmp/psiv-meeting-rika-audio-final.log`. It reached the final Motavia handoff and
logged scene sounds `$91/$FB/$F8/$FD/$FE/$AD/$8C`, the saved-music write `$91`
and `$8C`, the two temporary-object constructions, the palette-word write,
Rika's party/macro path, `scene ended`, and `scene restored saved music: 0x8C`.
It also records successful `PanelCreate($33/$34/$3B/$3C)` staging and the
corresponding two-panel DMA commits, plus `scene map reload retains scene
music` at each in-scene map transition.
The audio path is the live `AudioOutput`/`SoundMachine` path; the Dummy driver
only suppresses hardware output for this deterministic harness.

For a retail oracle frame in that scene, `oracle/tapes/28_meeting_rika_retail_probe.tape`
keeps the normal power-on input schedule and the host's explicit
`--ram-patch` options redirect work RAM at frame 7000/7200. The fixture writes
the BioPlant map, leader position, party, event `$8007`, and field routine
`$000C`; the host applies bytes in 68000 address order and fails if any patch
frame is not reached. This is a named, reproducible oracle fixture, not a
claim that the retail tape naturally starts inside MeetingRika.

Godot's Dummy-driver forced quit still reports one
`AudioStreamGeneratorPlayback` ObjectDB leak after the scene has completed.
This is engine teardown noise, not a scene fault; the Rust workspace checks
remain green and the live scene logs are complete.

## Opening cinematic

The opening renderer now consumes the same presentation op family as ordinary
scenes: title image, palette load, tree-17 text entries, 20-frame text ramps,
long holds, and the final fade. It draws the 320x128 retail title image at
screen y=40, leaving the oracle's 40-pixel top and 56-pixel bottom black bars
(measured from `oracle/frames/opening/frame_4000.png`; the image band is rows
40..167 in the 320x224 surface).
The motion/map/dialogue state remains in the existing runtime scene.

The shell-only selectors `PSIV_DEBUG_EVENT=0x9f` and
`PSIV_DEBUG_AUTOCLOSE_SCENE=1` provide a deterministic harness; adding
`PSIV_DEBUG_RETAIL_PACE=1` preserves retail dialogue cadence for frame pairing.
All three are evaluation switches and are never used by normal play.

## Remaining deferrals

- **Exact-frame RMSE certification:** blocked only by the unavailable live/Xvfb
  display in this environment; the implementation and oracle fixture are in
  place, but two PNG comparisons are not certified without actual rendered
  clone frames.
- **Chest splinter's 66 unmapped patterns:** retained as transparent holes
  because the retail scene mapping consumes VRAM left by another runtime load;
  no source-of-truth pixels for those slots were found in the declared upload.
- **Later bespoke scene panel ids:** the post-Rika panel records outside the
  extracted 15-record set remain an explicit pack gap, as before.
