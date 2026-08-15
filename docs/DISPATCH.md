# Dispatch tiers (2026-08-15)

How work leaves the lead's hands. The lane protocol is identical at every
tier: a precise written brief, explicit file ownership (never a shared hot
file across two live lanes — including files the lead granted a "narrow
right" on), no git commands from lanes, final report to the lead, and the
lead reviews/verifies/commits with explicit FILE paths.

| tier | when | how |
|---|---|---|
| Fable (main loop) | judgment, design, adjudication, integration, partner discussion | this session |
| Claude Opus lanes | context-heavy RE/implementation slices needing the repo's full doctrine | Agent tool, `model: "opus"` |
| **sol high** | heavier well-specified implementation, external | `codexobs NAME < brief.txt` (gpt-5.6-sol, high; run dir printed first, `last.md` holds the report) |
| **flash 3.7 high** | simple/mechanical self-contained tasks | `agyobs NAME < brief.txt` (gemini-3.7-flash-high; agentic, very fast) |
| luna max | fast tier alternative | codex `-m` id unresolved on this account; ask Peter |

Notes:
- The launchers are Cairn's, on PATH (`~/.local/bin/{codexobs,agyobs,rxobs}`),
  with observability run dirs under `~/.cache/{codexobs,agyobs}`. The bare
  `codex` shell function is broken (dead codex-pixellab path); use
  `~/.local/bin/codex` or `codexobs`.
- `agy --print` alone is response-only in practice — it will describe the
  work instead of doing it. Always dispatch through `agyobs`.
- External output gets the same review bar as lane output: the lead reads
  it, runs the verification, and commits. First proof: `tools/pack_diff.py`
  (flash 3.7, one shot, self-test + live-pack verification).
