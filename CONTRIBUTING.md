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
- Keep each concern in its layer: rules in `psiv-core`, orchestration in
  `psiv-runtime`, ROM decoding in Python, drawing and input in Godot
  ([runtime design](docs/RUNTIME_DESIGN.md)).
- Cite the cartridge for a rule (a disassembly symbol or ROM offset). A
  deliberate difference from the original is recorded in the
  [source notes](docs/source-notes/README.md).
- Commit no ROM-derived data: not the ROM, the generated pack, extracted
  images, captures or saves
  ([local inputs](AGENTS.md#protect-local-inputs-and-evidence)).

## Issues and labels

Work is tracked in GitHub issues. Every open issue carries one of these labels:

| Label | Meaning |
| --- | --- |
| `v1.0-blocker` | Must close before the 1.0 release |
| `gameplay-fidelity` | A rule differs from the cartridge |
| `presentation` | Drawing, animation, audio or timing differs |
| `tooling` | Tests, oracle, gate, CI, route runner, workflow |
| `post-release` | After 1.0: options, mods, nice-to-haves |

A known gap in the code is an issue link, not a comment explaining it away
([the codebase is memory](docs/AGENT_WORKFLOW.md#the-codebase-is-memory)).

## The record

Pull requests, issues and commit history are the project's record. Put the
reasoning, the evidence and the checks you ran in the PR description; do not
add status logs or ledgers to the docs. Docs describe how the game and the
tools work now.

Coding agents, and humans who want the full working rules, read
[AGENTS.md](AGENTS.md) and the [workflow](docs/AGENT_WORKFLOW.md).
