# PSIV emulator oracle

The retail cartridge as a scriptable behaviour oracle: deterministic input
tapes in, per-frame named-RAM logs out, for bit-comparison against `psiv-core`
replaying the same inputs.

Built 2026-08-15. Every number in the reference ledgers is re-derived by
`./oracle/verify.sh`, which fails loudly rather than drifting.

## What this runs on, and why it is not BizHawk

**It runs the Genesis Plus GX libretro core inside a purpose-built headless
host** (`host/psiv_oracle.c`, ~900 lines). Same emulation core BizHawk's
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
├── host/psiv_oracle.c      the headless libretro host runner
├── host/frame_dump.c       negotiated-format PNG video sink
├── host/ram_dump.c         raw work-RAM snapshot sink
├── host/ram_patch.c/.h     explicit per-frame retail-RAM fixture writes
├── host/libretro.h         minimal libretro ABI subset
├── build_core.sh           fetches + builds the pinned emulation core
├── route.py                plans a walking route over the pack's collision data
├── navigate.py             closed-loop tape authoring (route + observe + re-plan)
├── analyze_rng.py          per-frame RNG call census from a log
├── anim_sweep.py           press-offset sweep across the dialogue open animation
├── damage_census.py        same-matchup damage samples across shifted seed paths
├── checks.py               shared helpers for verify.sh's two lanes
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
oracle/frames/
```

`host/`, `tapes/`, `ram_map.json`, `ram_map.tsv`, `gen_ram_map.py`,
`build_core.sh` and `verify.sh` are harness code and **are** committed.

## Running it

```sh
./oracle/build_core.sh     # once: clone + build Genesis Plus GX (pinned commit)
./oracle/verify.sh         # fast lane: ~3 min, run this routinely
./oracle/verify.sh --full  # everything, incl. the battle tapes: tens of minutes
```

**Two lanes.** The fast lane runs the structural checks and the short tapes in
about three minutes: determinism, both byte-order accessors, first
control, walk timing, talk behaviour, the RNG transcription and per-phase call
counts, the object slot mapping, the wander RNG rule, Alys joining, and the
beside press. The full lane adds everything that needs a battle - the two
encounters, the level-up grind, escape and defend. The split is by tape cost,
not importance: tape 10 alone is ~55k frames, and the battle tapes are where
all the time goes.

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

### Video frame capture

The host can save exact, player-visible reference frames directly from the
libretro video callback:

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/07_first_battle.tape \
    --out /dev/null \
    --dump-frames 25000,31000 \
    --dump-frames-dir oracle/frames
```

`--dump-frames` is a comma-separated list of explicit, 1-based
`retro_run()` frame numbers. `--dump-frames-dir` is required with it; the
host creates that directory when it is absent and writes
`frame_<N>.png`. The list may contain at most 256 unique positive numbers.
The host fails if a requested frame is never emitted, rather than silently
producing a stale image.

The core negotiates `RETRO_PIXEL_FORMAT_RGB565` for this build. The sink also
decodes libretro's `0RGB1555` and `XRGB8888` software formats, converting all
three to dependency-free 8-bit RGB PNGs. `retro_get_system_av_info()` reports
the core's reset-time geometry as **256x192**, then Genesis Plus GX switches
the VDP to the retail **320x224** viewport on the first frame; the sink
ignores that one reset-mode callback and rejects any dumped callback that is
not exactly 320x224. The observed callback pitch is 1440 bytes, and no
overscan/max-width pixels are included.

The first checked-in reference set is:

| file | source | depicted moment |
|---|---|---|
| `oracle/frames/frame_25000.png` | tape 07 | battle command menu idle (`COMD` selected), two Zoran Bults and the three-party status bar |
| `oracle/frames/frame_31000.png` | tape 07 | post-battle field message while the text is still mid-draw (`Stop wasting time, there's not`) |
| `oracle/frames/frame_7000.png` | tape 02 | Piata field, Chaz facing the town interior and its NPCs |
| `oracle/frames/frame_7400.png` | tape 03 | NPC dialogue window with the first page fully visible |

`--groups` selects which RAM-map groups are logged, and on a long tape it is
the difference between a 2MB log and a 60MB one. Available groups, with their
field counts:

| group | n | group | n | group | n |
|---|---|---|---|---|---|
| `core` | 9 | `party` | 8 | `chars` | 87 |
| `input` | 4 | `flags` | 12 | `objects` | 384 |
| `pos` | 12 | `rng` | 5 | `bcmd` | 27 |
| `pos2` | 3 | `battle` | 29 | `text` | 24 |
| `collision` | 7 | `bhit` | 19 | `camera` | 18 |
| `window` | 16 | `enemy` | 56 | `flagbytes` | 52 |

Omitting `--groups` logs all 772 fields.

`--dump-ram <frame>:<path>` writes all 64KB of work RAM at one frame, in 68000
byte order, so the file offset of a byte is its address minus `$FFFF0000`.
Diffing two dumps is the tool for "where did that write go" when the RAM map
does not already have a column for it — which is exactly how the chest flag
bank was found after a mapped-column search returned a confident wrong answer.

`--dump-state <frame>:<path>` writes the named plane/CRAM/sprite buffers plus
the scroll receipt used by `oracle/decode_layout.py`: camera position and step
counters, H-int state, the generated H-scroll work buffer, the
`Chunk_Table`/VSRAM-shadow source, VDP registers, VSRAM, and the active VDP
H-scroll table. The VDP words are emitted in Genesis big-endian order and the
state header names the address space and region byte order. The host resolves
the pinned Genesis Plus GX local VDP symbols from the loaded core ELF, so no
third-party core ABI or emulation behavior is modified.

For the MeetingRika receipt, add this flag to the Tape 28 command below:

```sh
--dump-ram 7250:/tmp/psiv-scroll-check.ram \
--dump-state 7250:/tmp/psiv-scroll-check.json
PYTHONPATH=. python3 oracle/decode_layout.py \
  /tmp/psiv-scroll-check.json --label meeting-rika-7250 \
  --output /tmp/psiv-scroll-check-layout.json --grand-cross 0
```

At frame 7250 the decoded receipt is: all camera and step words zero;
`HInt_Addr=0x00000758` (the retail RTE), H-int split disabled; VDP H-scroll
full-screen at `$F400` with zero Plane A/B columns; VSRAM Plane A/B zero; and
the generated `$FFFF60E0` H-scroll buffer zero. The `$FFFF6000` bytes are
reported as an inactive `Chunk_Table`/VSRAM-shadow source, not falsely called
live VSRAM. The window-region report is scanlines 160..223 and the placement
provenance records `grand_cross=0` plus the measured `(1,1)` plane residue.

### Deterministic scene fixtures

`--ram-patch <frame>:<68000-address>:<hex-bytes>` is an explicit oracle-fixture
operation. At the start of the named emulated frame it writes the bytes in
retail 68000 address order, validates that the address is inside the 64KB work
RAM and that the frame exists, and fails at the end if any requested patch was
not reached. It is not tape syntax and is not a general cheat interface.

Tape 28 uses this to enter the retail MeetingRika scene without pretending
that a power-on input tape naturally begins there:

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/28_meeting_rika_retail_probe.tape \
    --out /dev/null \
    --ram-patch 7000:FFFFEC28:00AC \
    --ram-patch 7000:FFFFEC2A:0000 \
    --ram-patch 7000:FFFFEC4E:02 \
    --ram-patch 7000:FFFFEF00:0008 \
    --ram-patch 7000:FFFFF406:01F0 \
    --ram-patch 7000:FFFFF408:01A0 \
    --ram-patch 7000:FFFFF40A:00010203 \
    --ram-patch 7200:FFFFECA8:8007 \
    --ram-patch 7200:FFFFEF00:000C \
    --dump-frames 7200,7250,7300,7400,7600 \
    --dump-frames-dir /tmp/psiv-meeting-rika-oracle
```

The observed retail outputs include the MeetingRika dialogue frame at 7200
(partially typed `Professor! Thank goodness y...`) and frame 7250 (the
fully-typed `Professor! Thank goodness you’re safe!`). Those are genuine
libretro video callbacks from the pinned ROM/core fixture; they are the oracle
side of the clone RMSE pair, not clone screenshots. In the final replay,
`frame_7200.png` hashed to
`5e166d2c0d985d5e073ded24b3862b404dd0ac55501b96fb145c4a100411b3c2` and
`frame_7250.png` hashed to
`d8fc26ae6987e416ee75c02cd10ea4975e9be22485b8feeda84161aa489888c9`.

### Vehicle tape 29

Tape 29 uses the same fixture discipline for the mounted field and battle
surface. The first patch starts the normal Motavia map loader and sets
`EventFlag_PrincipalConfession` (`$FFFFF101:08`) so the retail
`RunEvent_ReenterPiata` startup branch does not immediately redirect the test.
The map loader settles normally. At frame 7200 the fixture then installs the
Land Rover selector and object state at a clear Motavia anchor. At frame 7605
it moves that anchor over raw standing collision `9`, producing the real retail
`Cannot get off!` window. At frame 7830 the fixture enters the common
`FieldRoutine_Battle` path while the selector is still mounted; that last write
is an entry-surface probe, not a claim that a natural random encounter happened
at that exact frame.

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/29_vehicle_land_rover_probe.tape \
    --groups core,pos,collision,vehicle,battle \
    --out /tmp/vehicle29-mounted-battle.csv \
    --ram-patch 7000:FFFFEC28:0000 \
    --ram-patch 7000:FFFFEC2A:FFFF \
    --ram-patch 7000:FFFFEF00:0008 \
    --ram-patch 7000:FFFFF400:0000 \
    --ram-patch 7000:FFFFECA8:FFFF \
    --ram-patch 7000:FFFFF101:08 \
    --ram-patch 7000:FFFFEC44:0000 \
    --ram-patch 7000:FFFFEC46:0000 \
    --ram-patch 7000:FFFFEC48:0124 \
    --ram-patch 7000:FFFFEC4A:0040 \
    --ram-patch 7200:FFFFEC20:0000 \
    --ram-patch 7200:FFFFF43C:0001 \
    --ram-patch 7200:FFFFC000:00B0 \
    --ram-patch 7200:FFFFC030:0920 \
    --ram-patch 7200:FFFFC032:0000 \
    --ram-patch 7200:FFFFC034:0200 \
    --ram-patch 7200:FFFFC036:0000 \
    --ram-patch 7200:FFFFC038:0920 \
    --ram-patch 7200:FFFFC03A:0200 \
    --ram-patch 7605:FFFFC030:0940 \
    --ram-patch 7605:FFFFC034:0480 \
    --ram-patch 7605:FFFFC038:0940 \
    --ram-patch 7605:FFFFC03A:0480 \
    --ram-patch 7830:FFFFEC20:0014 \
    --dump-frames 7605,7726,7830,7900 \
    --dump-frames-dir /tmp/psiv-vehicle29-oracle
```

The pinned replay produced the following state and video receipts:

| frame | receipt |
|---:|---|
| `7200` | `Vehicle_Index=1`, position `(2336,512)`, standing collision `0` |
| `7201` | right input advances four pixels, confirming 4 px/frame and an eight-frame 32-pixel step |
| `7300` | the vehicle remains at the solid boundary rather than crossing it |
| `7605` | standing collision is raw `9`; the selector remains mounted |
| `7726` | retail PNG visibly reads `Cannot get off!` |
| `7830` | `Game_Mode_Routine=14` while `Vehicle_Index=1` |
| `7850` | battle game mode is active with the mounted vehicle fighter |
| `7900` | battle routine has initialized three enemies while the vehicle remains selected |

The dumped PNG SHA-256 values from the final replay are:

```text
frame_7605.png  2cb50dd71b656517e1f424fe3a497ad429f06f9a504018079ac86fc199fcbc61
frame_7726.png  72928413566ced72c3bcfe626d55ada7e34c228e9feb0a61843045f04025e84
frame_7830.png  498ce5848d85c752bae7ead67d022079044dc355ef8bc429eb1c5de3dd866e9f
frame_7900.png  bc52c0c9edd58cefa3c17816ed999c5547513595e95883f4f07ee1a9bab483cb
```

A replay with the frame-7605 bad-terrain patches omitted dismounts by frame
8125. That is the success-side check for the same retail `Event_GettingOffVehicle`
path; the refusal and success cases are not inferred from the clone.

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
both paths against values the cartridge itself chose (see `RESULTS.md`).

## Tapes

| tape | what it proves |
|---|---|
| `01_newgame_to_first_control.tape` | the boot path is reachable deterministically from power-on; new-game starting state |
| `02_walk_timing.tape` | frames per cell, the four facing values, blocked-press behaviour |
| `03_npc_talk.tape` | talk range, dialogue open/close timing, text draw rate |
| `04_alys_joins.tape` | the first story gate: Alys joins and takes slot 1 |
| `05_principal_assignment.tape` | the principal's assignment scene (event `$8001`) |
| `06_basement_quest.tape` | the basement-stair NPCs, event `$0004`, Hahn joins |
| `07_first_battle.tape` | the first random encounter and the fight through to victory |
| `08_rng_characterization.tape` | RNG call counts per frame across title, field, walking and menu |
| `09_second_battle.tape` | a second encounter on a different seed path, including a critical hit |
| `10_levelup.tape` | three encounters back to back; Chaz reaches level 2 |
| `11_beside_press.tape` | Speak at an NPC that is adjacent but not in front |
| `12_escape.tape` | a successful escape attempt |
| `13_open_anim_press.tape` | Speak pressed inside the window-open animation |
| `14_defend.tape` | the DEFEND command and its `physical_prop` clobber |
| `15_text_hold.tape` | text draw acceleration while Speak is held |
| `16_page_boundary.tape` | holding Speak across the end of a page |
| `17_flag_alias.tape` | where a "temp" event flag write lands, on hardware |
| `18_flag_round_trip.tape` | that bit across a leave-and-reenter round trip |
| `19_chest_map_objects.tape` | object slots on a chest-bearing map, and opening a chest |
| `20_chest_round_trip.tape` | re-pressing an opened chest, and whether it is still open after leaving and returning |
| `21_second_chest.tape` | a second chest id, which is what pins the flag bank's bit arithmetic |
| `28_meeting_rika_retail_probe.tape` | power-on schedule plus explicit RAM patches for deterministic MeetingRika video frames |
| `29_vehicle_land_rover_probe.tape` | retail field boot plus a documented Land Rover movement, dismount-refusal and mounted-battle fixture |

`prelude_basement.tape` is a generated intermediate (`navigate.py` output) that
both battle tapes are built from; `find_battle.py` consumes it.

Tape A's presses are placed from the disassembly, not by trial: `GameMode_Title`
begins at f226; `TitleRoutine_FadingText` takes a START to skip; then
`TitleRoutine_PressStartButton` polls `Joypad_Pressed` for START over a
`$233` = 563 frame window; then, with empty SRAM, `CheckPS4String` fails and
`Title_NoSavedData` opens window 4 and waits for Speak/Camp/Start.

Tape A advances the opening scene with **Cancel (B)**, deliberately. Cancel
advances dialogue pages but does nothing once field control returns, so
presses that overrun the end of the scene cannot perturb the state being
measured. Using Speak here would fire a Talk on the first frame of control.


## Results and reference

The operational README stays short enough to audit at a glance. The detailed
oracle findings are split into two reference ledgers so each file remains
under the repository's 1,000-line maintenance limit:

- [`RESULTS.md`](RESULTS.md) — determinism, field/talk/RNG/battle evidence,
  and the chest/object findings through the second chest.
- [`RESULTS_CONTINUED.md`](RESULTS_CONTINUED.md) — the retracted flag-bank
  note, camera/object observations, story gates, RAM notes, tape-authoring
  guidance, open questions, and comparator feeding.

Regenerate the ledgers from the pinned ROM/core with `./oracle/verify.sh`;
the current operational findings remain the ones that script checks.
