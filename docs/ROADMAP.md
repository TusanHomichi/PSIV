# Roadmap

[Project overview](../README.md) · [Contributing](../CONTRIBUTING.md) ·
[Documentation index](README.md)

## Goal

Post PSIV publicly, let people play and mod it, then maintain it.

## Decisions (owner, 2026-10-14)

- **The 1.0 bar:** the unmodified game is completable natively from New Game
  to the Ending. Remaining fidelity and presentation gaps ship as a list of
  known differences (the `gameplay-fidelity` and `presentation` issues) and
  are fixed in maintenance.
- **Platforms for 1.0:** Linux and the Steam Deck. Windows comes later, as its
  own work item.
- **The record is commits, issues and pull requests.** Run logs and ledgers
  are not kept in the tree; git history holds the old ones. The docs carry no
  task graphs: active work is a GitHub issue with one of the labels
  `v1.0-blocker`, `gameplay-fidelity`, `presentation`, `tooling` or
  `post-release`.

## 1.0 checklist

The open items are the
[`v1.0-blocker` issues](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Av1.0-blocker).

- [ ] **The full campaign.** The headless route reaches the Ending, and the
  native game replays it.
- [ ] **No memory leak** in long native sessions
  ([#92](https://github.com/TusanHomichi/PSIV/issues/92)).
- [ ] **No untranscribed trigger scenes.** The event census allowlists reach
  zero ([#56](https://github.com/TusanHomichi/PSIV/issues/56),
  [#71](https://github.com/TusanHomichi/PSIV/issues/71),
  [#81](https://github.com/TusanHomichi/PSIV/issues/81),
  [#83](https://github.com/TusanHomichi/PSIV/issues/83)).
- [ ] **Hosted CI** for the checks that need no ROM
  ([#15](https://github.com/TusanHomichi/PSIV/issues/15)).
- [ ] **A release package** for Linux and the Steam Deck.
- [ ] **A README for players:** build the pack, run the game, controls, known
  differences.

## After 1.0

- Windows support.
- A mod overlay loader, so mods sit beside the original pack instead of
  editing it.
- Presentation fidelity: whole-scene staging, ability animations, audio timing
  ([`presentation` issues](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Apresentation)).
- Selectable battle RNG modes
  ([#6](https://github.com/TusanHomichi/PSIV/issues/6)).
- Everything else labeled
  [`post-release`](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Apost-release).

## What "done" means

A gameplay item closes when the mechanic has a cartridge-backed rule, a
regression test, and a native input check where one applies. A presentation
item closes on a comparison against the matching original frame or region.
The [workflow](AGENT_WORKFLOW.md) owns how work is planned, verified and
handed off.
