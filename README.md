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

Contributing with a coding agent? Start with [AGENTS.md](AGENTS.md) and the
[agent workflow](docs/AGENT_WORKFLOW.md).

## Where we are

**Combined post-Zio candidate checkpoint: October 3, 2026; not published.**

**Workflow setup is complete:** the [evidence-driven workflow](docs/AGENT_WORKFLOW.md)
is the standing default, and the [setup ledger](docs/records/WORKFLOW_SETUP.md) retains
the verified graph and checks. The docs were merged through PR #1.
**Historical native campaign outcome:** the connected post-Rika party rested in Zema,
crossed the newly opened northern bridge, and saved on Motavia `$00 (84,64)`
with all five alive and 1129 meseta. Fresh-process CONTINUE and save-byte
validation pass. See the
[historical native receipt](docs/field/TRAVEL.md#post-rika-northern-crossing-2026-09-23)
and [next task handoff](docs/ROADMAP.md#task-graph-handoff).

**Headless Session milestone (combined candidate, October 3):** the
29-chapter ordinary-pad route plays New Game through `Cutscene_ZioDefeated`
twice with identical tapes, digests and final camp-SAVE bytes. A fresh Session
loads that save and both tapes replay to the same digest. The
[combined route receipt](docs/campaign/RUNNER_LOG.md#combined-p1f2r2-headless-integration)
is separate from native input proof: generic Godot tape replay reaches initial
Aiedo from New Game, and an earned-save Zio segment passes ordinary CONTINUE,
camp SAVE and fresh-process CONTINUE. The current 11-pair presentation
certificate passes; the final Python gate repair/recheck and default-pack boot
remain pending. Full R2 remains in progress because its driver-retirement
criterion has not been met.

| Area | Current evidence |
| --- | --- |
| Connected playthrough | Ordinary-input routes complete the Academy, Tonoe/Alshline and Zema aftermath, with saved checkpoints and fresh CONTINUE checks. |
| BioPlant | Connected traversal, Rika join and escape are verified with all five alive, expected story flags, an ordinary Motavia save and fresh CONTINUE. The original healthy `$A7` save and failed attempts are preserved. |
| Post-Rika travel | Paid Zema recovery and the northern bridge crossing pass through ordinary input and SAVE/CONTINUE. The missing overworld page-hook consumer is repaired; the bridge's named 32×32 region matches retail exactly. |
| Combat and camp | Individual commands, implemented techniques/skills, all 26 usable battle-item records, recovery, shops, equipment, chests, travel and earned progression. STATE/ORDER supports undo, cancel and persistent formation changes. Ability coverage is still incomplete. |
| Recovery | A bounded BioPlant run wins one encounter, cures two poisoned members with ANTI, heals, saves and reloads. The playthrough driver now cures poison before HP recovery. |
| Presentation | Selected reference frames and four ORDER-menu regions match the cartridge exactly. Whole-scene, animation and UI fidelity still need work. |
| Data | The extractor covers all 361 real maps, character progression, dialogue, battle records, graphics and sound. Extracted records do not imply implemented gameplay. |

The [BioPlant checkpoint ledger](docs/campaign/BIOPLANT_NATIVE.md) and
[travel continuation](docs/field/TRAVEL.md#post-rika-northern-crossing-2026-09-23)
record the saves, hashes, successful checks and failed attempts. Save files, ROM-derived assets
and captures stay local; their paths in the ledgers are reproduction evidence,
not downloads included with the repository.

### Verified baseline

Code checkpoint [`82de4a3`](https://github.com/TusanHomichi/PSIV/commit/82de4a37fcce1799520bc033d251573244375445):

- **927 Python tests passed.**
- **898 Rust workspace tests passed** on the matching source; post-commit camp checks passed too.
- Formatting and strict Clippy passed.
- Native SAVE/CONTINUE and bounded oracle comparisons have separate receipts.

These are recorded local results, not a claim of CI coverage or a finished game.
The later [`e03700b`](https://github.com/TusanHomichi/PSIV/commit/e03700be2d71fcb298a3a877c3ebdf1bfcf85aad)
fix suppresses cancelled BROSE/RIMIT cues; its
[focused sound receipts](docs/sound/SOUND_INTEGRATION.md#cancelled-spell-cues-2026-09-15)
are separate from the full-suite baseline above. The workflow setup does not
rerun or recertify either gameplay result.

## Next milestones

1. Review the scoped Python gate repair, rerun the affected checks, and prove
   the primary default pack through ordinary boot/CONTINUE. Archive the bounded
   Zio checkpoint after the composed gate is accepted.
2. Close remaining battle abilities, enemy AI and camp command gaps as the
   campaign exposes them.
3. Next campaign route: post-Zio toward the spaceport/Zelan/Wren, with the ship
   destination-menu gap an explicit dependency. No later-arc play is claimed.
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
./oracle/build_core.sh   # pinned core sources; the sound crate builds from them
python3 -m psiv_tools pack "/path/to/Phantasy Star IV (USA).md" runtime-pack
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
PSIV_SAVE_DIR="$PWD/saves" godot --path godot
```

Use your Godot executable in place of `godot` if needed. Keep the default
1280×800 viewport for desktop checks; reduced captures can clip camp windows.
The [input table](docs/DEVELOPMENT.md#launch-the-native-game) lists the
current controls.

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
CLI details. The [runtime architecture](docs/RUNTIME_DESIGN.md) describes the
current layers and fidelity decisions.

## License

The project's original code and documentation use the **[MIT License](LICENSE)**,
copyright © 2026 Peter Permenter. MIT permits use, modification, distribution
and commercial reuse, subject to its notice requirements; see the
[MIT terms](https://opensource.org/license/mit).

This license does not relicense the original game's ROM, extracted game
content, or third-party components. Those retain their respective rights
and licenses.
