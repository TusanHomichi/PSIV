# Working on PSIV

PSIV reconstructs the US retail game as a native Rust/Godot runtime. Match
cartridge behavior, document deliberate deviations, and keep the unmodified
game playable without an editor or agent service. The release bar and
platforms are in [the roadmap](docs/ROADMAP.md).

<!-- house:rules v1 begin. Copied from FieldmouseWorks/burrow house/rules.md, verbatim. Change it there, not here: burrow's house sync rewrites everything between these markers. -->
## House rules

These hold in every repo. This file's own sections add to them and win where
they say something more specific.

- Fix the whole cause. No workarounds, shims, fallbacks or half-migrations:
  when you replace something, delete the old path in the same change.
- One fact, one home. Each rule or fact lives in one place; link to it instead
  of restating it.
- Before you edit, say what done looks like and how it will be checked. Simple
  work gets a short plan, not a ceremony.
- Report only the checks you actually ran, and what they said. Skipped,
  unavailable or "should pass" is never a pass. Keep local and CI results
  apart.
- A new check comes with proof that it fails on the bug it's for.
- Touching a file over about 1,000 lines? Split it in that change.
- The code is the memory. When something goes wrong, make the code, a type, a
  test or a check stop it happening again; a new rule in AGENTS.md is the last
  resort. Say in the PR which one you did.
- Keep going on work you were asked to do. Don't stop to ask again for what's
  already been asked for.
- Never commit what the repo keeps private: secrets, tokens, personal data, and
  anything listed under this file's "Private" section.
- `Closes #N` only when the issue is fully done; otherwise `Refs #N`.
- Tracked files never name a model. Which model does what is the machine's
  business, and model names go stale in weeks.

### Commits

Subjects are `area: plain sentence`, where the area is the module, directory or
tool the change is about: `autoland: merge only the head that was checked`.
Lowercase after the colon, no trailing period, no `feat:`/`fix:` prefixes. The
body, when there is one, says why.
<!-- house:rules end -->

## Start here

Check applicable instructions (including `AGENTS.override.md`),
`git status --short --branch`, worktrees and relevant build/game processes before
editing. Preserve others' changes. Active work is GitHub issues
([record policy](docs/AGENT_WORKFLOW.md#where-work-and-its-record-live)); use
[the roadmap](docs/ROADMAP.md) for the release bar and
[the workflow](docs/AGENT_WORKFLOW.md) for planning, authority, evidence and handoff.
Read only the relevant [subsystem document](docs/README.md) and source; use
[DEVELOPMENT.md](docs/DEVELOPMENT.md) when preparing checks and
[RUNTIME_DESIGN.md](docs/RUNTIME_DESIGN.md) for layer boundaries.
Find a feature's owning paths and focused test in the [feature map](docs/FEATURE_MAP.md).

Define an observable outcome and acceptance in the issue or PR before
implementation. New ideas do not change scope, permissions or acceptance.

## Roles

Roles, the delegation contract and routing honesty are owned by
[the workflow](docs/AGENT_WORKFLOW.md#roles-and-delegation).

## Fix the class, not the instance

- A defect, review finding or correction that exposes a recurring mistake is
  fixed as a class, at the strongest rung of the
  [correction ladder](docs/AGENT_WORKFLOW.md#correction-ladder). The PR names
  the rung it used.
- The codebase is memory: keep
  [one paved path per concern, with gaps as issue links](docs/AGENT_WORKFLOW.md#the-codebase-is-memory).
- Verify through the repository's own entry points; a check needed twice is
  promoted into the repo ([DEVELOPMENT.md](docs/DEVELOPMENT.md#gate-and-coverage)).
- Each rule has one [owning document](docs/README.md#rule-owners); link to it
  instead of restating it.

## Retail and layer boundaries

Each concern stays in the layer [RUNTIME_DESIGN.md](docs/RUNTIME_DESIGN.md#data-and-runtime-layers)
assigns it; do not duplicate rules in Godot or test drivers.
Preserve retail integer widths, signedness, flag banks, ordering and RNG semantics.
Cite symbols/offsets and record deviations in [docs/source-notes/README.md](docs/source-notes/README.md).
The reference fork contains Grand Cross rewrites: confirm US retail bytes, not
hack scene bodies or fork build-address comments. Ability names and genre
conventions do not establish rules. Follow recorded corrections; resolve conflicts
against source and fresh observations.

## Protect local inputs and evidence

- The ROM, `reference/`, generated packs, saves and captures are local assets.
  Honor [.gitignore](.gitignore); do not force-add them. The project's MIT
  license does not relicense original-game or third-party content.
- Do not run broad cleanup such as `git clean -fdx`: ignored directories can
  contain irreplaceable checkpoint evidence. Remove only known disposable
  outputs created for your task.
- Copy a source save to a separate run directory, set `PSIV_SAVE_DIR`
  explicitly, and retain its hash. Never overwrite the only verified save: the
  extension refuses a `--script` run without that variable, before it touches
  a save directory, so a driver cannot fall back to the checkout's `saves/`.
- Inspect a native driver's configuration before running it. Existing drivers
  can contain local paths, special fixtures or long campaign routes. Missing
  `build/` evidence on a fresh clone is expected; report that limitation.
- Rebuilding the GDExtension during a Godot run is safe: a relink writes a new
  file and the mapped one survives (cargo uplifts the library, and the linker,
  mold here, writes a new inode). Repository tools run Godot from a run root
  with its own library copy, and the editor reloads the checkout's library on
  focus by design. Never copy over a built library in place: that is the one
  remaining crash.

## Verify the claim you intend to make

- Start from the smallest useful check for the change
  ([workflow table](docs/AGENT_WORKFLOW.md#find-the-smallest-useful-verification));
  add meaningful regression coverage for gameplay fixes and a negative control
  for a new correctness check.
- The gate's commands, prerequisites and coverage limits, and the shared lock
  that serializes heavy runs, are in [DEVELOPMENT.md](docs/DEVELOPMENT.md#run-checks).
  Run every heavy check through the
  [heavy-run lock](docs/DEVELOPMENT.md#heavy-run-lock) in its class; the lock
  decides what may run side by side within the machine's memory budget.
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

Continue scoped repairs until acceptance passes, under the
[effort policy and permission record](docs/AGENT_WORKFLOW.md#authority-effort-and-continuation).
Put the result, checks and evidence in the PR; update a document only when the
behavior, commands or retail findings it describes changed, and the README or
roadmap when a release item closes. Continue ready authorized tasks without
asking again. Before a requested commit,
ensure writers are idle and inspect the staged list/diff. Finish by opening the
PR ([pull requests](#pull-requests)) and through owned cleanup; preserve
unrelated work and evidence.
Report actual checks, limits, Git state and the next concrete step.

## Pull requests

Open PRs ready for review, never as drafts, and leave them to the steward: it
asks the review bot for a review, and once the bot finds nothing on your head,
every review thread is resolved and CI is green, it merges the PR. Fix or
answer every finding: push the fix and reply with the commit, or reply with
why it's wrong, then resolve the thread. A PR touching a one-way path
(secrets, migrations, CODEOWNERS) gets a judgment first: it lands unless it
can't be undone or is the owner's to decide.

PSIV's CI is the `gate` status: once the reviews are clean, the steward has
the full gate run on the PR's head on the machine that holds the local inputs
([gate and coverage](docs/DEVELOPMENT.md#gate-and-coverage)). The review bots
never see the local inputs or the tracked paths derived from the cartridge; a
PR that changes those is judged by an agent on that machine instead.

## Code Review Rules

For reviewers, the review bot included. Flag as P1:

- a change that makes a check pass without fixing the thing it checks
- tests or checks weakened, skipped or deleted to get green
- secrets, tokens or personal data in the diff or the logs it adds
- behavior changed beyond what the PR says it does
- cartridge content committed: ROM bytes, extracted art, sound, text or
  tables, saves or captures, or a `.gitignore` entry for a local input
  loosened ([#149](https://github.com/TusanHomichi/PSIV/issues/149) is moving
  the ones already tracked out)
- a game rule without its retail basis (a disassembly symbol or ROM offset), or
  a deliberate difference from the cartridge not recorded in
  [the source notes](docs/source-notes/README.md)
- retail integer widths, signedness, flag banks, ordering or RNG semantics
  changed without new evidence
- a rule duplicated in Godot or a test driver instead of the layer
  [RUNTIME_DESIGN.md](docs/RUNTIME_DESIGN.md#data-and-runtime-layers) assigns it
- evidence edited to pass: a capture, receipt, digest or a halt added to the
  resume manifest without its issue
- a native run that can write a save without its own `PSIV_SAVE_DIR`
