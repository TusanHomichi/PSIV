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
├── build_host.py           compiles host/ into bin/psiv_oracle, build id included
├── host_binary.py          the binary to run: this tree's host, or a rebuild of it
├── scripts/                the analysis tools, one module each: navigate
│                           (closed-loop tape authoring over `route.py`),
│                           analyze_rng (per-frame RNG call census from a log),
│                           analyze_battle (battle ground truth from a log),
│                           anim_sweep (press offsets across the dialogue open
│                           animation), damage_census (same-matchup damage
│                           samples across shifted seed paths), find_battle,
│                           sweep_encounter
├── route.py                plans a walking route over the pack's collision data
├── rng_trace.py            checks a --rng-trace capture against its log
├── force/                  forces a chosen formation into a battle and captures
│                           it (tape, selectors, scout, draw, phases, durable,
│                           the sustained-party patch); `__main__.py` is
│                           `python3 -m oracle.force`
├── sweep/                  captures every Motavia formation and extracts each
│                           (the list, one formation's run, the batch);
│                           `__main__.py` is `python3 -m oracle.sweep`;
│                           `route_abilities` derives the abilities a route
│                           stretch can meet (one tool: a named stretch or a map
│                           pattern), `forced_cases` captures them, `zelan` and
│                           `arc` are the status/stat recipes, `observed` lists
│                           what a fixture directory shows running
├── fixture/                the replay-fixture extractor (the two CSVs, the roll
│                           derivation, the observations, the fixture's shape);
│                           `__main__.py` is `python3 -m oracle.fixture`
├── decode_layout.py        decodes a --dump-state receipt into a layout
├── reload_frames.py       forced field-reload measurements and retained evidence
├── scroll_state.py         the camera and VDP scroll receipt that reads
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

`oracle/` is a package, and there is one way into it: **every entry point is a
module run from the repository root** - `python3 -m oracle.force`,
`python3 -m oracle.sweep`, `python3 -m oracle.fixture`,
`python3 -m oracle.rng_trace`, `python3 -m oracle.decode_layout`,
`python3 -m oracle.build_host`, `python3 -m oracle.host_binary`, and
`python3 -m oracle.scripts.<name>` for the analysis tools - so no tool needs
`oracle/` or the repository root on `PYTHONPATH`, and every oracle import is
absolute (`from oracle import route`). Running a module by its file path is not
supported: no module under `oracle/` rewrites the interpreter's import path, so
an import of a sibling oracle module resolves only through the package.

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
`build_core.sh`, `build_host.py`, `host_binary.py` and `verify.sh` are harness
code and **are** committed.

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

### Building the host, and why it cannot be stale

```sh
python3 -m oracle.build_host      # ~1s: the sources and flags verify.sh uses
```

The build passes a SHA-256 over `host/*.c` and `*.h` in as
`-DPSIV_ORACLE_BUILD_ID`, and the host prints it back for `--build-id`. Nothing
used to tie the binary to its sources, so a checkout could drive the tapes with
a host older than itself - issue #61 was a three-week-old `bin/psiv_oracle`
with no `--rng-trace`, rebuilt by hand mid-run. Every entry point that
launches the host now asks `oracle/host_binary.py` for the path instead: it
compares what the binary reports with the fingerprint of the sources in front
of it and rebuilds through the entry point when they differ - about a second,
`gcc` and nothing else. Launchers that start together (a sweep at `--jobs 3`)
serialize on `oracle/bin/.psiv_oracle.lock`, so the host is compiled once.

From a shell:

```sh
python3 -m oracle.host_binary --path      # the path to run; rebuilds if it must
python3 -m oracle.host_binary --build-id  # the fingerprint that binary reports
```

`PSIV_ORACLE_NO_REBUILD=1` (also `yes`, `true`, `on`) refuses a stale host
instead, with an error naming both fingerprints, for a tree that must not be
written to; unset, or any other value, lets a launcher rebuild.
`tests/test_oracle_host_binary.py` owns the cases, including the one that fails
when a new launcher names the binary instead of asking for it.

One tape by hand (a bare shell run gets no freshness check of its own - build
once with `python3 -m oracle.build_host`, or ask for the path as above):

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
default; `--probe-endian` prints work-RAM diagnostics; `--build-id` prints the
sources the binary was compiled from (see above).

## Topic guides

- [Video and scene captures](CAPTURE.md): frames, crops and explicit scene fixtures.
- [Battle captures and recipes](BATTLES.md): RNG traces, forced battles, party scripts and sweeps.
- [Tape, log and RAM formats](FORMATS.md): buttons, byte order and determinism.
- [Tapes and authoring](TAPES.md): the tape inventory, encounter clock and authoring tools.

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

## Where the measurements live

Each finding the tapes above measured lives in the document that owns its
topic, beside the rule it confirms:

- walking, collision, objects that block and the story gates walking finds:
  [MOVEMENT.md](../docs/field/MOVEMENT.md);
- talk range, text timing, the hold-to-accelerate press and page advance:
  [SCENE_DIALOGUE.md](../docs/scenes/SCENE_DIALOGUE.md#measured-on-the-cartridge);
- the new-game starting state: [01_GameStart.md](../docs/scenes/01_GameStart.md#measured-on-the-cartridge);
- the RNG's per-frame schedule and the first battles (turn order, damage,
  misses, criticals, escape, DEFEND, rewards):
  [BATTLE_MEASUREMENTS.md](../docs/battle/BATTLE_MEASUREMENTS.md);
- the level-up and its stat lag: [PROGRESSION.md](../docs/camp/PROGRESSION.md#measured-on-the-cartridge);
- object slots and the wander RNG rule, and the wander timer's gate:
  [NPC_WANDER.md](../docs/field/NPC_WANDER.md#measured-on-the-cartridge);
- the camera columns: [CAMERA.md](../docs/field/CAMERA.md#measured-on-the-cartridge);
- chest slots, opening, the flag bank and the no-duplicate guard:
  [CHESTS.md](../docs/field/CHESTS.md#measured-on-the-cartridge);
- where the temp event flags land, and the map-load clear:
  [FIELD_STATE.md](../docs/field/FIELD_STATE.md#where-the-temp-event-flags-land-on-hardware).

`./oracle/verify.sh` regenerates the measurements they cite from the pinned
ROM and core.
