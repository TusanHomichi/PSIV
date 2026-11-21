# Reporting Bugs and Playtest Issues

[Project overview](../README.md) · [Player's guide](PLAYING.md) · [Contributing](../CONTRIBUTING.md) · [Feature map](FEATURE_MAP.md)

This guide is for **players** and **AI assistants / coding agents** filing an
issue on a player's behalf. If someone points you at this repository and asks
you to report a bug, crash, or weird behavior they hit while playing PSIV,
follow the steps below.

> **Want to fix a bug or contribute code instead?** Read
> [CONTRIBUTING.md](../CONTRIBUTING.md) and [AGENTS.md](../AGENTS.md).

## Quick checklist for AI assistants

1. **Check known issues and deliberate deviations first.**
   - Search open issues before filing a duplicate:
     - All open issues: `gh issue list -R TusanHomichi/PSIV --state open --search "<keywords>"`
     - Known gameplay differences: [`label:gameplay-fidelity`](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Agameplay-fidelity)
     - Known visual/audio differences: [`label:presentation`](https://github.com/TusanHomichi/PSIV/issues?q=is%3Aissue+is%3Aopen+label%3Apresentation)
   - Check [deliberate fixes to retail bugs](source-notes/README.md#deliberate-fixes-to-retail-battle-bugs)
     so you do not report an intentional fix as a regression.
2. **Gather local facts from the player's machine (with permission).**
   - See [What to collect](#what-to-collect) below for exact file paths on
     Linux/Steam Deck, Windows, and macOS.
3. **Never include private or copyrighted data.**
   - See [What never to attach](#what-never-to-attach).
4. **Format the issue using the template below** so maintainers can locate the
   owning subsystem immediately.
5. **Confirm with the player before posting publicly**, especially if attaching
   a save file or screenshot. File with `gh issue create`, or print the ready-to-paste
   Markdown and link to the [bug form](https://github.com/TusanHomichi/PSIV/issues/new?template=bug.yml).

## What to collect

Gather as many of these as apply — a partial report is still useful if you note
what is missing.

### 1. Version and platform

- **PSIV version:** Release archive/folder version (e.g. `1.0.0`, also noted in
  the download folder name) or `git rev-parse --short HEAD` for a source checkout.
- **OS and architecture:** e.g. Ubuntu 24.04 x86-64, Windows 11 x86-64, macOS 15
  Apple Silicon, SteamOS / Steam Deck.
- **GPU / renderer & display:** Shown near the top of `logs/godot.log` (e.g.
  renderer, GPU name, and on Linux whether running under X11, Wayland, or Gamescope).
- **Input device:** Keyboard, or controller model (Xbox, DualSense, Steam Deck, etc.).
- **ROM check:** Did the ROM pass PSIV's SHA-256 verification
  (`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`)? If the
  issue happened on the setup screen itself, include the exact error text shown.

### 2. Game log (`logs/godot.log`)

PSIV writes its runtime log inside the user data folder:

| System | User data folder | Log file |
| --- | --- | --- |
| Linux / Steam Deck | `~/.local/share/PSIV` (or `$XDG_DATA_HOME/PSIV`) | `logs/godot.log` |
| Windows | `%APPDATA%\PSIV` | `logs\godot.log` |
| macOS | `~/Library/Application Support/PSIV` | `logs/godot.log` |

Include:
- The engine/GPU startup lines near the top of `godot.log`.
- Any `ERROR:`, `WARNING:`, `pack setup:`, `pack builder:`, or panic/backtrace lines.
- The last ~40 lines if the game crashed or hung.

### 3. Save file (`saves/`)

A save file taken right before the bug happens is the fastest way to reproduce
gameplay, battle, cutscene, and menu issues.

- Saves live in the `saves/` subfolder of the user data folder above (the three
  in-game save slots).
- Check modification times to identify the most recent slot, and ask the player
  if they are okay attaching it to the GitHub issue (save files contain only
  small game-state slots, no ROM or personal data).

### 4. Where in the game and what happened

Be specific enough that someone can jump straight to the scene or battle:

- **Location:** Exact town, dungeon floor, or overworld planet (e.g. Piata
  Academy, Birth Valley B2, Motavia overworld near Aiedo).
- **Story point & party:** Current active party members, approximate levels,
  and the last story event completed.
- **In battle:** Enemy/formation names, exact technique/skill/item/macro used,
  and whether it happened on a specific turn or animation.
- **Visual/audio weirdness:** A screenshot or short clip helps hugely. Note
  whether the game window was fullscreen or windowed and at what resolution.
- **Original cartridge behavior:** What the US Genesis cartridge does at the
  same point, if the player knows (writing "unsure" is totally fine).

## What never to attach

- **Never** upload, paste, or link the ROM image, anything inside
  `runtime-pack/`, or extracted sprites, tiles, music, or data tables.
- **Redact personal paths** from log snippets (replace your local username in
  paths with `~/` or `<user>`).

## Issue title, areas, and labels

Use a short, scannable title prefixed with one area from this table:

| Area prefix | Covers | Owning docs / map |
| --- | --- | --- |
| `[setup]` | First-run ROM picker, pack builder, startup failure | `docs/PLAYING.md` |
| `[field]` | Towns, dungeons, overworld, walking, collision, NPCs, chests, vehicles, transitions | `docs/FEATURE_MAP.md` |
| `[battle]` | Combat math, turn order, techniques/skills/macros, enemy AI, battle UI/animations | `docs/FEATURE_MAP.md` |
| `[cutscene]` | Story scenes, comic panels, dialogue, triggers, ending | `docs/FEATURE_MAP.md` |
| `[camp]` | Camp menu, items, equip, status, order, shops, inns, save/load | `docs/FEATURE_MAP.md` |
| `[audio]` | Music, SFX, FM/PSG synth, crackling, missing or stuck tracks | `docs/FEATURE_MAP.md` |
| `[platform]` | Linux/Windows/macOS/Steam Deck packaging, windowing, scaling, controllers | `docs/PLAYING.md` |

Examples:
- `[battle] Macro skips Chaz's turn when target dies mid-round`
- `[platform] macOS Apple Silicon crashes on launch after selecting ROM`
- `[audio] Battle fanfare loops after escaping in Motavia overworld`

**Suggested labels** (only maintainers can apply labels via API if you lack
triage permission; if `gh issue create --label` fails, omit `--label` and name
the category in the body):

- Always `bug` for broken behavior or crashes.
- Add `gameplay-fidelity` if a game rule differs from the US cartridge.
- Add `presentation` if visuals, animation, audio, or timing differ from the cartridge.

## Markdown template for filing

Match these headings (they mirror [`.github/ISSUE_TEMPLATE/bug.yml`](../.github/ISSUE_TEMPLATE/bug.yml)):

```markdown
### Summary
<!-- 1-2 sentences describing the problem. -->

### Area and impact
- **Area:** `setup` | `field` | `battle` | `cutscene` | `camp` | `audio` | `platform`
- **Impact:** crash | softlock | gameplay difference | visual/audio glitch | other
- **Suggested labels:** `bug` (plus `gameplay-fidelity` or `presentation` if applicable)

### Where in the game
- **Location:** <!-- e.g. Piata Academy 1F / Motavia overworld -->
- **Party / story point:** <!-- e.g. Chaz, Alys, Hahn (Lv 6), just entered Birth Valley -->
- **Battle / action (if applicable):** <!-- enemies, ability, item, or macro -->

### What happened
<!-- What you did and what PSIV did instead. -->

### What the original does
<!-- What the US Genesis cartridge does at the same point, or "unsure". -->

### Steps to reproduce
1. ...
2. ...
3. ...

### Environment
- **PSIV version:** <!-- e.g. 1.0.0 release archive, or commit hash -->
- **Platform:** <!-- e.g. Windows 11 x86-64 / macOS 15 arm64 / Ubuntu 24.04 x86-64 -->
- **GPU / renderer / display:** <!-- from top of godot.log -->
- **Input:** <!-- keyboard / controller model -->
- **ROM SHA-256 matched:** yes / no / failed on setup screen

### Log excerpt
```text
<!-- Relevant lines from logs/godot.log (personal paths redacted), or "no errors in log". -->
```

### Attachments / save
<!-- Note whether a save slot, screenshot, or clip is attached. Never attach the ROM or runtime-pack. -->
```

*(Optional for coding agents that cloned the repository: if you verified the
owning module in [docs/FEATURE_MAP.md](FEATURE_MAP.md) and read the source, you
may add a short `### Likely owning files` section citing exact paths and line
numbers. Never guess file paths you did not inspect.)*

## How to submit

If the GitHub CLI (`gh`) is installed and authenticated:

```bash
gh issue create -R TusanHomichi/PSIV \
  --title "[<area>] Short, specific symptom" \
  --body-file /tmp/psiv-issue.md
```

(If you have triage permissions, you may add `--label bug` and any matching
category label; otherwise omit `--label` so the command succeeds for any user.)

If `gh` is not available, hand the player the finished Markdown and direct them
to open [a new bug report](https://github.com/TusanHomichi/PSIV/issues/new?template=bug.yml)
and paste the fields in. For general questions or ideas rather than bugs, use
[GitHub Discussions](https://github.com/TusanHomichi/PSIV/discussions).
