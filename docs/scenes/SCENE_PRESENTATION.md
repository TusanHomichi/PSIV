# Scene presentation consumption

This slice is the renderer half of the typed scene seam. `SceneRunner` still
owns scene control, blocking, actor semantics, flags, and map changes. The
runtime translates each `SceneEffect::Presentation` into one
`RuntimeEvent::ScenePresentation` at the same tick and in the same vector
position. `rust/psiv-godot/src/runtime_events.rs` consumes that event in order;
it does not replay or infer scene operations.

The ordering contract is covered by
`presentation_events_keep_scene_order_and_tick_boundaries` in
`rust/psiv-runtime/src/suites/next_arc.rs`.

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

Five battle pairs joined them on 2026-10-02 (#51, #64), all **0.000000**; the
decode, receipts and pins are in [`BATTLE_COMMAND_UI.md`](../battle/BATTLE_COMMAND_UI.md).

| pair | rmse | pairing |
|---|---:|---|
| `battle-strip` | **0.000000** | clone t334 (`strip,cursor=1,age=7`) ↔ tape-32 frame 25134, Alys's strip on TECH |
| `battle-tech` | **0.000000** | clone t600 (`tech,cursor=0,blink=14/1`) ↔ tape-32 frame 25400, the technique window |
| `battle-status` | **0.000000** | clone t200 (Chaz asleep, Alys paralyzed, Hahn sealed) ↔ tape-32 frame 25000 with three `Character_Stats` patches |
| `battle-status-2` | **0.000000** | clone t200 (poisoned, dead, asleep+sealed) ↔ the same frame, other patches |
| `battle-fusion` | **0.000000** | clone t714 (a defended round of formation `$D2`) ↔ tape-33 frame 25560, the MetaSlug |

The mid-round attack ([#52](https://github.com/TusanHomichi/PSIV/issues/52)) is
not certified: its fixture reads `rmse=20.254240` against tape-34 frame 25377
([BATTLE_COMMAND_UI.md](../battle/BATTLE_COMMAND_UI.md#mid-round-attack-52)).
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

`psiv_tools/presentation_rmse.py` crops the centred 960x672 3x surface from
`PSIV_DEBUG_SHOT`, normalizes it to 320x224, and reports RGB RMSE against an
oracle frame; the identity-path check is `rmse=0.000000`.

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
- **The plane residue.** The `--dump-state` receipt of the MeetingRika fixture at retail frame 7250
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
VSRAM words for this frame. The remaining
screen-space term is the measured retail plane/window residue
`(x=1,y=1)` pixel at `grand_cross=0`; its provenance is recorded in the JSON
as `placement_provenance`, not disguised as camera motion. The clone applies
that term only to scene panel planes and portraits. Dialogue window chrome,
glyphs, arrows, and the opening background retain their independent retail
origins.
  The dialogue window takes the same +1 X residue as the panels; the portrait
  is window chrome and does not.
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
  renders walk-down frame 1 with the retail 2px step bob.

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
| `Panel_Create`, `Panel_Destroy`, `Panel_DestroyAll` | Implemented for scene ops and dialogue `$F2` actions | Retail panel records are decoded from all non-empty banked `PanelPtrs` ranges. Scene ids remain compatible with the typed scene stream; dialogue word ids include `$30` and the other 162 action-referenced records. Retail destroy is an operand-free stack pop; the typed `PanelDestroy { id }` currently emits false art-ID mismatch warnings for Alshline's loop counters ([#74](https://github.com/TusanHomichi/PSIV/issues/74)). |
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
| `PresentationOp::PaletteToneFlash`, `GaruberkEyeArtCycle` | Logged, not drawn | Godot consumes both with the other palette and art records it does not draw (`cutscene/ops.rs`): `Event_Recovery`'s white flash ([102](102_Recovery.md)) and the Garuberk eyes' art cycle. The frames are the scene's `WaitFrames`, so timing is unaffected. |
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
MeetingRika (`0x8007`) and refuses to boot a scene a map-entry trigger has
already pre-empted.

## Remaining deferrals

- **Scene-wide fidelity:** the certified pairs are single frames; they do not
  certify whole scenes, and the Rika fixture's palette writes are unverified
  against a full scene.
- **Chest splinter's 66 unmapped patterns:** retained as transparent holes
  because the retail scene mapping consumes VRAM left by another runtime load;
  no source-of-truth pixels for those slots were found in the declared upload.
- **Palette-baked raster limit:** `UpdatePalette` and the three bespoke red
  effects use the decoded cutscene-layer redraw/overlay seam. A future CRAM
  renderer can replace that shell effect without changing the dialogue action
  timing or dispatch contract.
- **Teardown noise:** Godot's Dummy-driver forced quit reports one
  `AudioStreamGeneratorPlayback` ObjectDB leak after a scene completes.
