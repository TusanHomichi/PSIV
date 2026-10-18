# PSIV

**Phantasy Star IV, rebuilt as a native game.** PSIV reads the game's data out
of your own copy of the US Genesis/Mega Drive cartridge and runs it natively:
Python extracts the maps, graphics, sound, dialogue and battle tables into a
local data pack, Rust reproduces the game's rules frame by frame, and Godot draws
it and reads your controller. No emulator runs while you play, and the goal is
the original game's behavior, quirks included.

**How far it plays.** An automated playthrough, pressing the same buttons a
player would, plays from New Game to the Reunion at the Dezo Spaceport, after
the Elsydeon is found, and the native game replays it as far as the Air Castle. Every player technique and skill, and every
enemy ability up to Dark Force 2, is implemented from the cartridge's code; most
of them are also captured on the cartridge and replayed against it, and the
battle documents list the ones that are not
([player abilities](docs/battle/PLAYER_ABILITIES.md),
[enemy abilities](docs/battle/ENEMY_ABILITIES.md)). The rest of
the story, from the Reunion to the Ending, is the open work before 1.0
([#116](https://github.com/TusanHomichi/PSIV/issues/116)). Presentation (some
animations, whole-scene staging, audio timing) still differs from the original
in places; the [known differences](#known-differences) are listed.

**Platforms.** Linux and the Steam Deck today. Windows and macOS are part of 1.0
([#117](https://github.com/TusanHomichi/PSIV/issues/117)); the
[roadmap](docs/ROADMAP.md) owns the release bar and the platform list.

| I want to… | Start here |
| --- | --- |
| **Play** | [Quick start](#quick-start-linux) below |
| **Mod or contribute** | [Modding guide](docs/MODDING.md) and [contributing](CONTRIBUTING.md) |
| **Read how the cartridge works** | The [documentation index](docs/README.md#retail-reference) |
| **Work on it with a coding agent** | [AGENTS.md](AGENTS.md) and the [agent workflow](docs/AGENT_WORKFLOW.md) |

## Quick start (Linux)

PSIV runs from a source checkout until the release package lands
([#115](https://github.com/TusanHomichi/PSIV/issues/115)).

**You bring the ROM.** PSIV builds the game's graphics, sound, maps, text and
data tables from a ROM image of the US retail release that you supply, and does
not distribute them; what the repository does hold from the game is listed
under [license](#license-and-third-party-code). Check yours before you start; the SHA-256 must be exactly this
one:

```console
$ sha256sum "Phantasy Star IV (USA).md"
511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a  Phantasy Star IV (USA).md
```

(`.md` is the Mega Drive ROM extension, not Markdown.) The pack builder checks
the same hash and refuses any other image.

**Prerequisites**

| Tool | Version | Notes |
| --- | --- | --- |
| Python | 3.10 or newer | standard library only, to build the pack |
| Rust | the toolchain in [`rust-toolchain.toml`](rust-toolchain.toml) | install [rustup](https://rustup.rs/); it reads that file and fetches the toolchain on first use |
| Godot | the version in [`.godot-version`](.godot-version), standard build | from the [Godot download archive](https://godotengine.org/download/archive/) |
| git, GNU `patch`, `make`, a C compiler | any recent | fetch and compile the sound chip's sources |

**Commands**, from a directory of your choice:

```bash
git clone https://github.com/TusanHomichi/PSIV.git
cd PSIV
./oracle/build_core.sh
python3 -m psiv_tools pack "/path/to/Phantasy Star IV (USA).md" runtime-pack
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
export PSIV_GODOT=/path/to/godot
python3 tools/godot.py prepare
PSIV_SAVE_DIR="$PWD/saves" "$(python3 tools/godot.py)" --path godot
```

1. `./oracle/build_core.sh` fetches the pinned Genesis Plus GX sources into
   `oracle/gpgx-src/`. The game compiles one file of them, the sound chip
   emulation, into itself.
2. `pack` builds the data pack from your ROM into `runtime-pack/`. The ROM is an
   argument; it can live anywhere.
3. `cargo build` compiles the game's library for Godot.
4. `PSIV_GODOT` names your Godot executable (or put it on `PATH` as `godot` and
   skip that line). `python3 tools/godot.py prepare` checks it (a missing file,
   the wrong version or a build that is not `stable` is refused, with the
   reason) and writes the project's extension index (`godot/.godot/`, which Git
   ignores), which a fresh clone needs before the game's classes load.
5. The last line starts the game; `python3 tools/godot.py` prints the Godot
   executable it found.

Saves go to the directory `PSIV_SAVE_DIR` names. Load with the title screen's
CONTINUE, and save from the camp menu's STATE, then SAVE, as on the cartridge.

**On the Steam Deck**, build the pack on a Linux PC as above, then run
`tools/package/deploy-deck.sh <deck-address>`: it builds a Deck-compatible
release, puts the pack beside it, and copies both to `~/psiv` on the Deck over
SSH. On the Deck, add `~/psiv/psiv.x86_64` as a non-Steam game. Its requirements
are in the developer guide's [Steam Deck section](docs/DEVELOPMENT.md#package-for-the-steam-deck).

The [developer guide](docs/DEVELOPMENT.md) covers everything past playing:
the tests, the oracle and the checks a change runs.

### Controls

| Action | Keyboard | Controller |
| --- | --- | --- |
| Move, menu selection | Arrow keys or WASD | D-pad or left stick |
| Confirm, talk | Enter, Space or Z | A (bottom face button) |
| Cancel | Escape or X | B (right face button) |
| Camp menu | C | X (left face button) |
| Start | Tab | Start |

## Known differences

Where PSIV knowingly differs from the cartridge, there is an open issue:

- [Gameplay differences](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Agameplay-fidelity):
  a rule that does not yet match the original.
- [Presentation differences](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Apresentation):
  drawing, animation, audio or timing that looks or sounds different.
- A few original bugs are fixed on purpose; each is listed in the
  [deviation record](docs/source-notes/README.md#deliberate-fixes-to-retail-battle-bugs).

Found a difference that is not listed? [Open an issue](https://github.com/TusanHomichi/PSIV/issues/new/choose)
with what you saw, where in the game, and what the original does. Questions go
to [Discussions](https://github.com/TusanHomichi/PSIV/discussions).

## Status

Version 1.0 means the unmodified game is completable from New Game to the Ending
natively. The open items are the
[v1.0 blocker issues](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Av1.0-blocker),
and the [roadmap](docs/ROADMAP.md) keeps the checklist.

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
- ids and addresses taken from the ROM and the disassembly:
  `oracle/sweep/motavia_formations.json`
  (the 83 formation ids of the Motavia encounter groups, from
  `generated/formation_indexes.json`, ids only), the RAM addresses in
  `oracle/ram_map.json`, and the map, shop, item and position ids the
  campaign route `rust/psiv-campaign/routes/main.json` names;
- short byte signatures that identify ROM structures, in the extractor and its
  tests;
- names, numbers and short quoted lines of dialogue in the documents and tests;
- test fixtures that record game state observed in an emulator: the battle
  replay fixtures under `rust/psiv-core/src/battle/replay_fixtures/` (party and
  enemy stats, random draws, damage, turn order) and the oracle receipts under
  `oracle/fixtures/`;
- nine 8x8 tiles of the battle screen's damage numbers, copied from retail VRAM
  tiles `$5E4`-`$5EA`, `$5EE` and `$5EF` as pixel strings in `damage_pattern`
  (`rust/psiv-godot/src/battle/chrome.rs`), which the game draws its damage
  numbers from;
- one fixture that copies part of the ROM's battle tables (enemies, enemy
  skills, formations, equipment and techniques),
  `rust/psiv-core/src/battle/replay_fixtures/motavia_pack.json`, which
  [#149](https://github.com/TusanHomichi/PSIV/issues/149) removes before 1.0.

Third-party code is fetched at build time, never committed:

- **Genesis Plus GX** (`oracle/build_core.sh`, pinned commit
  `2d7131c5efa606f649d36e1685a8ca47c24f31b3`) is under a non-commercial license.
  Its emulator core is test tooling for the oracle and is never part of the
  game, with one exception:
- **Nuked OPN2** (`ym3438.c`, from the same checkout) is compiled into the game's
  library for the sound chip. It is under the LGPL-2.1-or-later; its notice is
  in the source, and how the release package carries it is
  [#115](https://github.com/TusanHomichi/PSIV/issues/115).
- **The `ps4disasm` disassembly**
  ([alechenninger/ps4disasm](https://github.com/alechenninger/ps4disasm)) is a
  research reference for developers; the game does not use it.
- **Godot** is under the MIT license, with its own third-party notices.
