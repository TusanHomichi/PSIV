# PSIV

**Phantasy Star IV, rebuilt as a native game for Linux and the Steam Deck.**

PSIV reads the game's data out of your own copy of the US Genesis/Mega Drive
cartridge and runs it natively: Python extracts the maps, graphics, sound,
dialogue and battle tables into a local data pack, Rust reproduces the game's
rules frame by frame, and Godot draws it and reads your controller. No emulator
runs while you play, and the goal is the original game's behavior, quirks
included.

**You bring the ROM.** PSIV contains no game data. It builds everything from a
ROM image of the US retail release that you supply; nothing derived from it is
distributed here.

[Roadmap](docs/ROADMAP.md) · [Contributing](CONTRIBUTING.md) ·
[Developer guide](docs/DEVELOPMENT.md) · [Documentation](docs/README.md) ·
[License](#license)

## Status

PSIV is not at 1.0 yet. An automated playthrough, pressing the same buttons a
player would, gets from New Game to Dark Force 2's defeat, and the native game
replays it as far as the Air Castle so far. Every player technique and skill,
and every enemy ability up to Dark Force 2, is checked against the cartridge.
The rest of the game, through to the Ending, is being integrated now.
Presentation (some animations, whole-scene staging, audio timing) still
differs from the original in places.

Version 1.0 means the unmodified game is completable from New Game to the
Ending natively. The checklist:

- [ ] The full campaign, New Game to the Ending, plays natively.
- [ ] Long sessions run without memory growth.
- [ ] Every story event that a trigger starts has its scene.
- [ ] Hosted CI runs the checks that need no ROM.
- [ ] A release package and player instructions for Linux and the Steam Deck.

The open items are the [v1.0 blocker issues](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Av1.0-blocker).
Windows support comes after 1.0.

## Play it

PSIV currently runs from a source checkout; a packaged release is part of the
1.0 work. You need:

- the US ROM, `Phantasy Star IV (USA).md` (`.md` is the Mega Drive ROM
  extension; the accepted SHA-256 is in the
  [developer guide](docs/DEVELOPMENT.md#prepare-the-local-pack));
- Python 3.10 or newer, a Rust toolchain with edition 2024, `git`, GNU `patch`
  and a C compiler;
- Godot, at the version the [developer guide](docs/DEVELOPMENT.md#current-environment)
  names.

### On Linux

Build the data pack from your ROM, build the game, and start it. The exact
commands are in the developer guide's
[prepare the local pack](docs/DEVELOPMENT.md#prepare-the-local-pack) and
[launch the native game](docs/DEVELOPMENT.md#launch-the-native-game) sections.
In short: clone this repository, fetch the disassembly and the sound core the
guide names, run `python3 -m psiv_tools pack <your ROM> runtime-pack`, build the
`psiv-godot` extension with Cargo, and open the `godot/` project with Godot.

Saves go to the directory `PSIV_SAVE_DIR` names, or to `saves/` in the
directory the game starts from when it is unset. Load with the title screen's
CONTINUE and save from the camp menu's STATE, then SAVE, as on the cartridge.

### On the Steam Deck

Build the pack on a Linux PC as above, then run
`tools/package/deploy-deck.sh <deck-address>`: it builds a Deck-compatible
release, puts the pack beside it, and copies both to `~/psiv` on the Deck over
SSH. On the Deck, add `~/psiv/psiv.x86_64` as a non-Steam game. This is the
developer path until the release package lands; its requirements are in the
developer guide's [Steam Deck section](docs/DEVELOPMENT.md#package-for-the-steam-deck).

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

If you find a difference that is not listed, please open an issue with what you
saw, where in the game, and what the original does.

## Modding

The data pack is ordinary files: PNG images and JSON tables (plus some binary
sound data) under `runtime-pack/`. Edit them and the game loads your edits,
as long as they still fit the pack's schema (it refuses a malformed table).
`python3 -m psiv_tools pack` rebuilds the pack from the ROM and overwrites
edits made in place, so keep your changes separately. A mod overlay loader, so
that mods sit beside the original pack instead of inside it, is planned after
1.0.

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md). Coding agents also read
[AGENTS.md](AGENTS.md), which holds the project's working rules.

## License

PSIV's original code and documentation are under the [MIT License](LICENSE),
copyright © 2026 Peter Permenter. That license does not cover the original
game, its ROM, anything extracted from it, or third-party components; those
keep their own rights and licenses.
