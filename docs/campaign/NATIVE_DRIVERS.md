# Native drivers: what the tape retires and what stays

Opened for node R2. Before R2, `tools/native/` held 33 bespoke Godot drivers
and three verifiers, one per hop of the campaign and one per UI fixture. The
generic replay driver (`tools/native/native_tape.gd`, entered through
`tools/verify_native_tape.py` and, for a whole run,
`tools/verify_native_route.py`) now replays the campaign runner's own pad tape
and compares every chapter save byte for byte
([R2 section](CAMPAIGN_RUNNER.md#r2-native-tape-replay-in-progress)). This
ledger records, driver by driver, what each proved, which chapter of the tape
covers it, and the decision.

## The rule behind the decisions

A driver is **retired** when it plays a stretch of the story route the tape
plays: the tape replays the same pads from New Game and its chapter saves
(party, stats, equipment, inventory, purse, flags, map, cell) match the native
game's bytes, which is a stronger state check than the driver's own
assertions. Ten of the twelve retired drivers are named as sources in
`rust/psiv-campaign/routes/main.json`, which was transcribed from them; the other
two are variants of a named one (`native_alshline_retreat.gd` of
`native_alshline.gd`, `native_bioplant_recovery.gd` of `native_bioplant.gd`).

A driver is **kept** when it starts from an isolated fixture or a debug
selector and exercises a branch the route never presses (a cancel, an undo, a
defeat, a full pack, a slot other than 1, an ability the policies do not
choose), or when its product is a capture or a comparison with the original
game. The tape carries no such branch: its pads come from the runner's
controllers and policies, and its only capture is one optional endpoint image.

What the tape proves and does not prove is stated in
[AGENTS.md](../../AGENTS.md#verify-the-claim-you-intend-to-make): state tests
prove state, ordinary native input proves the interaction path, fresh-process
CONTINUE proves persistence, and a capture proves only the frame it shows.
The continuous replay never loads a save, so persistence is checked by its own
entry point: `tools/verify_native_route.py --continue-probes` loads each chapter
save in a fresh process through the title's CONTINUE and requires the
snapshot to re-encode the same bytes.

## Inventory

Chapters are the 50 of `routes/main.json` at digest `40c7f505ecc5c357`
(index in parentheses).

### Retired: they replay a stretch of the route

| Driver | What it proved | Covering chapter | Decision |
| --- | --- | --- | --- |
| `native_route.gd` | One START, Piata and the Academy on foot, conversations, Igglanova's battle through the menu, the way out to Motavia | `academy` (0): 32,308 frames, 4 battles, the scripted Igglanova fight | Retired. It also took a camp SAVE and captured PNGs at fixed legs; the route SAVEs from chapter 1 on and the replay takes no per-leg capture |
| `native_motavia.gd` | CONTINUE from the Academy save, camp RES recovery, the way to Holt and Holt's conversation | `holt` (1) | Retired. Its CONTINUE half is the per-chapter probe |
| `native_tonoe.gd` | Holt to Rune and Dorin, the paid inn, the Dorin YES/NO prompts, Gryz joining | `rune-dorin` (2) | Retired |
| `native_alshline.gd` | Gryz's door, the basement, the Alshline chest, an ordinary SAVE back in town | `alshline` (4); the chest is the route's first `open_chest`. The grinding of `basement-training` (3) is the route's own addition | Retired |
| `native_alshline_retreat.gd` | The same quest using RUN on random encounters | The `run_unless_boss` policy of `rune-dorin` (2), `alshline` (4) and `zema-rescue` (5) presses RUN from the battle menu | Retired |
| `native_zema_return.gd` | Rest, back through the pass, Zema's rescue boss, the post-battle conversation, SAVE | `zema-rescue` (5) | Retired. Not carried: the resident-palette, 50-page and Igglanova-entrance observations (see the gaps below) |
| `native_zema_outfit.gd` | Purchases at two counters with exact prices, equipping through the camp, a paid rest, SAVE | `zema-outfit` (6): 6 buys, 7 equips, one `rest_inn` | Retired. Not carried: the hand-selector cancel check (see the gaps below) |
| `native_zema_training.gd` | Birth Valley battles and paid Zema rests to a target level | `zema-training` (7), policy `train_with_inn` | Retired |
| `native_zema_armour.gd` | Carbon suits bought and equipped for Alys and Chaz | `zema-armour` (8) | Retired |
| `native_bioplant.gd` | The BioPlant doors, elevators, encounters, poison cures and dialogue to Rika, with the alarm palette captures | `bioplant-elevators` (9), `bioplant-rika` (11), policy `bioplant_survival` | Retired. Not carried: the alarm captures `verify_native_alarm.py` reads |
| `native_bioplant_recovery.gd` | One BioPlant encounter, normal recovery, SAVE | `bioplant-elevators` (9): 4 battles, each followed by the camp recovery | Retired |
| `native_post_rika.gd` | Zema's inn, then the opened northern crossing to the bank | `north-bank` (12) | Retired |

### Kept: isolated fixtures, branches the route never presses, captures

| Driver | What it proves | What the tape does not carry | Decision |
| --- | --- | --- | --- |
| `native_continue.gd` + `verify_native_continue.py` | Fresh-process title CONTINUE from slot N, one field step, an ordinary SAVE into slot 2, and the exact bytes that may change between the two files | The tape saves slot 1 only and its CONTINUE probe reads slot 1. Its route-receipt input is now optional (the loaded slot is the reference) because the route drivers that wrote it are retired | Kept |
| `native_party_order.gd` + `verify_native_order.py` | STATE, ORDER pick, undo and cancel through ordinary input, the commit, a SAVE, and the comparison of the saved bytes and captures with the original game's RAM and frames | Chapter `bioplant-order` (10) proves the committed order and its save; no tape presses undo or cancel, and none compares with the original | Kept, and made self-contained: it no longer extends the retired route skeleton |
| `native_camp_abilities.gd` | Field RES and RECOVER from the camp: target cancel leaves HP and TP unchanged, exact TP and use costs | The route casts RES in recovery but never cancels, and never uses RECOVER | Kept (also the base of eight fixtures) |
| `native_chests.gd` | Meseta, item and full-pack chests: the discard, compact, return and necessary-item protections, the "already open" message | The route opens three chests with room in the pack | Kept |
| `native_pipes.gd` | ESCAPIPE and TELEPIPE from ITEM: a blocked cast is not consumed, browsing and cancel are free, the right duplicate is spent | No route objective uses a pipe | Kept |
| `native_travel.gd` | HINAS and RYUKA from TECH: destination, cancel, save | No route objective casts a travel spell | Kept |
| `native_status.gd` | A reordered five-member party in battle, three-digit summaries, every STATUS page, SAVE into slot 2 | The route never opens STATUS | Kept |
| `native_field_status.gd` | Poison steps, the defeat notice, the game-over title and CONTINUE again | A lost battle halts the runner; no tape contains a defeat | Kept |
| `native_progression.gd` | Learned-ability messages (TSU, WAT), slots and spent uses after normal attacks, SAVE into slot 2 | The saves carry the learned state; the messages and slot 2 are not asserted | Kept |
| `native_death_abilities.gd` | CRASH and BROSE through the command windows, spent uses and the presentation | The policies pick by estimated damage, never to test an instant-death rule | Kept |
| `native_rimit.gd` | RIMIT on a group, sleep and paralysis, paid TP | No policy casts it | Kept |
| `native_battle_recovery.gd` | ANTI, RIMPA, REVER and REGEN cast in battle by an injured original-record party | Policies cure with the camp after a fight and with the healing technique they choose | Kept |
| `native_enemy_attacks.gd` | DEFEND through COMD until Acid Breath runs, then a win | No policy defends | Kept |
| `native_combat.gd` | A real battle from the debug selector: RES, FOI on a second enemy, GELUN, with captures of the technique list and target page | Policy choices only; no capture | Kept (capture observer) |
| `native_skills.gd` | EARTH, VORTEX and VISION through the skill window, uses re-read after the round, with captures | As above | Kept (capture observer) |
| `native_items.gd` | Dynamite on an enemy, Monomate on an ally, poison cure, item reservation, with captures | As above | Kept (capture observer) |
| `native_fission.gd` | The Academy boss: Defend twice for both births, Dynamite on one creature, the replacement, with captures | `academy` (0) fights the boss with the policy, not this order of commands | Kept (capture observer) |
| `native_choices.gd` | YES, NO and Cancel at Chaz's house under retail-paced text, with captures | `aiedo-chaz-house` (14) answers the same prompt; it runs at its own pace and captures nothing | Kept (capture observer) |
| `native_dialogue.gd` | The post-Igglanova scene from the isolated `dialogue_resume` save, captured page by page without acknowledgement | State only | Kept (capture observer) |
| `native_rika_fixture.gd` | Rika's join scene from `PSIV_DEBUG_EVENT=8007`, every page and the Holt exit effect, no input | `bioplant-rika` (11) proves the join's state | Kept (capture observer) |

### Blocked: covered, but a document outside this lane's write set runs it

| Driver | What it proved | Covering chapter | Why it is still here |
| --- | --- | --- | --- |
| `native_opening.gd` | One Up press after the auto-acknowledged opening has returned control | `academy` (0) starts from START and the first pad is the first gameplay frame | `docs/campaign/PLAYABILITY_FOUNDATIONS.md` runs it in a `bash` block, so `tools/check_docs.py` fails if the file goes. Delete it with that block |
| `verify_native_alarm.py` | The BioPlant alarm palette ramp and its four-frame cadence in captured PNGs | None. Its only producer, `native_bioplant.gd`, is retired and the tape takes no mid-run capture | `docs/campaign/BIOPLANT_NATIVE.md` runs it in a `bash` block. It has no producer left, so it is dead code until the tape driver captures at chosen frames |

## Checks on the kept drivers this lane touched

Both ran under the shared heavy lock from chapter saves of the 2026-10-06 route
(`PSIV_DEBUG_ROUTE=1`, a copied save directory, the optimized extension):

- `native_party_order.gd` after it left the route skeleton, against the
  original driver from the same commit, both started from `09-bioplant-elevators`
  and rendered under Xvfb: the same eight checkpoint names with identical
  states, the same slot bytes after the SAVE, and all seven captures
  byte-identical. A headless pair gave the same states and bytes.
- `native_continue.gd` with no route receipt, from `16-aiedo-shopping`: CONTINUE,
  one step down, SAVE into slot 2; `verify_native_continue.py` then reports
  `complete` (only the standing Y word, the header's slot number and its
  checksums changed).

The other eighteen kept drivers and `verify_native_order.py` against the original
game were not re-run in this lane; none of them reads a retired driver or its
output, but their health against today's pack is not re-proven here.

## What the retirement leaves uncovered

These were proved by a retired driver and are carried by nothing today:

- **Mid-scene captures.** The route drivers saved PNGs at fixed legs and
  scenes; the replay's one capture is the endpoint. The alarm cadence check
  above is the casualty. Capturing at named frames in `native_tape.gd` would
  restore all of them.
- **Resident palette, 50 dialogue pages and Igglanova's visible entrance**
  (`native_zema_return.gd`): presentation observations of the Zema rescue.
- **The equipment hand-selector cancel** (`native_zema_outfit.gd`): cancelling
  the hand choice mutates neither equipment nor inventory. `suites::camp_equipment`
  covers the accepted path only.
- **Per-chapter replay.** A chapter cannot be replayed alone, because a save
  holds no RNG state ([why](CAMPAIGN_RUNNER.md#r2-native-tape-replay-in-progress)).

## References left in other documents

Documents outside this lane's write set still name retired drivers in prose
(`docs/campaign/AIEDO.md`, `BIOPLANT_NATIVE.md`, `PLAYABILITY_ACADEMY.md`,
`PLAYABILITY_HOLT_TONOE.md`, `PLAYABILITY_FOUNDATIONS.md`, `docs/field/TRAVEL.md`,
`docs/camp/PARTY_ORDER.md`) and in two comments in `rust/psiv-godot/src/`
(`battle/ui/mod.rs`). They are historical ledgers of dated runs and none is in a
command block, so the checks pass; they should be annotated or trimmed in their
own change.
