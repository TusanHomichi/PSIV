# Contributing with an LLM or coding agent

[Project instructions](../AGENTS.md) · [Setup and checks](DEVELOPMENT.md) ·
[Roadmap](ROADMAP.md) · [Documentation index](README.md)

Start an agent with the repository's `AGENTS.md`. If your tool does not load
that file, explicitly ask it to read it. Keep durable instructions there;
avoid maintaining separate, conflicting copies for individual tools.

## Give it a bounded task

Name a behavior, a relevant source/ledger, and an observable completion check.
For example:

> Read AGENTS.md and docs/PARTY_ORDER.md. Investigate the reported ORDER cancel
> regression against the existing rules and tests. Reproduce it before changing
> behavior, keep the fix scoped, and report the checks you actually ran. Preserve
> all local saves and unrelated edits.

That is an example task prompt, not a report of a known current regression.
For current work, choose an open item from the roadmap. Include the desired
commit/push scope and any time or compute limit in your task prompt.

## Find the smallest useful verification

All commands below run from the repository root. Prepare local prerequisites
from [DEVELOPMENT.md](DEVELOPMENT.md) before running asset-dependent tests.

| Change | Start with | Extend verification when needed |
| --- | --- | --- |
| ROM decoding / pack | Relevant `tests/test_*.py` file | Full Python suite and Rust pack consumers if the pack schema changes |
| Battle / field rules | Relevant `psiv-core` tests | Runtime integration, original-game comparison and native input |
| Camp / persistence | Relevant `psiv-runtime/tests/` target | Ordinary input, SAVE, fresh process, CONTINUE and state comparison |
| Godot UI / scenes | Build the extension and run the affected interaction | Inspect matching original/native captures; record viewport and region |
| Sound | Relevant `psiv-sound` tests | Register/timing comparisons and playback checks for the affected path |
| Documentation | Verify paths and commands, then `git diff --check` | Code tests only if source also changes |

Examples of selecting existing test targets:

```bash
PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_psiv_tools.py'
CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml -p psiv-runtime --test camp_order -- --test-threads=1
```

These examples do not cover every change. The full Python/Rust/format/Clippy
commands live in the setup guide. Oracle setup and its fast/full lanes live
in [oracle/README.md](../oracle/README.md). Do not rerun expensive suites after
an unchanged, applicable pass without a new reason.

## Reproduce without damaging the checkpoint

Before a campaign run, identify the source commit and dirty changes, pack
manifest hash, source save hash, driver and executable. Use a separate output
directory under `build/` and a separate `PSIV_SAVE_DIR`. Inspect driver paths
and stop conditions before launch. Record any fixture injection explicitly.

Check that the intended build is actually running: a loaded shared library
can outlive a source edit. Rebuild with the relevant Godot process stopped.
Use the documented desktop viewport for UI checks; tiny screenshots can clip
menus even when state assertions pass.

A failure is useful evidence. Keep its logs and resulting state separately
from a successful save. Do not repair party resources or flags behind the
driver's back and then describe the run as ordinary connected play.

## Receipt and handoff template

Put the concise result in the relevant subsystem ledger. Detailed logs and
ROM-derived artifacts stay local; commit reproduction instructions and hashes.
Use repo-relative paths in shared documentation so another checkout can follow
them. Identify unavailable local inputs instead of assuming they ship in Git.

```text
Task and intended behavior:
Source commit, branch, and relevant dirty changes:
Files changed and why:
Retail basis (symbols/offsets, ledger references):
Environment (tool versions, executable, viewport):
Inputs (pack identity, source save hash, fixture assumptions):
Commands actually run:
Results (passed / failed / skipped / not run):
Evidence type (state / native input / persistence / visual comparison):
Outputs (local paths and hashes, resulting party/map/flags where relevant):
Known limits and remaining failures:
Next concrete step:
Git state (commit/push status and any remaining edits):
```

For a small docs fix, omit irrelevant gameplay fields. For a campaign milestone,
retain enough input and save provenance to distinguish a continuation from an
isolated fixture. Keep dated historical results intact and add corrections
explicitly when new evidence overturns an earlier interpretation.
