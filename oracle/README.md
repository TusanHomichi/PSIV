# PSIV emulator oracle

The retail cartridge as a scriptable behaviour oracle: deterministic input
tapes in, per-frame named-RAM logs out, for bit-comparison against `psiv-core`
replaying the same inputs.

Built 2026-08-15. Every number in the Results section is re-derived by
`./oracle/verify.sh`, which fails loudly rather than drifting.

## What this runs on, and why it is not BizHawk

**It runs the Genesis Plus GX libretro core inside a purpose-built headless
host** (`host/psiv_oracle.c`, ~700 lines). Same emulation core BizHawk's
Genesis support is built on, no GUI anywhere in the loop.

BizHawk was tried first and was made to work far enough to prove it was the
wrong tool here:

- BizHawk 2.11.1's Linux build is .NET Framework 4.8 targeted and runs under
  **Mono**, which Fedora 44 packages (`mono-core` 6.14.1) but which cannot be
  installed without root on this machine.
- It was nonetheless brought up: the Mono RPMs were unpacked into a user
  prefix and mounted over `/usr` and `/etc` with a `bubblewrap` overlay in a
  user namespace. `mono --version` ran, and EmuHawk got as far as loading its
  own assemblies and initialising WinForms.
- At that point the stack for a *determinism* tool was: extracted RPMs, a
  namespace overlay, Xvfb, Mono, WinForms, a GUI event loop, and Lua driving
  input injection — six layers of scaffolding between the tape and the CPU,
  every one of them a place for a frame to slip.

The libretro host replaces all of it. It has no window, no audio device, no
timer, and no frame pacing; it calls `retro_run()` in a loop and reads
`retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM)` directly. Determinism is
structural rather than configured, RAM access is a raw pointer instead of a
scripting-language bridge, and the whole thing builds with `gcc` and `-ldl`.
The Mono/BizHawk scaffolding was deleted after the experiment; nothing here
depends on it.

The tradeoff is that there is no Lua console for ad-hoc poking. The RAM map
and the tape format are data files instead, which is what the comparator
wants anyway.

## Layout

```
oracle/
├── host/psiv_oracle.c      the headless libretro host
├── host/libretro.h         minimal libretro ABI subset
├── build_core.sh           fetches + builds the pinned emulation core
├── core/…_libretro.so      built core (not committed)
├── gpgx-src/               core source checkout (not committed)
├── ram_map.json            RAM map, source of truth, with disassembly cites
├── gen_ram_map.py          emits ram_map.tsv from the json
├── ram_map.tsv             generated flat map the C host reads
├── tapes/*.tape            input tapes
├── logs/*.csv              output logs (not committed)
└── verify.sh               the verification run
```

### `.gitignore`

Add to the repo's `.gitignore` — the core source and binary are third-party
and large, and the logs are regenerable Sega-derived output:

```
oracle/gpgx-src/
oracle/core/
oracle/logs/
oracle/bin/
```

`host/`, `tapes/`, `ram_map.json`, `ram_map.tsv`, `gen_ram_map.py`,
`build_core.sh` and `verify.sh` are harness code and **are** committed.

## Running it

```sh
./oracle/build_core.sh     # once: clone + build Genesis Plus GX (pinned commit)
./oracle/verify.sh         # build the host, prove determinism, re-derive findings
```

One tape by hand:

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/02_walk_timing.tape \
    --groups core,pos,collision \
    --out  oracle/logs/walk.csv
```

Other flags: `--dump-options` lists every option the core declares with its
default; `--probe-endian` prints work-RAM diagnostics.

`--groups` selects which RAM-map groups are logged. Available groups: `core`,
`input`, `pos`, `pos2`, `collision`, `window`, `party`, `flags`, `rng`.
Omitting `--groups` logs all 66 fields.

## Tape format

Line-oriented, one step per line:

```
<frames> <buttons> [mark]
```

- `frames` — how many consecutive frames to hold this state, at least 1.
- `buttons` — `.` for nothing, otherwise letters from `UDLRABCS`
  (up/down/left/right, Genesis A, B, C, Start). `A`, `B` and `C` are the
  physical Genesis buttons; see the button mapping note below.
- `mark` — optional label, recorded in the log on the step's first frame.
  Marks are how findings get cited: every frame number in this document is a
  mark or an offset from one.

A run of steps repeats with a block (blocks do not nest):

```
repeat 336
4 C
12 .
end
```

Blank lines and `#` comments are ignored. **Playback always starts from
power-on**, so a tape plus a pinned core build fully determines the run; there
are no savestates in the loop.

### Button mapping (this bit is a trap)

The disassembly names buttons by *function*, and the mapping to physical
Genesis buttons is not the obvious one:

| Genesis button | joypad bit | disassembly name | field effect |
|---|---|---|---|
| B | 4 (`$10`) | `ButtonCancel` | nothing in field control |
| C | 5 (`$20`) | `ButtonSpeak`  | Talk — `FieldRoutine_Interaction` |
| A | 6 (`$40`) | `ButtonCamp`   | opens the camp menu |
| Start | 7 (`$80`) | `ButtonStart` | settings window |

(`ps4.constants.asm:1877-1892`; confirmed by pressing each in turn and
watching `Game_Mode_Routine` go to 8, 4, and $10 respectively.)

Genesis Plus GX maps libretro `Y`/`B`/`A` onto Mega Drive A/B/C, which the
host handles; tapes are written in Mega Drive letters.

## Log format

CSV with three provenance comment lines, then `frame,mark,buttons` and one
column per enabled RAM field. `frame` is 1-based and counts `retro_run()`
calls from power-on. Values are decimal, or fixed-width hex for fields flagged
`hex` in the map.

## RAM map

`ram_map.json` is the source of truth; **every address is transcribed from
`reference/ps4disasm/ps4.constants.asm` and carries the line it came from**.
Nothing was inferred by watching memory. Struct fields are recorded as base +
offset with both citations so the arithmetic is auditable — for example
`c1_facing` is `Character_1` (`constants:2086`, `$FFFFC000`) plus `facing_dir`
(`constants:107`, `+6`).

`gen_ram_map.py` emits the flat `ram_map.tsv` the C host parses, validating
sizes, alignment and address range on the way. `verify.sh` regenerates and
diffs it, so the two cannot drift apart silently.

### Work-RAM byte order

Genesis Plus GX builds with `-DLSB_FIRST`, which stores `work_ram` in
**host-native 16-bit word order**. The 68000 core reads bytes as
`READ_BYTE(base, addr) == base[addr^1]` and words as
`*(uint16 *)(base + addr)` (`core/macros.h`, `core/m68k/m68kcpu.h:854-882`).
So:

- byte at 68000 address `A` → `ram[A ^ 1]`
- word at even `A` → little-endian `uint16` load at `ram + A`
- long at even `A` → `word(A) << 16 | word(A + 2)`

Get this backwards and every log line is quietly wrong, so `verify.sh` proves
both paths against values the cartridge itself chose (see Results).

## Tapes

| tape | what it proves |
|---|---|
| `01_newgame_to_first_control.tape` | the boot path is reachable deterministically from power-on; new-game starting state |
| `02_walk_timing.tape` | frames per cell, the four facing values, blocked-press behaviour |
| `03_npc_talk.tape` | talk range, dialogue open/close timing, text draw rate |

Tape A's presses are placed from the disassembly, not by trial: `GameMode_Title`
begins at f226; `TitleRoutine_FadingText` takes a START to skip; then
`TitleRoutine_PressStartButton` polls `Joypad_Pressed` for START over a
`$233` = 563 frame window; then, with empty SRAM, `CheckPS4String` fails and
`Title_NoSavedData` opens window 4 and waits for Speak/Camp/Start.

Tape A advances the opening scene with **Cancel (B)**, deliberately. Cancel
advances dialogue pages but does nothing once field control returns, so
presses that overrun the end of the scene cannot perturb the state being
measured. Using Speak here would fire a Talk on the first frame of control.

## Results

Frame numbers below cite marks in the named log; regenerate with `verify.sh`.

### Determinism

Tape A run twice from power-on produces **byte-identical** logs,
`sha256 89e0f2871c9f4400a34364b2a2ec99c5b55cb8051343fbab4ba9115c86ddde5e`.
Pinned in `verify.sh`.

Core: **Genesis Plus GX v1.7.4**, libretro build, commit
`2d7131c5efa606f649d36e1685a8ca47c24f31b3`, region NTSC.

Core options are pinned explicitly in `host/psiv_oracle.c` and **validated
against the core's declared value list at startup, aborting on a mismatch**.
That guard exists because a wrong value is otherwise silent: Genesis Plus GX
parses its overclock option with `atoi()`, so an invalid `"1x"` became a 1%
clock divisor and produced a log full of plausible-looking zeroes rather than
an error. Anything not pinned falls through to the core's compiled-in default,
which is fixed for a fixed core build; `--dump-options` prints the full set.

### New-game starting state (answers the overworld lane)

**First sustained `FieldRoutine_Controls` frame: 6456** (`logs/01_newgame.csv`,
CSV line 6461).

| | |
|---|---|
| map | `$13` = `MapID_PiataAcademy_F1` (`constants:1086`) |
| position | (768, 288) px = **cell (48, 18)** |
| facing | 0 = DOWN |
| party | `Current_Party_Slots` = `00FFFFFF` — **Chaz alone**, slots 2-5 empty (`$FF`) |
| world | 0 (Motavia) |

The opening runs `Event_GameStart` (`$9F`) across maps `$11` → `$5E` → `$54`
→ `$00` → `$13`, then `Event_PiataChazAlone` (`$A0`) plays unattended for 226
frames (f6230-6455) before handing over.

Character IDs observed during the intro: **Chaz = 0, Alys = 1**. Party slots
pass through `0001FFFF` (Chaz, Alys) and then `0100FFFF` — Alys in slot 1 —
which corroborates the `Event_AlysFound` note in `EVENT_ENGINE_SCOUT.md` that
**Alys leads the opening party**. By first control she has left and Chaz is
alone.

The intro genuinely requires input: with no presses at all it never leaves map
`$11`.

### Walk timing

**Exactly 8.00 frames per 16px cell, all four directions** (`logs/02_walk_timing.csv`).
Position advances 2 px/frame; `x_step_duration` / `y_step_duration` count down
from `$1000` to 0 in steps of `$0200`, which is 8 states. This confirms the
8-frames-per-cell figure `RUNTIME_DESIGN.md` expected.

Facing values confirmed as `constants:107` states: 0 DOWN, 4 UP, 8 RIGHT,
`$C` LEFT.

**Any direction press commits a full 16px step — there is no turn-in-place.**
A 1-frame tap moves a whole cell (`walk_up` leg; a 1-frame LEFT tap at f7185
had already moved 2px on the press frame and completed the cell 8 frames
later). A character only turns without moving when the target cell is blocked.
This is a real constraint on scene authoring and on any "face this way" op.

### Collision

Blocked presses turn the character to face the obstacle and hold position.
Terrain blocking shows up as collision type `08` in the direction field
(`Tile_Collision_*`), matching the solid type in `RUNTIME_DESIGN.md`.

**Field objects block movement independently of the terrain collision grid.**
Walking UP from the start cell stops at (752, 160) with `coll_up` reading
`00` — walkable terrain — because the NPC at (752, 144) is in the way. The
collision array describes terrain only, so `psiv-core` needs a separate
object-occupancy test; a pure grid lookup will let the player walk through
NPCs.

### Talk range: facing only, not beside

**Settled, and it contradicts the remembered behaviour.**
`Interaction_ChkObjects` (`ps4.asm:118729`) builds a probe point from the
player's position projected to its step destination, plus a one-cell offset in
the facing direction from `FieldObj_MovementsTbl`, then accepts an object only
if both `|dx|` and `|dy|` are `<= $80000`. In the 16.16 fixed point these
positions use, `$80000` is **8.0 pixels**. The accept box is therefore 16×16
px — exactly one cell — centred one cell ahead of facing.

An NPC directly beside the player is 16 px off-axis, which fails the test.
**Standing beside an NPC and pressing Speak cannot talk to it.** The
transcription already in `RUNTIME_DESIGN.md` ("projects one cell ahead of the
party's facing (±8px)") is correct as written; the recollection of beside-talk
is not supported by the cartridge.

Empirically confirmed on the facing side: Speak while facing an adjacent NPC
opens the dialogue (`logs/03_npc_talk.csv`, mark `speak`). A fully clean
beside-adjacent-but-not-facing press could not be staged on this map, because
committing a full step on every direction press makes it hard to turn while
staying adjacent — see Still open.

### Dialogue timing

From `logs/03_npc_talk.csv`:

- Speak pressed f7185 → `Game_Mode_Routine` = 8 (`FieldRoutine_Interaction`).
- f7187 `Window_Render_Mode` = `$0600` — the open animation starts.
- f7196 `Windows_Opened_Num` = 1, render mode back to 0. **Window open takes
  9 frames; 11 frames from press to open.**
- The window then stays open with no input for the full 300-frame observation
  window. **The final page does not auto-close; it requires a press.**
- A press closes it over 9 frames (f7793 → f7802) and control returns at
  f7803.

**Text draws progressively at one character every 3 frames.** Sampling
`Win_Tile_Buffer` (`$FFFFE220`, `constants:2106`) every frame during the
dialogue shows writes at f7197, 7200, 7203, 7206, 7209, … in a strict 3-frame
cadence, with occasional 6-frame gaps (consistent with `$F9` pauses or line
breaks). At 60 Hz that is **20 characters per second**.

This contradicts the current implementation: `RUNTIME_DESIGN.md` records
dialogue text as instant, with `$F9` treated as the only pause. Retail is a
3-frame-per-character typewriter. The cartridge outranks the doc.

## Still open

- **Beside-talk empirical confirmation.** Answered from code above with a
  citation, and the code is unambiguous, but no tape yet stages the exact
  adjacent-not-facing press. It needs a map spot where the cell the player
  would turn toward is blocked while an NPC sits perpendicular. Worth one
  targeted tape to close the loop.
- **Accept press during the window-open animation.** Untested. The 9-frame
  open animation (f7187-7196) is now a known window, so the tape is easy: press
  Speak inside it and see whether the page advances immediately on completion
  (buffered) or not (dropped).
- **Multi-page dialogue boundaries.** The talk in tape C spans two pages, but
  no logged field distinguishes page N from page N+1 — the first advance press
  changed nothing in the RAM map. Finding the page-index variable would let
  tapes assert per-page behaviour.
- **Scroll-arrow art.** `Text_Scroll_Arrow` (`$FFFFC2C0`) is in the map and its
  `offscreen_flag` is logged, but the arrow's on/off timing was not
  characterised.
- **Battle and RNG.** Untouched. `RNG_Seed` (`$FFFFEF0C`) is already in the
  map and updates every frame, so an RNG-stream comparison is ready to build.

## Feeding a comparator

The intended shape for comparing against `psiv-core`:

1. `psiv-core` gains a replay entry point taking the same tape file and
   emitting the same CSV columns — per frame, its own values for the mapped
   fields.
2. Comparison is a column-wise diff keyed on `frame`. Because the oracle log
   is byte-reproducible, a golden log can be committed as a fixture and the
   core checked against it without an emulator in CI.
3. Not every column is a fair target. `rng_seed` and the raw `game_mode_*`
   dispatch values are cartridge implementation detail; the meaningful
   comparison set is position, facing, step durations, collision, party slots,
   event flags, and window open/close frames. Suggest declaring a per-tape
   column subset in the fixture rather than diffing everything.
4. Frame alignment is exact — both sides count frames from power-on and the
   tape is the same file — so a mismatch is a real behavioural divergence, not
   drift.

The RAM map is deliberately readable from Rust: `ram_map.json` carries the
addresses, sizes and groups, so `psiv-data` can load the same file the oracle
uses instead of re-declaring the field list.
