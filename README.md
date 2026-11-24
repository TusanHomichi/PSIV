# PSIV

**Phantasy Star IV, rebuilt as a native game.** PSIV reads the game's data out
of your own copy of the US Sega Genesis/Mega Drive cartridge and runs it
natively: a built-in pack builder extracts the maps, graphics, sound, dialogue
and battle tables into a local data pack, Rust reproduces the game's rules
frame by frame, and Godot draws it and reads your controller. No emulator runs
while you play, and the goal is the original game's behavior, quirks included.

| I want to… | Start here |
| --- | --- |
| **Play now** | [Quick start](#quick-start) below & [Player's guide](docs/PLAYING.md) |
| **Install or create mods** | [Modding guide](docs/MODDING.md) (shareable `mods/` folders & `psiv-pack mod`) |
| **Report a bug or weirdness** | [Bug reporting guide](docs/REPORTING.md) (works for humans & AI assistants) |
| **Help out / contribute** | [Contributing](CONTRIBUTING.md) (works for humans & AI assistants) |
| **Build from source** | [Build from source](#build-from-source) below |
| **Read how the cartridge works** | [Documentation index](docs/README.md#retail-reference) |
| **Work on it with a coding agent** | [AGENTS.md](AGENTS.md) & [Agent workflow](docs/AGENT_WORKFLOW.md) |

## Quick start

Play in three steps — **no Godot, Rust or Python install required** (or point
your AI assistant at this repo and ask it to follow
[Setting up with an AI assistant](docs/PLAYING.md#setting-up-with-an-ai-assistant)):

1. **Download** the archive for your system from the
   [Releases page](https://github.com/TusanHomichi/PSIV/releases) and extract
   it anywhere.
2. **Start the game** (`psiv.x86_64`, `PSIV.exe`, or `PSIV.app`).
3. **Supply your US Phantasy Star IV ROM once** — click **Choose your ROM
   file** or drag and drop the ROM onto the setup window. PSIV verifies the
   ROM, builds its data pack in a few seconds into your user data folder, and
   starts the game.

| Platform | Download archive | Launch | Notes |
| --- | --- | --- | --- |
| **Linux & Steam Deck** (x86-64) | `PSIV-<version>-linux-x86_64.tar.gz` | `./psiv.x86_64` | On Steam Deck, run once in Desktop Mode to pick your ROM, then add as a Non-Steam Game for Game Mode. |
| **Windows** (64-bit) | `PSIV-<version>-windows-x86_64.zip` | `PSIV.exe` | Extract the zip first. On SmartScreen, choose **More info → Run anyway**. |
| **macOS** (Apple silicon & Intel) | `PSIV-<version>-macos-universal.zip` | `PSIV.app` | Unsigned/unnotarized: on first open, allow it in **System Settings → Privacy & Security → Open Anyway**. |

> **Bring your own ROM:** PSIV ships none of Sega's game assets. It accepts
> only the unheadered US retail ROM (`.md` / `.bin` / `.gen`) with SHA-256:
> ```text
> 511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
> ```
> See the [player's guide](docs/PLAYING.md) for OS-specific first-run steps,
> how to check your ROM hash, and where your pack and saves are stored.

### Controls

| Action | Keyboard | Controller |
| --- | --- | --- |
| Move / menu cursor | Arrow keys or WASD | D-pad or left stick |
| Confirm / talk (Genesis C) | Enter, Space or Z | A (bottom face button) |
| Cancel (Genesis B) | Escape or X | B (right face button) |
| Camp menu (Genesis A) | C | X (left face button) |
| Start | Tab | Start |

Save anytime on the field or overworld from **Camp (`C`) → STATE → SAVE**, and
load from the title screen's **CONTINUE**.

## How far it plays & platforms

- **Full campaign playable start to finish.** An automated playthrough pressing
  the same buttons a player does completes New Game through the Ending and its
  final Start press in 84 chapters; the native game replays all 84 chapters in
  one continuous tape with every chapter save matching byte for byte.
- **Rules & abilities.** Every player technique and skill, all 83 distinct
  regular enemy abilities through the final boss, vehicles, overworld planet
  maps, Hunter's Guild, shops/inns, saves, sound driver, and ending credits are
  implemented from the cartridge's code. Most abilities are also captured on
  the cartridge and replayed against it
  ([player abilities](docs/battle/PLAYER_ABILITIES.md),
  [enemy abilities](docs/battle/ENEMY_ABILITIES.md)).
- **Platforms.** Release downloads target **Linux & Steam Deck**, **Windows**,
  and **macOS**. Linux and the Steam Deck are played on real hardware; Windows
  and macOS archives are built and first-run smoke-tested on native GitHub
  runners ([#117](https://github.com/TusanHomichi/PSIV/issues/117)), and
  playtest reports on Windows and macOS are very welcome. The
  [roadmap](docs/ROADMAP.md) owns the release bar and platform list.

## Build from source

If you want to run straight from Git (or hack on the code) instead of using a
release archive, you need:

| Tool | Version | Notes |
| --- | --- | --- |
| Python | 3.10 or newer | standard library only, for the checkout launcher (`tools/godot.py`) |
| Rust | the toolchain in [`rust-toolchain.toml`](rust-toolchain.toml) | install [rustup](https://rustup.rs/); it fetches the pinned toolchain on first use |
| Godot | the version in [`.godot-version`](.godot-version), standard build | from the [Godot download archive](https://godotengine.org/download/archive/) |
| git, GNU `patch`, `make`, a C compiler | any recent | fetch and compile the sound chip's sources |

From a directory of your choice:

```bash
git clone https://github.com/TusanHomichi/PSIV.git
cd PSIV
./oracle/build_core.sh
cargo run --release --manifest-path rust/Cargo.toml -p psiv-pack -- "/path/to/Phantasy Star IV (USA).md" runtime-pack
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
export PSIV_GODOT=/path/to/godot
python3 tools/godot.py prepare
PSIV_SAVE_DIR="$PWD/saves" python3 tools/godot.py run
```

1. `./oracle/build_core.sh` fetches the pinned Genesis Plus GX sources into
   `oracle/gpgx-src/`. The game compiles one file of them, the sound chip
   emulation, into itself.
2. `psiv-pack` builds the data pack from your ROM into `runtime-pack/` in a
   few seconds. The ROM can live anywhere.
3. `cargo build` compiles the GDExtension library for Godot.
4. `PSIV_GODOT` names your Godot executable (or put it on `PATH` as `godot` and
   skip that line). `python3 tools/godot.py prepare` verifies the Godot build
   and writes `godot/.godot/`, which a fresh clone needs before the game's
   classes load.
5. `python3 tools/godot.py run` starts the game from an isolated run root.
   Godot arguments go after `--` (`python3 tools/godot.py run -- --help`), and
   saves go to `PSIV_SAVE_DIR`.

The [developer guide](docs/DEVELOPMENT.md) covers everything past playing:
tests, Steam Deck branch builds, the cartridge oracle, and the verification
gate.

## Known differences

Presentation (some animations, whole-scene staging, audio timing) still differs
from the original cartridge in places. Where PSIV knowingly differs, there is
an open issue:

- [Gameplay differences](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Agameplay-fidelity):
  a rule that does not yet match the original.
- [Presentation differences](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Apresentation):
  drawing, animation, audio or timing that looks or sounds different.
- A few original cartridge bugs are fixed on purpose; each is listed in the
  [deviation record](docs/source-notes/README.md#deliberate-fixes-to-retail-battle-bugs).

Found a bug or unlisted difference? Follow the
[bug reporting guide](docs/REPORTING.md) or
[open an issue](https://github.com/TusanHomichi/PSIV/issues/new/choose) with
what you saw, where in the game, and your `logs/godot.log`. You can also point
an AI assistant straight at this repo to gather those details and file the
issue (or help fix it via [CONTRIBUTING.md](CONTRIBUTING.md)). Questions go to
[Discussions](https://github.com/TusanHomichi/PSIV/discussions).

## Status

**Version 1.0.x is out** ([latest release](https://github.com/TusanHomichi/PSIV/releases/latest)):
the unmodified US game is completable natively from New Game to the Ending with
ready-to-play archives for Linux/Steam Deck, Windows, and macOS, plus shareable
mod folders with automatic save isolation ([modding guide](docs/MODDING.md)).
Ongoing post-1.0 work is tracked on the [roadmap](docs/ROADMAP.md) and in open
issues.

## License and third-party code

PSIV's original code and documentation are under the [MIT License](LICENSE),
copyright © 2026 Peter Permenter. The license covers PSIV's own work and nothing
else: not the original game, its ROM, the data pack built from it, extracted
images or sound, captures, or saves. None of those is tracked here; the ROM, the
pack, `generated/`, saves and emulator captures stay local, ignored by Git.

What the repository does hold from the game:

- the game's rules and event scripts, rewritten in Rust from the disassembly,
  with the cartridge's own ids, flags, offsets and constants, and citations of
  the disassembly by symbol and line;
- ids and addresses taken from the ROM and the disassembly: the RAM addresses
  in `oracle/ram_map.json`, and the map, shop, item and position ids the
  campaign route `rust/psiv-campaign/routes/main.json` names;
- short byte signatures that identify ROM structures, in the extractor and its
  tests;
- names, numbers and short quoted lines of dialogue in the documents and tests;
- test fixtures that record game state observed in an emulator: the battle
  replay fixtures under `rust/psiv-core/src/battle/replay_fixtures/` (party and
  enemy stats, random draws, damage, turn order; the records they replay
  against come from the local pack) and the oracle receipts under
  `oracle/fixtures/`.

The battle tables themselves (enemies, their abilities, formations, equipment
and techniques) and the game's art stay in the local pack: the replay tests
read the tables from there and skip without it, and `tools/check_rom_tables.py`
fails a change that copies the tables, or a tile written as pixel strings, into
the tree.

Third-party code is fetched at build time, never committed:

- **Genesis Plus GX** (`oracle/build_core.sh`, pinned commit
  `2d7131c5efa606f649d36e1685a8ca47c24f31b3`) is under a non-commercial license.
  Its emulator core is test tooling for the oracle and is never part of the
  game, with one exception:
- **Nuked OPN2** (`ym3438.c`, from the same checkout) is compiled into the game's
  library for the sound chip. It is under the LGPL-2.1-or-later; its notice is
  in the source, and each release download carries its license in
  `THIRD-PARTY-NOTICES.txt` and its source in `nuked-opn2/`.
- **The `ps4disasm` disassembly**
  ([alechenninger/ps4disasm](https://github.com/alechenninger/ps4disasm)) is a
  research reference for developers; the game does not use it.
- **Godot** is under the MIT license, with its own third-party notices.
