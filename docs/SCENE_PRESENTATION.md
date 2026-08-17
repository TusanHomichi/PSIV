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
| `LoadArt` | Deferred | The shell records the retail ROM source and destination tile; standalone Nemesis art promotion is not complete, so it does not invent pixels. |
| `LoadTitleImage` | Implemented | Retail opening art/mapping/palette decode is emitted as a 320x128 background. |
| `DrawTextToPlane` | Implemented for opening tree 17 | Four rows at the retail `$840A/$858A/$870A/$888A` positions; ordinary scene dialogue remains `DialogueWindow`-owned. |
| `IntroTextFadeUp/Down` | Implemented | Text-only 20-frame ramp, sampled every four ticks by the retail `0x222` channel step. |
| `SetRenderSpritesInCutscene` | Implemented | Gates party, follower, and map NPC visibility while a scene is active. |
| `ObjectAnimation`, `SetObjectDestination` | Implemented for existing map sprites | Literal slot/id/art/frame records are retained and matching map nodes are animated/placed. A standalone Nemesis decode for temporary `$C340/$C4C0` objects is deferred; unmatched objects are logged rather than fabricated. |
| `PlaySound` | Implemented | Routes through the live `AudioOutput`/`SoundMachine` path, separate from battle SFX dispatch. |
| `SetSavedMusic` | Implemented | Stores the retail one-byte restore word; zero clears it. Scene end, battle close, and non-scene map reload consume it and replay the sound through `AudioOutput`. |
| `WaitFrames` | Implemented | Runtime owns the blocking count; Godot records the op and does not create a second timer. |
| `PresentationOp::RebuildSprites` / `ReloadMapChunks` | Implemented | Rebuilds map visuals at the ordered event. |
| palette-word/red-fade/window/portrait records | Preserved and logged | Typed records are no longer dropped. Exact per-CRAM recolouring and generic scene window/portrait surfaces remain deferred because their retail VRAM assets are not yet promoted into this pack. |

## Oracle provenance

The source ROM is `Phantasy Star IV (USA).md`, SHA-256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`.
`oracle/tapes/27_opening_scene_presentation.tape` is the deterministic
power-on-to-first-control schedule for `Event_GameStart ($9F)`. Its captures
are in `oracle/frames/opening/`; the frame-4000 state is
`oracle/states/opening/frame_4000.json`.

The pack decoder asserts all 15 scene-panel records, both Enigma planes for
each record, their retail coordinates, and the opening image addresses in
`tests/test_presentation_pack.py`. That test also invokes
`oracle/decode_layout.py` against the committed frame-4000 state; its
`self_check.passed` value is `true` and all four layout checks pass.

Godot capture comparison uses `psiv_tools/presentation_rmse.py`. It crops the
centred 960x672 3x surface from `PSIV_DEBUG_SHOT`, normalizes it to 320x224,
and reports RGB RMSE against an oracle frame. The identity-path tool check is
`rmse=0.000000`.

A real capture ran on the live Wayland session at integration (2026-08-16):
opening narration, harness tick 1500 vs `oracle/frames/opening/frame_4000.png`
— **rmse=6.59**. Getting there surfaced and fixed three placement defects the
log-only harness could not see: the title image drew at y=64 instead of the
oracle-measured y=40; the narration text used a 16-pixel line pitch from
screen row 9 instead of the plane-derived 24-pixel pitch from row 8 ($840A +
n*0x180); and lines were dynamically centred instead of left-aligned at the
plane's x=40 origin. Text rows and columns now match the oracle exactly
(rows 67-76/91-100/115-124/140-148; line starts x=40). The residual error is
the intro's flying sprite sitting at a different point on its path plus text
fade state — the debug harness auto-closes dialogue, so its tick timeline is
compressed relative to the retail tape; exact-frame pairing needs a
retail-paced harness mode if a tighter number is ever required.

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
`PSIV_DEBUG_AUTOCLOSE_SCENE=1` provide a deterministic headless harness; the
latter is an evaluation switch and is never used by normal play.
