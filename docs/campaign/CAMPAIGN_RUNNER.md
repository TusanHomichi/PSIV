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
  between two cells until a condition holds (grinding the route requires),
  with an optional `refuge` list the patrol runs when a member falls or a
  living member drops under half HP;
- fight scripted battles; random battles use a policy that issues commands
  through the battle menu with the same pad input a player would use.

Every chapter ends in assertions on flags, party, map and inventory taken from
the scene transcriptions, and an ordinary SAVE. The runner halts on the first
failed objective, unsupported ability, scene fault, lost battle or stuck walk,
and writes a report: chapter, objective, frame, map, cell, mode and the last
events.

## The runner

`rust/psiv-campaign` plays a route. It builds a `Session`, then presses one
joypad byte per frame until each objective is met, reading the session's
views to decide the next press. The durable record of its runs, with every
halt and its diagnosis, is the [runner log](RUNNER_LOG.md).

```text
cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json \
    --save-dir build/campaign/run1
rust/target/release/psiv-campaign replay build/campaign/run1/run.tape
```

`run <route> [--from-chapter ID] [--until-chapter ID] [--save-dir DIR]
[--tape OUT] [--report OUT] [--pack DIR]`. The save directory defaults to
`build/campaign`, the tape to `<save-dir>/run.tape`, and a halt's report to
`<save-dir>/halt-report.json`. Exit status: 0 completed, 2 halted (a report was
written), 1 a usage or setup error. Release builds play the whole route in
about half a second; a debug build is an order of magnitude slower and plays
chapter one in three seconds.

**The boundary.** The runner calls `Session::frame(pad)`, answers the camp's
SAVE with `Session::finish_camp_save`, and reads `Session::runtime()` and its
`&self` methods. It never calls `Session::runtime_mut` or a `&mut Runtime`
method, and a test greps for it (`the_runner_reaches_no_runtime_mutator`). The
one exception is construction, in `src/start.rs`: `Runtime::new_game`,
`enable_battles` and `start_event` for START, `Runtime::from_save` for a chapter
save, all on a runtime nobody has played yet. Node S5 owns title and
construction; when it makes those crate-private, the runner needs a `Session`
constructor that takes the pack and the battle files and does exactly this.

**Controllers**, one per objective kind, each in its own module and each
producing pads from views only:

| Objective | Module | What it presses |
| --- | --- | --- |
| `go_to`, `go_to_map`, `patrol` | `walk.rs` | a direction for a step, re-planned from the real cell whenever the party comes to rest somewhere the plan did not expect; R0's planner over the live map, and a warp graph built from the flags the game holds; a yes/no prompt that a scene opens on the target map (Chaz's house offers a rest on arrival) ends `go_to_map` with the prompt open, for the next `answer` |
| `talk`, `open_chest`, `interact`, `answer` | `talk.rs` | walks next to the object (or across its counter), turns, presses Speak, and reads what opened; Cancel is retail's direct NO |
| `buy`, `sell`, `rest_inn` | `shopping.rs` | the shop view's pages, rows and cursors |
| `equip`, `use_technique`, `use_item`, `reorder`, `save` | `camping.rs` | the camp view's pages; SAVE is answered by the driver with the file the runner writes |
| random and scripted battles | `battle.rs`, `policy.rs` | the battle view's menus, one press at a time |
| `expect` | `expect.rs` | nothing: it settles the game and reads flags, map, cell, party and purse |

`field.rs` is the loop every controller calls first: it plays frames until the
game hands control back, turning finished pages with Speak, acknowledging chest
results, fighting battles with the chapter's policy, and waiting out scenes.

**Battle policy.** `random_battle_policy` names one (`policy.rs`). Everyone
attacks the first living enemy; the first member who can cures the most hurt
one below half HP with a healing technique, or failing that a healing item;
`run_unless_boss` runs from random encounters and fights scripted battles.
After a battle the party is cured through the camp (`recovery.rs`). Losing is a
halt. `attack_all`, `heal_then_attack`, `train_with_inn` and
`bioplant_survival` resolve to the default policy; a route that needs another
behaviour gets a type in `policy.rs` first.

**Halts and the report.** Each objective has a frame budget. The run stops at
the first of: a missed `expect` or closing assertion, an exhausted budget, an
unsupported enemy ability, a scene fault (a faulted or missing scene, an
unsupported trigger, an unpacked warp target, a refused battle round), a lost
battle, an unreachable target, a menu with no such entry, an object the route
named that is not the one the game reached, or a state the objective cannot
continue from. The report is JSON: `chapter`, `objective_index`,
`objective_kind`, `objective`, `halt {kind, detail}`, `frame`, `map`, `cell`,
`facing`, `mode`, `party` (HP, TP, level, status), `money`, the last 50 runtime
`events`, `surroundings` (the collision values around the party),
`nearby_warps`, `view` (the open dialogue, shop, camp, battle or chest) and the
`battles` fought. `PSIV_CAMPAIGN_TRACE=1` prints every event and every battle
decision to stderr as the run goes.

**Tape, saves and replay.** The tape is every frame's pad byte, run-length
encoded in a text file (`src/tape.rs` documents the format). A chapter that
completes writes `<save-dir>/NN-id/slot_1.sram`, and `--from-chapter ID` starts
from the save the chapter before it ended on; the tape then names that save by
hash. Route `save` objectives write `<save-dir>/route/slot_N.sram` through the
camp's SAVE page. `replay <tape> [--from-save FILE]` plays the tape back and
prints the digest of the final state (map, cell, facing, purse, party and the
whole persistent snapshot), which a run printed too and a replay must
reproduce. `inspect <slot.sram>` prints a save's
position, purse, party (levels, HP, stats, equipment), pack and set event flags
through the runtime's own views: the question a route author asks of every
chapter save. A save loaded mid-route restarts the frame counter and RNG, so a
`--from-chapter` run is the same game from there but not the same frames as the
full run; only the full run from New Game is the route's evidence.

**Tests.** `cargo test --manifest-path rust/Cargo.toml -p psiv-campaign --
--test-threads=1` runs the planner and validator suites and
`tests/runner.rs`: chapter one twice gives identical tapes, digests and saves;
a tape replays to its digest, also from a chapter save; a wrong object index,
an expectation that cannot hold, an unreachable cell and a spent budget each
halt naming the objective; the exit statuses. The whole route is `#[ignore]`d:
`cargo test --release --manifest-path rust/Cargo.toml -p psiv-campaign --test
runner -- --ignored --test-threads=1`. Cases that need the pack skip with a
message when it is absent.

## Task graph

```yaml
outcome: "A fresh install plays New Game to the Ending headlessly from the route file with ordinary pad input, and milestone tapes replay identically in Godot"
canonical_record: "docs/campaign/CAMPAIGN_RUNNER.md#task-graph"
authority: "Owner 2026-10-01: campaign runner approach; commit, push, PR and merge once the full gate is green (docs/AGENT_WORKFLOW.md#authority-effort-and-continuation)"
effort_policy: "Continue scoped repairs until acceptance passes; no fixed cycle limit (inherited)"
exclusions: ["modding", "visual-parity claims beyond existing certifications", "gameplay changes that are not cartridge behavior"]
next_action: "S6 and C (C1: Aiedo through Zio defeated) in parallel"
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
    evidence: ["lane s2-session (base 64d9d7b), 2026-10-01: Session::frame(pad) + Session::window_tick() own the dialogue input half, a pending $F6 and the field or scene tick; the dismiss latch and the scene/window starvation live in rust/psiv-runtime/src/session.rs; build/lane-evidence/S2-NOTES.md indexes the raw output", "workspace 1094 passed / 0 failed / 1 ignored (63 targets); clippy --workspace --all-targets -D warnings clean; fmt --check clean; examples build; academy_route exits 0 (build/lane-evidence/{workspace-tests,clippy,fmt,examples-build,academy-route}.txt)", "test names: psiv-runtime +5, nothing removed (three FieldObj_MovementsTbl mask cases, the headless opening test and its negative control), psiv-godot +0 of 57 (build/lane-evidence/testnames-*.diff)", "headless opening test rust/psiv-runtime/tests/session_opening.rs: Runtime::new_game through Session::frame with pads only reaches map $13, standing cell (48,18), Chaz alone, no scene, 500 meseta, town flags 80008040; negative control (neutral pads) leaves the box up and the scene active (build/lane-evidence/negative-control.txt)", "fidelity fix: the pad->Input direction order is now the cartridge's FieldObj_MovementsTbl (ps4.asm:93675): an opposing pair cancels and a horizontal beats a vertical, where the shell's read_input was Up first; named in rust/psiv-runtime/src/pad.rs and pinned by pad::tests", "native opening smoke (tools/native/native_opening.gd) run headless through the shipped shell reaches the recorded state at tick 3360: map $13 (48,18), Chaz alone, no scene, 500 meseta, town 80008040 (build/lane-evidence/native-opening.txt); oracle/frames is absent in the lane, so the certified captures remain the integration step", "grep -n 'tick(' rust/psiv-godot/src/*.rs: lib.rs has no Runtime::tick; the only runtime ticks left are battle/mod.rs:363 (S3), camp/mod.rs:356 and shop.rs:264 (S4) (build/lane-evidence/tick-grep.txt)", "follow-up (same lane, after review): a runtime can no longer exist without its dialogue pack — GameData::load reads dialogue/ with the rest of the pack, GameData::dialogue shares it, and save::construct_runtime installs it, so the title's START and CONTINUE get a message box; load_dialogue is retired and Runtime::dialogue_pack() is no longer an Option. Acceptance test a_new_game_runtime_opens_the_openings_first_scene_dialogue (Runtime::new_game, no other setup, opens the opening's first SceneDialogue) and the negative control dialogue::glue::tests::a_runner_without_the_pack_cannot_open_the_openings_first_line both pass; the opening smoke under PSIV_DEBUG_RETAIL_PACE=1 (autoclose off, real boxes) reaches first control with 0 tree-absent errors; CONTINUE boot verified (build/lane-evidence/S2-NOTES.md)", "psiv-campaign 27/27 and the full workspace stay green; rust/psiv-campaign untouched"]
    integration_evidence: ["certify on 213b348: 6/6 pairs 0.000000, every capture byte-identical to main's #47 receipt (20261002T070040Z-a4da324)", "gate 20261002T072235Z-213b348: 1191 Python, 1097 Rust passed", "fidelity fix verified against pinned ps4.asm:93675 FieldObj_MovementsTbl: horizontal beats vertical, opposing pair cancels", "run 2: a Runtime cannot be built without its dialogue pack (main's Godot-side prepare_runtime removed at integration)", "open: Session::window_tick is a second per-frame call until S4 moves Interact routing; S4's acceptance folds it into frame"]
    state: verified
  - id: S3
    outcome: "Battle menus and battle end in psiv-runtime, driven by pad input"
    depends_on: [S2]
    acceptance: "RoundOrders are built only inside the runtime from menu input; outcome and rewards come from the battle itself; vehicle skill counts live in game state; the 87-fixture oracle replay stays exact; battle idle certification stays 0.000000"
    evidence: ["lane s3-battle (base dedf6b5), 2026-10-02: the battle is a session mode. rust/psiv-runtime/src/session/battle/ owns it — mod.rs (the loop), menu/{mod,commands}.rs (the main options, the per-character window, the mounted window, the eligibility and reservation rules), view.rs (the BattleView the shell draws), narration.rs (the retail lines and beats, moved from Godot), queue.rs (a runtime timeline with its sound/animation sidecars) and presentation.rs. Session::frame dispatches to it and Frame::battle carries the view, the sound cues, the start cue and the close edge; godot's battle/commands.rs, ui_input.rs, timeline.rs and sfx.rs are gone, battle/ui.rs (1,000 lines) became battle/ui/{mod,build,draw}.rs, battle/menu_draw.rs draws the window, and build/lane-evidence/S3-NOTES.md indexes the raw output", "workspace 1106 passed / 0 failed / 1 ignored (64 targets); clippy --workspace --all-targets -D warnings clean; fmt --check clean; examples build; academy_route exits 0 and fights four battles through the menu (build/lane-evidence/{workspace-tests,clippy,fmt,examples-build,academy-route}.txt)", "grep -rn 'RoundOrders|battle_round|finish_battle_for_outcome|start_battle_timeline' rust/psiv-godot/src prints nothing (build/lane-evidence/battle-seam-grep.txt); the shell's remaining runtime ticks are camp, shop and the boot fixtures, and the battle's Neutral tick is in session/battle/mod.rs (build/lane-evidence/tick-grep.txt)", "headless acceptance test rust/psiv-runtime/tests/session_battle.rs: a field state on AcademyBasement (map $15) whose first landing rolls tape 07's formation ($8A, two Zoran Bults at $0E/$1A) is fought with pads only — COMD, TECH -> RES -> a party target, ATTACK, the confirm-waiting results pages, victory, meseta and experience in game state, the return to the field. Two negative controls print what they saw: a pad that never confirms leaves the battle running with the command window up and the party untouched (party [20, 53, 21] after 600 neutral frames), and cancelling out of a target list returns to Actions with the same actor and no order (build/lane-evidence/negative-controls.txt)", "buttons are the cartridge's where its routines name them: accept is ButtonSpeak|ButtonCamp and the main options wrap on Up/Down (Battle_MainOptions, ps4.asm:1916; Battle_UpdateRedCursor2, ps4.asm:1572), the post-battle pages take any face button (ps4.asm:4706, 6351), and the mounted window's cancel is ButtonCancel (ps4.asm:7463). The per-character and list windows keep the shell's four-direction mapping because this port's window is one vertical list where the cartridge's is a horizontal five-icon strip, and the native input drivers steer those lists with ui_up/ui_down; rust/psiv-runtime/src/session/battle/menu/mod.rs records the difference", "fidelity fix (with test): the mounted skill window reads the battle record's live uses instead of the shell's own copy, which the shell decremented at battle/ui_input.rs:102 while the engine decrements the record when the command resolves (psiv-core/src/battle/engine.rs, Command::VehicleSkill); pinned by session::battle::menu::commands::tests::mounted_skill_slots_read_the_live_battle_record", "deviation (presentation only, named): an enemy attack animation that begins on the frame its predecessor's beat ends now gets its art-clock advance on that same frame, because the shell applies the frame's view before advancing the animation clock, where the old shell advanced first inside advance_frame. The battle's first beat, each round's opening beat and the epilogue's pages are exact; no certified capture covers a mid-round attack", "the debug selectors keep their paths: PSIV_DEBUG_BATTLE=0x88 draws the tape-07 command-idle fixture from a static view with no runtime battle (and no field frame: the presentation still owns the frame), 0x89 runs the newly-exact probe through Session::debug_battle_probe, and a real formation id goes through Session::debug_battle; the shell's one new seam is Session::abort_battle, the presentation-failure escape its own error paths already answered (finish_battle_for_outcome(Escaped, 0))", "test names: psiv-runtime +25, nothing removed (10 command-window tests, 4 narration tests, the queue test and the dwell table moved from psiv-godot; 5 new pad/narration cases and 3 integration tests); psiv-godot +0 of 58, 16 removed, every one of them moved (build/lane-evidence/testnames-*.diff)", "native smoke (lane, headless, extension built from this revision): PSIV_DEBUG_BATTLE=0x88 runs the tape-07 command-idle fixture with no error and one 'battle started' line, and PSIV_DEBUG_BATTLE=0x8a starts a real formation-$8A battle through the session — the tick-200 state dump reads Chaz (fighter 1, 25 HP) against two ZORAN BULTs (fighters 6 and 7, 25 HP), the tape-07 roster, with the menu waiting for input (build/lane-evidence/smoke-battle-0x{88,8a}.txt and -state.json)", "open: the integration step must certify battle-0x88 at tick 200 and the 87 oracle replay fixtures; oracle/frames is absent in the lane, so no capture was compared here"]
    integration_evidence: ["merged with S3/S4 at 479b823 (session/mod.rs reconciled by hand: Mode::{Field, Battle} beside the menu state, one-call frame); certify 20261002T094329Z-479b823: 6/6 pairs 0.000000, every capture byte-identical to main 213b348; gate 20261002T094917Z-479b823: 1191 Python, 1127 Rust passed", "filed: #51 per-character list window is vertical where the cartridge draws a horizontal strip; #52 back-to-back enemy attack animation starts one frame early"]
    state: verified
  - id: S4
    outcome: "Shop, inn, camp, loot and their catalogs in psiv-runtime/psiv-data"
    depends_on: [S2]
    acceptance: "shops.json loads only through psiv-data; sell price, equip/unequip and auto-target rules have runtime tests; camp root certification stays 0.000000"
    integration_evidence: ["merged with S3/S4 at 479b823 (session/mod.rs reconciled by hand: Mode::{Field, Battle} beside the menu state, one-call frame); certify 20261002T094329Z-479b823: 6/6 pairs 0.000000, every capture byte-identical to main 213b348; gate 20261002T094917Z-479b823: 1191 Python, 1127 Rust passed", "review sent back once: one key pressed two cartridge buttons (Cancel set Camp) and menus dropped the Speak|Camp confirm (ps4.asm:135155); fixed with one Godot action per button", "filed: #49 shop greeting/BUY-SELL Cancel flow, #50 same-frame panel op and dialogue close; #39 Aiedo inn event still open"]
    state: verified
    evidence: ["lane s4-shopcamp (base dedf6b5): Session::frame(pad) now owns the shop and inn window (session/shop.rs), the camp menu and chest windows (session/camp/), Interact routing (counter to shop, else NPC talk, facing included) and the scene-line, choice and nothing-here window openers (session/route.rs); Session::window_tick is folded into frame (grep window_tick rust: no hits); psiv-data loads shops.json (ShopData); equip against unequip decides from CampCharacter::equipment_ids; psiv-godot keeps drawing only (shop/catalog.rs, shop/input.rs, camp/input.rs, camp/travel.rs gone; grep for shop_buy|shop_sell|shop_stay|use_camp_item|use_camp_ability|equip_camp_item|order_camp_party|shops.json in rust/psiv-godot/src prints nothing)", "tests rust/psiv-runtime/tests/session_menus.rs, pads only: Piata buy, sell at half the record price word for an unstocked item, empty-pack refusal, inn night (HP/TP/status restored, 5 per head), camp heal technique, equip and unequip, ORDER, SAVE request, three chest windows; negative controls a_purchase_the_purse_cannot_cover_is_refused_with_no_state_change, an_inn_bill_the_purse_cannot_cover_changes_nothing and camp_pressed_while_a_scene_runs_does_not_open_the_menu pass; the chest confirm default failed before its fix (solo party read YES)", "tools/certify.py on the lane build: 6/6 pairs rmse 0.000000 and every capture byte-identical to main 213b348's receipt, camp-root included (build/lane-evidence/certify.txt, capture-compare.txt); native opening smoke reaches map $13 (48,18), Chaz alone, 500 meseta, town 80008040, no ERROR lines; PSIV_DEBUG_SHOP=12 screenshot shows the session's shop view", "academy_route exits 0; motavia_route's camp heal ran through pad presses (CAMP RES: Alys 22 HP) before the copied save stopped it at an unrelated walk", "fixes (separate): sell price reads the item record price word for any item (ps4.asm:135570), chest confirm gets its own cursor so a lone member keeps the documented default NO (docs/field/CHESTS.md); open: #39 left as AiedoEventPending (the scene must run mid-transaction)", "review repairs (second commit): each cartridge button has its own Godot action (ui_accept Speak, ui_cancel Cancel, psiv_camp Camp, psiv_start Start; controller button 6 moved from ui_accept to Start and the title confirms on Start) and the session menus use the cartridge's masks (Speak|Camp confirm, Cancel back, Start closes the camp or dismisses a result line; ps4.asm citations in session/camp/mod.rs and session/shop.rs); 15 tools/native drivers that opened the camp with ui_cancel press psiv_camp and parse with --check-only; the panel-layout byte lives in the Runtime (scene_panel_sprites, cleared on a dialogue close and scene end) and cutscene/state.rs reads it; debug.rs opens the debug counter in the Session directly and title.rs and lib.rs build every Session through new_session; the BUY list opens on its first row after a SELL visit (ps4.asm:135182, 135508)"]
  - id: S5
    outcome: "Title, CONTINUE/START/ERASE and game over in the Session; runtime mutators crate-private"
    depends_on: [S3, S4]
    acceptance: "psiv-godot compiles against the Session view API only (a mutator call from Godot fails to compile); title certification stays 0.000000; fresh-process CONTINUE verified"
    evidence: ["lane s5-title (base 016cdc7), 2026-10-02: the title and a defeat are session modes and every slot operation runs inside the session. rust/psiv-runtime/src/session/title.rs owns the phases and their frame counts (the retail #$233 Press Start hold, ps4.asm:86926), the CONTINUE/START/ERASE DATA rows and their buttons (TitleRoutine_PickOption, ps4.asm:87512-87513), the three-row slot lists and the ARE YOU SURE? window (ps4.asm:87908-87956); session/game_over.rs owns the fourteen fade frames that gate the title's return, and session/notices.rs owns the field-status windows the defeat queues (DisplayPerishedMessage, ps4.asm:117087). START builds the retail initializer and CONTINUE loads a slot inside the session, so the shell answers no request", "saves: one store type constructed with the run directory. SaveStore (rust/psiv-runtime/src/save.rs) is the only path to a slot file and the session keeps it across START and CONTINUE; the shell resolves PSIV_SAVE_DIR and refuses a scripted run without it (rust/psiv-godot/src/save_dir.rs) and hands the store over at construction (configure_session). Camp SAVE writes inline and reports the result line on the frame it happened; Frame::save_request and Session::finish_camp_save are gone, and the shell's round trip with them (Frame::camp_save_error keeps only its log line)", "boundary (correction ladder rung 1): Session::runtime_mut is removed and every Runtime mutator the shell used is crate-private, so psiv-godot compiles against Session::frame, its constructors and Session::runtime() -> &Runtime. A shell that tries a mutator through that reference fails to compile for the right reason: the two compile_fail doctests on Session fail with E0596 (build/lane-evidence/negative-controls.md, compile-fail/*.rs), and grep -rn 'runtime_mut' rust/psiv-godot/src rust/psiv-runtime/src prints nothing (build/lane-evidence/no-runtime-mut.txt)", "debug fixtures: psiv-runtime/src/session/debug.rs holds the certification fixtures (camp_fixture for PSIV_DEBUG_CAMP=1 with the tape-22 object positions and the receipt's settled camera, scene_fixture for PSIV_DEBUG_EVENT=0x8007) and the debug selectors (debug_battle, debug_battle_probe, debug_start_event, debug_mount_vehicle); psiv-godot/src/boot.rs keeps only the selectors' list, the fallback spawn and the retail-initializer test. The #44 guard moved with the fixture (session::debug_tests::the_camp_fixture_does_not_replay_piata_chaz_alone, negative control: a blank GameState does replay the trigger)", "headless tests (rust/psiv-runtime/tests/session_title.rs, pads only): power-on reaches the certified title state (clone tick 480 = reveal, ticks 279, no window) and START opens the opening's first dialogue; CONTINUE loads the slot a camp SAVE wrote in the same test (same map, cell, party, purse and flags); ERASE DATA zeroes the picked slot, drops its row and leaves the other slot loading; a defeat (Chaz at one HP, poisoned, on Motavia) reaches game over and the title after the fourteen fade frames. Negative controls in the same file: CONTINUE and ERASE DATA on an empty row return entered/erased None with no state change at all, and session_menus::a_save_without_a_run_directory_is_refused refuses with no file written (build/lane-evidence/negative-controls.md)", "buttons: the option rows confirm on ButtonSpeak|ButtonCamp|ButtonStart. The port's held-pad reader had dropped Camp; this lane restores it with a citation and a unit test (session::title::title_tests::the_option_rows_confirm_on_speak_camp_and_start), and an accept press no longer doubles as a cursor step (the cartridge's confirm branch runs first, ps4.asm:87514-87534; the_accept_button_does_not_step_the_cursor)", "kept, uncited port behavior, listed not changed: the Press Start hold opens the option window where the cartridge moves to TitleRoutine_CharPortraits (ps4.asm:86941) and the port has no portraits or scrolling-text phase; every phase accepts the option window's three buttons where the Press Start hold leaves only on ButtonStart (ps4.asm:86934); the Sega and reveal phases advance on a press, which TitleRoutine_FadingText never reads (ps4.asm:86220-86387); ARE YOU SURE? opens on YES where the cartridge initializes Window_Option_Index_3 to the other row (ps4.asm:87905-87908); the title is silent where the cartridge plays SFXID_Selection and SFXID_EnemyKilled (ps4.asm:87930-87936); Cancel opens the cartridge's sound test (ps4.asm:87517-87526) and does nothing here. All six are recorded in the module doc of session/title.rs", "files: session/mod.rs 537 + the new session/{title,title/title_tests,game_over,notices,saves,debug,debug_tests}.rs; psiv-godot/src/title.rs is presentation only now (726 lines, was 987) and every touched file is under 1,000 lines (python3 tools/size_guard.py: 0 problem(s))", "test names: psiv-runtime 201 -> 218 (one case replaced, nothing else removed: the camp SAVE test now expects the store to write), psiv-godot 41 -> 40 (the only removal is the #44 fixture test, which moved into the runtime as session::debug_tests::the_camp_fixture_does_not_replay_piata_chaz_alone); psiv-core, psiv-data, psiv-sound and psiv-campaign unchanged (build/lane-evidence/testnames-before-after.txt, testnames-psiv-{runtime,godot}.{before,after}.txt)", "open: the mutators the crate's own integration tests and examples drive directly (enable_battles, tick, battle_round, finish_battle_for_outcome, set_rng_seed, save_slot, start_event, dialogue_closed/choice/ended, open_scene_dialogue, return_to_field, repair_legacy_progression, order_camp_party, use_camp_item, shop_stay, set_event_flag, set_vehicle_index, set_camera, face_npc, set_field_suspended, ending_continue, acknowledge_field_notice, open_status_dialogue, finish_battle_absorbing) stay pub: a pub(crate) mutator is invisible to a target that lives outside the crate, so closing those needs the harnesses on the session first. 68 of the crate's 91 state-changing methods are crate-private now; docs/RUNTIME_DESIGN.md names the remainder", "open: the certified captures are the integration step. oracle/frames is absent in the lane, so no capture was compared here; the title pair (clone tick 480) and the five others ran through the moved paths only in the sense that the state they render is produced by the same code on the same frames (the title's own frame count, its phase lengths and the shell's per-frame presentation are unchanged)", "gate (lane, one heavy command at a time): cargo test --workspace 1145 passed / 0 failed, the two compile_fail doctests on Session included (build/lane-evidence/workspace-tests.txt); clippy --workspace --all-targets -D warnings and fmt --all --check clean (clippy.txt, fmt.txt); cargo build -p psiv-runtime --examples and --example academy_route exit 0 (examples-build.txt, academy-route.txt); python3 tools/size_guard.py, python3 tools/check_docs.py and python3 -m unittest tests.test_feature_map exit 0 (python-checks.txt); git diff --check clean"]
    integration_evidence: ["merged with R1 and #54 at d2d4096; runner camp SAVE moved onto the Session store; full route digest unchanged by S5 (52ccbf8d... before #54, 5e80a50e... after)", "certify 20261002T143246Z-d2d4096: 6/6 0.000000, captures byte-identical to main", "gate 20261002T143834Z-d2d4096: 1191 Python, 1177 Rust passed", "boundary only partly locked: 23 Runtime mutators remain pub for out-of-crate callers; node S6"]
    state: verified
  - id: R0
    outcome: "Route planner (cross-map warp graph + in-map BFS over the runtime's own FieldMap) and the route-file format with a pack validator; rust/psiv-campaign"
    depends_on: []
    acceptance: "Motavia (84,64) to Aiedo plans one warp (0x100706), arrival (47,83), 67 steps, matching docs/campaign/AIEDO.md's independent oracle/route.py result; validator accepts routes/main.json and rejects mutated copies"
    state: verified
    evidence: ["psiv-campaign 27/27 and psiv-runtime suites green at bf763ee", "validate routes/main.json: 13 chapters, 196 objectives, 100 warps, 3672 steps, 0 errors", "review: the planner's attach_chests copy replaced by one public fresh-entry builder, field_map_entered, also used by map_change and save", "12 objectives marked verify:true (R1 resolves them by running)"]
  - id: S6
    outcome: "Every state-changing Runtime method is crate-private: the runtime's integration tests, examples and the campaign runner build and drive games only through Session constructors and Session::frame"
    depends_on: [S5, R1]
    acceptance: "Session constructors cover new game (with battles armed and the opening started), CONTINUE from a store or a slot file, and the debug fixtures; psiv-runtime tests/examples and rust/psiv-campaign use only them, Session::frame and &self views; no pub fn on Runtime takes &mut self (grep proof); a compile_fail doctest builds a Runtime outside the crate and calls a mutator; the full route digest is unchanged; certify 6/6 byte-identical"
    state: ready
  - id: R1
    outcome: "Campaign runner binary driving a Session from routes/main.json; chapters through the post-Rika checkpoint"
    depends_on: [S5, R0]
    acceptance: "The runner plays New Game to the north bank (Motavia $00 (84,64), five members) from the route file with pad input only, writes a pad tape and chapter saves, and a deliberately broken objective halts with a report (negative control)"
    evidence: ["lane r1-runner (base 016cdc7), 2026-10-02: rust/psiv-campaign gains the runner (docs: 'The runner' above; record: RUNNER_LOG.md). `psiv-campaign run routes/main.json` plays all 13 chapters from New Game to the Aiedo arrival with pad presses only: 175,497 frames, 92 random battles and 2 scripted, exit 0, final-state digest 52ccbf8d9a7de61a, in about half a second in a release build. The north-bank checkpoint holds (Motavia $00 (84,64), Gryz, Alys, Chaz, Hahn and Rika alive, flag $35) and the Aiedo chapter arrives at $54 (47,83) after its one warp", "tape and replay: the run writes its pad tape (text, run-length encoded, src/tape.rs) and a chapter save per chapter; `psiv-campaign replay <tape> [--from-save FILE]` reproduces the digest, also for a tape that starts from a chapter save; two full runs printed the same digest", "tests: `cargo test --manifest-path rust/Cargo.toml -p psiv-campaign -- --test-threads=1` passes (50 passed, 1 ignored: the whole route, run in release); tests/runner.rs holds the determinism case (chapter one twice: identical tapes, digests and chapter saves), replay, resume from a chapter save, a boundary guard that greps the runner for runtime mutators, and the negative controls: a wrong object index halts at that objective with exit 2 and a report naming chapter, index and kind, an expectation that cannot hold halts at its objective, an unreachable cell, a spent budget, a pack with no item to use", "12 `verify` objectives resolved by playing and their flags dropped (RUNNER_LOG.md, 'verify items resolved'): Hahn and Dorin are object 0, Gryz's storage door is object 1 (the route had 0), Igglanova's press starts interaction area 0 rather than an object, the Holt scene lands the party at Zema (30,11) not (60,21), the Mile detour is unnecessary, the patrol condition holds, and the elevator doors' `opens` cells are walkable", "one port defect diagnosed, route works past it with a player's alternative: RunEvent_MileSandWorm is Unsupported unconditionally (rust/psiv-core/src/trigger_custom.rs:43) where the cartridge answers NoEvent outside its box without touching the RNG (ps4.asm:116449), so every visit to Mile halts; the route skips Mile and rests at the Piata inn (RUNNER_LOG.md, H1)", "open for S5: the runner builds its Session through Runtime::new_game, enable_battles, start_event and from_save in src/start.rs; when S5 makes them crate-private the runner needs a Session constructor over the pack and the battle files"]
    integration_evidence: ["independent rerun at integration: run exit 0, 175,497 frames, digest 52ccbf8d9a7de61a, tape sha256 dd5ff60f... identical to the lane evidence; replay reproduces the digest", "gate 20261002T114112Z-b1d38ea: 1191 Python, 1150 Rust passed", "halt H1 filed as #54 (four custom triggers Unsupported); Mile removed from the route until fixed", "boundary: construction in start.rs still calls Runtime constructors and enable_battles/start_event; reconciled with S5"]
    state: verified
  - id: R2
    outcome: "Tape replay in Godot"
    depends_on: [R1]
    acceptance: "One generic Godot replay driver reproduces R1's tape and chapter-save bytes; the bespoke tools/native drivers it covers are retired"
    state: pending
  - id: C
    outcome: "Route chapters to the Ending, one lane per blocker class"
    depends_on: [R1]
    acceptance: "The runner reaches Game_Cleared_Flag from New Game; each blocker it hit is fixed with a regression test or filed as an issue with a link from the route"
    evidence: ["lane c1-motavia (base 8c19769): routes/main.json grows to 18 chapters and reaches the Zio Fort's Juza room (map $87, (32,21)) with all five alive at level 12 to 13: 457,356 frames, digest 96d2835a8633def8, tape sha256 53adf11f092bb99982c299aa2620b75a4078f6a4fe3ba69f0adae0a3d749bd57, identical on three runs, replay reproduces the digest; scenes 28 and 29 pass in-route; RUNNER_LOG.md H13 to H16", "the route stops at Juza: H16, enemy 114's ZAN and FORCEFLASH are not run by the engine, and the stairs to F3 and F4 open only after his battle; H15 lists the unsupported abilities on the way (FUSION, FIREBREATH, DEBAN) that the route passes only by running; H13 (#39) does not block the story"]
    state: pending
```
