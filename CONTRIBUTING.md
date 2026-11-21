# Contributing to PSIV

Thanks for helping. PSIV reproduces the US retail game's behavior; a change is
judged by whether it matches the cartridge, not by whether it seems better.

> **Just want to report a bug or weirdness?** See
> [docs/REPORTING.md](docs/REPORTING.md) (works for both players and AI
> assistants).

## Using an AI coding agent to contribute

If you point a coding agent at a fork of PSIV to fix a bug or work on an issue,
this section is the shortest path to a clean pull request that passes CI on the
first try.

1. **Pick an issue and check for open PRs.**
   - Look for [`good first issue`](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22),
     [`help wanted`](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22),
     [`gameplay-fidelity`](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Agameplay-fidelity),
     [`presentation`](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Apresentation),
     or [`post-release`](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Apost-release).
   - Check open PRs (`gh pr list -R TusanHomichi/PSIV`) so you do not duplicate
     work already in flight.
2. **Locate the owning code and tests first.**
   - [docs/FEATURE_MAP.md](docs/FEATURE_MAP.md) maps every subsystem to its
     owning files and focused test command.
   - [docs/RUNTIME_DESIGN.md](docs/RUNTIME_DESIGN.md#data-and-runtime-layers)
     defines the layer boundary: game rules live in `rust/psiv-core` /
     `rust/psiv-runtime`, never duplicated in Godot (`rust/psiv-godot`) or test
     drivers.
   - [docs/README.md](docs/README.md) indexes all retail research docs and the
     single-owner rules table.
3. **Know what runs with vs. without a local ROM.**
   - **Without a ROM:** You can run `cargo fmt --check`, `cargo clippy`,
     non-pack `cargo test` suites, `python3 tools/check_docs.py`,
     `python3 tools/size_guard.py`, and pure-logic Python tests. That is plenty
     for docs, tooling, setup/UI polish, packaging, and core refactors.
   - **With your own US retail ROM:** Follow
     [Build from source](README.md#build-from-source) to build `runtime-pack/`
     and compile `psiv-godot`, then run the pack-backed battle/field/runtime
     tests and launch the game with `tools/godot.py run`.
   - **External fork PRs vs. maintainer machine:** Some sections of
     [AGENTS.md](AGENTS.md) and [docs/AGENT_WORKFLOW.md](docs/AGENT_WORKFLOW.md)
     refer to the maintainer's internal pipeline (`steward`, `burrow`, `gate`,
     local oracle captures). External contributors do **not** need those: open a
     normal ready-for-review PR from your fork; hosted CI runs the public checks
     and the maintainer runs the local cartridge gate on review.
4. **Match CI's `house` and review rules on the first try.**
   - **Commit subjects & PR title:** `area: plain lowercase sentence` — no
     trailing period, no `feat:`/`fix:` prefix (e.g.
     `battle: fix Giwat targeting on dead slot`, `docs: add reporting guide`).
   - **PR body:** Fill in [`.github/pull_request_template.md`](.github/pull_request_template.md)
     (`## What and why`, `## Evidence`, `## Checks run`, `## Correction-ladder rung`).
   - **Language & file size:** New lasting code starts in Rust; keep code files
     under 1,000 lines (`python3 tools/size_guard.py`).
   - **No private/ROM content or host paths:** Never commit ROM bytes, packs,
     saves, captures, or copied tables (`tools/check_rom_tables.py` checks), and
     never write host-absolute home paths or pinned Godot/Rust version literals
     in docs (`tools/check_docs.py` checks).
   - **Honest reporting:** Report only the exact commands you ran and their
     actual results; note clearly if pack-backed or visual checks were skipped
     for lack of a local ROM/oracle.

## Set up

Follow the [developer guide](docs/DEVELOPMENT.md): it lists the tools, how to
build the data pack from your own ROM, and how to launch the game. Find the
code and focused test that own a feature in the [feature map](docs/FEATURE_MAP.md).

## Before you open a pull request

- Run the checks in the developer guide's [run checks](docs/DEVELOPMENT.md#run-checks)
  section that apply to your change, and say in the PR which ones you ran and
  what they reported. A gameplay fix comes with a regression test.
- Keep each concern in the layer the
  [runtime design](docs/RUNTIME_DESIGN.md#data-and-runtime-layers) assigns it.
- Cite the cartridge for a rule (a disassembly symbol or ROM offset). A
  deliberate difference from the original is recorded in the
  [source notes](docs/source-notes/README.md).
- Commit no ROM, pack, `generated/` output, extracted images or sound, captures
  or saves ([local inputs](AGENTS.md#protect-local-inputs-and-evidence)), and
  no table copied from the ROM: a test reads tables from the local pack. What
  the repository does hold from the game (observed-state fixtures, byte
  signatures, citations) is listed in the
  [README](README.md#license-and-third-party-code).

## Questions, issues and labels

Questions and ideas go to [Discussions](https://github.com/TusanHomichi/PSIV/discussions)
(Q&A for questions, Show and tell for mods); issues are for bugs and tracked
work. See the [bug reporting guide](docs/REPORTING.md) for what to include
(both humans and AI assistants) or open the
[issue form](https://github.com/TusanHomichi/PSIV/issues/new/choose) directly.

These are the labels:

| Label | Meaning |
| --- | --- |
| `bug` | Something isn't working |
| `gameplay-fidelity` | A rule differs from the cartridge; listed in the known differences |
| `presentation` | Drawing, animation, audio or timing differs; listed in the known differences |
| `accessibility` | A barrier for players with disabilities |
| `enhancement` | A new feature or request |
| `documentation` | Improvements or additions to documentation |
| `tooling` | Tests, oracle, gate, CI, route runner, workflow |
| `guard` | Make a recurring mistake unrepresentable or statically caught (correction-ladder rung 1 or 2) |
| `v1.0-blocker` | Must close before the 1.0 release |
| `post-release` | After 1.0: options, mods, nice-to-haves |
| `good first issue` | Small and self-contained, with the files and the check named in the issue |
| `help wanted` | The maintainer would welcome a pull request |

A known gap in the code is an issue link, not a comment explaining it away
([the codebase is memory](docs/AGENT_WORKFLOW.md#the-codebase-is-memory)).

## The record

Put the reasoning, the evidence and the checks you ran in the PR description
(the [pull request template](.github/pull_request_template.md) asks for them);
the [record policy](docs/AGENT_WORKFLOW.md#where-work-and-its-record-live)
says where everything else lives. Security problems are reported privately
([SECURITY.md](SECURITY.md)).

Coding agents, and humans who want the full working rules, read
[AGENTS.md](AGENTS.md) and the [workflow](docs/AGENT_WORKFLOW.md).
