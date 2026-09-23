# Working on PSIV

PSIV reconstructs the US retail game as a native Rust/Godot runtime. Match
cartridge behavior, document deliberate deviations, and keep the unmodified
game playable without an editor or agent service. Modding follows a complete
base-game playthrough.

## Start here

Check applicable instructions (including `AGENTS.override.md`),
`git status --short --branch`, worktrees and relevant build/game processes before
editing. Preserve others' changes. Use [README.md](README.md) for current scope,
[the roadmap](docs/ROADMAP.md) for the canonical task-graph pointer, and
[the workflow](docs/AGENT_WORKFLOW.md) for planning, authority, evidence and handoff.
Read only the relevant [subsystem ledger](docs/README.md) and source; use
[DEVELOPMENT.md](docs/DEVELOPMENT.md) when preparing checks and
[RUNTIME_DESIGN.md](docs/RUNTIME_DESIGN.md) for layer boundaries.

Define an observable outcome and acceptance before implementation. Substantive
dependent work uses one graph; simple changes use a short plan. Verified results
unlock dependencies. New ideas do not change scope, permissions or acceptance.

## Model roles

Select model and effort explicitly. This is the owner's standing division of
work, not a loose preference:

- `gpt-6-astra` / `max`: orchestration: concise scoping, architecture and
  permission decisions, graph selection, review and coordination. Astra does
  not routinely write code, tests/fixtures/harnesses or docs, or run and monitor
  repeated check loops.
- `gpt-6-sol` / `max`: owns complex implementation end-to-end, including
  relevant checks, debugging and repairs, candidate freeze, and evidence handoff.
- `gpt-6-luna` / `max`: owns bounded exploration and routine implementation,
  test runs, log triage, receipts and routine docs through checks and closeout.
- Claude Code host (owner decision, 2026-09-23): `claude-opus-5-5` holds the
  orchestration role above; DeepSeek `deepseek-flash` (`deepseek-v4-flash`) at
  `max` effort, launched through `ds-lane` (Reasonix in an isolated worktree),
  owns implementation in the Sol/Luna roles. See the workflow's
  [Claude Code host section](docs/AGENT_WORKFLOW.md#claude-code-host-ds-lane).
- DeepSeek outside that route: optional supervised narrow helper, only for a
  concrete benefit; independently verify results and record actual routing,
  corrections and reliability.

Delegate complete outcomes with exact inputs, observable acceptance, file and
resource ownership, forbidden writes, and concurrency constraints. The assigned
worker owns implementation, relevant checks, failure repair, evidence and
closeout; a failed check does not return routine work to Astra. Escalate concrete
architectural or permission blockers. The parent remains accountable for
independent review and integration, but that does not mean personally doing
implementation or evidence plumbing. Review concise receipts with proportionate
diff/raw-artifact spot checks; do not duplicate the worker's investigation or
rerun passing checks without a concrete cause. Keep context packets bounded and
results compact, with evidence paths. Long-running checks and process monitoring
stay worker-owned; no parent busy polling. Preserve independent verification,
serialized expensive runs, safety rules, and writers-idle integration.

Only an explicit owner instruction or an actually unavailable route changes
this division. Report unavailable routes and actual requested/observed routing;
never silently fall back to Astra or claim an instruction edit changed a running
model. See the workflow for host verification and role exceptions.

## Retail and layer boundaries

Keep deterministic rules in `psiv-core`, orchestration in `psiv-runtime`, pack
decoding/schema in Python/`psiv-data`, sound in `psiv-sound`, and presentation/input
in Godot/the bridge. Do not duplicate rules in Godot or test drivers.
Preserve retail integer widths, signedness, flag banks, ordering and RNG semantics.
Cite symbols/offsets and record deviations in [SOURCE_NOTES.md](SOURCE_NOTES.md).
The reference fork contains Grand Cross rewrites: confirm US retail bytes, not
hack scene bodies or fork build-address comments. Ability names and genre
conventions do not establish rules. Follow ledger corrections; resolve conflicts
against source and fresh observations.

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

- Use focused existing checks; add meaningful regression coverage for gameplay
  fixes and a negative control for a new correctness check. Documentation needs
  link/command review and `git diff --check`, not a full gameplay run.
- Full-gate commands and prerequisites are in [DEVELOPMENT.md](docs/DEVELOPMENT.md).
  Run Python, Rust and oracle heavy checks sequentially; full Rust tests require
  `CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1`.
- Freeze a reviewed candidate before expensive gates. Relevant code, tests or
  gate-input changes invalidate affected results. Preserve actual commands,
  revisions, hashes, timing, exit status, failures and skips; inspect raw evidence.
- State tests prove state. Ordinary native input proves the interaction path.
  Fresh-process CONTINUE proves persistence. Isolated debug/RAM fixtures are
  useful, but do not prove a connected campaign route. Neither extraction nor
  scene transcription establishes campaign completion.
- Visual parity requires inspected captures against the matching original
  state. Name the compared frame/region and viewport; an exact crop does not
  certify a whole scene. Keep state and visual results separate.
- Keep local, default, browser and hosted results distinct. Never reuse a dated
  pass as current or alter evidence to make it pass. Label self-review honestly.

## Finish the task

Continue scoped repairs until acceptance passes, with no fixed cycle limit
(owner decision, 2026-09-23). This does not expand scope or grant publication or
spending authority. Reuse existing authorization; see the workflow's permission
record. Update the relevant ledger, and the overview/roadmap when a milestone
closes. Archive completed graphs before advancing; retain one next action.
Continue ready authorized tasks without asking again. Before a requested commit,
ensure writers are idle and inspect the staged list/diff. Finish through any
authorized integration and owned cleanup; preserve unrelated work and evidence.
Report actual checks, limits, Git state and the next concrete step.
