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
├── host/rng_trace.c/.h     battle-roll capture (included by psiv_oracle.c)
├── host/ram_patch.c/.h     explicit per-frame retail-RAM fixture writes
├── host/libretro.h         minimal libretro ABI subset
├── build_core.sh           fetches + builds the pinned emulation core
├── patches/*.patch         our patches to that checkout, applied in order
├── route.py                plans a walking route over the pack's collision data
├── navigate.py             closed-loop tape authoring (route + observe + re-plan)
├── analyze_rng.py          per-frame RNG call census from a log
├── rng_trace.py            checks a --rng-trace capture against its log
├── force_battle.py         forces a chosen formation into a battle and captures it
├── fixture/                the replay-fixture extractor battle_fixture.py drives
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

`--dump-state <frame>:<path>` (up to 128 frame receipts per oracle run) writes
the named plane/CRAM/sprite buffers plus
the scroll receipt used by `oracle/decode_layout.py`: camera position and step
counters, H-int state, the generated H-scroll work buffer, the
`Chunk_Table`/VSRAM-shadow source, VDP registers, VSRAM, and the active VDP
H-scroll table and a raw `vdp_vram` 64 KiB receipt. The VDP words are emitted in Genesis big-endian order and the
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

### RNG trace: the cartridge's own battle rolls

PSIV's battle rolls are not pseudo-random in the usual sense. `UpdateRNGSeed2`
(`ps4.asm:86097`, ROM `$04239E`) is four instructions:

```
	move.w	$8(a5), d0		; the VDP HV counter at $C00008
	add.w	(Main_Frame_Count).w, d0
	sub.w	(RNG_Seed).w, d0	; d0 = the roll the caller receives
	ror	(RNG_Seed).w		; and the seed's high word rotates
```

so every roll is a function of *where the beam was* when the 68000 read the
counter. That is hardware timing `psiv-core` deliberately does not reproduce
(see `docs/RUNTIME_DESIGN.md`, "RNG design"); the port takes rolls through its
`Rolls` trait instead, and this flag captures the stream that trait replays.

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/07_first_battle.tape \
    --groups core,battle,bhit,enemy,chars,rng,vehicle \
    --rng-trace oracle/logs/tape07_rolls.csv \
    --out  oracle/logs/tape07_battle.csv
python3 oracle/rng_trace.py check oracle/logs/tape07_rolls.csv \
    oracle/logs/tape07_battle.csv
python3 oracle/battle_fixture.py --trace oracle/logs/tape07_rolls.csv \
    --log oracle/logs/tape07_battle.csv \
    --out rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- tape07
```

The last two steps are the replay: `oracle/battle_fixture.py` (the CLI of the
`oracle/fixture/` package) writes the battle's start state, its rolls with the
frame and role of each, and what the RAM log shows every action doing, and
`psiv-core` replays it - `rust/psiv-core/src/battle/replay/`, whose
`data.rs` is the one test that replays **every** fixture in
`rust/psiv-core/src/battle/replay_fixtures/` and holds each one that does not
match to its entry in `divergences.json`; the verdicts are in
[`BATTLE_ORACLE_REPLAY.md`](../docs/BATTLE_ORACLE_REPLAY.md) and
[`BATTLE_ORACLE_FORCED.md`](../docs/BATTLE_ORACLE_FORCED.md).

The host writes one row per call, in frame order:

| column | meaning |
|---|---|
| `frame` | emulated frame the call happened in (1-based, `retro_run()` count) |
| `call_index_in_frame` | 0-based index of this call within its frame |
| `pc` | 68000 program counter the core reported for the HV read |
| `hv` | the HV word the counter returned for that read |
| `frame_count` | `Main_Frame_Count` (`$FFFFEF1C`) the instructions saw |
| `seed_before` | `RNG_Seed` (`$FFFFEF0C`) longword the instructions saw |
| `roll` | `(hv + frame_count - seed_high) & $FFFF`, the value left in `d0`: the subtrahend is the word at `$FFFFEF0C`, the longword's high half |
| `seed_after` | the seed after `ror (RNG_Seed).w`, low word carried |

**The core patch.** `oracle/patches/0001-rng-hv-trace.patch` is ours; upstream
Genesis Plus GX has no such hook. It makes `vdp_hvc_r()` record `(pc, hv)` for
every HV counter read and exports
`psiv_hv_trace_enable/reset/count/dropped/get`, which the host resolves with
`dlsym` and drains once per frame. With tracing off the record is one
predictable branch and the value is returned unchanged, so emulation is
identical - `verify.sh` runs against the patched core and still passes.
`build_core.sh` applies every `oracle/patches/*.patch` after the pinned
checkout, in file-name order; re-applying is a no-op, and a tree that is not
the pinned revision fails loudly instead of building something unreproducible.
See [`patches/README.md`](patches/README.md) for what those patches must obey.
A checkout shipped without `gpgx-src/.git` (how a built one travels) is used as
it stands, so the build works offline: the patches are then what pins the files
this project depends on, and any other revision fails the build rather than
producing numbers nothing was measured against.

**Which reads are calls.** The trace keeps the reads the PC says came from
`move.w $8(a5),d0` at `$04239E`. Genesis Plus GX reports the counter with the
instruction's extension word already fetched, so the access arrives as
`$0423A2` - the address of the `add.w` that follows it. The accepted window is
`$04239E-$0423A6`, from the read itself to the start of the `sub.w (RNG_Seed).w`
two instructions later, so a core that reports the PC at another point inside
or just after the access still matches; neither of the other two instructions
reads the counter, so the window cannot admit a record that is not one of
these. Every matched row carries the PC it matched on, which keeps the choice
visible in the data. Tape 07 has no other HV reader at all - all 136 records
match - so nothing there rests on the window's width.

**Why those are the seeds.** `RNG_Seed` and `Main_Frame_Count` are not in the
core's records: the host reads them from work RAM around each frame and chains
the frame's calls, because `UpdateRNGSeed2` only rotates the high word, so the
next call starts from the word the previous one left. Two things can move the
seed between calls, and both are accounted for:

- The VBlank handler (`ps4.asm:612-625`) applies `UpdateRNGSeed` (the 41x
  multiply) and bumps `Main_Frame_Count` in one block, so a frame whose counter
  steps by exactly one is a frame whose rolls chain from the multiplied seed.
  That is the whole rule, and it is what the log's counter column says.
- Genesis Plus GX triggers VINT at the top of the emulated frame
  (`core/system.c`, `system_frame_gen`: the VCount is set to
  `bitmap.viewport.h` and the interrupt is taken before the vblank and visible
  lines run), so that block precedes every read of the frame rather than
  following the frame's last call. Tape 07's rolls sit in the visible lines
  (V counter `$14-$55`), where the game's attack code runs.

**What `rng_trace.py check` proves.** It re-derives each row's `seed_after` and
insists that a frame's calls chain into each other, that its first call starts
from the log's seed for the frame before it or from that seed after one
`UpdateRNGSeed`, that its last `seed_after` is the log's `rng_seed` for the
frame, and that every row's `frame_count` is that frame's logged
`Main_Frame_Count`; the anchor and the counter step must agree. It prints the
first mismatch and exits non-zero, and the host refuses to call the run
trustworthy for the same reason before that. What stays unproven is the
`frame_count` column's *timing* - it is the frame's value sampled after the
frame, justified by the frame order above rather than by the chain - and any
frame the log does not cover, which is counted and reported as skipped. Playing
these rolls back through `psiv-core`'s damage path against the same battle in
the log is the end-to-end check that closes that gap, and
[`BATTLE_ORACLE_REPLAY.md`](../docs/BATTLE_ORACLE_REPLAY.md) is that check for
tapes 07 and 09.

**The `roll` column, and the word it subtracts.** `sub.w (RNG_Seed).w, d0` at
`$0423A6` reads the word at `$FFFFEF0C`, which on a big-endian 68000 is the
**high** half of the `RNG_Seed` longword (`ps4.constants.asm:2328`) - the same
word `ror (RNG_Seed).w` at `$0423AA` rotates. The host's `rng_trace_roll`
(`oracle/host/rng_trace.h`) subtracts that word, and `oracle/rng_trace.py`'s
`roll_for` re-derives the same arithmetic, as `oracle/battle_fixture.py`
insists row by row: a capture whose column subtracts the low half at
`$FFFFEF0E` is rejected with the frame and call of the first row that does,
rather than replayed.

That was not always so, and the capture was wrong for a while. O1 built the
host with `seed_lo`, O2 found the column was a per-frame-constant shift of the
cartridge's rolls, and `rng_trace.py check` could not see it because its own
`roll_for` repeated the same subtraction - the two agreed with each other and
with nothing else. The cartridge settled it against tape 07's RAM log: the
high-half derivation reproduces the battle's nine turn-order addends and all
six of its damage values, the low-half one reproduces none of them
([`BATTLE_ORACLE_REPLAY.md`](../docs/BATTLE_ORACLE_REPLAY.md)). The fix landed
in the host and in the checker, the capture was regenerated, and the two can no
longer drift apart unnoticed: `tests/test_oracle_rng_trace.py` compiles
`oracle/host/rng_trace.h` into a probe and compares it with the checker's
`roll_for` against numbers written out from the disassembly, and
`oracle/battle_fixture.py` refuses a trace whose column is not the cartridge's.

Captured with the low-half subtraction by an otherwise identical host, the same
tape and core differ in the `roll` column alone, and in all 136 rows: `hv`,
`pc`, `frame_count`, `seed_before` and `seed_after` come out the same in both,
each roll's shift is exactly `seed_lo - seed_hi`, and the seed chain still
closes on the RAM log exactly.

The capture on tape 07: 136 rolls in 15 frames, between frames 24807 and 30306
of the battle at 24794-30428, every one of them in the visible lines. The
counts line up with the disassembly: single rolls are `Battle_CalculateChances`
(`ps4.asm:17339`, one call), runs of 16 are `Battle_CalculateDamage`'s loop
(`ps4.asm:17381`, `moveq #$F,d7` with `dbf d7,-`), and the frame where two
attackers each take a damage roll carries two of those runs (32 calls). Frame
29711, first two of its sixteen rows:

```
29711,0,0423A2,2292,28815,21E817F3,7139,10F417F3
29711,1,0423A2,23F3,28815,10F417F3,838E,087A17F3
```

(The same two rows from a host built with the pre-fix subtraction read `7B2E`
and `7C8F`, `seed_after` included - the low half's subtraction of the same raw
columns, `$2292 + 28815 - $17F3` and `$23F3 + 28815 - $17F3`. The columns differ
in `roll` alone: `7139` = `$2292 + 28815 - $21E8` and `838E` = `$23F3 + 28815 -
$10F4`.)

Tape 09's capture is the same three steps on its own tape and window: 137 rolls
in 16 frames, between frames 25015 and 31786 of the battle at 25002-31908, all
of them in the visible lines, with its fixture extracted by `--tape`,
`--battle-first` and `--battle-last` and nothing else. Both traces' pins, both
fixtures' provenance and both replays' draw accounting are in
[`BATTLE_ORACLE_REPLAY.md`](../docs/BATTLE_ORACLE_REPLAY.md).

### Forced battles: capturing any formation on demand

A tape can only meet the formations its own RNG path draws, which is a problem
when the ability under test belongs to an enemy the tapes never reach.
`oracle/force_battle.py` forces one:

```sh
python3 oracle/force_battle.py --formation 0x5E --out build/forced/helex \
    --require-ability 2
```

Its runs log `core,battle,bhit,enemy,chars,rng,vehicle` - the vehicle group
because a vehicle battle's party side is built from `Vehicle_Stats` and its
saved record, which a fixture's `vehicle` section reads. It takes a base tape
whose field prefix walks into an encounter (tape 07's, by default), cuts it at
the frame that encounter fires on, and drives the fight with a fixed input
policy (`attack`: one `C` press every 16 frames, tape 07's
own; `defend` is tape 14's menu walk). Two things decide which formation the
load builds, and the tool forces both: the *group* (the map's encounter byte, a
position-grid cell, or a vehicle table) with one patch a frame after the
encounter fires, and the *entry* within the group's 32 formation ids by
patching `RNG_Seed`'s high word - one frame before the formation draw - to the
value that makes `UpdateRNGSeed2`'s roll land where it should.

Five oracle runs per capture: a scout, a probe that measures the draw, an
untrimmed preview, the trimmed capture, and a re-run to byte-compare it, with
`oracle/rng_trace.py check` on the result. The output directory holds the
composed tape, the `--ram-patch` list, every run's log and trace, and a
`report.json` with the window, the outcome, the ability ids observed and every
sha256. [`BATTLE_ORACLE_FORCED.md`](../docs/BATTLE_ORACLE_FORCED.md) is the
ledger: the mechanism with its citations, three captures (Helex/FLAME BOLT,
Fanbite/SPIRAL BLD, Desrt Leach/SAND STORM) and their limits.

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

### Natural tape 31: verified prefix

`oracle/tapes/31_natural_land_rover.tape` is the natural-input companion to
tape 29. It starts at power-on and contains only scheduled joypad inputs: no
RAM patches, save-state loads, or fixture writes. The route reaches the live
Piata inn, the Zema/Igglanova sequence, Birth Valley B1, and the real
`Cutscene_ProfHolt` completion before returning to Motavia. Its canonical
replay log is `oracle/logs/31_natural_land_rover.csv`.

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/31_natural_land_rover.tape \
    --groups core,pos,collision,party,battle,flags,chars,vehicle \
    --out oracle/logs/31_natural_land_rover.csv
```

The replay is 131,130 frames. The route receipt is:

| frame | mark | map / leader | selector | flags |
|---:|---|---|---:|---|
| `81,409` | `piata_inn_recovered` | `0019 / (656,496)` | `0000` | `01FF0400` |
| `102,951` | `zema_town_warp` | `0024 / (1568,1312)` | `0000` | `01FF0400` |
| `103,071` | `zema_town_cross` | `0024 / (496,768)` | `0000` | `01FF0C00` |
| `103,675` | `zema_to_birth_valley` | `002B / (496,160)` | `0000` | `01FF0C00` |
| `113,419` | `birth_valley_b1_enter` | `002C / (528,224)` | `0000` | `01FF0C00` |
| `129,139` | `holt_scene_done` | `0024 / (480,160)` | `0000` | `01FF8C00` |
| `131,011` | `natural_prefix_holt_world` | `0000 / (1584,1312)` | `0000` | `01FF8C00` |

The selector is `0000` on all 131,130 rows. This is an honest partial, not a
vehicle receipt: the tape stops before the later Zema/Krup/Tonoe/Rune/
Alshline/Zio/BioPlant/Rika chain and Machine Center B1 Part2's
`GettingLandRover`. Consequently there are no natural mount, vehicle-terrain,
dismount-refusal, successful-dismount, or mounted-encounter marks to report;
tape 29 remains the documented fixture for those surfaces.

The stop has a concrete route reason. Holt leaves Chaz/Alys/Hahn at `27/44/13`
HP; the shortest attempted Piata healing detour hit three natural formations
whose retail rolls refused RUN, and ordinary combat inputs left only Alys
alive before the inn inputs could be consumed. Zema's inn is still locked
before `IgglanovaZemaDefeated`, so the remaining route needs a new battle-safe
natural plan rather than a fake continuation.

The clone-side `psiv-replay` check uses the same tape and starts at the
engine-aligned `await_control` mark (frame `6,539`). It compares cleanly for
560 frames through frame `7,098` across 33 modeled columns. The first later
divergence, frame `7,099`, is the already-known static-NPC versus retail
wander position; NPC/wander internals are outside this slice. The clone does
not claim vehicle parity where the natural tape has no vehicle state.

### Fixture sweep

The remaining RAM-patched receipts are the MeetingRika capture (tape 28) and
the mounted vehicle surface (tape 29). Both intentionally enter late or
stateful surfaces that are not cheap extensions of an existing natural tape;
the new tape 31 prefix is the only fixture debt naturalized in this slice.

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

CSV with three provenance comment lines (four with `--rng-trace`, which adds
`# rng-trace=<name>`), then `frame,mark,buttons` and one column per enabled RAM
field. `frame` is 1-based and counts `retro_run()`
calls from power-on. Values are decimal, or fixed-width hex for fields flagged
`hex` in the map.

Input paths are written as given; the `# rng-trace=` line is the one line that
names an *output* of the run, so it carries the trace's basename and not the
path it was written to (`oracle/host/provenance.h`). Two runs that differ only
in their output directories therefore produce byte-identical traces and
byte-identical logs, which is what lets the ledger pin a capture by sha256 and
compare it against another run.

## RAM map

`ram_map.json` is the source of truth; **every address is transcribed from
`reference/ps4disasm/ps4.constants.asm` and carries the line it came from**,
except where a field's `source` names `ps4.asm` instead: an address the
constants file does not name, recovered from the operand the code itself uses.
`enemy_ability_index` (`$FFFFEEA8`) is the only such field so far - it is
`Enemy_Attack`'s ability re-roll word, read at `ps4.asm:19149` and written at
`ps4.asm:19151` - and the clears that reach it, the oracle measurements and what
the port does with it are in `docs/BATTLE_ORACLE_REPLAY.md`. Nothing was
inferred by watching memory. Struct fields are recorded as base + offset with
both citations so the arithmetic is auditable — for example `c1_facing` is
`Character_1` (`constants:2086`, `$FFFFC000`) plus `facing_dir`
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
| `31_natural_land_rover.tape` | power-on natural route through the verified Holt prefix; full Land Rover acquisition remains downstream |

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
