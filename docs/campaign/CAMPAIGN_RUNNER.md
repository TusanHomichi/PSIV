# Campaign runner

Opened 2026-10-01. Owner decision: replace per-segment native drivers and
segment ledgers with one headless campaign runner that plays New Game to the
Ending from a route file using ordinary joypad input, and halts at the first
blocker with a report. Each blocker becomes a fix lane. Native Godot checks
run at milestone saves by replaying the runner's own input tape.

## Why the previous method stalled

Through 2026-09-29 the native route reached the post-Rika north bank, roughly
the first sixth of the campaign, using 33 bespoke drivers in `tools/native/`
and one ledger per hop. The Aiedo hop (one warp, 67 steps) produced a 443-line
research ledger before any movement ([AIEDO.md](AIEDO.md)). That rate cannot
finish the game.

The root cause is architectural. An audit on 2026-10-01 found that the only
complete game loop lives in `psiv-godot`:

| Area | Lives in `psiv-godot` today | Runtime has |
| --- | --- | --- |
| Mode dispatch (`Game_Mode_Index`) | title, notices, game over, battle, pending `$F6`, dialogue, shop, camp, field priority; field input starvation; `set_field_suspended`, which changes the shared RNG stream (`lib.rs:460-655`) | `tick(Input)` for field and scenes only |
| Dialogue | `$FA` FlagCheck, `$F6` events, Yes/No branches, `Ctrl::Action`, `$F2` flag writes, `$F7` resume, a copied flag bank that can go stale (`dialogue/text_flow.rs`, `dialogue.rs`, `boot.rs`) | nothing that reads `Ctrl::` |
| Battle | command eligibility, target paging, defaults, item reservation, RUN, vehicle skill counts kept in a local copy; outcome and reward scraped from narration and passed back (`battle/commands.rs`, `battle/ui_input.rs`, `battle/ui.rs`) | rounds, plus a redundant `outcome()`/`pools()` |
| Shop, camp, loot | shop catalog parsed outside `psiv-data`, sell-price lookup, every menu state machine, equip decided by comparing a display string (`shop/`, `camp/`) | the state mutations only |
| Title and game over | CONTINUE/START/ERASE flow, empty-slot rules, game-over return (`title.rs`, `field_status.rs`) | constructors only |

Progression timing also lives there: the dialogue open animation and
typewriter gate page advance, battle dwell beats decide when the menu
reopens, and ORDER auto-closes on a frame count. A headless driver therefore
re-implements a subset (`rust/psiv-runtime/examples/support/mod.rs` copies
the dialogue preamble walk, skips mid-message branches and `$F2` writes, and
diverges from Godot's RNG stream). That is why each hop needed a Godot
driver, and why a headless route is not evidence for the shipped game.

## Target architecture

`psiv-runtime` gains one `Session`: the cartridge's main loop. Its whole
input surface is `Session::frame(pad)`, one call per 60 Hz frame with the
joypad state. It owns mode dispatch, the dialogue interpreter, battle command
menus and battle end, shop, inn, camp, loot, title, game over and every
frame-counted gate that affects progression. It exposes a read-only view
for presentation and a stream of presentation events (sound, transitions).

Godot sends the pad state each physics frame and draws the view. It decides
nothing about game state.

Consequences:

- The runner and the shipped game execute the same code on the same frames,
  so a headless run is evidence for the native game's rules and input path.
- A runner run emits a pad tape. Replaying that tape in Godot reproduces the
  run frame for frame, which replaces bespoke native drivers with one generic
  replay driver.
- The rule "Godot presents and sends input" becomes enforceable: runtime
  mutators become crate-private behind `Session`, so the Godot crate cannot
  call them (correction ladder rung 1).

## Route file

A route is an ordered list of chapters following the story order in
[docs/scenes](../scenes/README.md). Each chapter holds objectives the runner
resolves into pad input:

- reach a map and cell, path-finding across the pack's warp graph;
- talk to an object, answer a choice, open a chest, use a field ability;
- buy, sell, equip, rest at an inn, reorder the party;
- `interact` with a cell (doors and elevators that open walls), and `patrol`
  between two cells until a condition holds (grinding the route requires);
- fight scripted battles; random battles use a policy that issues commands
  through the battle menu with the same pad input a player would use.

Every chapter ends in assertions on flags, party, map and inventory taken from
the scene transcriptions, and an ordinary SAVE. The runner halts on the first
failed objective, unsupported ability, scene fault, lost battle or stuck walk,
and writes a report: chapter, objective, frame, map, cell, mode and the last
events.

## Task graph

```yaml
outcome: "A fresh install plays New Game to the Ending headlessly from the route file with ordinary pad input, and milestone tapes replay identically in Godot"
canonical_record: "docs/campaign/CAMPAIGN_RUNNER.md#task-graph"
authority: "Owner 2026-10-01: campaign runner approach; commit, push, PR and merge once the full gate is green (docs/AGENT_WORKFLOW.md#authority-effort-and-continuation)"
effort_policy: "Continue scoped repairs until acceptance passes; no fixed cycle limit (inherited)"
exclusions: ["modding", "visual-parity claims beyond existing certifications", "gameplay changes that are not cartridge behavior"]
next_action: "S2"
nodes:
  - id: S1
    outcome: "Dialogue interpreter in psiv-runtime: control codes, branches, choices, actions, $F2/$F6/$F7, live flags, typewriter and open-animation gates, driven by a cartridge-layout Pad"
    depends_on: []
    acceptance: "psiv-godot no longer matches on Ctrl::; the window renders a runtime view; the TextFlow pagination test still matches all 2,736 entries; mid-message branch, $F2 and live-flag regression tests; the headless example harness's own dialogue walk is deleted; opening p1/p2 and MeetingRika certifications stay 0.000000"
    evidence: ["build/lane-evidence/workspace-tests.txt", "build/lane-evidence/clippy.txt", "build/lane-evidence/fmt.txt", "build/lane-evidence/examples-build.txt", "build/lane-evidence/python-checks.txt", "build/lane-evidence/testnames-psiv-godot.diff", "build/lane-evidence/testnames-psiv-runtime.diff", "build/lane-evidence/ctrl-grep.txt", "build/lane-evidence/negative-controls.md", "integration: tools/certify.py on candidate 46810a3 vs main e9e58a2 baseline: meeting-rika, title, battle 0.000000 on both; opening-p2 and camp-root captures byte-identical to baseline; opening-p1 differs by the expected two-frame retail-pace shift (dialogue close t278 -> t276); opening and camp pairs had already rotted on main (#44), so their 0.000000 is restored there, not here", "integration: native opening smoke (title START, trigger 124, Up) reaches map $13 (48,18), Chaz alone, 500 meseta, town 80008040"]
    state: verified
  - id: S2
    outcome: "Session::frame(pad) owns mode dispatch, field, scenes, dialogue, pending $F6 and field suspension; Godot's lib.rs dispatcher calls it"
    depends_on: [S1]
    acceptance: "Godot's per-frame dispatcher is replaced by Session::frame; the RNG-relevant field suspension and the dismiss-press latch are decided in the runtime; all existing tests pass; the six certified captures stay at rmse 0.000000; the opening native driver passes unchanged"
    evidence: ["lane s2-session (base 64d9d7b), 2026-10-01: Session::frame(pad) + Session::window_tick() own the dialogue input half, a pending $F6 and the field or scene tick; the dismiss latch and the scene/window starvation live in rust/psiv-runtime/src/session.rs; build/lane-evidence/S2-NOTES.md indexes the raw output", "workspace 1094 passed / 0 failed / 1 ignored (63 targets); clippy --workspace --all-targets -D warnings clean; fmt --check clean; examples build; academy_route exits 0 (build/lane-evidence/{workspace-tests,clippy,fmt,examples-build,academy-route}.txt)", "test names: psiv-runtime +5, nothing removed (three FieldObj_MovementsTbl mask cases, the headless opening test and its negative control), psiv-godot +0 of 57 (build/lane-evidence/testnames-*.diff)", "headless opening test rust/psiv-runtime/tests/session_opening.rs: Runtime::new_game through Session::frame with pads only reaches map $13, standing cell (48,18), Chaz alone, no scene, 500 meseta, town flags 80008040; negative control (neutral pads) leaves the box up and the scene active (build/lane-evidence/negative-control.txt)", "fidelity fix: the pad->Input direction order is now the cartridge's FieldObj_MovementsTbl (ps4.asm:93675): an opposing pair cancels and a horizontal beats a vertical, where the shell's read_input was Up first; named in rust/psiv-runtime/src/pad.rs and pinned by pad::tests", "native opening smoke (tools/native/native_opening.gd) run headless through the shipped shell reaches the recorded state at tick 3360: map $13 (48,18), Chaz alone, no scene, 500 meseta, town 80008040 (build/lane-evidence/native-opening.txt); oracle/frames is absent in the lane, so the certified captures remain the integration step", "grep -n 'tick(' rust/psiv-godot/src/*.rs: lib.rs has no Runtime::tick; the only runtime ticks left are battle/mod.rs:363 (S3), camp/mod.rs:356 and shop.rs:264 (S4) (build/lane-evidence/tick-grep.txt)", "psiv-campaign 27/27 and the full workspace stay green; rust/psiv-campaign untouched"]
    state: pending
  - id: S3
    outcome: "Battle menus and battle end in psiv-runtime, driven by pad input"
    depends_on: [S2]
    acceptance: "RoundOrders are built only inside the runtime from menu input; outcome and rewards come from the battle itself; vehicle skill counts live in game state; the 87-fixture oracle replay stays exact; battle idle certification stays 0.000000"
    state: pending
  - id: S4
    outcome: "Shop, inn, camp, loot and their catalogs in psiv-runtime/psiv-data"
    depends_on: [S2]
    acceptance: "shops.json loads only through psiv-data; sell price, equip/unequip and auto-target rules have runtime tests; camp root certification stays 0.000000"
    state: pending
  - id: S5
    outcome: "Title, CONTINUE/START/ERASE and game over in the Session; runtime mutators crate-private"
    depends_on: [S3, S4]
    acceptance: "psiv-godot compiles against the Session view API only (a mutator call from Godot fails to compile); title certification stays 0.000000; fresh-process CONTINUE verified"
    state: pending
  - id: R0
    outcome: "Route planner (cross-map warp graph + in-map BFS over the runtime's own FieldMap) and the route-file format with a pack validator; rust/psiv-campaign"
    depends_on: []
    acceptance: "Motavia (84,64) to Aiedo plans one warp (0x100706), arrival (47,83), 67 steps, matching docs/campaign/AIEDO.md's independent oracle/route.py result; validator accepts routes/main.json and rejects mutated copies"
    state: verified
    evidence: ["psiv-campaign 27/27 and psiv-runtime suites green at bf763ee", "validate routes/main.json: 13 chapters, 196 objectives, 100 warps, 3672 steps, 0 errors", "review: the planner's attach_chests copy replaced by one public fresh-entry builder, field_map_entered, also used by map_change and save", "12 objectives marked verify:true (R1 resolves them by running)"]
  - id: R1
    outcome: "Campaign runner binary driving a Session from routes/main.json; chapters through the post-Rika checkpoint"
    depends_on: [S5, R0]
    acceptance: "The runner plays New Game to the north bank (Motavia $00 (84,64), five members) from the route file with pad input only, writes a pad tape and chapter saves, and a deliberately broken objective halts with a report (negative control)"
    state: pending
  - id: R2
    outcome: "Tape replay in Godot"
    depends_on: [R1]
    acceptance: "One generic Godot replay driver reproduces R1's tape and chapter-save bytes; the bespoke tools/native drivers it covers are retired"
    state: pending
  - id: C
    outcome: "Route chapters to the Ending, one lane per blocker class"
    depends_on: [R1]
    acceptance: "The runner reaches Game_Cleared_Flag from New Game; each blocker it hit is fixed with a regression test or filed as an issue with a link from the route"
    state: pending
```
