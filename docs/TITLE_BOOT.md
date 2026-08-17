# Retail title and boot front door

This slice records the retail boot surface without committing ROM-derived
pixels. The committed oracle is [`oracle/tapes/25_title_boot.tape`](../oracle/tapes/25_title_boot.tape). The decoded JSON, reference PNGs, and runtime pack
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
The extracted PNGs bake the settled `Pal_TitleScreen`; Godot's current fade is
an alpha/black phase treatment, while `palette_cycle.json` is the complete
numeric contract for a future per-entry CRAM replay.

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
same validated `Runtime::load_slot` path used by boot; it never writes save
data. It exposes a slot picker after CONTINUE, refuses empty slots, and keeps
ERASE DATA non-destructive until save-erasure ownership is explicitly added.

`rust/psiv-godot/src/title.rs` renders the decoded assets at the 3x camera
scale and drives Sega -> reveal -> Press Start -> menu. `boot.rs` preserves
the fast paths for `PSIV_LOAD_SLOT`, `PSIV_DEBUG_BATTLE`, `PSIV_DEBUG_CAMP`,
`PSIV_DEBUG_SHOP`, and `PSIV_DEBUG_SHOT`; those selectors never wait behind the
retail front door. For visual QA only, `PSIV_DEBUG_TITLE_SHOT=1` opts an
existing `PSIV_DEBUG_SHOT` capture into the title; ordinary debug screenshots
continue to bypass the front door.

## Opening cinematic boundary

`rust/psiv-core/src/scenes/game_start.rs` already transcribes the opening
event's movement/dialogue/map sequence. The missing presentation surface is
the renderer-specific part: `InitVramAndCram`, the two prologue title-image
loads, `DrawTextToPlane`, the 20-step colour ramps, and the two long text-page
holds. START therefore releases the title overlay to the existing new-game
runtime in this slice; wiring those operations and proving their timing
against a retail tape is the next opening-cinematic slice. CONTINUE goes
straight to the validated saved field state, as retail does.
