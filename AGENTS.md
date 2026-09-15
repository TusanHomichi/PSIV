# Working on PSIV

PSIV reconstructs the US retail game as a native Rust/Godot runtime. Match
cartridge behavior, document deliberate deviations, and keep the unmodified
game playable without an editor or agent service. Modding follows a complete
base-game playthrough.

## Start here

1. Read [README.md](README.md) and [the roadmap](docs/ROADMAP.md) for current scope.
2. Check `git status --short --branch` and the relevant source before editing.
   Preserve unrelated work and identify any active build/game processes.
3. Read [development setup](docs/DEVELOPMENT.md), then the relevant subsystem
   ledger from [the documentation index](docs/README.md).
4. Use [the agent workflow](docs/AGENT_WORKFLOW.md) for task selection,
   verification and handoffs. A dated result describes its recorded revision;
   refresh evidence before claiming it applies to your change.

Dated research and verification ledgers retain their measurement context.
Follow explicit corrections in the ledgers; resolve contradictions against
source and fresh observations. Do not infer
campaign completion from extracted data or transcribed scenes.

## Put changes in the right layer

| Layer | Responsibility |
| --- | --- |
| `psiv_tools/`, `tests/` | Python ROM decoding, extraction, pack generation and their tests |
| `rust/psiv-data/` | Typed pack schema, loading and validation |
| `rust/psiv-core/` | Deterministic game rules and state; keep engine types and presentation out |
| `rust/psiv-runtime/` | Orchestration, persistence integration and presentation-facing API |
| `rust/psiv-godot/`, `godot/` | Godot bridge, rendering, menus and input |
| `rust/psiv-sound/` | Sound driver and chip emulation |
| `oracle/`, `tools/` | Original-game comparisons, native input drivers and development fixtures |

Keep gameplay rules in Rust instead of duplicating them in Godot or test
drivers. Preserve the cartridge's integer widths, signedness, flag banks,
ordering and RNG semantics where relevant. Cite disassembly symbols/offsets
and record discrepancies in [SOURCE_NOTES.md](SOURCE_NOTES.md). The reference
fork contains Grand Cross rewrites: confirm retail bytes instead of copying
hack scene bodies or treating fork build-address comments as ROM offsets.
A plausible genre convention or ability name is not enough to establish a retail rule.

## Protect local inputs and evidence

- The ROM, `reference/`, generated packs, saves and captures are local assets.
  Honor [.gitignore](.gitignore); do not force-add them. The project's MIT
  license does not relicense original-game or third-party content.
- Do not run broad cleanup such as `git clean -fdx`: ignored directories can
  contain irreplaceable checkpoint evidence. Remove only known disposable
  outputs created for your task.
- Copy a source save to a separate run directory, set `PSIV_SAVE_DIR`
  explicitly, and retain its hash. Never overwrite the only verified save.
- Inspect a native driver's configuration before running it. Existing drivers
  can contain local paths, special fixtures or long campaign routes. Missing
  `build/` evidence on a fresh clone is expected; report that limitation.
- Do not rebuild a GDExtension while a running Godot process has it loaded.
  Use the configured launcher or close the relevant instance first.

## Verify the claim you intend to make

- Run focused checks for the changed behavior. Add regression coverage for
  gameplay fixes; a docs-only change needs link/command review and
  `git diff --check`, not a fresh full gameplay run.
- Use the exact commands and prerequisites in [DEVELOPMENT.md](docs/DEVELOPMENT.md).
  Serialize full Rust tests with `CARGO_BUILD_JOBS=1` and `--test-threads=1`;
  avoid overlapping expensive Rust, Python and oracle runs.
- State tests prove state. Ordinary native input proves the interaction path.
  Fresh-process CONTINUE proves persistence. Isolated debug/RAM fixtures are
  useful, but do not prove a connected campaign route.
- Visual parity requires inspected captures against the matching original
  state. Name the compared frame/region and viewport; an exact crop does not
  certify a whole scene. Keep state and visual results separate.
- Record actual outcomes, including skips, failures and unrun checks. Never
  reuse an earlier pass as a new result or alter evidence to make it pass.

## Finish the task

Update the relevant ledger when behavior or evidence changes. Update the
overview/roadmap when a milestone closes, rather than duplicating changing
status in this file. Report changed behavior, commands/results, remaining
limits and the next concrete step. Before a requested commit, ensure other
writers are idle and review the staged file list and diff. Follow the user's
existing commit/push authorization; do not invent extra approval gates.
