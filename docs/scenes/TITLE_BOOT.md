# Retail title and boot front door

This slice records the retail boot surface without committing ROM-derived
pixels. The committed oracle is [`oracle/tapes/25_title_boot.tape`](../../oracle/tapes/25_title_boot.tape). The decoded JSON, reference PNGs, and runtime pack
are generated artifacts under ignored paths.

## Oracle layout contract

Tape 25 captures the Sega logo, title reveal, settled title, and the no-save
menu. `oracle/layouts/title/` contains state decodes at frames 25, 50, 75,
100, 125, 150, 175, 200, 300, 400, 401, 450, 500, 550, 600, and 650. The
populated-SRAM gate is captured separately at `populated_state_650.json`.

| Surface | Plane | Cell rect | Pixel rect | VRAM pattern range | CRAM line |
|---|---|---:|---:|---:|---:|
| Sega logo | A | `(12,11) 17x5` | `(96,88) 136x40` | `$010..$055` | 0 |
| PSIV logo | A | `(11,3) 17x13` | `(88,24) 136x104` | `$010..$091` | 0 |
| subtitle | A | `(5,17) 29x3` | `(40,136) 232x24` | `$092..$0C9` | 0 |
| Press Start | A | `(11,22) 18x1` | `(88,176) 144x8` | `$0D8..$0E2` | 2 |
| copyright | A | `(12,25) 17x1` | `(96,200) 136x8` | `$0CA..$0D6` | 1 |
| title background | B | `(0,0) 40x28` | `(0,0) 320x224` | `$0E3..$280` observed | 3, priority |

The two `$010` ranges deliberately overlap in VRAM. The decoder uses the
captured phase to distinguish the Sega mapping from the later title mapping;
it retains the raw observed words and coverage in every layout JSON.

The settled Plane B frame is byte-equal to the concatenation of the retail
`TitleBGLeftPart` (6 cells), `TitleBarBGBottomPart` (8 cells), and
`TitleBGRightPart` (26 cells). The separate `TitleBarBGTopPart` mapping
(8x28 cells at screen x=6) is emitted as `titlebarbgtoppart.png` and is used
for the sparse transfer surface before the settled background arrives.

`oracle/layouts/title/palette_cycle.json` preserves all four 16-word CRAM
lines at every captured frame plus changed-entry indexes. This is the numeric
fade/cycling evidence; it is not inferred from a screenshot.

The sampled animation is: Sega fade/logo through frame 200, the narrow title
background transfer at frames 300-401, settled logo and subtitle at frame 450,
Press Start/copyright from frame 500, and the no-save menu after acceptance.
Godot keeps the same decoded phase boundaries and uses the existing 3x camera;
the title frame is letterboxed with black outside the retail 320x224 surface.

The per-frame CRAM replay is now emitted, not merely recorded. The pack carries
16 captured frame states (25 through 650) and re-encodes all 7 indexed title
surfaces against the captured 64 CRAM words: background, transfer, and the five
title assets. `title/palette_cycle.json` remains the numeric source record;
`title/replay/frame_<N>/` is the runtime surface set for the sampled fade and
background phases. The Press Start prompt is the exception: retail's
`DoPressStartButtonCyclingPal` updates its palette entry every four
`Main_Frame_Count` frames through the 32-word table at
`ps4.asm:87014-87065`. `title.rs` generates those 32 prompt textures and
selects the calibrated main-frame phase, so the prompt fades through the
retail dark/red/orange cycle instead of staying on the last sparse replay
sample.

## Art extraction and pack surface

`psiv_tools/title_pack.py` calls the existing verified `planes.MAPPINGS`,
`art_tiles`, `decode_mapping`, and `named_cram` paths. The five visible title
assets use the retail Nemesis records:

- `ArtNem_SegaLogo`
- `ArtNem_TitlePSTitle`
- `ArtNem_TitleTheEndOfTheMillennium`
- `ArtNem_PressStartButton`
- `ArtNem_TitleCopyrightText`

The background uses `ArtNem_TitleBackground` with its four retail Enigma
mappings. Every emitted record carries the ROM offset, compression, consumed
length, compressed/decompressed hashes, VRAM tile, and exact decoded mapping
range. `build_pack()` emits these additively under `runtime-pack/title/` and
adds the small `title` manifest fragment; no PNG or generated layout is
tracked.

## Menu gate and Godot flow

The empty SRAM capture opens a `14x3` window at `(13,12)` and decodes one
option: `START` at `(16,13)`. A valid slot-0 SRAM capture opens the retail
`14x7` window at `(13,10)` and decodes `CONTINUE`, `START`, and `ERASE DATA`
at rows 11, 13, and 15 respectively. The title code gates `CONTINUE` on the
same validated `Runtime::load_slot` path used by boot and exposes a three-slot
picker that refuses empty slots. `ERASE DATA` opens the selected populated slot
picker, asks `ARE YOU SURE?`, and on YES calls the retail-equivalent
`Runtime::erase_slot` operation. It clears only that file's 0x1400-byte
physical payload and leaves the 0x200-byte common header untouched; the erased
slot is then removed from the title's available-slot view. The exact retail
flow and byte boundary are recorded in [`SAVE_SCOUT.md`](../camp/SAVE_SCOUT.md).

`rust/psiv-godot/src/title.rs` renders the decoded assets at the 3x camera
scale and drives Sega -> reveal -> Press Start -> menu. `boot.rs` preserves
the fast paths for `PSIV_LOAD_SLOT`, `PSIV_DEBUG_BATTLE`, `PSIV_DEBUG_CAMP`,
`PSIV_DEBUG_SHOP`, and `PSIV_DEBUG_SHOT`; those selectors never wait behind the
retail front door. For visual QA only, `PSIV_DEBUG_TITLE_SHOT=1` opts an
existing `PSIV_DEBUG_SHOT` capture into the title; ordinary debug screenshots
continue to bypass the front door.

## Exact-frame certification

The settled title fixture is retail tape 25 frame **450**: Plane B is settled,
the PSIV logo and subtitle are present, and the capture is within the fixed
title hold. The oracle reference is
`oracle/frames/title/frame_450.png`, SHA-256
`8cebd30d62a7b5b0c3ad634ec6efc5ab6ab62e088d6166835d447c843df18c10`.
The deterministic clone tick is **480** with `PSIV_DEBUG_TITLE_SHOT=1`:

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

The existing certified clone pair is tick **480** against frame 450 at RMSE
**0.000000**. The new Press Start palette-cycle code does not affect that
pre-prompt frame. A mandated post-change recapture is currently blocked: the
sandbox's Xvfb cannot bind its X11 socket (`/tmp/.X11-unix` is host-owned by
`nobody`, and a private namespace is denied `bind(2)`). A previous
Vulkan/live smoke capture measured **0.01995**, but it is explicitly
non-certification evidence and does not replace the certified pair.

## Opening cinematic boundary

`rust/psiv-core/src/scenes/game_start.rs` already transcribes the opening
event's movement/dialogue/map sequence. Its renderer-specific surface is now
consumed by the Godot cutscene layer: `InitVramAndCram`, the prologue title
image, `DrawTextToPlane`, the 20-frame colour ramps, and the two long text-page
holds. The ordered event seam, decoded assets, oracle tape, and current
coverage limits are recorded in [`SCENE_PRESENTATION.md`](SCENE_PRESENTATION.md).
CONTINUE still goes straight to the validated saved field state, as retail
does.
