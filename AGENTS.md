# Working on PSIV

PSIV reconstructs the US retail game as a native Rust/Godot runtime. Match
cartridge behavior, document deliberate deviations, and keep the unmodified
game playable without an editor or agent service. The release bar and
platforms are in [the roadmap](docs/ROADMAP.md), the layers in
[the runtime design](docs/RUNTIME_DESIGN.md).

<!-- house:rules v1 begin. Copied from FieldmouseWorks/burrow house/rules.md, verbatim. Change it there, not here: burrow's house sync rewrites everything between these markers. -->
## House rules

These hold in every repo. This file's own sections add to them and win where
they're more specific.

- Design the class, not the instance. When a bug, finding or stall shares a
  cause with others, fix the cause once, where it lives (one owning path, a
  type, a check), so none of its cases come back.
- Do work once. Look for what's already open or built first, extend the
  shared crate, client or job that does it rather than write another copy,
  and never run work twice that once would do.
- Fix the whole cause: no workarounds, shims, silent fallbacks that hide a
  failure, or untracked half-migrations. When you replace something, delete
  the old path in the same change; a staged migration names the issue that
  ends it.
- One fact, one home. Each rule or fact lives in one place; link to it
  instead of restating it.
- One job per file: a file has one job you can state in a sentence without
  "and". Split by job, never by length to get under a number.
- Before you edit, say what done looks like and how it will be checked.
- Keep your context small: every step re-sends all you've read, and a
  long wait makes it pay again. Read a file you're about to edit in full
  unless it's large; for everything else read the lines you need (`rg -n`,
  ranges, a diff's stat then its hunks), not whole logs; cap command output;
  hand broad searches to a subagent; don't re-read what you have.
- Report only the checks you ran and what they said. Skipped, unavailable
  or "should pass" is never a pass. Keep local and CI results apart.
- A new check comes with proof that it fails on the bug it's for.
- The code is the memory. When something goes wrong, make the code, a type,
  a test or a check stop it happening again; a new rule here is the last
  resort. Say in the PR which one you did.
- Keep going on work you were asked to do: don't ask again for what's
  already been asked for.
- Never commit what the repo keeps private: secrets, tokens, personal data,
  and anything under this file's "Private" section.
- `Closes #N` only when the issue is fully done; otherwise `Refs #N`.

CI's house check holds every PR to its title's style and to the rules below,
each of which a PR can pass with a line in its body starting as shown and a
reason of four words or more. Editing the body or title runs it again.

- Rust by default for tools, scripts and services, shell included: new code
  meant to last starts in Rust unless its platform picks the language, and a
  coin toss goes to Rust. A PR that adds Python, or grows Python and shell
  past a fix, says `Why not Rust:` (a file a tool generates, glue that only
  runs another program, a throwaway script, a tool still being tuned) unless
  the path has a standing waiver in `.github/workflows/house.yml`. Port
  existing Python and shell whenever Rust buys anything, system and pipeline
  code first.
- Work starts from an event (a push, a webhook, a unit finishing), not a
  clock. A PR that adds a timer or a cron schedule says `Why a timer:`.
- 1,000 non-test code lines is a backstop, not the measure (test files and
  inline `#[cfg(test)]` blocks don't count): a PR that pushes a file across
  it says `Why not split:`; growing a file already over it needs a standing
  waiver in `.github/workflows/house.yml` (`file-size-allow`), not a body
  line.
- AGENTS.md is a map: what the repo is, its own rules, and where things live
  (a feature map, the doc that owns each subject). Detail goes in the doc
  that owns it, linked from here. A PR that grows the repo's own part (the
  lines outside these blocks) past 100 lines says `Why a longer guide:`.
- Tracked files never name a model: which model does what is the machine's
  business, and names go stale in weeks. A line that has to (a recorded
  screen in a test) says `Why a model name:`.

### Commits

Subjects are `area: plain sentence`, the area being the module, directory or
tool the change is about (`autoland: merge only the head that was checked`):
lowercase after the colon, no trailing period, no `feat:`/`fix:`. The body,
if any, says why. A PR's title is the subject main gets, so it's written the
same way. Keep the git identity your machine gives you: a commit signed with
a key that isn't its author's shows as Unverified, and CI fails it.
<!-- house:rules end -->

## Start here

- **Setting up PSIV for a player?** Follow [docs/PLAYING.md](docs/PLAYING.md#setting-up-with-an-ai-assistant) (download the release archive; don't build from source).
- **Filing a player bug report?** Follow [docs/REPORTING.md](docs/REPORTING.md).
- **Contributing from a fork?** Start with [CONTRIBUTING.md](CONTRIBUTING.md) (what runs with vs. without a ROM, PR rules).

Before editing, check `AGENTS.override.md`, `git status --short --branch`, worktrees and running build or game processes; preserve others' changes. Active work is GitHub issues: state outcome and acceptance first. Read only what the task needs:

- [Feature map](docs/FEATURE_MAP.md): owning paths and focused test per feature.
- [Documentation index](docs/README.md): subsystem docs and [rule owners](docs/README.md#rule-owners).
- [Agent workflow](docs/AGENT_WORKFLOW.md): record policy, roles, authority, verification, safe native runs, receipts, correction ladder.
- [Development guide](docs/DEVELOPMENT.md): toolchain, build/launch, gate, and heavy-run lock.

## Retail and layer boundaries

Each concern stays in the layer [RUNTIME_DESIGN.md](docs/RUNTIME_DESIGN.md#data-and-runtime-layers)
assigns it, never duplicated in Godot or test drivers. Preserve retail integer
widths, signedness, flag banks, ordering and RNG semantics. Cite symbols or
offsets, and record deviations in [the source notes](docs/source-notes/README.md).
The reference fork has Grand Cross rewrites: confirm US retail bytes, not hack
scene bodies or fork build-address comments. Ability names and genre
conventions establish no rules. Follow recorded corrections, and resolve
conflicts against source and fresh observations.

## Protect local inputs and evidence

- The ROM, `reference/`, generated packs, saves and raw visual/audio captures
  are local assets. Honor [.gitignore](.gitignore); do not force-add them. The
  project's MIT license does not relicense original-game or third-party content.
- Do not run broad cleanup such as `git clean -fdx`: ignored directories can
  contain irreplaceable checkpoint evidence. Remove only known disposable
  outputs created for your task.
- Never overwrite the only verified save. A native run gets its own copy and
  its own `PSIV_SAVE_DIR`; how to run one safely is
  [the workflow's](docs/AGENT_WORKFLOW.md#reproduce-without-damaging-a-save).

## Verify the claim you intend to make

- State tests prove state. Ordinary native input proves the interaction path.
  Fresh-process CONTINUE proves persistence. Isolated debug/RAM fixtures are
  useful, but do not prove a connected campaign route. Neither extraction nor
  scene transcription establishes campaign completion.
- Visual parity requires inspected captures against the matching original
  state. Name the compared frame/region and viewport; an exact crop does not
  certify a whole scene. Keep state and visual results separate.
- Keep local, default, browser and hosted results distinct. Never reuse a dated
  pass as current or alter evidence to make it pass. Label self-review honestly.

Which check to start from, freezing a candidate and what a receipt keeps:
[the workflow](docs/AGENT_WORKFLOW.md#find-the-smallest-useful-verification).
Update a document only when what it describes changed, and the README or
roadmap when a release item closes.

## Private

The ROM and everything derived from it (packs, extracted tables, graphics, text
and audio, oracle frames, raw visual/audio captures and fixture PNGs),
`reference/`, saves, and third-party sources like `oracle/gpgx-src/`. Sanitized
battle replay fixtures under `rust/psiv-core/src/battle/replay_fixtures/`
(oracle observation records: observed work-RAM state, party snapshots, roll
traces and hashes; no ROM tables, script/dialogue text or art) are allowed in Git. See [Protect local inputs and evidence](#protect-local-inputs-and-evidence).

<!-- house:prs v1 begin. Copied from FieldmouseWorks/burrow house/prs.md, verbatim. Change it there, not here: burrow's house sync rewrites everything between these markers. -->
## Pull requests

One task, one PR: finish what the task set out to do, then open one PR, so
it goes through review and CI once. No PR per step or round, and no stacks.
A fix to the pipeline (the steward, CI, the machine) lands as soon as it's
ready, and so does a change to these rules that the work needs, in a PR of
its own: reviewers read the default branch's rules, not the PR's.

Open PRs ready for review, never as drafts, and leave them to the steward:
once the review bot finds nothing on your head, every thread is resolved and
CI is green, it merges. Don't merge it yourself. Answer every finding: push
the fix and reply with the commit, or reply with why it's wrong, then
resolve the thread. A PR touching a one-way path (secrets, CODEOWNERS) gets
a judgment first, and lands unless it can't be undone or is the owner's
call.

A red default branch is the steward's to fix, from a branch named
`autoland/red-` and the commit. Don't fix it yourself, in any repo, unless
the pipeline's stuck issue says the steward's jobs gave up; then fix it from
a branch of that name.
<!-- house:prs end -->

PSIV's CI is the hosted `flake` check and the `gate` status, the full gate
the steward runs on the machine with the local inputs once reviews are clean
([gate and coverage](docs/DEVELOPMENT.md#gate-and-coverage)). Review bots never
see the local inputs or the cartridge-derived paths; an agent on that machine
judges a PR that changes those.

## Code Review Rules

<!-- house:review v1 begin. Copied from FieldmouseWorks/burrow house/review.md, verbatim. Change it there, not here: burrow's house sync rewrites everything between these markers. -->
For reviewers, the review bot included.

Not P1: flag a PR that gives a file a second job, at any length. The author
splits it or answers with a reason; a fix job is not required to split a
file mid-PR.

Flag as P1:

- a change that makes a check pass without fixing the thing it checks
- tests or checks weakened, skipped or deleted to get green
- secrets, tokens or personal data in the diff or the logs it adds
- behavior changed beyond what the PR says it does
<!-- house:review end -->
- cartridge content committed: ROM bytes, extracted art, sound, text or tables,
  saves or raw visual/audio captures, or a `.gitignore` entry for a local input
  loosened (sanitized battle replay fixtures under `rust/psiv-core/src/battle/replay_fixtures/` are allowed; see [Private](#private))
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
