# PSIV

**Phantasy Star IV, rebuilt as a native Rust/Godot game.**

PSIV pairs a cartridge-data extractor with a native game runtime. Python turns
a user-supplied US ROM into a local asset/data pack; Rust runs the game rules,
and Godot provides the desktop presentation and input. An emulator is used
for development comparisons; playing the native build does not require one.

The goal is a complete, faithful playthrough on a modern computer, followed
by modding tools. **This is an active development build, with the campaign
still in progress.**

[Current status](#where-we-are) · [Roadmap](docs/ROADMAP.md) ·
[Build and run](docs/DEVELOPMENT.md) · [Documentation](docs/README.md) · [MIT](LICENSE)

## Where we are

**Checkpoint: September 15, 2026.**

| Area | Current evidence |
| --- | --- |
| Connected playthrough | Ordinary-input routes complete the Academy, Tonoe/Alshline and Zema aftermath, with saved checkpoints and fresh CONTINUE checks. |
| BioPlant | Entrance doors, alarm and elevators work. A full-health `$A7` save is verified; an onward attempt reached `$A9` but lost Hahn. The connected Rika join and escape remain unfinished. |
| Combat and camp | Individual commands, implemented techniques/skills, all 26 usable battle-item records, recovery, shops, equipment, chests, travel and earned progression. STATE/ORDER supports undo, cancel and persistent formation changes. Ability coverage is still incomplete. |
| Recovery | A bounded BioPlant run wins one encounter, cures two poisoned members with ANTI, heals, saves and reloads. The playthrough driver now cures poison before HP recovery. |
| Presentation | Selected reference frames and four ORDER-menu regions match the cartridge exactly. Whole-scene, animation and UI fidelity still need work. |
| Data | The extractor covers all 361 real maps, character progression, dialogue, battle records, graphics and sound. Extracted records do not imply implemented gameplay. |

The [BioPlant checkpoint ledger](docs/BIOPLANT_NATIVE.md) records the saves,
hashes, successful checks and failed attempts. Save files, ROM-derived assets
and captures stay local; their paths in the ledgers are reproduction evidence,
not downloads included with the repository.

### Verified baseline

Code checkpoint [`82de4a3`](https://github.com/TusanHomichi/PSIV/commit/82de4a37fcce1799520bc033d251573244375445):

- **927 Python tests passed.**
- **898 Rust workspace tests passed** on the matching source; post-commit camp checks passed too.
- Formatting and strict Clippy passed.
- Native SAVE/CONTINUE and bounded oracle comparisons have separate receipts.

These are recorded local results, not a claim of CI coverage or a finished game.

## Next milestones

1. Complete the connected BioPlant route through Rika, with surviving party,
   correct story flags and a verified save/restart.
2. Close remaining battle abilities, enemy AI and camp command gaps as the
   campaign exposes them.
3. Verify later story, vehicle and boss progression through ordinary input.
4. Finish presentation fidelity and produce a reproducible desktop package.
5. Build modding tools after the unmodified game is playable end to end.

See the [roadmap](docs/ROADMAP.md) for concrete completion criteria.

## Build and run

The current GDExtension configuration targets **Linux x86-64**. The tested
Godot version is **4.7.1**; other platforms need build/export work.

You need Python, a Rust toolchain supporting edition 2024, Godot, and the
supported US ROM. The ROM and generated assets are not distributed here.

```bash
git clone https://github.com/TusanHomichi/PSIV.git
cd PSIV
git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm
python3 -m psiv_tools pack "/path/to/Phantasy Star IV (USA).md" runtime-pack
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
PSIV_SAVE_DIR="$PWD/saves" godot --path godot
```

Use your Godot executable in place of `godot` if needed. Keep the default
1280×800 viewport for desktop checks; reduced captures can clip camp windows.
Arrow keys move; Enter/Space/Z confirm or interact; Escape/X open camp or cancel.

[Developer setup and verification](docs/DEVELOPMENT.md) covers the ROM hash,
save directories, test commands and local launcher assumptions.

## Inside the project

| Path | Responsibility |
| --- | --- |
| `psiv_tools/` | ROM readers, decompression, extraction and pack generation |
| `rust/psiv-data/` | Typed pack loading and validation |
| `rust/psiv-core/` | Deterministic field, battle, story and persistence rules |
| `rust/psiv-runtime/` | Game orchestration and the presentation-facing API |
| `rust/psiv-godot/`, `godot/` | Desktop rendering, menus and input |
| `rust/psiv-sound/` | Sound-driver interpretation and chip emulation |
| `oracle/`, `tools/` | Cartridge comparisons and native verification drivers |

The [extraction reference](docs/EXTRACTION.md) preserves the table census and
CLI details. Older implementation notes live in the
[archive](docs/IMPLEMENTATION_HISTORY.md).

## License

The project's original code and documentation use the **[MIT License](LICENSE)**,
copyright © 2026 Peter Permenter. MIT permits use, modification, distribution
and commercial reuse, subject to its notice requirements; see the
[MIT terms](https://opensource.org/license/mit).

This license does not relicense the original game's ROM, extracted game
content, or third-party components. Those retain their respective rights
and licenses.
