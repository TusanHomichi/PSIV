# Contributing to PSIV

Thanks for helping. PSIV reproduces the US retail game's behavior; a change is
judged by whether it matches the cartridge, not by whether it seems better.

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
- Commit no ROM-derived data: not the ROM, the generated pack, extracted
  images, captures or saves
  ([local inputs](AGENTS.md#protect-local-inputs-and-evidence)).

## Questions, issues and labels

Questions and ideas go to [Discussions](https://github.com/TusanHomichi/PSIV/discussions)
(Q&A for questions, Show and tell for mods); issues are for bugs and tracked
work. A bug report gives the commit, the platform, your ROM's SHA-256 check and,
when it helps, a save; the [issue form](https://github.com/TusanHomichi/PSIV/issues/new/choose)
asks for each.

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
