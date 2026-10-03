# Early runner halt receipts (H1–H16)

Historical diagnoses moved from the [runner log](RUNNER_LOG.md#halts-and-their-diagnoses).
That log remains the current receipt and anchor index; these paragraphs retain
the original source and run context.

### H1: the Mile sand worm trigger halts every visit to Mile

**Fixed (issue #54, run R1-5).** `trigger_custom.rs` now transcribes `RunEvent_MileSandWorm` with the field RNG supplied through `TriggerContext` (one draw only when the flag and box pass), and the three other formerly unsupported custom triggers (`RidingElevator`, `EnterGrbkTwDoor`, `PenguFeedStolen`); the `Unsupported` variants are gone. The route visits Mile again, in `holt` and in `rune-dorin` (the Mile inn). Arrival at pixel y `$310` is outside the worm's box, so no RNG draw and no event. Event `$71`'s scene (`Event_MileSandWormBattle`, `ps4.asm:152222`) is not registered in `rust/psiv-core/src/scenes/mod.rs`; a player who walks into the box and rolls `& $1F == 0` meets `SceneMissing`. The same holds for `$37`/`$38` (Garuberk tower door) and `$96` (Pengu Feed stolen). The diagnosis below is kept as found.

- **Halt:** R1-1, chapter `holt`, objective 0 (`go_to_map 29 via_warp 1`), on the
  frame the party arrives in Mile (map `$1D`, cell (20,49)):
  `scene_fault: TriggerUnsupported { trigger: 112 }` (`$70`).
- **Cause (port):** `rust/psiv-core/src/trigger_custom.rs:43` answers
  `TriggerResult::Unsupported(.., Unsupported::Rng)` for `MileSandWorm`
  unconditionally, and `trigger_custom.rs:434` pins it (`the_four_undecidable_routines_say_so`).
  The trigger table binds `$5C..$70` to it (`rust/psiv-core/src/trigger_table.rs:407`).
  Most of the routine is decidable without any RNG.
- **Cartridge:** `RunEvent_MileSandWorm`, `reference/ps4disasm/ps4.asm:116449`:
  it tests `EventFlag_MileSandWorm` (`$1B`, `ps4.constants.asm:1505`) and
  returns NoEvent when set; returns NoEvent unless the leader's `curr_x_pos`
  is at most `$170` and `curr_y_pos` is within `$150..$2B0`; only then calls
  `UpdateRNGSeed` and fires event `$71` when `RNG_Seed & $1F == 0`. Arrival is
  at pixel y `$310`, so the cartridge answers NoEvent, takes no RNG draw, and
  the party walks on. The port halts where the cartridge does nothing.
- **Fix lane:** decide the flag test and the position box in `trigger_custom.rs`
  as the routine does, and raise `Unsupported::Rng` only inside the box with the
  flag clear (or give `TriggerContext` the shared RNG and run the draw and event
  `$71`, which is the real fix). Add the position cases as unit tests. Until
  then every visit to Mile halts a runner, and a player who walks into the
  worm's box meets the same gap.
- **Route alternative used:** Mile is not needed. The two Mile objectives of
  `holt` (a detour before Zema, flagged `verify` by R0) are gone; the route goes
  to Zema directly and every later claim still holds. The Mile inn at the start
  of `rune-dorin` is replaced by the Piata inn: the Zema inn is shut
  (`psiv-campaign plan --from-map 36 --from-cell 31,49 --to-map 39` with the
  flags the route holds finds no walk; with `event:0x33` and `event:0x37` it
  finds the 23-step one), so `go_to_map 25`, stand at (41,32), `rest_inn`,
  `go_to_map 0`. A player can choose that inn.

### H2: the Holt scene's landing cell (route claim)

`expect` after Professor Holt claimed Zema (60,21). The scene writes
`Map_Start = ($3C,$14)` in 8-pixel units (`docs/scenes/13_ProfHolt.md:26`):
cell (30,10), and the port stands the party one row south, (30,11), as at first
control. The route now says (30,11). No port defect: the claim halved the wrong
unit.

### H3: the Zema inn is shut early (not a defect)

`rune-dorin` first replaced the Mile inn with the Zema inn and halted with
`unreachable: no walk from (31,49) fires warp 4 of map 0x24`. The inn's door is
patched shut until the Igglanova rescue (static plan evidence under H1). The
story keeps the town sealed; the route uses the Piata inn.

### H4: a lost battle in the Valley Maze (runner gap)

The default policy only attacked, never cured between battles and never
revived; Chaz and Hahn fell in the first maze fights and Rune walked alone until
the party was wiped (`lost_battle`). The runner now cures the party through the
camp after each battle (`rust/psiv-campaign/src/recovery.rs`: the same TECH menu
a player uses, a healing technique from whoever has the TP, until nobody is
below 70%), and the in-battle threshold is 50%. With the cure in place R1-4
walks the maze with nobody down.

### H5: Gryz's storage door is object 1 (route `verify`)

`alshline` objective 7 named object 0; the press reaches object 1
(`talk` halts `wrong_object` naming both). Route fixed.

### H6: a lost battle in the Alshline basement (runner gap)

The chapters `alshline` and `zema-rescue` name `run_unless_boss`; the runner
resolved every name to the default policy and fought a basement formation it
cannot beat. `run_unless_boss` now runs from random encounters (RUN on the main
options until it works) and fights scripted battles.

### H7 to H11: runner gaps and route claims, fixed

- **H7** `open_chest` halted `wrong_object`: a chest press opens a chest
  window, not a dialogue. The talk controller reads the loot window as opened.
- **H8** `equip` for a member who wears something in every slot: the EQUIP
  menu opens its list only from an empty slot and takes off what a full slot
  holds (`rust/psiv-runtime/src/session/camp/equipment.rs`). The controller
  clears the item's own hand, or a head or body piece it puts back on after.
- **H9** the BioPlant elevator: after event `$13` the door cells are walkable
  map-change ground (collision 1) while the warp record is a ground trigger, so
  the stepping rule never "fires" it and R0's flood found no walk. The
  controller walks into the warp's trigger area and lets the game's elevator
  event do the rest. Confirms the route's `opens` claims.
- **H10** `bioplant-rika` objective 8: the Rika scene fires on the way and
  carries the party off the map, so the `go_to`'s target no longer exists. A
  scene that ends the walk off the target map now ends the objective; the next
  objective's `expect` asserts what the scene left.
- **H11** north bank: the `expect` claimed at least 1103 meseta, one native run's
  purse; a New Game run had 914. The purse is what the battles paid, not a rule,
  so the clause is gone.

An earlier gap, found on the first academy run: `Igglanova: talk` presses Speak
and the engine starts interaction area 0 of map 23 (event `$6B`), not an object's
dialogue; the controller accepts an area event as what the press opened.

### H12: the Alshline basement kills an under-levelled party after the walk shifted

Restoring Mile moved the frame at which every later random battle rolls (the
RNG stream is a function of the frame count and the pad history), so R1-5's
first attempt met a different basement sequence: formation 177 on map `$48` took
Chaz to 11/39 and Hahn to 8/30 in one round, `run_unless_boss` kept pressing RUN
(each failed run is another round), and the party was wiped inside five
encounters (`lost_battle`, chapter `alshline`, objective 13, frame 74,971).
R1-4's six-battle basement walk was a lucky draw, not a margin: Chaz was L4 and
Hahn L3 entering it. Not a port defect.

Fix, in `rust/psiv-campaign`: a new chapter `basement-training` between
`rune-dorin` and `alshline` rests at the Tonoe inn and then patrols the strip of
Motavia outside Tonoe's gate (the only ground reachable from it without a warp)
until the party is level 6. `patrol` gained an optional `refuge`: objectives run
when a member has fallen or a living member is below half HP after the camp
cure (`Driver::needs_refuge`), here an inn trip out and back, after which the
patrol resumes. Without it the first patrol attempt trained until the healers
ran dry and the party died on the strip. R1-5 trains in 22 battles with one
refuge and enters the basement at Alys L8, Chaz L6, Hahn L6, Gryz L7.

### H13: the Aiedo inn returns `AiedoEventPending` (#39)

**Implementation present in candidate `792a059`; acceptance held after
2026-10-03 review.** It was never on the critical path.

- **Halt:** C1-1, `rest_inn` at the supermarket counter (50,32) facing up:
  `unexpected_state -- the counter refused: "Aiedo rest event pending."`
  (`build/c1/evidence/h-inn-39-report.json`).
- **Cause (port):** `rust/psiv-runtime/src/shop.rs:126-127` returned
  `InnResult::AiedoEventPending` when selector 6 was rested at with `$42` and
  `$46` clear, after `RecoverStats` and before the bill; the shop window showed
  the placeholder at `rust/psiv-runtime/src/session/shop.rs:397`; nothing ever
  ran `Event_GirlsSneakingOut`, whose transcription was registered
  (`docs/scenes/27_GirlsSneakingOut.md`).
- **Cartridge:** after `RecoverStats` the counter tests selector 6,
  `EventFlag_Zio` and `EventFlag_GirlsCaught` and calls
  `Event_GirlsSneakingOut` with the palette block, the bill, the selector and
  the text variant saved around it (`ps4.asm:136391-136410`); the bill is
  taken after the scene (`sub.l d0, (Current_Money).w`, `:136414`), and the
  window is destroyed for the scene's run and rebuilt on the "rest well" line
  afterwards (`:136415-136483`). `docs/camp/SHOPS.md` "Finding 3" carries the step table.
- **Fix:** the inn transaction is two halves — `inn_begin` prices the bill and
  runs `RecoverStats`, `inn_charge` deducts it — and `Session` runs the scene
  between them through the same menu-starts-scene hand-off the vehicle items
  use (`rust/psiv-runtime/src/session/menu_scene.rs`). `InnResult` is gone.
  Existing hand-crafted save/pad receipts are isolated input/state evidence;
  the connected F2 route uses Chaz's house instead of this inn. The current
  workspace-test log's named tests appear `ok`; the pack-gated tests were
  available in that run, but remain isolated pad/state evidence rather than
  connected route coverage. The named isolated receipts are:
  `tests/session_menu_scenes.rs::the_aiedo_inn_rest_runs_the_scene_between_recovery_and_the_bill`
  (money unchanged and the party restored before the scene, 500 → 400 after it,
  `$46` set, the window back on its result line) and
  `::the_aiedo_inn_is_an_ordinary_night_once_either_flag_is_set`.
- **Additional review blocker:** `Session::install_runtime()` leaves
  `menu_scene` uncleared while resetting the other per-frame fields. A stale
  inn transaction may therefore cross START/CONTINUE. F2-R clears it and
  tests both installation paths; see the repair receipt above.
- **Does it block the story?** No, and it still does not. `$46` is read only by
  Aiedo's own map data (`MapDataMan_AiedoSupermarket`, `MapDataMan_AiedoPrison`,
  `ps4.asm:109146-109178`, which show or hide the two girls) and by this inn;
  no trigger the next arc uses reads it (`RunEvent_SavingDemi` tests `$42` only,
  `docs/scenes/31_DemiRescue.md`). The route rests at Chaz's house instead,
  which is a free rest the cartridge offers (scenes 28 and 29, passing in
  `aiedo-chaz-house`). A chapter that rests at the inn can now assert `$46`, and
  the prison map should show the girls.

### H14: the fort needs a trained party and a rest that is not the Aiedo inn

Route and runner, fixed in this lane.

- C1-3 walked the fort's first corridors at level 8 to 11 with Rika at level 3:
  `lost_battle` on map `$84` (Zio Fort F1), the party wiped inside three
  encounters even with `run_unless_boss` (a failed RUN is another round of
  enemy attacks).
- Fix: a new chapter `aiedo-training` patrols the open ground east of Aiedo's
  gate (Motavia (40,56) to (50,56)) until every member is level 12, with a
  `refuge` that rests at Chaz's house (a free rest) when a member falls or the
  healers run dry, and rests there once more before leaving so the party enters
  the passageway whole. 171 battles, 281,447 frames. The first draft stopped at
  level 10 and arrived at Juza with three members down (C1-5 draft).
- Runner: `walk.rs` now ends `go_to_map` at once when a scene's yes/no prompt
  opens on the target map (Chaz's house asks on arrival), leaving the prompt
  for the next `answer` objective; any other prompt still halts as before.
  `tests/runner.rs::an_arrival_prompt_is_the_next_objectives_to_answer`
  (ignored, release) passes and halts when the `answer` is removed.
- A refuge bug found on the way: a `go_to_map` with `via_warp` names a warp of
  the *current* map. The refuge's first leg runs on Motavia, where warp 3 is
  not Aiedo's gate, so it walked the party across the continent into poison
  and a defeat (one draft run). The route names no `via_warp` there now.

### H15: passageway and Zio Fort encounters roll unsupported abilities

**Open port defects.** Random battles on the only road to the Zio Fort fault
the battle presentation when an enemy rolls an ability the engine does not run
(`BattleEvent::UnsupportedAbility`, emitted at
`rust/psiv-core/src/battle/engine.rs:745` and `:847`; the session turns it into
a fault at `rust/psiv-runtime/src/session/battle/mod.rs:386-391`).

| Ability | Carrier and where | Halt | Cartridge |
| --- | --- | --- | --- |
| 18 FUSION (`$12`) | 34 ZolSlug, formation 211 (three ZolSlugs), Passageway `$81`, the condition `EnemyAI_ZolSlugs` holds once exactly two remain (`rust/psiv-core/src/battle/enemy_ai.rs:106,463-466`) | `unsupported ability 18 for fighter 7`, frame 171,935 (C1-2); `build/c1/evidence/h15-fusion-report.json` | `ps4.asm:23017` (the condition); the routine is `EnemyAttack_Blob` (`ps4.asm:19241`, label `:23043`); ledger row `docs/battle/ENEMY_ABILITIES.md:276` (class unknown) |
| 33 FIREBREATH (`$21`) | 83 Ripper (regular list `[0,0,0,0,0,33,33,33]`), formations 219, 220 and 226 in the Zio Fort (`$82`, `$84`); 25 formations in all | `unsupported ability 33` (C1-3 variant, C1-4); `build/c1/evidence/h14-untrained-report.json`, `h16a-airslash-report.json` | `EnemyAttack_Ripper` `ps4.asm:21587` to `loc_23D0A` `ps4.asm:47507`; ledger row `ENEMY_ABILITIES.md:196` (damage, †) |
| 45 DEBAN (`$2D`) | 70 ShadowSabr, conditional slots `[45,45,44,44]` under conditions `[7,7,2,2]`, formation 226 (F1) | `unsupported ability 45 for fighter 7` (C1-4, first draft) | `EnemyAttack_ShadowSabr` `ps4.asm:21933`: arm `$2C` at `loc_F85E`, arm `$2D` at `loc_F89A`; the ledger lists only `$2C` (Airslash) for these carriers and `$2D` for 116 Radhin, so its conditional-ability table omits ShadowSabr's `$2D` |

- **Disposition:** the route runs from every encounter on the way
  (`run_unless_boss`, chapters `passage-to-zio-fort` and `zio-fort-approach`),
  which a player can choose. It is not a fix: with the draws of C1-3 and C1-4
  the same walk halts, and run C1-5 passes only because every fight in it
  ended by RUN before such an ability rolled. A change to the route anywhere
  upstream shifts the draws.
- **Smallest change:** one `DamageRoute` entry each in
  `rust/psiv-core/src/battle/enemy_damage.rs` (`DAMAGE_SKILL_ROUTES`, line 421)
  for FIREBREATH on 83 and its sibling carriers and for the two ShadowSabr arms,
  transcribed from the routines above; FUSION is not a damage ability (effect
  `$1F`) and needs its own transcription. FIREBREATH is the largest class: the
  ledger counts 54 formations (+3 boss) across the Zio Fort, Ladea Tower, Island
  Cave and later maps, so every dungeon on this arc meets it.

**C2 update.** FIREBREATH (83 Ripper and its siblings) is implemented, so the
fort's and the tower's FIREBREATH encounters are fought. FUSION and DEBAN are
still open: the passageway and fort walks run from every encounter, and C2's
final runs never rolled them. C2 met EVIL EYE (ability 76) in the Ladea Tower:
[H21](RUNNER_LOG.md#h21-evil-eye-ability-76-in-the-ladea-tower-58).

### H16: Juza's battle rolls ZAN and FORCEFLASH, which the engine does not run

**Resolved by lane a1-damage (ZAN, FORCEFLASH, FIREBREATH and CORRSION are run
and oracle-verified); the route fights Juza from lane C2 on. The diagnosis
below is kept as it was written.**

- **Halt:** C1-7, chapter `zio-fort-juza` (scratch), objective 0 (`talk npc 0`
  at Juza, `$87` (32,19)): the dialogue fires event `$40`, event battle 3
  begins, and Juza's first action faults: `unsupported ability 71 for fighter 6`
  (frame 458,343; `build/c1/run-4/halt-juza.json`). An earlier draw halted on
  ability 86 for the same fighter.
- **Cause (port):** event battle 3 is one JUZA (enemy 114, 1,523 HP) with the
  regular list `[64,64,68,68,71,71,86,86]`. `DAMAGE_SKILL_ROUTES` carries 114's
  WAT `$40` (`rust/psiv-core/src/battle/enemy_damage.rs:626`) and FOI `$44`
  (`:654`) and nothing for ZAN `$47` or FORCEFLASH `$56`, which are half of
  his draws. A fight of more than a couple of rounds cannot avoid them.
- **Cartridge:** `EnemyAttack_Juza` `ps4.asm:20575`; ZAN `loc_2AF18`
  (`ps4.asm:56372`); FORCEFLASH `loc_2A300` (`ps4.asm:55527`); ledger rows
  `ENEMY_ABILITIES.md:228` and `:241` (both damage, unsupported; FORCEFLASH
  is also carried by 115 Greneris and 116 Radhin, ZAN by 100 TechMaster).
- **Why no alternative:** the stairs to F3 are patched shut until `$41` and
  `$48` are set (`ZioFortJuzaRoom` map effect entry `$38`: with both clear the
  four cells at (24,18) are written to collision 0), `$41` is set by talking to
  Juza, `$48` by the trigger after his battle (`docs/scenes/50_JuzaDefeated.md`),
  and F3 and F4 (the Demi rescue) are reachable only from that room (the
  planner finds no other chain to `$8A` or `$8B`).
- **Smallest change:** two `DamageRoute` entries for enemy 114 (ZAN, FORCEFLASH),
  traced to the routines above, with the regression test that fights event
  battle 3 to its end.
- **Forecast, not evidence:** the scripted battles ahead are event battle 4
  (enemy 152, ZIO, ability 112, "implemented" in the ledger), 5 (117
  GY-LAGUIAH, abilities 33 and `[0,0,0,0]`; FIREBREATH is unsupported) and 6
  (139 ZIO, ability 84 BLACK WAVE, the ledger lists it unsupported with 0
  formations); Ladea Tower's carriers include VOICE `$34` and FIREBREATH.
  The next lane should expect those halts after H16 is fixed. The party also
  arrives rich (21,832 meseta) with Hahn holding a shield and no weapon; shopping
  at Aiedo's weapon shop (`$5B`) and supermarket is a legitimate step before
  the fight, and Chaz's house is the rest.
