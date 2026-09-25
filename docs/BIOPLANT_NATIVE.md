# Native BioPlant progression

**BioPlant checkpoint (2026-09-23):** the connected healthy `$A7` continuation
reaches Rika, joins her to the original four-person party, and escapes to
Motavia `$00 (99,83)` with everyone alive. Ordinary SAVE and fresh-process
CONTINUE pass; one Down and SAVE 2 reach `(99,84)` with only the expected
position and slot/checksum bytes changed. See the [current receipt](#connected-rika-continuation-2026-09-23).
The original full-health `$A7 (68,66)` source and all failed attempts remain
intact. Older failures and isolated visual fixtures below retain their own
revision/input scope; they do not describe the new checkpoint.

The subsequent [northern-crossing continuation](TRAVEL.md#post-rika-northern-crossing-2026-09-23)
rests this same connected party in Zema and advances to Motavia `(84,64)`.
It is the current campaign source; this ledger retains the unchanged BioPlant
save and its original candidate/evidence scope.

The connected campaign source is the restored-Zema save. Before the
dungeon, `tools/native/native_zema_outfit.gd` uses ordinary inn, shop, equipment and
save input. The driver checks exact prices and inventory changes. Its seven
purchases cost 2470 meseta; the four-person Zema inn costs 80. This driver
also checks the right/left hand selector and cancellation without inventory
mutation. The original driver assigned CIRCLET to Alys; the cartridge's
`0x050C` usable-by mask rejects that. It now goes to Hahn. The failed attempt
remains separate from a successful route receipt.

## Source-backed implementation (2026-09-15)

The earlier synthetic Rika outcome test relocates past these systems. It
does not demonstrate that a player can enter and traverse the BioPlant.

* **Birth Valley door, event `$08`:** `$06B92E..$06B9AD` in the US ROM;
  `Event_BioPlantDoorOpening` in `reference/ps4disasm/ps4.asm`. At the
  leader's current chunk and the chunk 32 pixels below it, write pairs
  `$42/$44`, `$43/$45`, `$29/$2A`. Each pair is held for `$10+1=17`
  VBlank calls. Play DoorOpened and set temporary flag 0 after the animation.
  Birth Valley B1's existing map-load effect reopens these chunks from that
  temporary flag. Its interaction rectangle is cells `(22..23,15)`, with
  the player standing immediately below it. The runtime scene now writes
  each stage's collision and graphics atomically and then sets the flag.
* **Elevator door, event `$13`:** `$06C2BC..$06C33D`. Check the chunk at
  the leader's current pixel position on the collision-selected plane. If
  it is `$4F`, play ElevatorOpen and write `$50,$51,$52,$53`. Each stage
  runs map updates, refreshes the selected plane and waits for its DMA
  VBlank. The following `dbf` is a CPU delay, not another VBlank loop.
  The interaction guard and four-stage scene are implemented.
* **Elevator ride, event `$14`:** `$06C33E..$06C477`. Trigger `$0D`
  checks chunk `$53` at `(leader.x, leader.y+16)` on the collision-selected
  plane. Play RidingElevator, fade out, apply the ordinary transition
  record, refresh, write `$53` at the destination, fade in, move down 16
  pixels, overlap followers, then play ElevatorOpen and write
  `$52,$51,$50,$4F` in the chunk behind the party. The trigger currently
  resolves against the live collision-plane chunk grid. The event consumes
  the map's normal transition record, recasts the party, opens the destination,
  walks one cell down, overlaps followers, and closes the door behind them.
* **Alarm, event `$12`:** `$ED52=3` is a delay byte, despite
  the existing `lines` field name. `Pal_VariableFadeToRed` copies all 64
  palette words, then performs eight green/blue decrements, holding each
  for `3+1=4` VBlanks. The reverse routine increments each component
  until its saved value, also eight stages of four frames. Red stays
  unchanged. The core now blocks for those 32-frame operations. A Godot
  screen shader maps the extracted RGB565 green/blue ramps back to levels,
  applies each decrement/increment, and leaves red unchanged. The map and
  sprites remain underneath the effect. Native captures verify its pixels.

The regenerated chunk atlas includes all original door-animation tiles and
their four collision values. Runtime writes replace only their chunks and
retain live objects. Elevator identity comes from the raw collision-selected
plane, including active patches; it is never inferred from collision nibbles.

Three real-pack runtime regressions verify the entrance's 17-frame stages,
opening persistence through reload and ordinary warp, a complete elevator
round trip with all destination chunks and the exit step, and both 32-frame
alarm fades before dialogue. Native campaign evidence remains separate from
these isolated fixtures.

## Route and encounter census

The ordinary route is Zema → Birth Valley `$2B` → Birth Valley B1 `$2C`
→ `$A2` → `$A3` (alarm) → `$A4` → `$A6` → `$A7` → `$A9` → `$AA`
→ `$AB` → `$AC` (Rika) → `$AD` during the scene. `$A8` is a side room.

Encounter groups 21–24 contain Sensor Bit (3), Neowhistle (6), Gicefalgue
(11), Guilgenova (13), Arm Drone (41), FlattrPlnt (75), and Ismounos (89).
Their current records request physical attacks, poison-on-hit, Guilgenova
Fission 7, and Acid Breath 51. Those gameplay routines are implemented;
continuous native survival, progression and presentation still need proof.

Native run receipts and restart results are recorded below after validation.

## Prepared campaign checkpoint

`build/native-bioplant/outfit/route/route.json` completes the seven purchases,
paid inn, all equipment changes, cancellation check and ordinary SAVE.
The original rescued-Zema slot remains SHA256
`75ce5fb3fb92660d9adfdb459059c516b00c0016fa078f5ed0428608d0916a40`.
The equipped source is `build/native-bioplant/outfit/saves/slot_1.sram`,
SHA256 `6f257d112740505f92ad7163759da559e9d5d4f8bdb646ea1cbbb2155cf0871e`.
Its fresh title Continue, one Down and SAVE 2 pass; the validator permits
only leader Y at logical 0x309 (48 to 64) and the slot/checksum header.

The compiled library for the outfit, training, armour and isolated Rika runs
before CROSSCUT was enabled is SHA256
`ed5312fe23d6262b5e896ef7de97530593faacc0d99331b2b9c45dc99bc11b8d`.
The regenerated full-pack manifest is SHA256
`1d145dff1c6254a2ff30c01033c68ec14440e5efdee98fc5e2a0268dc36a4366`.
Rust workspace: 894 tests pass; strict Clippy passes. The 81 relevant Python
checks pass across the full selection and a targeted rerun after correcting
an NPC-census assertion: chests share the sprite atlas but are counted
separately from NPC interaction records.

`campaign-flag-probe/` retains an early route-driver failure after the actual
Birth Valley door successfully opened. JSON represents temporary flags as
floats; Godot array membership tested them against an integer. The corrected
driver explicitly converts those values. This failure did not require a
runtime flag or map-collision change.

## Alarm render check

The connected run captures all 16 palette stages in
`campaign-retreat/route/alarm-{out,in}-{1..8}.png`. Each stage begins exactly four
native ticks after the previous one. `alarm-pixels.json` compares every pixel
with the restored frame transformed through the original component levels:
all 16 stages have zero differing pixels, including zero changes to red.
The fully red frame, restored frame and open elevator were visually inspected.
This verifies the component fade on the native scene; it is not a comparison
against an emulator recording of the whole scene.

Reproduce this check with:

```sh
python tools/native/verify_native_alarm.py build/native-bioplant/campaign-retreat/route
```

## Preserved traversal failure

`campaign-retreat/route/failure.json` reaches `$A9` through the entrance,
alarm and three elevator rides. Its fourth encounter is formation `$B7`,
three Neowhistles. Repeated ordinary RUN failures leave Alys, Chaz and Hahn
dead and Gryz at 17 HP; the driver stops rather than editing party state.
The escape calculation was checked against `Battle_ProcessRUN` at `$549A`:
highest party agility versus the formation's run chance, scale 2 and base
failure 40. This encounter has run chance 20. The failure does not establish
a defect in that calculation.

`campaign-support-exhaustion/` preserves the following attempt and its
driver version. It wins six fights and reaches `$A7`, gaining Chaz and Hahn
a level, but exhausts their RES budget. Its inherited Alshline planner
casts SANER/GELUN on weak groups and uses expensive single-target magic
instead of Alys's two slashers. Its full-HP field policy also spends RES on
tiny injuries. The driver stops with `route needs an inn`.

The current BioPlant driver uses ordinary attacks against weak groups,
GELUN against stronger groups, Gryz's available CRASH uses and Alys's
VORTEX against durable single targets. Hahn reserves TP for RES; field
healing stops at 90% HP. This is an input-strategy change, with no runtime
stat, encounter, escape or damage adjustment. The prepared source save
remains unchanged.

`campaign-ambush/` records that revised driver's remaining survival failure:
formation `$C2`, four Gicefalgues on `$A7`. Chaz and Hahn die; Alys and Gryz
win with 15 and 44 HP. Neither this failure nor the earlier RUN attempt is
used as a successful campaign checkpoint.

`tools/native/native_zema_training.gd` prepares a connected save using ordinary
Birth Valley battles, level-up awards, RES, a paid Zema inn and SAVE. Its
default minimum is level six for each party member, plus enough earned
meseta for two carbon suits and the final rest. The patrol caches walking
directions but still submits every step through normal input and re-reads
collision after battles. This preparation captures at 320×224 and renders
eight frames per simulated second; the game still receives its ordinary
60 physics ticks. It does not establish animation-frame parity.
`tools/native/native_zema_armour.gd` then buys and equips the suits for Alys and Chaz,
using the same checked purchase/equipment menu helpers as the first outfit.

## Fresh Rika opening comparison

`rika-fixture/opening-rmse.json` compares the first fully revealed dialogue
page with a freshly generated original-game frame 7250 from the documented
`28_meeting_rika_retail_probe.tape` fixture. The normalized 320×224 images
have **zero differing pixels and RMSE 0.000000** with the recorded `ed5312fe...`
library and regenerated pack. Both sides deliberately use isolated scene fixtures;
this certifies that one presentation frame, not the connected campaign or
the complete scene. The layered introduction and SEED-room pages were also
visually inspected.

Whole-scene parity remains open. In particular, the per-entry palette write
at `$074826` (`SetPaletteWords`, offset `$18`, two `$000E` words) is still
logged by the Godot presentation consumer rather than applied to the baked
map palette. The new BioPlant alarm shader does not implement that separate
operation.

The isolated fixture completed at native tick 13982 with Rika in slot five
and both escape/join flags set. It captured 115 distinct dialogue pages;
the introduction, SEED room and explosion panels were inspected. Its final
capture was taken during the map transition, so it is not a settled-field
image. The observer now waits for that transition in future runs. The exact
observer used for this receipt is retained as `rika-fixture/driver.gd`.

## Connected training checkpoint

`training/route/route.json` completes 64 ordinary victories, one paid inn and
SAVE outside Zema at `(99,83)`, tick 64729. `training/validation.json` checks
the original formation rewards against the saved character records: each
party member gains exactly 665 XP, earnings total 1155 meseta, and the inn
subtracts 80. Inventory and story flags remain unchanged. Final money is
1588; Alys/Chaz/Hahn/Gryz are levels 7/6/6/7, with HP 53/53/45/76 and full
TP. Chaz learns CROSSCUT normally.

The training slot SHA256 is
`9c418908cc4a1f4e641eaf7931523fd9595a84b2111659019f4f68cff0c051ab`.
The equipped-Zema source still has its original `6f257d11...` hash. Initial
training preflights were stopped to bound the target and reduce rendering
work; only the completed `training/` route supplies a campaign checkpoint.

`armour/route/route.json` continues that exact save, buys two CRBN-SUITs for
1100 meseta, equips Alys and Chaz, and saves at `(99,83)` with 488 meseta.
The equipment captures were inspected. The resulting slot SHA256 is
`0494b7205b8626a8be700f46452e135457ce5df2060cd14195e2b7bdc60a9e95`.
Fresh title Continue, one Down and SAVE 2 pass in `armour/continued/`;
only logical payload Y at `$309` changes from 48 to 64, with valid slot and
checksum headers. This armoured save supplies the next connected run.

## CROSSCUT gameplay

The trained attempt in `campaign-crosscut-unavailable/` exposed a missing
dispatcher: Chaz had learned CROSSCUT, but the battle menu kept it unavailable.
The native core now enables skill 1 using its existing cartridge record
`01 85 11 50 06 10 00 00` at `$2A9D28`.

`BattleObj_Crosscut` calls `loc_3B684` twice, setting the parent to state 7
(`loc_9848`) 25 animation ticks apart. Each living-target hit independently
uses ATK, target DEF, weapon element and power 80, with 16 damage RNG draws.
`loc_B75A` skips a target killed by the first hit. The implementation emits
two damage events, stops on death without retargeting the second hit, and
spends one learned-slot use with no TP. Two regressions check independent
85/96 damage outcomes, 32 draws, exact resource consumption, and the first-hit
kill path with only 16 draws and one death award.

The Godot consumer displays the existing skill narration and damage reactions;
the original CROSSCUT sword animation and its 25-frame spacing remain open.
The route planner now uses SANER once against strong groups of at least three,
so recovery can execute before the next enemy volley, while preserving TP
against weak encounters.

After this change, all 896 workspace tests and strict Clippy pass. The native
library for the next connected attempt is SHA256
`651630d96cce6962da07976da67a7de7692a7688fa64c617c7eda4c17fcd2ebc`.
The pack is unchanged; `campaign/source.json` records all driver and input
hashes for that attempt.

The first CROSSCUT-enabled run survives formation `$C2` but loses Alys in
the later `$C1` surprise attack. `campaign-support-not-cast/` preserves the
failure; `campaign-command-probe/` records every menu transition and narration.
The opening ambush queues only enemies. The driver had marked SANER and GELUN
as complete when selecting them, even though that opening round discarded all
party actions. It now records support only after the actual cast narration.
This bookkeeping fix does not change ambush priority or enemy damage.

The route also makes normal menu SAVE checkpoints on arrival at `$A7`, `$AA`
and `$AC`, preserving copies as `saved-map-XXX.sram` alongside their state
receipts. `PSIV_BIOPLANT_RESUME_MAP` selects the matching remaining route leg
when continuing one of those saves. Recovery uses the game's title CONTINUE;
it never restores HP, resources, locations or flags through a debug write.

`campaign/route/saved-map-0A7.sram` reaches the first large floor at `(68,66)`
with full resources and 495 meseta. Its SHA256 is
`88f62eceab4d1b777e824f7dd7dc0577183a8bf8716de73b0583bfb2595d78b3`.
The fresh restart in `continued-A7/` passes the byte-level Continue validator.
The campaign's subsequent fourth fight still kills Alys; the actual cast
trace now confirms SANER/GELUN execution. Its complete 16-stage alarm capture
passes the pixel and cadence validator with the CROSSCUT-enabled library.
`resumed-A7/` continues the preserved elevator save and adds Gryz's existing
16-TP group BROSE to the ordinary command strategy.

That resumed attempt also ends with `route needs revival`: the first surprise
attack and following enemy volley kill Alys and Hahn before their actions.
BROSE spends its actual 16 TP and defeats one target; Chaz and Gryz win the
fight. The pre-fight checkpoint remains healthy and unchanged. No connected
Rika join or escape is claimed. Further traversal needs a survivable ordinary
play strategy or additional earned preparation; these failures alone do not
justify changing the cartridge's encounter, initiative or damage rules.

## ORDER checkpoint (2026-09-15)

STATE/ORDER was still a stub. Its original chooser is now implemented;
`docs/PARTY_ORDER.md` records the source, targeting consequence and visual
scope. `build/native-order/native-final/route/route.json` completes ordinary
pick, undo, cancel, Gryz/Alys/Chaz/Hahn selection and SAVE at tick 474.

The cartridge emulator loads the original native `$A7` save directly using
`--load-sram`, without RAM patches. The ORDER tape produces the same party
bytes at `$F40A`. The native open, picked, undone and completed-list regions
each match an original frame exactly: RMSE 0, zero differing pixels and no
masked pixels. This does not certify the surrounding camp summary or camera.
All four comparisons and artifact hashes are in
`build/native-order/native-final/order-validation.json`.

The new source save is `build/native-order/native-final/saves/slot_1.sram`,
SHA256 `f3e721e68d5af82fcbdc4f3b033f7d982f725b32902db119f6a682bf8a2819c5`.
Relative to the healthy source above, its only four changed bytes are logical
payload `$30A..$30D`: `[1,0,2,4]` becomes `[4,1,0,2]`. Even the additive
checksum is unchanged. Coordinates, HP/TP, skill uses, levels, gear, money,
inventory and flags remain intact.

Fresh title CONTINUE, one Down and SAVE 2 pass in
`build/native-order/native-final/continue/validation.json`. Only logical Y
at `$309` changes from 32 to 48, plus valid slot/checksum header bytes.
SAVE 2 is SHA256
`b1446e65fb7741401ead2d03000ae6ecf72022c48f16fcf76b49f5804cd70a28`.
The original ordered SAVE 1 remains untouched.

The final ORDER library is SHA256
`07e9e51c6d3d046892eaa7b32b24247054603b926b63428b5e46b5d9e1afb900`;
the pack manifest remains `1d145dff1c6254a2ff30c01033c68ec14440e5efdee98fc5e2a0268dc36a4366`.
All 898 Rust workspace tests pass with `--test-threads=1`; strict Clippy
passes. An earlier parallel test process exited 143 without a complete
result and is retained separately from the successful serial log.

The BioPlant input driver now checks that the party order it loaded survives
Rika's join, rather than requiring the former fixed four-character order.

### Onward attempt with Gryz leading

`build/native-order/campaign/route/failure.json` records six victories,
including traversal of the `$A7` elevator into `$A9`. At tick 12515, map
`$A9`, cell `(25,39)`, the sixth fight ends with Hahn dead and the driver
stops with `route needs revival`. Final HP in the new order is
`[56,33,4,0]`; TP is `[6,16,1,5]`. Gryz and Chaz are poisoned. Alys reaches
level 8, Chaz level 7, and money rises from 495 to 1945. These are unsaved
failed-run observations, not a new continuation checkpoint.

The trial demonstrates that the original ORDER mechanism gets the party
past the first repeated ambush failure. It does not complete the BioPlant.
That input strategy leaves poison untreated, exhausts Chaz's healing
TP, and never uses his six EARTH charges; those are concrete recovery and
control choices for a later attempt, not grounds for altering enemy rules.
The exact driver used by this run is copied to
`build/native-order/campaign/driver.gd`.

The campaign's normal SAVE on `$A7` arrival is byte-identical to the verified
ordered source (`f3e721e6...`). Both that source and the pre-ORDER healthy
save (`88f62ece...`) retain their full recorded hashes. No native/oracle
process remains running at this checkpoint.

## Bounded poison-recovery check

The BioPlant input driver now casts learned ANTI on poisoned living members
before ordinary HP recovery, choosing an eligible caster with at least 2 TP.
Each cast asserts that only its target's poison bit clears, HP stays unchanged,
and exactly 2 TP leaves the caster. It then closes the cure menu before RES.
This changes the automated play strategy; the existing game rules are unchanged.

`tools/native/native_bioplant_recovery.gd` limits the connected proof to one encounter,
two cures, ordinary healing and SAVE. It passes in `build/native-order/recovery/`:
Gryz and Chaz are cured, Hahn spends 4 TP, and all four survive. The save is at
`$A7 (75,61)`, tick 3093, with 627 meseta; HP `[70,61,53,45]`, TP `[6,34,19,38]`,
and all persistent status bytes zero. The new separate source is
`build/native-order/recovery/saves/slot_1.sram`, SHA256
`50d94ce996a34f3f7174ab27a8beeeb1fedd1406df842f0e4c99332c96f0b72c`.

Fresh CONTINUE, one Down and SAVE 2 pass the byte-level validator: only logical
Y `$309` changes from 208 to 224, plus valid slot/checksum headers. The original
full-health ORDER save remains `f3e721e6...`. Driver copies and hashes are beside
the recovery receipt. The 320-pixel captures clip camp windows and do not
establish menu visual parity; this run verifies input, state and persistence.

`recovery-assert-type/` preserves the first probe, where ANTI worked but a
dictionary comparison rejected integer zero against JSON floating-point zero.
The assertion now preserves the decoded numeric type; the corrected run passes.
No Rust or pack changes were needed, and no further dungeon trial was started.

## Connected Rika continuation (2026-09-23)

This bounded continuation starts from the healthy Gryz-leading `$A7 (68,66)`
save above, not the partial-health recovery fixture. Acceptance is an ordinary
route through Rika's join and escape, the original four members alive with
Rika added in slot 5, event flags `$34/$35`, Motavia `$00`, normal SAVE, and
fresh-process CONTINUE with state and resources preserved. The retail basis
is [Meeting Rika](scenes/30_MeetingRika.md): join at `$07484E..$074853`, escape
flag at `$074890..$074899`, joined flag at `$074A2E..$074A35`, and final map
load at `$074A36..$074A65`. This does not certify whole-scene presentation.

### Candidate and preserved failure

Base revision is `501fcdddb8b922f5d6001c2c4f777c955f66ab65`; the only executable
change is normal dialogue input in `tools/native/native_bioplant.gd`. The frozen
driver SHA256 is
`aace43a6415ae300aa42274b74f17802960c24eef0268514076f4a0add832fe3`.
The current-source extension build passed and remains SHA256
`836f59d181658262432a59a8ff489f00c3bee5646833fd27a34e4c641ab8dc6c`.
The full pack manifest remains `1d145dff...`; the protected source save remains
`f3e721e6...` (full hashes above and in the input receipt).

All new raw evidence lives under ignored `build/native-bioplant-20260923/`.
`inputs.json`, `build-receipt.json`, `candidate-review.json` and per-attempt
`source.json` record actual revisions, hashes, commands and environment.
Driver snapshots accompany each attempt. These local inputs and receipts are
not included in a fresh clone.

`attempt-01/` won three encounters and reached Rika's first dialogue with
HP `[73,61,53,45]`, TP `[6,22,1,24]` and 1203 meseta. The ready page never
advanced: the shared route driver waits during dialogue and this subclass
did not submit a normal confirm. The parent stopped only that owned process
group after 361.642577 seconds (process status `-15`), preserving
`dialogue-stall.json`, logs and its ordinary `$AC (25,35)` checkpoint:
`route/saved-map-0AC.sram`, SHA256
`7f56bf356d4843140df0fc977c3264617a6d990f9ca125144776564721d8fe30`.
This is a preserved driver failure, not a completed route or a combat defect.

The repair submits `ui_accept` only on ready, non-choice dialogue pages,
respecting held-key release and cooldown. It logs/counts ordinary accepts in
the final route state. No battle strategy, Rust rule or pack changed, and no
party resources were injected. Scene auto-close and scene injection are disabled.
The rerun starts again from the original healthy `$A7` source.

The bounded helper was explicitly requested as `gpt-6-luna` / `max`; child
executor metadata was not exposed, so the requested route is not independent
proof of its actual model. Parent `gpt-6-astra` / `max` reviewed the source
findings and exact diff, owns native verification and integration, and labels
that review as self-review of the integrated candidate. Sol and DeepSeek were
not needed. The helper's artifacts and parent parse/whitespace checks agree;
the following native gates independently verify its substantive result.

### Completed route and restart

`attempt-02/route/route.json` records three victories, two ordinary ANTI cures,
115 normal dialogue accepts and a successful SAVE. Rika joins the loaded
party `[Gryz, Alys, Chaz, Hahn]` as its fifth member. At Motavia `$00 (99,83)`,
the saved party has HP `[73,61,53,45,39]`, TP `[6,22,1,24,25]`, all persistent
status bytes zero, flags `$34/$35`, and 1203 meseta. The original four members'
status/resources and the inventory match the pre-scene `$AC` checkpoint.
Chaz still has all six EARTH charges; this result required no tactical change.

The final ordinary save is `attempt-02/saves/slot_1.sram`, SHA256
`49dda77fae6dc79bb7a2ea8e6b249b2b3202f8e61ee9b7bb44c3626aee4d3790`.
The completed route receipt SHA256 is
`365cff345cd24df77161c8cfb5d14e2afa6f6510746761f1701762e5b813996c`.

`continue-01/` starts a separate process and a separate copy of that save.
Title CONTINUE preserves party/resources/inventory/flags; ordinary Down moves
to `(99,84)`, followed by SAVE 2. `tools/native/verify_native_continue.py` passes:
only logical payload byte `$309` changes, `48 -> 64`, plus header offsets
`$13..$17` for the slot and valid checksums. Both source copies are unchanged.
The resaved slot 2 SHA256 is
`6f1caeda94831a3fd44354d89f71976f4b396db31cf8e2fb0640fa4a885b43f4`;
`continue-01/flow/validation.json` is
`7f3c0fc99dd112802ee4cb2aed3240e3eb37b689d88ca2c56af8c805708241c5`.

| Local check | UTC start -> finish on 2026-09-23 | Seconds | Exit |
| --- | --- | ---: | ---: |
| `CARGO_BUILD_JOBS=1 cargo build --manifest-path rust/Cargo.toml -p psiv-godot` | 15:59:39.989873 -> 15:59:40.816084 | 0.826222 | 0 |
| Godot `--headless --check-only` on the repaired BioPlant script | 16:13:13.860057 -> 16:13:14.207786 | 0.347731 | 0 |
| Connected `attempt-02` | 16:13:19.588264 -> 16:19:57.589290 | 398.001031 | 0 |
| Fresh `continue-01` | 16:22:03.131399 -> 16:22:21.489316 | 18.357920 | 0 |
| Existing save-byte validator | 16:22:21.489611 -> 16:22:21.604015 | 0.114417 | 0 |

Native commands use Godot `4.7.1.stable.official.a13da4feb`, Xvfb, software
GL, Dummy audio, `--fixed-fps 8`, `--disable-vsync` and `1280x800` resolution.
Each wrapper strips inherited `PSIV_*` variables, supplies an isolated
`PSIV_SAVE_DIR`, and enables only the read-only `PSIV_DEBUG_ROUTE` probe and
the driver's documented route/continue configuration. The original source is
copied before launch, never edited. Exact argument arrays and environment
are in the per-run receipts; local orchestration commands were:

```bash
python3 build/native-bioplant-20260923/run_attempt.py attempt-02
python3 build/native-bioplant-20260923/run_continue.py
```

Those ignored wrappers are evidence helpers, not installed automation or
fresh-clone prerequisites. The tracked entrypoints are
`tools/native/native_bioplant.gd`, `tools/native/native_continue.gd` and
`tools/native/verify_native_continue.py`; copy the recorded configuration to a new
output directory when reproducing. The latter's exact invocation is in
`continue-01/receipt.json`.

Parent inspected the 1280x800 Rika-panel, FILE SAVED and fresh-continued
field captures. The dialogue capture can land after the confirm cleared its
text, so it is not a matched ready-page frame. No new original-game visual,
animation-timing or audio parity is claimed. Xvfb input-method/VSync warnings
and the Godot exit warning about one ObjectDB instance remain in the logs;
they did not fail these gates. No Rust gameplay source changed, so the historical
full Python/Rust/Clippy/oracle results were not rerun or relabeled. Documentation
links/commands, whitespace, final diff and artifact read-back close this task.

This closes the connected BioPlant milestone for the recorded source, driver,
input and route. It is not a full-campaign or broad survival-rate claim.
At this gate's closure, campaign changes were local and uncommitted; the
docs-only PR authority did not extend to them. A later explicit owner request
authorized integration. The original source and all attempts are retained.

### Archived BioPlant task graph

The owner's 2026-09-23 continuation and unlimited scoped-repair decision own
this completed graph. The separate workflow-docs PR grant did not authorize
campaign publication. Gates, failures and review are recorded above.

```yaml
outcome: "Continue the healthy A7 save through Rika join/escape and fresh-process CONTINUE"
canonical_record: "docs/BIOPLANT_NATIVE.md#archived-bioplant-task-graph"
base_revision: "501fcdddb8b922f5d6001c2c4f777c955f66ab65"
scope: "Ordinary-input campaign continuation and scoped source-backed repairs; no debug state injection"
effort_policy: "Owner 2026-09-23: scoped repairs until acceptance passes; no fixed cycle limit"
common_inputs: ["docs/BIOPLANT_NATIVE.md", "docs/DEVELOPMENT.md", "tools/native/native_bioplant.gd"]
next_action: "Closed; see the roadmap for the separately assigned Redshirt assessment"
nodes:
  - id: BP-01
    outcome: "Re-anchor and isolate the healthy Gryz-leading A7 checkpoint"
    depends_on: []
    owner: "gpt-6-astra / max; bounded source acceptance audit by gpt-6-luna / max"
    inputs: ["build/native-order/native-final/saves/slot_1.sram", "runtime-pack/manifest.json", "common_inputs"]
    acceptance: "Expected save/ROM/pack hashes match; current-source extension builds; copied save and launch configuration are recorded"
    state: verified
    evidence: ["build/native-bioplant-20260923/inputs.json", "build/native-bioplant-20260923/build-receipt.json", "build/native-bioplant-20260923/attempt-01/source.json"]
    effort_policy: inherited
  - id: BP-02
    outcome: "Reach Rika and escape with the campaign party alive through ordinary inputs"
    depends_on: [BP-01]
    owner: "gpt-6-astra / max orchestration and review; gpt-6-luna / max bounded driver repair"
    inputs: ["BP-01 receipt", "tools/native/native_bioplant.gd", "retail-backed join/escape acceptance"]
    acceptance: "Completed route receipt, expected party and story flags, surviving members and an ordinary saved checkpoint; failed attempts retained"
    state: verified
    evidence: ["build/native-bioplant-20260923/attempt-01/dialogue-stall.json", "build/native-bioplant-20260923/candidate-review.json", "build/native-bioplant-20260923/attempt-02/receipt.json", "build/native-bioplant-20260923/attempt-02/route/route.json"]
    effort_policy: inherited
  - id: BP-03
    outcome: "Prove the escaped checkpoint persists through a fresh process"
    depends_on: [BP-02]
    owner: "gpt-6-astra / max (serialized native gate)"
    inputs: ["BP-02 save/hash/route receipt", "tools/native/native_continue.gd", "tools/native/verify_native_continue.py"]
    acceptance: "Fresh title CONTINUE retains party/resources/inventory/flags; safe ordinary movement and new-slot SAVE pass byte-level validation"
    state: verified
    evidence: ["build/native-bioplant-20260923/continue-01/receipt.json", "build/native-bioplant-20260923/continue-01/flow/validation.json"]
    effort_policy: inherited
  - id: BP-04
    outcome: "Review applicable checks, preserve receipts and close the campaign milestone honestly"
    depends_on: [BP-03]
    owner: "gpt-6-astra / max"
    inputs: ["final diff and exact inputs", "BP-01/BP-02/BP-03 receipts", "applicable source/native checks"]
    acceptance: "Required focused/full gates for actual changes pass; captures inspected within stated scope; ledger and overview updated; graph archived and owned processes stopped"
    state: verified
    evidence: ["build/native-bioplant-20260923/final-receipt.json", "build/native-bioplant-20260923/readback.json"]
    effort_policy: inherited
```
