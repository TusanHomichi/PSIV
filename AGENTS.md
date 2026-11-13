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
- Fix the whole cause: no workarounds, shims, fallbacks or half-migrations.
  When you replace something, delete the old path in the same change.
- One fact, one home. Each rule or fact lives in one place; link to it
  instead of restating it.
- Pipeline options live in the settings source: change them through its
  door, read them with `proj settings show`, and link the
  [settings reference](https://github.com/FieldmouseWorks/burrow/blob/main/docs/settings-reference.md)
  rather than restate a value or where one is kept.
- Before you edit, say what done looks like and how it will be checked.
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

- Rust by default, shell included: new code meant to last starts in Rust
  unless its platform picks the language, and a coin toss goes to Rust. A PR
  that adds Python, or grows Python and shell past a fix, says `Why not
  Rust:` (a file a tool generates, glue that only runs another program, a
  throwaway script, a tool still being tuned). Port existing Python and
  shell whenever Rust buys anything, system and pipeline code first.
- Work starts from an event (a push, a webhook, a unit finishing), not a
  clock. A PR that adds a timer or a cron schedule says `Why a timer:`.
- Split a code file before it passes 1,000 lines, or say `Why not split:`.
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

Before editing, check `AGENTS.override.md`, `git status --short --branch`,
worktrees and running build or game processes; preserve others' changes. Work
is GitHub issues: define an observable outcome and acceptance there first, and
new ideas don't change scope. Read only what the task needs:

- [Feature map](docs/FEATURE_MAP.md): each feature's owning paths and focused test.
- [Documentation index](docs/README.md): every subsystem document, and the
  [rule owners](docs/README.md#rule-owners), each rule's one home.
- [Agent workflow](docs/AGENT_WORKFLOW.md): record policy, roles, authority,
  verification, safe native runs, receipts, the correction ladder.
- [Development guide](docs/DEVELOPMENT.md): toolchain, build and launch, the
  gate, and the heavy-run lock every heavy check runs under.

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

- The ROM, `reference/`, generated packs, saves and captures are local assets.
  Honor [.gitignore](.gitignore); do not force-add them. The project's MIT
  license does not relicense original-game or third-party content.
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

The ROM and everything derived from it (packs, extracted tables, graphics,
text and audio, oracle frames, captures and fixture PNGs), `reference/`, saves,
and third-party sources such as `oracle/gpgx-src/`. How to keep them out of
Git and safe: [Protect local inputs and evidence](#protect-local-inputs-and-evidence).

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
