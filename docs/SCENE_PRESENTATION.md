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

## Pixel-exactness pass (2026-08-17)

The CRAM widening is now receipt-backed and centralized in
[`COLOR_PIPELINE.md`](COLOR_PIPELINE.md). The pack was regenerated from the
Grand Cross build (`grand_cross=0`); no linear or bit-replication widening
remains in the Python pack emitters.

The MeetingRika geometry scout resolved the portrait delta to a retail window
group choice. `WinGroup_Dialogue` record 2 is `(5,13)` = `(40,104)` pixels;
the scene/event branch in `TextCtrlCode_Portrait` selects `WinGroup_Event`
record 4, `(3,14)` = `(24,112)`. The clone now marks scene-owned dialogue and
applies `(-16,+8)` to the packed talk portrait, plus the measured `(1,1)`
plane residue. That produces the observed retail-visible delta from `(43,108)`
to `(27,115)` as `(-16,+7)` after the residue/content edge is included.

The text scout did not support an arbitrary seven-pixel shift. Retail's
`TextBufferToPlane` writes screen tile `(4,$15)` (`ps4.asm:143409`), a 32x4
map of 8x16 glyphs. `dialogue.rs` now names that contract explicitly as an
8-pixel text origin and 16-pixel line pitch. In the normalized frame-7250
receipts, both lines occupy rows 171..180 and 186..198 in the old clone and
retail captures; the source-backed text geometry is already aligned. The
reported row-151/158 discrepancy is therefore not implemented as an
unsupported shift that would break the aligned second line.

Fixture phase is pinned: title clone tick **480** pairs with
`oracle/frames/title/frame_450.png` (settled title, no Press Start), and camp
clone tick **60** pairs with `oracle/frames/frame_7675.png`, mark
`camp_root_idle`. The pre-fix title diagnostic at clone tick 750 was the
wrong blink phase (`RMSE 19.408013`); it is not used for certification.

### Re-certification ledger

The numbers below distinguish the already-certified baseline pairs from the
post-change captures still required for the battle/camp fixture work. The
required Xvfb command was attempted with `--display-driver x11
--rendering-method gl_compatibility --rendering-driver opengl3`; this sandbox
cannot create the X11 display, so no missing post-change RMSE is fabricated.

| pair | before | after | required pairing | status |
|---|---:|---:|---|---|
| opening page 1 | 6.587 | **0.000000** | clone t3550 ↔ opening frame 4000 | certified baseline |
| opening page 2 | 6.578 | **0.000000** | clone t4550 ↔ opening frame 5200 | certified baseline |
| MeetingRika | 36.3 | **0.000000** | clone t160 ↔ frame 7250 | certified baseline |
| battle `0x88` | **12.185497** (lastpairs) | not measured after final enemy receipt fix | clone t200 ↔ frame 25000 | Xvfb listener blocked |
| title | 19.408013 (wrong-phase t750) | **0.000000** baseline; post-blink recapture blocked | clone t480 ↔ title frame 450 | certified pre-prompt baseline |
| camp root | **14.456810** (lastpairs) | not measured after final anchor/plane fix | clone t60 ↔ frame 7675 (`camp_root_idle`) | Xvfb listener blocked |

The exact commands below are the integration handoff. They must be run one
at a time under Xvfb, with `PSIV_DEBUG_SCENE_TICKS=1` on scene captures, and
the settled tick from that same log must be recorded beside each RMSE.

### Last-two-pair receipt recheck (2026-08-17)

Both oracle references were replayed from their source tapes before changing
the clone. The regenerated PNG hashes are unchanged: Tape 07 frame 25000 is
`761fb241a2360d222fdf1538be1af89b7bfb9cd09376a6157733fd8f71e773c4`, and Tape
22 frame 7675 is
`9bf283d9f48b4c0d959eb297ca0b1a997b62f227385ec25cae921e078ceaeca0`.

#### Battle command idle

`oracle/tapes/07_first_battle.tape` produces `frame_25000.png`, the settled
command-idle battle. The RAM receipt at `$41F0` is
`0F 05 00 00 02 03 00 0A 0E 0A 1A FF ...`: the two Zoran Bult position bytes
are `$0E` and `$1A`, which decode to body anchors **(88,72)** and **(184,72)**
with six-cell body runs at Plane A columns 11..16 and 23..28. Those enemy
constants were already receipt-correct in the clone.

The party layout receipt is center-out rather than status-strip order:

| fighter slot | character | screen origin | Plane A destination | idle pose grid |
|---:|---|---:|---:|---:|
| 1 | Alys | `(136,120)` | row 15, col 17 | `$FFFF3000` |
| 2 | Chaz | `(88,120)` | row 15, col 11 | `$FFFF3048` |
| 3 | Hahn | `(184,120)` | row 15, col 23 | `$FFFF3090` |

The fresh VDP SAT region at the retail SAT base is all zero and the checked-in
`battle_command_idle` layout has no visible SAT entries. Position/pose proof
for this idle frame is therefore the Plane A tile runs, `$41F0`, and the
overlay metadata; no SAT attributes are invented here. The clone now assigns
the character records to those receipt-backed fighter slots, preserving the
retail idle pose selected by each slot.

#### Camp root idle

`oracle/tapes/22_camp_menu_layout.tape`, mark `camp_root_idle`, produces
`frame_7675.png`. The receipt is map `$13`, world/map-index-2 `0/$FFFF`, Chaz
at `($2F0,$140)`, and settled camera `($258,$E8)`. The live object slots at
that frame are:

| slot | object id | x | y | facing | map index |
|---:|---:|---:|---:|---|---:|
| 0 | `$803C` | 736 | 226 | down | 1 |
| 1 | `$803C` | 743 | 128 | left | 3 |
| 2 | `$803C` | 624 | 128 | left | 0 |
| 3 | `$803C` | 352 | 112 | down | 0 |
| 4 | `$803C` | 320 | 256 | right | 0 |
| 5 | `$8040` | 271 | 160 | left | 0 |
| 6 | `$803C` | 256 | 112 | down | 0 |
| 7 | `$8068` | 592 | 240 | down | 0 |

The debug camp fixture keeps the complete receipt-backed map/player/party
setup, places all eight runtime objects at these pixel positions and facings,
and suspends field wandering during the debug lead-in. This removes the
non-deterministic post-spawn drift without changing normal gameplay.

The fresh VDP/SAT dump is durable at
`oracle/states/camp_root_idle_vdp_7675.json`. Its visible SAT entries decode
as follows (8-byte entries, link-walk from index zero):

| entry | screen x,y | size | tile | attr | receipt role |
|---:|---:|---:|---:|---:|---|
| 1 | `(152,86)` | 16×32 | `$534` | `$08` | Chaz field party |
| 2 | `(-8,6)` | 16×32 | `$3E7` | `$08` | visible Alys |
| 3 | `(136,-8)` | 16×32 | `$287` | `$0C` | visible type-2 NPC |

The clone's map record is only a spawn fallback: the draw node is initialized
from the runtime cell plus sub-cell offset, which preserves the receipt's
`(136,-8)` object after the debug fixture moves it. The suspended type-2
object retains walk-down frame 1 in this fixture. Chaz's field-party node uses
the live camp anchor, removing the one-cell standing shift and matching SAT
`(152,86)`.

The status-window Plane A decode is also explicit. At screen row 2, columns
26..37 are
`C6F3 C683 C6C0 C6B9 C6D2 C680 C680 C68C C696 C680 C7DB CEF3`.
Therefore the clone draws `LV` at cell `(33,2)`, the decimal tile run at
`(36,2)`, leaves cell `(35,2)` as the window blank, and never writes the
border at `(37,2)`.

## Op coverage

| Operation family | Shell status | Evidence / limit |
|---|---|---|
| `InitVramAndCram` | Implemented | Clears staged/visible scene planes and opening text. |
| `FadeIn`, `FadeOut` | Implemented | Seven CRAM-equivalent levels, two-frame stepping, 14 renderer ticks; the cover is above cutscene planes and below dialogue. |
| `Panel_Create`, `Panel_Destroy`, `Panel_DestroyAll` | Implemented for scene ops and dialogue `$F2` actions | Retail panel records are decoded from all non-empty banked `PanelPtrs` ranges. Scene ids remain compatible with the typed scene stream; dialogue word ids include `$30` and the other 162 action-referenced records. Destroy is stack-pop, and an id mismatch warns. |
| `DmaPlanes` | Implemented | Staged panels become visible only at the DMA event. |
| `LoadPalette` | Implemented | Decoded word records are loaded and length-checked from `presentation/panels.json`. Pixel assets bake their retail palette for Godot's texture path. |
| `LoadArt` | Implemented for all 7 decoded scene writes | `presentation/load_art/` carries each Nemesis payload, source address, destination tile, map context, consumed/decompressed size, and hash. The four object-consuming writes also feed the temporary-object sheets. |
| `LoadTitleImage` | Implemented | Retail opening art/mapping/palette decode is emitted as a 320x128 background. |
| `DrawTextToPlane` | Implemented for opening tree 17 | Four rows at the retail `$840A/$858A/$870A/$888A` positions; ordinary scene dialogue remains `DialogueWindow`-owned. |
| `IntroTextFadeUp/Down` | Implemented | Text-only 20-frame ramp, sampled every four ticks by the retail `0x222` channel step. |
| `SetRenderSpritesInCutscene` | Implemented | Gates party, follower, and map NPC visibility while a scene is active. |
| `ObjectAnimation`, `SetObjectDestination` | Implemented for map sprites and the 6 standalone object keys | MeetingRika's two Rika keys (`$18/$26A`, `$18/$55C`) use the raw field-art source at `$292D00`; Holt, RuneFlaeli, Igglanova, and the chest splinter use their decoded `LoadArt` payloads. Sheets are gated until the matching art upload is consumed and are rendered at the scene destination. |
| `PlaySound` | Implemented | Routes through the live `AudioOutput`/`SoundMachine` path, separate from battle SFX dispatch. |
| dialogue `Ctrl::Action` | Implemented | `TextFlow` stops at the retail byte position; `DialogueWindow` releases the action after the preceding glyphs, and `Field` routes panels, sounds, palette effects, and flags through the existing seams. See [`DIALOGUE_ACTIONS.md`](DIALOGUE_ACTIONS.md). |
| `SetSavedMusic` | Implemented | Stores the retail one-byte restore word; zero clears it. Scene end, battle close, and non-scene map reload consume it and replay the sound through `AudioOutput`. |
| `WaitFrames` | Implemented | Runtime owns the blocking count; Godot records the op and does not create a second timer. |
| `PresentationOp::RebuildSprites` / `ReloadMapChunks` | Implemented | Rebuilds map visuals at the ordered event. |
| palette-word/red-fade/window/portrait records | Implemented for the decoded generic set | Generic `WindowDestroy/Create`, `LoadWindowTiles`, `LoadPortrait`, and `DrawPortrait` now drive the runtime window layer. The Meseta window roles come from `dialogue/set.window.png`; `shopkeeper_2` is emitted as a 48x48, 36-tile portrait from art `$29DE1E`, mapping `$2A2B36`, destination tile `$55C`. |

### Additive asset coverage

The rebuilt pack reports this exact census in
`runtime-pack/presentation/panels.json`:

| Surface | Count | Result |
|---|---:|---|
| scene-owned panel records | 15 | exact decoded scene records |
| dialogue action panel records | 163 | every distinct `$F2 LoadPanel` id, including `$30` |
| ending-only panel records | 50 | `$8021` post-battle story sequence |
| total panel records | 228 | all PNGs present in the runtime manifest |
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

The pack decoder asserts all 228 panel records (15 scene-owned, 163
dialogue-action records and 50 ending-only records), all 7 `LoadArt` source /
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
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 \
  PSIV_DEBUG_EVENT=0x9f \
  PSIV_DEBUG_AUTOCLOSE_SCENE=1 \
  PSIV_DEBUG_RETAIL_PACE=1 \
  PSIV_DEBUG_SCENE_TICKS=1 \
  PSIV_DEBUG_SHOT=/tmp/psiv-opening-retail.png \
  PSIV_DEBUG_SHOT_FRAME=<clone-tick> \
  /home/peter/.local/bin/psiv-godot-4.7.1 \
  --display-driver x11 --rendering-method gl_compatibility --rendering-driver opengl3 --audio-driver Dummy \
  --log-file /tmp/psiv-opening-retail-godot.log \
  --path godot --quit-after <clone-tick-plus-one>
python3 psiv_tools/presentation_rmse.py \
  /tmp/psiv-opening-retail.png oracle/frames/opening/frame_4000.png

xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 \
  PSIV_DEBUG_EVENT=0x8007 \
  PSIV_DEBUG_AUTOCLOSE_SCENE=1 \
  PSIV_DEBUG_RETAIL_PACE=1 \
  PSIV_DEBUG_SCENE_TICKS=1 \
  PSIV_DEBUG_SHOT=/tmp/psiv-meeting-rika-retail.png \
  PSIV_DEBUG_SHOT_FRAME=<clone-tick> \
  /home/peter/.local/bin/psiv-godot-4.7.1 \
  --display-driver x11 --rendering-method gl_compatibility --rendering-driver opengl3 --audio-driver Dummy \
  --log-file /tmp/psiv-meeting-rika-retail-godot.log \
  --path godot --quit-after <clone-tick-plus-one>
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

### Retail scroll receipt

The new `--dump-state` receipt was decoded from the MeetingRika fixture at
retail frame 7250 (`/tmp/psiv-scroll-check.json`). The host resolves the
Genesis Plus GX local VDP symbols from the loaded ELF; it does not change the
third-party core ABI. `oracle/decode_layout.py` reports these numbers with
`grand_cross=0`:

| input | decoded value | provenance |
|---|---:|---|
| foreground camera `(X,Y)` | `(0, 0)` pixels; raw `0x00000000` / `0x00000000` | `ps4.constants.asm`, `Camera_X_Pos_FG` / `Camera_Y_Pos_FG`, 16.16 work-RAM words at `$FFFFEF94` / `$FFFFEF90` |
| background camera `(X,Y)` | `(0, 0)` pixels; raw `0x00000000` / `0x00000000` | `Camera_X_Pos_BG` / `Camera_Y_Pos_BG` at `$FFFFEF9C` / `$FFFFEF98` |
| camera step counters | all four `0x00000000` | `Camera_X/Y_Step_Counter_FG/BG` at `$FFFFEC50..$FFFFEC5F` |
| H-int jump/address | `0x4EF9`, `0x00000758` | `HInt_Jump` / `HInt_Addr` at `$FFFFECB0/$FFFFECB2`; `0x758` is the retail `HInt` `RTE` |
| split state | disabled; mode `0`, cursor `0`, slope `0x00000000`, flags `0x0000` | `$FFFFECB8..$FFFFECC1`, with the inactive address selecting the RTE |
| VDP H-scroll mode/base | full-screen, base `$F400` | VDP register 11 mode bits `0`; register 13 value `$3D` |
| VDP H-scroll columns | Plane A/B `0x0000` for the first 224 line pairs | VDP H-scroll table at `$F400`, emitted as `vdp_hscroll_table` |
| VDP VSRAM | Plane A/B `0x0000` | emitted as `vdp_vsram`, Genesis big-endian words |
| work H-scroll buffer | `0x0000` for all 224 words | `$FFFF60E0`, the generated per-line buffer used by the split path |
| VSRAM shadow source | inactive for this receipt; first words `0x6040, 0x61DC, 0x0010, 0x0010` | `$FFFF6000` `Chunk_Table`; the H-int path consumes it only when enabled |

The dialogue window occupies scanlines **160..223**; the H-int/VDP receipt
covers all 224 scanlines. H-int split is false, so both planes use the live VDP
VSRAM words for this frame. The remaining
screen-space term is the measured retail plane/window residue
`(x=1,y=1)` pixel at `grand_cross=0`; its provenance is recorded in the JSON
as `placement_provenance`, not disguised as camera motion. The clone applies
that term only to scene panel planes and portraits. Dialogue window chrome,
glyphs, arrows, and the opening background retain their independent retail
origins.

**Known certified pairs (integration, 2026-08-17, post-COLOR_PIPELINE ramp):**

| Pair | Clone tick | Oracle frame | RMSE |
|---|---:|---:|---:|
| Opening narration page 1 | 3550 | `opening/frame_4000.png` | **0.000000** |
| Opening narration page 2 | 4550 | `opening/frame_5200.png` | **0.000000** |
| MeetingRika settled Chaz page | 160 | tape-28 `frame_7250` (`d8fc26ae…`) | **0.000000** |
| Title settled, pre-prompt | 480 | `title/frame_450.png` | **0.000000** |

The battle and camp rows from the earlier table were state-mismatched
diagnostics, not certifications. Their post-fix captures remain blocked by
the Xvfb listener and are listed separately below.

The opening narration is **pixel-identical to the emulator** — the entire
pre-ramp ~6.58 residual was the linear-vs-GPGX palette widening, not the
flying sprite. Hold ticks shifted to 3550/4550 with the wave-6 dialogue
timeline (holds now t3093–3993 and t4083–4983; re-derive from
`PSIV_DEBUG_SCENE_TICKS=1` whenever scene content changes). Capture
doctrine addition: export `LIBGL_ALWAYS_SOFTWARE=1` for Xvfb captures —
without it, Mesa's amdgpu probe can fail in a background session and Godot
silently falls back to the live Wayland desktop, whose real input corrupts
the timeline.

MeetingRika reached zero through three receipts: the dialogue window needed
the same +1 X plane-origin residue as the panels (rows 176–208 dropped
62→13); the portrait had the residue double-applied on top of the box's own
+1 (correlation demanded exactly (-1,-1); the portrait is window chrome, so
the residue term was removed); and the pack's scroll-arrow PNG was
mis-composed — the two 8x8 tiles were byte-concatenated into the 16-wide
image instead of row-interleaved as a 2x1 VDP sprite. The arrow's true
pixels were receipt-verified against the frame-7250 SAT/VRAM dump (they are
`ArtNem_Font` tiles $36/$37 exactly as scouted; earlier shape confusion was
a threshold artifact — index-1 dark pixels vs white-only). Note the settled
window is narrow: full reveal ~t158, dismiss ~t163.

Title certifies at t480 in the settled pre-prompt pair — the window between
reveal-settle and the prompt/copyright draw, matching pre-prompt oracle frame
450. The Press Start cadence is now implemented from the retail palette-cycle
routine; a post-change capture is blocked by this sandbox's Xvfb listener,
but the changed prompt is not visible in this certified pair.

The original open pairs were state problems, not fixture selection problems.
The battle fixture now uses the tape's fresh 25/25, 10/10 Chaz and two-Zoran
formation; its party plane is receipt-exact, the Zoran overlay phase is pinned
to the frame-25000 VDP words, and enemy CRAM widening uses the canonical GPGX
ramp. The camp fixture now uses 500 MST and the tape's map/position; its
renderer consumes live NPC pixel positions, the receipt's frozen type-2 frame,
the camp field-party anchor, and the split `LV` label/numeric tile runs. The
required post-fix RMSEs remain unclaimed until the compliant capture can run.

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

The MeetingRika blocker that motivated this slice is fixed in code and pack:
oracle frame 7250's professor/Rika picture is dialogue `Ctrl::Action`
`LoadPanel` panel `$30`, and `$30` is now decoded and dispatched through the
cutscene panel stack. The oracle command from `oracle/README.md` was run on
2026-08-16; frame 7250 has SHA-256
`d8fc26ae6987e416ee75c02cd10ea4975e9be22485b8feeda84161aa489888c9`.
The pre-fix clone capture's settled Chaz page — fully typed
`Professor! / Thank goodness you're safe!`, panel up, field blanked — was
clone tick 162–164 with real RMSE **59.434935**; the diagnostic 1-pixel shift
probe was **37.9**, with columns 96–128 and 288–320 matching exactly.

The runtime now applies the decoded `(1,1)` plane residue to the panel and
portrait placement. A post-fix RMSE is **not claimed here**: this sandbox
cannot create an X11 socket (`bind(2)` is denied even in a private namespace),
so the required fresh Xvfb capture did not run. The ready-to-run command below
must record the settled tick from its own `PSIV_DEBUG_SCENE_TICKS=1` log and
then produce the post-fix number during integration.

Integration-time defects fixed while certifying (2026-08-17):

* Retail dialogue during scenes now plays over the blanked field:
  `InitVramAndCram` hides the map layers until the scene's own map redraw
  restores them (the oracle shows panels over black; the opening's first
  dialogue precedes its `LoadMap`).
* The retail-pace hold gated on the flow's page end, which the flow reaches
  before the typewriter reveals the glyphs — the auto-advance spam added
  accelerated frames and corrupted the cadence. The gate now requires the
  page fully revealed and the open animation finished.
* The dialogue window anchored to the viewport bottom, drifting ≈21 pixels
  low whenever the viewport is taller than the 3x surface; it now anchors
  to the centred 320x224 retail surface, the same rule as the panel layer.

**MeetingRika capture command for integration:**

```sh
GODOT=/home/peter/.local/bin/psiv-godot-4.7.1
xvfb-run -a env \
  PSIV_DEBUG_EVENT=0x8007 \
  PSIV_DEBUG_AUTOCLOSE_SCENE=1 \
  PSIV_DEBUG_RETAIL_PACE=1 \
  PSIV_DEBUG_SCENE_TICKS=1 \
  PSIV_DEBUG_SHOT=/tmp/psiv-meeting-rika-retail.png \
  PSIV_DEBUG_SHOT_FRAME=<settled-clone-tick> \
  timeout 180s "$GODOT" --display-driver x11 --rendering-method gl_compatibility --rendering-driver opengl3 --audio-driver Dummy \
  --log-file /tmp/psiv-meeting-rika-retail-godot.log \
  --path godot --quit-after 300000 \
  > /tmp/psiv-meeting-rika-retail.log 2>&1
python3 psiv_tools/presentation_rmse.py \
  /tmp/psiv-meeting-rika-retail.png \
  /tmp/psiv-meeting-rika-oracle/frame_7250.png
```

Choose `<settled-clone-tick>` from the same run's scene-tick log after the
`LoadPanel(0x030)` action and its DMA commit; rerun with that concrete tick to
produce the certified pair. The oracle frame 7250 and frame 7300 are identical,
so a clone settled window is valid when the log proves it is inside the same
post-action hold.

## Broader exact-frame certification surface

These are the deterministic fixtures and reference hashes for the final
certification pass. The battle and camp values shown below are the lastpairs
measurements before the receipt-driven compositor fixes; no post-final RMSE
is claimed until the pairs run under the required Xvfb/X11 harness.

| surface | deterministic fixture / clone tick | oracle frame and SHA-256 | RMSE | status |
|---|---|---|---:|---|
| battle command idle | `PSIV_DEBUG_BATTLE=0x88`, shot tick **200** | `oracle/frames/frame_25000.png`, `761fb241a2360d222fdf1538be1af89b7bfb9cd09376a6157733fd8f71e773c4` | lastpairs **12.185497**; final capture blocked | final fix is receipt-pinned in `oracle/states/battle_command_idle_vdp_25000.json` |
| title settled | `PSIV_DEBUG_TITLE_SHOT=1`, shot tick **480** | `oracle/frames/title/frame_450.png`, `8cebd30d62a7b5b0c3ad634ec6efc5ab6ab62e088d6166835d447c843df18c10` | prior **0.000000**; post-blink recapture blocked | expected unchanged because Press Start is invisible |
| camp root idle | `PSIV_DEBUG_CAMP=1`, shot tick **60**; oracle mark `camp_root_idle` | `oracle/frames/frame_7675.png`, `9bf283d9f48b4c0d959eb297ca0b1a997b62f227385ec25cae921e078ceaeca0` | lastpairs **14.456810**; final capture blocked | final fix is receipt-pinned in `oracle/states/camp_root_idle_vdp_7675.json` |

Title certification:

```sh
GODOT=/home/peter/.local/bin/psiv-godot-4.7.1
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 PSIV_DEBUG_SCENE_TICKS=1 \
  PSIV_DEBUG_TITLE_SHOT=1 \
  PSIV_DEBUG_SHOT=/tmp/psiv-title-xvfb-480.png \
  PSIV_DEBUG_SHOT_FRAME=480 \
  timeout 180s "$GODOT" --log-file /tmp/psiv-title-godot.log \
  --display-driver x11 --rendering-method gl_compatibility --rendering-driver opengl3 --audio-driver Dummy \
  --path godot --quit-after 300000 \
  > /tmp/psiv-title-xvfb-480.log 2>&1
python3 psiv_tools/presentation_rmse.py \
  /tmp/psiv-title-xvfb-480.png oracle/frames/title/frame_450.png
```

Camp certification:

```sh
GODOT=/home/peter/.local/bin/psiv-godot-4.7.1
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 PSIV_DEBUG_SCENE_TICKS=1 \
  PSIV_DEBUG_CAMP=1 \
  PSIV_DEBUG_SHOT=/tmp/psiv-camp-xvfb-60.png \
  PSIV_DEBUG_SHOT_FRAME=60 \
  timeout 180s "$GODOT" --log-file /tmp/psiv-camp-godot.log \
  --display-driver x11 --rendering-method gl_compatibility --rendering-driver opengl3 --audio-driver Dummy \
  --path godot --quit-after 300000 \
  > /tmp/psiv-camp-xvfb-60.log 2>&1
python3 psiv_tools/presentation_rmse.py \
  /tmp/psiv-camp-xvfb-60.png oracle/frames/frame_7675.png
```

Battle certification uses the same wrapper and a settled shot at tick 200:

```sh
GODOT=/home/peter/.local/bin/psiv-godot-4.7.1
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 PSIV_DEBUG_SCENE_TICKS=1 \
  PSIV_DEBUG_BATTLE=0x88 \
  PSIV_DEBUG_SHOT=/tmp/psiv-battle-xvfb-200.png \
  PSIV_DEBUG_SHOT_FRAME=200 \
  timeout 180s "$GODOT" --log-file /tmp/psiv-battle-godot.log \
  --display-driver x11 --rendering-method gl_compatibility --rendering-driver opengl3 --audio-driver Dummy \
  --path godot --quit-after 300000 \
  > /tmp/psiv-battle-xvfb-200.log 2>&1
python3 psiv_tools/presentation_rmse.py \
  /tmp/psiv-battle-xvfb-200.png oracle/frames/frame_25000.png
```

The old title smoke number (**0.01995** at tick 450) used a Vulkan/live
capture and is deliberately not a certification. The same rule applies to
the old camp PNGs: visual evidence is useful, but it cannot pin the surface
under this capture doctrine. On 2026-08-17 the mandated wrapper failed before
Godot started: `/tmp/.X11-unix` is owned by `nobody` and contains stale
`X0`/`X1` sockets, so Xvfb reports `Owner of /tmp/.X11-unix should be set to
root` and cannot bind its display. The additional non-destructive TCP test
(`xvfb-run -a -l -s '-nolisten unix -screen 0 1280x800x24'`) also failed with
`unable to open display`; this sandbox cannot bind TCP either. Therefore the
three commands above have exact expected pairings but no post-fix RMSE is
claimed here. The post-change battle attempt also returned before Godot could
create `/tmp/psiv-battle-after.png`; the Xvfb stderr was
`XSERVTransSocketCreateListener: failed to bind listener`, followed by
`Fatal server error: Cannot establish any listening sockets`. The stale global
socket directory was left untouched.

## Runtime evidence

The historical live opening harness (`PSIV_DEBUG_EVENT=0x9f`, 7,000 frames)
completed in `/tmp/psiv-opening-final.log`: it traversed the scene, emitted the 64-word
palette load, both 900-frame holds and 90/120-frame pauses, then logged
`scene ended` followed by `scene restored saved music: 0x84`.

The historical live Meeting Rika smoke harness (`PSIV_DEBUG_EVENT=0x8007`,
4,500 frames) is in `/tmp/psiv-meeting-rika-audio-final.log`. It used the old
`(1,1)` debug spawn and is not a retail-paced or Xvfb certification. It reached
the final Motavia handoff and
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

- **Fresh visual certification:** integration still needs the post-fix
  MeetingRika RMSE plus the title and camp RMSEs from the commands above. The
  oracle hashes and deterministic ticks are pinned; this sandbox cannot bind
  the Xvfb socket needed to produce the clone images.
- **Chest splinter's 66 unmapped patterns:** retained as transparent holes
  because the retail scene mapping consumes VRAM left by another runtime load;
  no source-of-truth pixels for those slots were found in the declared upload.
- **Palette-baked raster limit:** `UpdatePalette` and the three bespoke red
  effects use the decoded cutscene-layer redraw/overlay seam. A future CRAM
  renderer can replace that shell effect without changing the dialogue action
  timing or dispatch contract.
