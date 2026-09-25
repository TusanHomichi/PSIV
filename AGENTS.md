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
Find a feature's owning paths and focused test in the [feature map](docs/FEATURE_MAP.md).

Define an observable outcome and acceptance before implementation. Substantive
dependent work uses one graph; simple changes use a short plan. Verified results
unlock dependencies. New ideas do not change scope, permissions or acceptance.

## Roles

An orchestrator scopes, decides architecture and permissions, reviews and
integrates; delivery workers own implementation through checks, repair,
evidence and closeout. The split, delegation contract and routing honesty
rules are in [the workflow](docs/AGENT_WORKFLOW.md#roles-and-delegation).
Which model or tool fills a role is host-local configuration, never tracked
guidance.

## Fix the class, not the instance

- A defect, review finding or correction that exposes a recurring mistake is
  fixed as a class, at the strongest rung of the
  [correction ladder](docs/AGENT_WORKFLOW.md#correction-ladder). The PR names
  the rung it used.
- The codebase is memory: keep
  [one paved path per concern](docs/AGENT_WORKFLOW.md#the-codebase-is-memory).
  A known gap is an issue link, not a code comment.
- Verify through the repository's own entry points; a check needed twice is
  promoted into the repo ([DEVELOPMENT.md](docs/DEVELOPMENT.md#gate-and-coverage)).
- Each rule has one [owning document](docs/README.md#rule-owners); link to it
  instead of restating it.

## Retail and layer boundaries

Keep deterministic rules in `psiv-core`, orchestration in `psiv-runtime`, pack
decoding/schema in Python/`psiv-data`, sound in `psiv-sound`, and presentation/input
in Godot/the bridge. Do not duplicate rules in Godot or test drivers.
Preserve retail integer widths, signedness, flag banks, ordering and RNG semantics.
Cite symbols/offsets and record deviations in [docs/source-notes/README.md](docs/source-notes/README.md).
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

- Start from the smallest useful check for the change
  ([workflow table](docs/AGENT_WORKFLOW.md#find-the-smallest-useful-verification));
  add meaningful regression coverage for gameplay fixes and a negative control
  for a new correctness check.
- The gate's commands, prerequisites and coverage limits are in
  [DEVELOPMENT.md](docs/DEVELOPMENT.md#run-checks). Run Python, Rust and oracle
  heavy checks sequentially; parallel pack-loading runs exhaust memory.
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
