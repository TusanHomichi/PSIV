# Scene presentation consumption

This slice is the renderer half of the typed scene seam. `SceneRunner` still
owns scene control, blocking, actor semantics, flags, and map changes. The
runtime translates each `SceneEffect::Presentation` into one
`RuntimeEvent::ScenePresentation` at the same tick and in the same vector
position. `rust/psiv-godot/src/runtime_events.rs` consumes that event in order;
it does not replay or infer scene operations. Each event captures
`panel_dialogue_mode` at that operation, so the op’s sprite visibility never reads a
later dialogue close or scene end from the completed frame.

The ordering contract is covered by
`presentation_events_keep_scene_order_and_tick_boundaries` in
`rust/psiv-runtime/src/suites/presentation_order.rs`.
The ROM-free renderer snapshot regression is
`rust/psiv-godot/tests/scene_visibility.rs`, run with
`cargo test --manifest-path rust/Cargo.toml -p psiv-godot --test scene_visibility`
under the [extension lock](../DEVELOPMENT.md#heavy-run-lock).

## Certified pairs

**All six certified screens are pixel-identical to the GPGX oracle.**

| pair | rmse | pairing |
|---|---:|---|
| opening page 1 | **0.000000** | clone t3550 (real title START) ↔ opening frame 4000, tape 27 |
| opening page 2 | **0.000000** | clone t4550 (real title START) ↔ opening frame 5200, tape 27 |
| MeetingRika | **0.000000** | clone t160 ↔ tape-28 frame 7250 |
| title (pre-prompt) | **0.000000** | clone t480 ↔ title frame 450 |
| battle `0x88` | **0.000000** | clone t200 ↔ tape-07 frame 25000, `--fixed-fps 60` |
| camp root | **0.000000** | clone t60 ↔ tape-22 frame 7675, `--fixed-fps 60` |

The palette scenes' first pair joined them on 2026-11-08
([#162](https://github.com/TusanHomichi/PSIV/issues/162)):

| pair | rmse | pairing |
|---|---:|---|
| `anger-tower-barrier` | **0.000000** | clone t108 (`PSIV_DEBUG_EVENT=0x99`) ↔ tape-37 frame 7384: `Event_EnterAngerTower`'s tone at saturation, lines 0, 1 and 3 white and line 2 held; region `1,1,319,223` (the native field's first row and column are black, [#409](https://github.com/TusanHomichi/PSIV/issues/409)) |

With the hold dropped from the CRAM model the pair fails (`rmse=12.012963`).

The ending's first scene joined them on 2026-11-09, the first scene pair at a
camera other than (0,0):

| pair | rmse | pairing |
|---|---:|---|
| `ending-elsydeon` | **0.000000** | clone t600 (`PSIV_DEBUG_EVENT=0x8021`) ↔ tape-38 frame 8930: `Cutscene_Ending`'s panels and window over map `$13`, before the Elsydeon break |
| `ending-elsydeon-tone` | **0.000000** | clone t638 ↔ tape-38 frame 8984: the break's tone at its peak, every line and the backdrop `$AAA`; region `1,1,319,223` ([#409](https://github.com/TusanHomichi/PSIV/issues/409)) |

With the old floored surface origin and its one-pixel compensation, the pair
fails (`rmse=55.057226`): panels, window and portrait one pixel right, and the
panels one pixel down ([placement findings](#placement-findings), the surface origin).
Every plateau of the break's tone matches inside that region, ticks 622-657
against frames 8968-9003, four frames a step; the untoned frame scores
`rmse=161.656685` against the peak, and the step before it `rmse=30.119675`.

The recovery tile's flashes joined them on 2026-11-09
([#101](https://github.com/TusanHomichi/PSIV/issues/101)):

| pair | rmse | pairing |
|---|---:|---|
| `recovery-flash` | **0.000000** | clone t78 (`PSIV_DEBUG_EVENT=0x21`) ↔ tape-42 frame 7246: `Event_Recovery`'s second flash at its peak; region `1,72,319,152` (the map's wandering NPC above, and #409's first column) |

Each step captured matches inside that region: the first flash's rise, peak
and restore (ticks 30-58 against frames 7194-7226, the restore's first step
not the brighten's mirror), the second's start and peak (62, 66, 78), and the
untoned frame again after the third (142). The untoned frame scores `rmse=78.342520` against the
peak, and the step before it `rmse=16.148053`.

The table-driven scenes have two further pairs
([#68](https://github.com/TusanHomichi/PSIV/issues/68)):

| pair | rmse | pairing |
|---|---:|---|
| `rykros-palette-cycle` | **0.000000** | clone t1270 (`PSIV_DEBUG_EVENT=0x801B`) ↔ tape-43 frame 8580: third nine-word row, planet visible but still dark; full 320×224 surface |
| `zio-barrier-ramp` | **0.000000** | clone t270 (`PSIV_DEBUG_EVENT=0x30`) ↔ tape-44 frame 7462: second three-word row, `$AAA/$0A8/$066`, and the scene's frozen beam mapping; region `1,1,319,223` (#409) |

Rykros's scene-owned panels `$18B/$18F/$190` are extracted alongside the
other panels; the runtime palette test checks every `PanelCreate` in that
scene against the pack, with an empty palette pack as its negative control.
The Zio fixture sets `$63`, the five-person party and the courtyard position.
Its oracle reload settles before the event; the recipe gives both fixtures
thirty field-object calls before entry rather than injecting an animation pose.
The beam routines' counters and the scene's object-update/map-update distinction
are described in [Zio Fort Barrier](90_ZioFortBarrier.md#beam-presentation).

Five battle pairs joined them on 2026-10-02 (#51, #64), all **0.000000**; the
decode, receipts and pins are in [`BATTLE_COMMAND_UI.md`](../battle/BATTLE_COMMAND_UI.md).

| pair | rmse | pairing |
|---|---:|---|
| `battle-strip` | **0.000000** | clone t334 (`strip,cursor=1,age=7`) ↔ tape-32 frame 25134, Alys's strip on TECH |
| `battle-tech` | **0.000000** | clone t600 (`tech,cursor=0,blink=14/1`) ↔ tape-32 frame 25400, the technique window |
| `battle-status` | **0.000000** | clone t200 (Chaz asleep, Alys paralyzed, Hahn sealed) ↔ tape-32 frame 25000 with three `Character_Stats` patches |
| `battle-status-2` | **0.000000** | clone t200 (poisoned, dead, asleep+sealed) ↔ the same frame, other patches |
| `battle-fusion` | **0.000000** | clone t758 (a defended round of formation `$D2`, blink seed 21/0) ↔ tape-33 frame 25560, the MetaSlug |

Three full-frame mid-round attack pairs joined them with
[#427](https://github.com/TusanHomichi/PSIV/pull/427), resolving
[#52](https://github.com/TusanHomichi/PSIV/issues/52):

| pair | rmse | pairing |
|---|---:|---|
| `battle-attack-last-first-mapping` | **0.000000** | clone t139 ↔ tape-34 frame 25370 |
| `battle-attack-next-mapping` | **0.000000** | clone t140 ↔ tape-34 frame 25371 |
| `battle-attack-strike` | **0.000000** | clone t147 ↔ tape-34 frame 25378 |

These use `top,timeline=attack`, phase 419, with the 1280×800 native viewport
normalized to the full 320×224 retail surface. The frame-offset control in
attack region `108,104,8,24` gives **0.000000** for t139 against frame 25370
and **80.813851** when t140 is substituted against that same oracle frame.
Timing and remaining choreography limits are recorded in
[BATTLE_COMMAND_UI.md](../battle/BATTLE_COMMAND_UI.md#mid-round-attack-52);
these isolated fixtures do not certify the complete windup/recovery or a campaign route.
Each battle pair names a `Session` fixture (`PSIV_DEBUG_BATTLE_WINDOW`,
`rust/psiv-runtime/src/session/battle/fixture.rs`).

Run every pair with `python3 tools/certify.py` (it rebuilds the debug extension
first); it pins each pair's clone tick and oracle-frame hash and writes a receipt
under `build/certify/`. The oracle frames regenerate from their tapes and are
hash-checked: `oracle/frames/opening/frame_4000.png` and `frame_5200.png` come
from `oracle/tapes/27_opening_scene_presentation.tape` (`--dump-frames
4000,5200`; tape 01 yields byte-identical frames), and certify's `TAPE_27`
recipe rebuilds both.

### Fixtures

- **The opening** is certified through the title's real START
  (`PSIV_DEBUG_TITLE_SHOT=1 PSIV_DEBUG_TITLE_AUTOSTART=1`, `Runtime::new_game`),
  not a fixture. On that timeline (`PSIV_DEBUG_SCENE_TICKS=1`; the scene starts
  at t46) page 1's text ramps up at t3018 and holds to t3918, page 2 at t4008 to
  t4908; ticks 3550 and 4550 sit inside them. t3017 and t3018, the ramp's first
  frames, read `rmse=31.137426`; t3060, t3093 and t3550 read `0.000000`. The
  window ends are the op log's `IntroTextFadeDown`, not a pixel measurement.
  Re-derive the ticks whenever scene content changes.
- **The camp pair** starts from the retail new-game banks plus the two flags
  the opening earns (`post_opening_game`; regression test
  `post_opening_state_does_not_replay_piata_chaz_alone`), because a blank state
  on Academy F1 lets map-entry trigger 124 (PiataChazAlone) pre-empt it. The
  debug event and camp selectors refuse with a clear error when a map-entry scene
  is already running.
- **MeetingRika** uses tape 28's position for the `0x8007` fixture (leader pixel
  `($1F0,$1A0)`, the retail trigger's exact Y). Its settled window is narrow:
  full reveal ~t158, dismiss ~t163. Oracle frames 7250 and 7300 are identical.
- **The title** pairs at t480 with pre-prompt oracle frame 450: the window
  between the reveal settling and the prompt and copyright draw.
- **`PSIV_DEBUG_RETAIL_PACE`** keeps the retail 3-frame-per-character
  typewriter, holds a completed page for 4 frames, and dismisses it with a
  one-frame `ButtonSpeak` press in the pad, so the next page's first glyph
  frame types at the held cadence. It advances only a page that is fully
  revealed with its open animation finished, and never a `Choice` page.

### Capture doctrine

`tools/certify.py` starts one X server per worker, `Xvfb -displayfd <fd> -screen
0 640x480x24 -nolisten tcp`, and runs each pair as `DISPLAY=:<n>
LIBGL_ALWAYS_SOFTWARE=1 … --display-driver x11 --rendering-method
gl_compatibility --rendering-driver opengl3 --audio-driver Dummy --fixed-fps
60`. Each part is load-bearing:

- **Xvfb, not the desktop.** A focused window on a real session receives real
  input, and the typewriter's hold-to-accelerate makes the timeline
  input-dependent.
- **Software GL.** Without `LIBGL_ALWAYS_SOFTWARE=1`, Mesa's GPU probe can fail
  in a background session and Godot falls back to the live desktop.
- **`--fixed-fps 60`** for any pair whose content animates: without it,
  Godot's physics catch-up under uneven render pacing makes the captured
  previous-frame texture reflect a load-dependent tick.

`presentation-rmse` (`rust/psiv-compare`) crops the centred 960x672 3x
surface from `PSIV_DEBUG_SHOT`, normalizes it to 320x224, and reports RGB RMSE
against an oracle frame, over the whole frame or one `--region
x,y,width,height`; the identity-path check is `rmse=0.000000`.

## Placement findings

- **CRAM widening** is the GPGX ramp, centralized in
  [`COLOR_PIPELINE.md`](../field/COLOR_PIPELINE.md); no linear or
  bit-replication widening remains in the Python pack emitters. The opening's
  whole pre-ramp residual was that widening.
- **The scene portrait's window group.** `WinGroup_Dialogue` record 2 is
  `(5,13)` = `(40,104)` pixels; the scene/event branch in
  `TextCtrlCode_Portrait` selects `WinGroup_Event` record 4, `(3,14)` =
  `(24,112)`. The port marks scene-owned dialogue and applies `(-16,+8)` to the
  packed talk portrait.
- **Text origin.** `TextBufferToPlane` writes screen tile `(4,$15)`
  (`ps4.asm:143409`), a 32x4 map of 8x16 glyphs: an 8-pixel text origin and a
  16-pixel line pitch (`dialogue.rs`).
- **The plane scroll.** The `--dump-state` receipt of the MeetingRika fixture at retail frame 7250
gives the scroll. The host resolves the
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
VSRAM words for this frame. No screen-space term remains: panels, the window
and its portrait sit at their retail coordinates in the surface.
- **The surface origin.** Every screen-space layer (panels, the opening text,
  the dialogue window) is placed at the top-left of the centred 320x224
  surface, read back through the inverse canvas transform and rounded to a
  whole pixel (`retail_surface.rs`). A 1280-wide viewport is 426.67 world
  pixels at 3x, so the read-back carries f32 error; flooring it lost a pixel at
  camera (0,0), which this document once recorded as a measured `(1,1)`
  plane/window residue, and the compensation drew every other camera position
  one pixel right and down (the `ending-elsydeon` pair).
- **The scroll arrow** is two 8x8 `ArtNem_Font` tiles (`$36`/`$37`)
  row-interleaved as a 2x1 VDP sprite.
- **Scenes draw over a blanked field.** `InitVramAndCram` hides the map layers
  until the scene's own map redraw restores them (the opening's first dialogue
  precedes its `LoadMap`).
- **The dialogue window anchors** to the centred 320x224 retail surface, the
  same rule as the panel layer, not the viewport bottom.
- **Battle and camp pins.** The enemy overlay's debug pin is
  `ORACLE_ENEMY_PHASE_TICKS = 20`, solved from two observed phase tuples
  against the clock arithmetic (K = 170 rendered advances at the fixed-fps
  capture), with retail's tuple `(2,2,1)` proven per piece against the decoded
  frame-25000 plane and VRAM cells; the battle chrome carries the
  `$0620 → $0600` fill remap in the strip, the font sheet and `palette[14]`;
  camp's HP/TP colon is menu-font glyph 51 (tile `$6B4`); and the receipt NPC
  renders walk-down frame 1.

### The battle and camp receipts

Tape 07 frame 25000 is
`761fb241a2360d222fdf1538be1af89b7bfb9cd09376a6157733fd8f71e773c4`, and tape
22 frame 7675 is
`9bf283d9f48b4c0d959eb297ca0b1a997b62f227385ec25cae921e078ceaeca0`.

#### Battle command idle

`oracle/tapes/07_first_battle.tape` produces `frame_25000.png`, the settled
command-idle battle. The RAM receipt at `$41F0` is
`0F 05 00 00 02 03 00 0A 0E 0A 1A FF ...`: the two Zoran Bult position bytes
are `$0E` and `$1A`, which decode to body anchors **(88,72)** and **(184,72)**
with six-cell body runs at Plane A columns 11..16 and 23..28. 

The party layout receipt is center-out rather than status-strip order:

| fighter slot | character | screen origin | Plane A destination | idle pose grid |
|---:|---|---:|---:|---:|
| 1 | Alys | `(136,120)` | row 15, col 17 | `$FFFF3000` |
| 2 | Chaz | `(88,120)` | row 15, col 11 | `$FFFF3048` |
| 3 | Hahn | `(184,120)` | row 15, col 23 | `$FFFF3090` |

The fresh VDP SAT region at the retail SAT base is all zero and the checked-in
`battle_command_idle` layout has no visible SAT entries. Position/pose proof
for this idle frame is therefore the Plane A tile runs, `$41F0`, and the
overlay metadata; no SAT attributes are invented here. The port assigns
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
and suspends field wandering during the debug lead-in, so the post-spawn
drift does not reach the capture; normal play is unchanged.

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

Correction, 2026-11-01 ([#269](https://github.com/TusanHomichi/PSIV/issues/269)):
this fixture once drew slot 0 two pixels up as a "walk bob". There is no bob.
`Field_FillSpriteAttributes` (`ps4.asm:89283`) writes each SAT y as
`sprite_y_pos` plus the mapping's y byte, with nothing from the walk phase, and
this receipt's three visible objects (slot 0, slot 7 and Chaz) all sit at
object y minus camera y minus 2, whatever they are doing: the sheet's own
origin already gives that. The fixture's -2 was cancelling the +2 of the
half-cell anchor the motion path then counted twice; when
[#222](https://github.com/TusanHomichi/PSIV/pull/222) drew the anchor once, the
-2 alone was left and this pair went to rmse 5.071417. The oracle frame was
right throughout; the -2 is gone.

The status-window Plane A decode is also explicit. At screen row 2, columns
26..37 are
`C6F3 C683 C6C0 C6B9 C6D2 C680 C680 C68C C696 C680 C7DB CEF3`.
Therefore the port draws `LV` at cell `(33,2)`, the decimal tile run at
`(36,2)`, leaves cell `(35,2)` as the window blank, and never writes the
border at `(37,2)`.

## Op coverage

| Operation family | Shell status | Evidence / limit |
|---|---|---|
| `InitVramAndCram` | Implemented | Clears staged/visible scene planes and opening text. |
| `FadeIn`, `FadeOut` | Implemented | Seven CRAM-equivalent levels, two-frame stepping, 14 renderer ticks; the cover is above cutscene planes and below dialogue. |
| `Panel_Create`, `Panel_Destroy`, `Panel_DestroyAll` | Implemented for scene ops and dialogue `$F2` actions | Retail panel records are decoded from all non-empty banked `PanelPtrs` ranges. Scene ids remain compatible with the typed scene stream; dialogue word ids include `$30` and the other 162 action-referenced records. Retail destroy is the operand-free typed `PanelDestroyLast` stack pop; loop counters are not panel-art IDs. Only an empty staged stack produces a destroy warning. |
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
| `SetSavedMusic` | Implemented for scene writes | Runtime stores the persistent retail `Saved_Sound_Index` byte, including zero, and decides conditional writes before emitting audio ops. Real-session Godot restores read that word without consuming it; the one-shot cue is confined to runtime-less debug fixtures. Boarding retains the chosen sound at scene end without a synthetic restart. Exact map-transition replay timing still lacks the runtime's `$ECED` music-change edge; a still-zero word uses the shell's existing map fallback, not a certified cartridge zero/stop rule. |
| `WaitFrames` | Implemented | Runtime owns the blocking count; Godot records the op and does not create a second timer. |
| `PresentationOp::RebuildSprites` / `ReloadMapChunks` | Implemented | Rebuilds map visuals at the ordered event. |
| `ReplaceMapLayout` | Implemented through `MapRefreshed` | The runtime installs the pack's layout variant decoded from the two ROM sources and emits `MapRefreshed`; Godot's existing handler redraws the map from the runtime view, which names the variant's PNG. `Event_GaruberkTwEyeAction1` ([104](104_GaruberkTowerEyes.md)). |
| Palette records: `PaletteIncreaseTone`, `PaletteRestoreTone`, `PaletteToneFlash`, `PaletteCopy`, `SetPaletteWords`, `AddPaletteWord`, `RykrosPaletteCycle`, `PaletteRampFromTable`, `PaletteCycleFromTable`, `CounterPaletteCycle`, `PaletteGrowIn`, `VariablePaletteFade`, `TwoLinePaletteFade`, `FadeToRed`/`FadeFromRed`, `LoadPalette`, `ReloadMapPalette`, `InitVramAndCram`, panel palettes | Implemented, drawn per line | The runtime's CRAM model applies every one frame by frame ([CRAM state](#cram-state)), and the map, field sprites, scene panels and dialogue window draw from it ([Drawing the CRAM](#drawing-the-cram)). Native captures of these scenes against oracle frames are [#162](https://github.com/TusanHomichi/PSIV/issues/162) and [#68](https://github.com/TusanHomichi/PSIV/issues/68). |
| `GaruberkEyeArtCycle`, `ShadowHighlight` | Logged, not drawn | Godot consumes both (`cutscene/ops.rs`): the Garuberk eyes' art cycle and `Event_ZioNurvus`'s VDP register 12 writes. The frames are the scene's waits, so timing is unaffected. |
| palette-word/red-fade/window/portrait records | Implemented for the decoded generic set | Generic `WindowDestroy/Create`, `LoadWindowTiles`, `LoadPortrait`, and `DrawPortrait` now drive the runtime window layer. The Meseta window roles come from `dialogue/set.window.png`; `shopkeeper_2` is emitted as a 48x48, 36-tile portrait from art `$29DE1E`, mapping `$2A2B36`, destination tile `$55C`. |

### Additive asset coverage

The rebuilt pack reports this exact census in
`runtime-pack/presentation/panels.json`:

| Surface | Count | Result |
|---|---:|---|
| scene-owned panel records | 18 | exact decoded scene records |
| dialogue action panel records | 163 | every distinct `$F2 LoadPanel` id, including `$30` |
| ending-only panel records | 50 | `$8021` post-battle story sequence |
| total panel records | 231 | all PNGs present in the runtime manifest |
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

The pack decoder asserts all 231 panel records (18 scene-owned, 163
dialogue-action records and 50 ending-only records), all 7 `LoadArt` source /
destination pairs, the 6 temporary-object keys, the generic portrait's art /
mapping / tile contract, both Enigma planes for each panel, their retail
coordinates, and the opening image addresses in
`tests/test_presentation_pack.py`. That test also invokes
`oracle/decode_layout.py` against the committed frame-4000 state; its
`self_check.passed` value is `true` and all four layout checks pass.

Godot capture comparison uses `presentation-rmse`, described under the
capture doctrine above.

## Opening cinematic

The opening renderer consumes the same presentation op family as ordinary
scenes: title image, palette load, tree-17 text entries, 20-frame text ramps,
long holds, and the final fade. It draws the 320x128 retail title image at
screen y=40, leaving the oracle's 40-pixel top and 56-pixel bottom black bars
(measured from `oracle/frames/opening/frame_4000.png`; the image band is rows
40..167 in the 320x224 surface).
The motion/map/dialogue state remains in the existing runtime scene.

The shell-only selectors `PSIV_DEBUG_TITLE_AUTOSTART=1` (real title START) and
`PSIV_DEBUG_AUTOCLOSE_SCENE=1` provide a deterministic harness; adding
`PSIV_DEBUG_RETAIL_PACE=1` preserves retail dialogue cadence for frame pairing.
All are evaluation switches and are never used by normal play. The old
`PSIV_DEBUG_EVENT=0x9f` fixture is gone (#44); `PSIV_DEBUG_EVENT` still serves
MeetingRika (`0x8007`) and the ship's menu (`0x800D`), and refuses to boot a
scene a map-entry trigger has already pre-empted. `0x99` loads the Anger
Tower with Elsydeon held, where the map's own entry trigger starts
`Event_EnterAngerTower`: the barrier's tone with line 2 held.

## Remaining deferrals

- **Scene-wide fidelity:** the certified pairs are single frames; they do not
  certify whole scenes, and the Rika fixture's palette writes are unverified
  against a full scene.
- **Chest splinter's 66 unmapped patterns:** retained as transparent holes
  because the retail scene mapping consumes VRAM left by another runtime load;
  no source-of-truth pixels for those slots were found in the declared upload.
- **Surfaces outside the live palette:** the shop, camp and choice menus,
  and the dialogue action `SabotageAlarmRedPalette`'s red overlay still draw
  baked colours.
- **Rykros's tone:** its cycle has a certified frame above; the preceding tone
  still needs its own oracle pair
  ([#162](https://github.com/TusanHomichi/PSIV/issues/162)).

## CRAM state

`Runtime::cram()` is `Palette_Table_Buffer` (`$FFFFFB00`, 64 words) as `VInt`
would upload it this frame, with a revision for upload suppression and the
list of ROM palettes a scene asked for that the pack lacks. Its one writer is
`rust/psiv-runtime/src/cram.rs`; the arithmetic is `rust/psiv-core/src/palette.rs`,
each function transcribed from the routine it names with its own masks and
borrows. Writers:

- a map load fills both buffers from the record's 64-word shadow (lines 0, 1
  and 3 from the map, line 2 `Pal_Init_Line_3`), then makes each
  `MapDataManager` palette copy whose path's gates hold, in dispatcher order,
  into both buffers: the load's `Pal_FadeIn` copies the live buffer to the
  second after `MapDataManager` runs (`ps4.asm:107579-107630`). Rescued Zema's
  and petrified Birth Valley's NPC sheets draw from those words. The map
  update programs write single words;
- a scene's map load whose next op is `Pal_VariableFadeIn`
  (`SceneRunner::fades_in_after_map_load`) keeps the words the frame before
  uploaded live and holds the load's words in the buffer, unshown:
  `RefreshMap` (`ps4.asm:121767`) returns with no VBlank, and the fade-in
  copies and clears the buffer (`ps4.asm:122071-122079`) before its first
  ([#612](https://github.com/TusanHomichi/PSIV/issues/612)). The fade-in
  copies the held words, a map update write lands in them, and any other
  record puts them live first;
- `InitVramAndCram` copies the live buffer to the second one and loads
  `Pal_Init` (`$09F2BC`, the `lea` at `$05A68A`); `LoadPalette` and
  `ReloadMapPalette` write the live buffer; `Panel_Create` (scene ops and
  dialogue `$F2` panels) copies sixteen words per drawn layer to
  `(art tile >> 8) & $70` bytes in (`ps4.asm:144392-144401`);
- boarding copies 17 words from each vehicle palette (`$29754E`, `$29756E`,
  `$29758E`) at live word 48: line 3 and word 0 of the adjacent saved buffer
  (`$06B9EA`, `$06BAC2`, `$06BB9A`). Vehicle sheets draw their indices through
  the same live CRAM path as party and NPC sheets;
- `Cutscene_Alshline` copies `PiataBasementB2_Palettes`' 14 words (`$052848`)
  to line 3 before it builds Igglanova (`$07444E`, `MapDataManagerJmpTbl`
  entry `$D0`); words 62 and 63 keep Zema's;
- `PaletteCopy` is every `trap #1`/`trap #0`/fill into either buffer;
  `SetPaletteWords` and `AddPaletteWord` take word offsets;
- the loops step once per runtime tick, starting on the tick the scene emits
  them, with that tick's `Main_Frame_Count`: `Pal_IncreaseTone` and its
  per-frame hold copy, the `loc_58666` restore, the recovery flashes (red and
  green only: both mask blue and keep it), the Rykros cycle, the Zio ramp, the
  engine light, the Le Roof colour cycle and fade-in, both red fades, both
  variable fades and their two-line forms (`TwoLinePaletteFade`: words 0-31
  only; the fade up starts from the live words, with no copy or clear). The Le Roof cycle (`loc_70638`, `$070638`) also steps the
  index byte `$FFFFECF3`, which the map programs share: the runtime keeps it
  with their counters (`MapUpdateView`), cleared by each map load as
  `ps4.asm:107593` clears it, and lends it to the CRAM model.

The ROM words come from the pack: `presentation/panels.json`'s `palettes`
(`PALETTE_REQUESTS` in `psiv_tools/presentation_pack.py`, and every drawn panel
layer's palette), read by `psiv_data::ScenePalettes`.

Corrections this model brought, each checked against the US image:

- `Event_Recovery`'s copy is `move.w #$1F, d7` / `trap #1` at `$06D384`, and
  `trap #1` copies words: 32 words, not 64.
- `loc_58666`, the restore after `Pal_IncreaseTone` (`$073856`, `$078220`,
  `$0796E4`), steps all three components of the whole word with a borrow; it
  was recorded as the recovery flash's restore. It is `PaletteRestoreTone`, and
  Rykros and the ending gained the record their restore loops lacked.
- `loc_78296`'s delays (`$0782D2`: `$07,$05,$07,$07,$07,$07,$FF`) are `dbf`
  counts, 46 frames in all, and the cycle now blocks the scene for them; the
  record held the raw bytes and did not block.
- `Pal_VariableFadeIn`/`Out` last as long as the palette's largest nibble
  needs: the scene blocks on `Blocked::Palette` until the runtime answers with
  the count (`SceneRunner::release_palette`). `Event_Phonon` and the ending
  grew by those frames.
- `SetPaletteWords` offsets are word offsets: Vahal's barrier (`$0732EC`), the
  girls' sneaking-out palette (`$06D506`) and MeetingRika's (`$074826`) were
  byte offsets. `Event_ZioNurvus`'s `$8C89`/`$8C81` (`$06F2FA`, `$06F3AA`) are
  VDP register 12 writes, now `ShadowHighlight`.
- The Le Roof scenes' colour loops (`$070538`, `$070552`, `$070570`;
  `$0783F2`, `$07840E`) and the fade-in they share (`loc_705D6`, 71 passes)
  run `loc_70638` and `VInt_Prepare` only. They were `Wait`s, which update the
  map, with no colour cycle and no fade-in; they are now `CounterPaletteCycle`
  and `PaletteGrowIn`, which block for their passes with no map update (#591).
- The same scenes' fades are `Pal_VariableFadeOut_Lines_1_2` (`$05B222`) and
  `Pal_VariableFadeToPal2` (`$05B26E`), variable passes over words 0-31, not
  `Pal_VariableFadeToRed`'s eight stages; and both load Le Roof's line
  (`loc_1DE258`) into the second buffer and clear its words 16-31 before the
  fade up (`$0704D8`, `$078398`). The fade up left red in words 2-10 and was
  21 frames short (#591).
- Every scene's fade records are now checked against its bytes: the `jsr`s
  to the six variable fades (`Pal_VariableFadeOut`/`In`, `$05B168`/`$05B1B8`;
  the two-line pair; `Pal_VariableFadeToRed`/`FromRed`, `$05B2BE`/`$05B30E`)
  inside its documented ranges, in order, with each call's `$FFFFED52` mode
  (`scenes::palette_fades_image`: its `CALLS` table is read off the US image
  when it is present, and every scene is checked against the table without
  it; the scenes not yet fixed are listed with
  [#610](https://github.com/TusanHomichi/PSIV/issues/610)). It found
  `Event_Juza` (`$06FB3A`) and both of `Event_ReFaze`'s fades (`$0715DA`,
  mode 5; `$07172C`, mode 2) recorded as red fades, a shell fade-out, or not
  at all; they are now `TwoLinePaletteFade`. It also found
  `Cutscene_CrashLaanding`'s red fade (`$076710`), which is
  `Pal_VariableFadeOut` mode 9, and the ending's `$17C` panel fading in
  at mode 5 where `$079B1E` sets 1 (#596).
### Drawing the CRAM

The field shell copies `Runtime::cram()` into one shared palette at the top
of every drawn frame (`Field::sync_map_palette`), and every surface whose
art is indexed draws from it:

- the map's index textures, lines 0 and 1, through the map shader
  (`field_map.rs`), its palette set when the shader is installed;
- field sprite sheets (party, followers, NPCs, vehicles) and scene temporary
  objects (`presentation/panels.json`'s `temporary_objects`) on the line their
  pack record names (`palette.cram_line`, from the object's `$13` byte);
- scene panels, each layer on its art tile's line (bits 13-14);
- the dialogue window, whose tiles, glyphs and portraits are line 2;
- the backdrop, CRAM word 0 (VDP register 7 `$8700`: palette 0, colour 0),
  as the renderer's clear colour.

Sheets, temporary objects and panels keep each pixel's palette index
(`psiv_data::color::index_png`) and are recoloured
(`psiv_data::color::recolor`) when the words change, on the frame they are
drawn (`rust/psiv-godot/src/cram_art.rs`). A map record without the 64-word
shadow is not live (`CramView::live`): every surface on it, the map's base,
overlay, layout variant and patch atlas pictures included, draws its baked
colours. The window's art is baked in
`Pal_Init_Line_3`'s colours, which are distinct, so its shader finds each
pixel's index by colour. The red fades are the CRAM model's too: the
screen-space red effect that stood in for them is gone, since it would now
apply twice.

`tools/native/native_temporary_palette.gd` checks a temporary object in the
running shell: `Cutscene_Alshline` from `PSIV_DEBUG_EVENT=0x8005` (the scene
loads Zema itself). On Igglanova's build tick it requires the line-3 copy,
unchanged lines 0 and 2, and every opaque pixel a live line-3 colour; once the
object is inside the camera's 320×224 view it captures the sheet, the current
frame and the viewport before and during Zema's line-1 cycle, and requires
line 3 and every sheet pixel, alpha included, unchanged. It is a debug-event
fixture, not a campaign route or an original-game comparison; the wrong-line
negative control is in `cutscene/manifest.rs`'s tests. The driver's first
failure is final: it stops the run and no later capture can exit 0 over it.
`rust/psiv-native-verify/tests/temporary_palette_driver.rs` runs it against a
stub game and faults each check in turn (the copy, then line 3 or a sheet pixel
before the first capture and during the cycle); each must exit nonzero. It
resolves Godot through `tools/godot.py`, takes a `godot` heavy-lock slot for
each run and skips without the pinned Godot or `xvfb-run`:
`cargo test --manifest-path rust/Cargo.toml -p psiv-native-verify --test temporary_palette_driver`.
From the repository root, with the extension and local pack built:

```bash
mkdir -p build/temporary-palette/saves
PSIV_DEBUG_EVENT=0x8005 PSIV_DEBUG_ROUTE=1 PSIV_DEBUG_AUTOCLOSE_SCENE=1 \
PSIV_DEBUG_RETAIL_PACE=1 PSIV_SAVE_DIR="$PWD/build/temporary-palette/saves" \
PSIV_TEMPORARY_OUTPUT="$PWD/build/temporary-palette" \
python3 tools/heavy_lock.py run --class godot -- xvfb-run -a \
python3 tools/godot.py run -- --rendering-driver opengl3 --fixed-fps 60 \
  --audio-driver Dummy --script "$PWD/tools/native/native_temporary_palette.gd"
```

Deviation: the fades whose frames the scene does not spend (`FadeIn`,
`FadeOut`, `InitVramAndCram`'s fade) write only their copy
([source notes](../source-notes/README.md#other-recorded-deviations)).
