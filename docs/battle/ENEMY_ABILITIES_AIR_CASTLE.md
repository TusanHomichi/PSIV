# Enemy abilities on the Air Castle stretch (lane A5)

The evidence behind the A5 rows of [`ENEMY_ABILITIES.md`](ENEMY_ABILITIES.md): the
abilities the campaign route meets from the Ice Digger to the Air Castle and the
three fixed battles behind it - Xe-A-Thoul's room (event battle 14), Lashiec (16)
and Dark Force 2 (17) - read out of the disassembly, implemented in `psiv-core`,
captured on the cartridge and replayed exactly. The per-ability status and class
live in the inventory; the damage chains in
[`ENEMY_DAMAGE_ROUTES.md`](ENEMY_DAMAGE_ROUTES.md) ("A5 damage chains"); the status
and stat arms in [`ENEMY_EFFECT_ABILITIES.md`](ENEMY_EFFECT_ABILITIES.md) section 8;
the reload in [`ENEMY_FUSION.md`](ENEMY_FUSION.md). The census that set the work is
section 7 below, generated.

## 1. What runs now

| ability | carriers | chain | port |
|---|---|---|---|
| `$31` GRA, `$32` GIGRA | 73 DimensWorm, 74 OuterBeast | `loc_F7BE` → `$2E0`/`$2DC`/`$2D8`; five-slot request at line 39021 | `enemy_damage`, all party, `ObjectDraws::GraSparks` (section 2) |
| `$2C` AIRSLASH | 70, 71, 72 sabres (condition 2) | `loc_F85E` → `$29C`/`$2A0`; five-slot request at line 40404 | `enemy_damage`, all party |
| `$2D` DEBAN | 70, 71, 72 sabres (condition 7) | `loc_F89A` → `$2A8`; seal test at line 40129 | `enemy_effect`, the arm's defence guard plus the seal test |
| `$35` GIZAN | 77 TechPlant, 101 DarkWitch, 122-125 the DElmLars family | `$304`, `$3DC`, `$7B0`; each tests the caster's seal | `enemy_damage`, all party, sealable |
| `$5C` THNDRBLAST | 123 XeAThoul (condition 18) | `loc_E052` → `$7C0`: `bset #1` on enemy slots 1-3 (lines 53955-53961), then `loc_24BB6` | `enemy_damage`, `AllPartyParalyzingTrio` |
| `$4E` DTHSPELL | 107 Spector | `loc_EB50` → `$708`: `loc_250A2` then the kill | `enemy_effect`, `AbilityEffect_Death`, range 8; 112/113's `$72C` is read but uncaptured, so not routed (the effect table admits a captured carrier, or one sharing a captured carrier's arm and object) |
| `$4C` EVIL EYE | 131 DarkForce2 (beside the Haunt pair) | `loc_DB30` → `$850`: **two** effect calls (lines 63417, 63418) | `enemy_effect`, `Calls::TwiceLastDecides` |
| `$5F` THNDHALBRT | 128 Lashiec | `$7E4` → `loc_24A9E` | `enemy_damage`, all party |
| `$60` POSESSION | 128 Lashiec | `$7E8` → `loc_250A2`, `bset #3` on the target | `enemy_effect`, `AbilityEffect_SleepParalyze` (no damage request: the class is status) |
| `$61` ANOTHRGATE | 128 Lashiec | `$7F4` → `$7F8` (236 calls) → `loc_24A9E` | `enemy_damage`, all party, `ObjectDraws::AnotherGate` |
| `$62` REINFORCE | 128 Lashiec (condition 17) | `$7EC`: raises `$FFFFEE86`, then `loc_24C68` | `enemy_effect`, `AbilityEffect_IncreaseStats` (+20, range 3) and `AiFlags::reinforced` |
| `$64` SHDWBREATH | 131 DarkForce2 | `loc_DAA8` → `$844` → `loc_24B20` | `enemy_damage`, single |
| `$65` LIGHTSHOWR | 131 DarkForce2 | `loc_DAEC` → `$848` → `loc_24A9E` | `enemy_damage`, all party |
| first action | 131 DarkForce2 | latch `$FFFFEE87` → object `$83C` (`loc_3161C`) | `scripted_flag::first_action`, `FirstZioAction::DarkForceReveal` |
| `$3A`, `$3B` COMBINE | 84 BladeRight, 86 HakenLeft (conditions 13, 14) | `loc_F3B6` / `loc_F2E4` → `$354`: reload from `loc_23D00` | `enemy_fusion`, the pack's inline formation (section 4) |

Every chain was walked from its arm through every object it loads and every
state routine those objects jump to, for `UpdateRNGSeed2` calls, `move.w #$C`
requests and seal tests. Only GRA/GIGRA and ANOTHRGATE call the generator.

## 2. GRA's spark chain

`$2DC` loads one spark (`$2E4`, `loc_1C58E`) per occupied, living party slot, in
slot order; each takes one call and counts down `(roll & 7) + 6` frames. A spark
whose count runs out marks itself fired and - unless `$FFFFEE85` reads `$A` -
loads the next spark with one more call. A spark frees its slot on the first run
at or after firing on which its five two-frame mappings are done (its eleventh
run). `Battle_LoadObject` puts a new spark in the first free slot from
`$FFFFDA00`: a slot after its parent's runs the same frame, an earlier one the
next. `$2E0` raises `$FFFFEE85` thirty frames after the first sparks (its count
of `$30 + $1E` against `$2DC`'s `8 + 20 + 20` from the same sprite test), from a
slot past every spark's, so a spark firing in that frame still draws.

`rust/psiv-core/src/battle/enemy_damage/sparks.rs` simulates exactly that. Its
test takes the three GRA turns of the first capture call for call (12, 12 and 13
calls with three living members). **Negative control:** with the window at 29
frames instead of 30, `every_fixture_replays_as_recorded` fails on
`air_castle/gra` at f24871, round 1 (log 59 damage, port 61). The model's bound -
at most two live sparks a chain, ten with five members, which fit below `$2E0`'s
slot - is in the module's comment. GIGRA (`$32`, 74 OuterBeast) reaches the same
arm and objects; the only reader of the id on the way is a palette choice, so it
shares the route uncaptured.

## 3. Corrections found on the way

- **The partner arms `$0D`/`$0E` were inverted.** `EnemyAI_HakenLeftExists` counts
  HakenLefts into `d5` from `moveq #-1` and fires on zero - exactly one partner,
  and no second of the actor's own kind (`ps4.asm:21628-21653`). The port fired for
  a BladeRight with no partner at all, so the Air Castle's BladeRights (formations
  `$16A`, `$175`, `$176`: no HakenLeft) "combined" where the cartridge never does -
  the COMBINE halts of [H38](../campaign/RUNNER_LOG_ICEDIGGER.md#h38-the-air-castles-walk-meets-abilities-the-engine-does-not-run).
  Fixed in `enemy_ai.rs` (`pair_ready`); the BladeRights of group 52 now breathe
  fire. **Negative control:** the old rule makes `air_castle/combine` diverge at
  round 2's end state (slot 6 STR 90 for 80).
- **`Battle_GetCharHighestAgility` compares signed bytes** (`cmp.b d1, d0 / ble.s`
  from `moveq #0, d1`, `ps4.asm:17472-17483`): an agility of `$80` or more never
  wins. `Roster::highest_party_agility` took the unsigned maximum; the AIRSLASH
  capture's party (agility 255) opened with an ambush the port called a
  preemptive strike.
- **`EnemyAI_ThreeXeAThouls` clears bit 4 before it reads the slots**
  (`ps4.asm:20366-20371`), so a failed scan still clears it.
- **The caster's seal.** Sixteen objects test `btst #4, $16(a1)` on the caster and
  end the turn without their effect when it is set (`ps4.asm:38047`, `38228`,
  `38389`, `39984`, `40129`, `41375`, `44729`, `45257`, `46636`, `46778`, `54387`,
  `55555`, `55692`, `56109`, `56242`, `56505`), several through a caster prelude
  shared by a whole routine's objects. Every registered route whose arm reaches one
  is now `sealable` - the A5 rows and, as a class, the earlier GIWAT, WAT, FOI, ZAN,
  FORCEFLASH, RIMIT, DORAN, GELUN, SEALS, VOL, DEBAN and RES/GIRES arms. Each sealed
  exit was read to its end state (no request, no effect call, no call to the
  generator). The two census tests pin the sets; no capture yet shows a sealed
  caster.
- **DarkForce2's EVIL EYE calls the effect twice** (`loc_30F10`): the Haunt pair's
  once. **SHDWBREATH is DarkForce2's single damage request**, not a scripted turn;
  **POSESSION makes no damage request**. The inventory's classes are corrected.
- **Event battles in fixtures.** `loc_B62A` discards the opening roll's verdict for
  a non-negative `Event_Battle_Index` (`ps4.asm:17444-17446`), and the RAM log has
  no column for that byte: `python3 -m oracle.fixture --event-battle N` records it,
  and the replay starts such a fixture as a boss battle.

## 4. COMBINE and the inline formations

`$354` (`loc_23C84`, `ps4.asm:47469-47503`) runs everything on its first frame:
clear the 32 objects at `$FFFFD800` and the four enemy fighter words, copy ten
bytes from `loc_23D00` over `Enemy_Formation_Data`, call `loc_14D46`, set
`Battle_Routine` `$16`. Fusion's `BattleObj_Fusion` does the same from `loc_1A2F4`.
The copy takes the header, so `Enemy_Run_Chance` (`Enemy_Formation_Data + 1`)
becomes the record's.

The records are decoded by `psiv_tools.formations.extract_inline_formations` (each
accepted only when exactly one `lea (<offset>).l, a0` loads it), carried in
`battle/formations.json` as `inline_formations` (`psiv-data`'s `InlineFormation`),
installed by `encounters::battle_data` and looked up by label in
`enemy_fusion.rs`; Fusion moved onto the same path, so no formation bytes are in
the engine's source. The replay mirror gets them from `oracle.sweep.replay_pack`.

COMBINE's arms clear `$24(a4)` before the object loads (`ps4.asm:21550`, `21621`),
so no frame of the log holds `$3A`/`$3B`: the extractor files the turn as an
attack that resolved no slot, the comparator accepts `EnemiesFused` for it, and
the recipe checks the capture by the enemy the reload seats. In the capture the
HakenLeft won the race (`$3B`).

## 5. Captures

`python3 -m oracle.sweep.air_castle --capture --extract --work <dir>` reproduces
the set (one case at a time, `--only NAME`); its cases are listed with `--list`.
Party-script cases declare their party fixture (`oracle.sweep.player_capture`'s
patches) and use a `--prepare-script` scout. Every capture ran twice
byte-identical and passed `python3 -m oracle.rng_trace check`; the fixtures are in
`rust/psiv-core/src/battle/replay_fixtures/air_castle/` and replay exactly
through `every_fixture_replays_as_recorded`, with `divergences.json` still empty.

| fixture | battle | party | ability observed (round) | rounds kept | trace sha256 |
|---|---|---|---|---|---|
| `gra` | `$167`, two DimensWorm | tape 07, durable | GRA (1, 2, 4) | 4 | f51982e06855 |
| `deban` | `$164`, two FrostSaber | tape 07, durable | DEBAN (3, filed `wasted`: its effect is on the enemy side the log does not read) | 6 | 4a24dd179d54 |
| `dthspell` | `$16D`, two Spector | tape 07, durable | DTHSPELL (1, 3; the party falls in 3) | 3 | 09b4b7836783 |
| `airslash` | `$164`, two FrostSaber | script: Alys FOI twice | AIRSLASH (3, 4) | 4 | 64276204890e |
| `combine` | `$1BA`, BladeRight x2 and HakenLeft | script: Chaz fells one BladeRight | COMBINE `$3B` (2); TwinArms seated | 2 (round 3 is TwinArms' BLADESHINE) | 187ec000d642 |
| `xe_a_thoul` | event 14 | script: Alys ZAN | THNDRBLAST (2), GIZAN (4), GIWAT | 4 | 45ab646f7541 |
| `lashiec` | event 16 | script, defences 999 | ANOTHRGATE (1, 3, 6), THNDHALBRT (2, 4), POSESSION (5) | 6 | 712a10c17a40 |
| `reinforce` | event 16 | script, dexterity 120 | REINFORCE (5), then victory | 5 | f7e964c50fdb |
| `dark_force_2` | event 17 | tape 07, durable | first action (1), LIGHTSHOWR (2, 4, 5, 8), EVIL EYE (3), SHDWBREATH (6, 7) | 8 | 39ff800109f7 |

Retained failures (in the lane's `build/a5-evidence/`): three AIRSLASH delays with
the tape party (it deals 1 damage to FrostSaber), the first COMBINE scripts (an
opening ambush, then a missed swing), and a Lashiec script whose trace did not
close at f26021-26022 (a call straddling a frame boundary) and whose party fell.

**Negative controls**, each run against `every_fixture_replays_as_recorded`: the
spark window at 29 (section 2), the old partner rule (section 3), ANOTHRGATE at 235
calls (fails), and the inline formation's lea count at zero or two
(`tests/test_inline_formations.py`).

## 6. The route

Both runs used a release `psiv-campaign` built from the lane's tree and a pack
built from it (`python3 -m psiv_tools pack`, which carries the inline formations):

- **`routes/main.json` from power-on** halts in chapter `esper-mansion`, objective
  1: `expect_failed -- vehicle is 2, expected 0`, after 3,698,034 frames, digest
  `ff9fbc995dc5b82a` - the cutscene field reload owned by another lane, and the
  same frames and digest as the base revision's own probe of the route
  (`build/c26-route-probe2`): nothing in this lane changes the route's prefix.
- **The dropped chapter, restored.** `air-castle-xe-athoul-room` from
  [H38](../campaign/RUNNER_LOG_ICEDIGGER.md#h38-the-air-castles-walk-meets-abilities-the-engine-does-not-run),
  without its `wait 7`, appended to a scratch copy of the route and run with
  `--from-chapter` from the `air-castle-arrival` save of the base's last
  completed candidate (`slot_1.sram` SHA-256 `06feecfa…41d7`): **completed**,
  11,284 frames, 16 random battles, digest `99f2b2e5ace1a112`, the party at the
  foot of the Xe-A-Thoul room (map `$184`, cell (31,51)). The walk no longer needs
  a pause to re-roll its battles. The chapter is not in the committed route; its
  restoration, with `Event_XeAThoulBeforeBattle` and its fight, is the campaign
  lane's.

## 7. The census

The tables below are `python3 -m oracle.sweep.route_abilities --stretch
dezolis-air-castle`'s output, regenerated with `--update-doc` (a test fails when
they drift). The census is static: it lists BladeRight's conditional COMBINE
although no group-52 formation holds a HakenLeft, so the corrected arm never fires
there and TwinArms (BLADESHINE, HAKEN BOLT) is not met on this stretch.

<!-- route_abilities:begin -->

Maps and the groups they draw

| map | symbol | named by | groups on foot | vehicle groups |
|---|---|---|---|---|
| `001` | Dezolis | chapter dezolis-ice-digger; chapter dezolis-saving-kyra; chapter gumbious-torch-stolen; chapter air-castle-arrival | 11, 12 | 13 |
| `0D4` | DezoSpaceport | chapter air-castle-arrival | - | - |
| `12D` | Meese | chapter meese-raja-sick; chapter dezolis-saving-kyra | - | - |
| `133` | MeeseClinic | chapter meese-raja-sick; chapter dezolis-saving-kyra | - | - |
| `134` | MeeseClinic_F1 | chapter meese-raja-sick | - | - |
| `160` | GumbiousEntrance | chapter gumbious-torch-stolen | - | - |
| `162` | Gumbious_F1 | chapter gumbious-torch-stolen | - | - |
| `166` | EspMansionEntrance | chapter esper-mansion | - | - |
| `167` | EspMansion | chapter esper-mansion | - | - |
| `16A` | EspMansionNorth | chapter esper-mansion | - | - |
| `16D` | EspMansionCourtyard | chapter esper-mansion | - | - |
| `16E` | InnerSanctuary | chapter esper-inner-sanctuary | - | - |
| `16F` | InnerSanctuary_B1 | chapter esper-inner-sanctuary | - | - |
| `170` | AirCastle_Part6 | Air Castle walk to the Xe-A-Thoul room | - | - |
| `171` | AirCastle | chapter air-castle-arrival; Air Castle walk to the Xe-A-Thoul room | - | - |
| `172` | AirCastle_Part2 | Air Castle walk to the Xe-A-Thoul room | 52 | - |
| `173` | AirCastle_Part3 | Air Castle walk to the Xe-A-Thoul room | 52 | - |
| `178` | AirCastle_F1_Part2 | Air Castle walk to the Xe-A-Thoul room | 52 | - |
| `17F` | AirCastle_Part7 | Air Castle walk to the Xe-A-Thoul room | 52 | - |
| `181` | AirCastle_F1 | Air Castle walk to the Xe-A-Thoul room | 52 | - |
| `184` | AirCastleXeAThoulRoom | Air Castle walk to the Xe-A-Thoul room | 52 | - |
| `18D` | Zelan | chapter dezolis-ice-digger | - | - |

Event battles

| index | scenes | enemies |
|---|---|---|
| 10 | Event_CarnivorousTrees / Event_SavingKyra | 129 ? |
| 14 | Event_XeAThoulBeforeBattle | 123 XeAThoul |
| 16 | Event_LashiecAppearance | 128 Lashiec |
| 17 | Event_DarkForce2 | 131 DarkForce2 |

Abilities the scope can meet

| ability | effect | class | ledger | carriers | foot maps | vehicle maps | events |
|---|---|---|---|---|---|---|---|
| `$02` FLAME BOLT | `$01` | damage | implemented | 0 Helex | 1 | 0 | - |
| `$04` LASRCANNON | `$01` | damage | implemented | 4 ProtectBit | 0 | 1 | - |
| `$13` CELL SPLIT | `$01` | damage | implemented | 37 SnowSlug | 1 | 0 | - |
| `$1F` DBL SLASH | `$01` | damage | implemented | 145 RedMole | 1 | 0 | - |
| `$21` FIREBREATH | `$01` | damage | implemented | 59 StoneHeads, 84 BladeRight | 6 | 0 | - |
| `$22` RAY BREATH | `$01` | damage | implemented | 118 LwAddmer | 0 | 1 | - |
| `$23` SUPERSONIC | `$01` | damage | implemented | 69 BiterFly (conditional:8), 142 Skytiara | 1 | 0 | - |
| `$24` POISONMIST | `$1B` | status/stat effect | implemented | 57 Mistralgec | 1 | 0 | - |
| `$2B` NEEDLE | `$01` | damage | implemented | 68 Rajago, 69 BiterFly | 1 | 0 | - |
| `$2C` AIRSLASH | `$01` | damage | implemented | 71 FrostSaber (conditional:2) | 6 | 0 | - |
| `$2D` DEBAN | `$0A` | status/stat effect | implemented | 71 FrostSaber (conditional:7) | 6 | 0 | - |
| `$2E` GIWAT | `$01` | damage | implemented | 71 FrostSaber, 123 XeAThoul | 6 | 0 | 14 |
| `$30` DISTORTION | `$01` | damage | implemented | 73 DimensWorm (conditional:12) | 6 | 0 | - |
| `$31` GRA | `$01` | damage | implemented | 73 DimensWorm | 6 | 0 | - |
| `$35` GIZAN | `$01` | damage | implemented | 123 XeAThoul | 0 | 0 | 14 |
| `$3A` COMBINE | `$1F` | unknown | implemented | 84 BladeRight (conditional:13) | 6 | 0 | - |
| `$4C` EVIL EYE | `$07` | status/stat effect | implemented | 107 Spector, 131 DarkForce2 | 6 | 0 | 17 |
| `$4D` CORRSION | `$01` | damage | implemented | 107 Spector | 6 | 0 | - |
| `$4E` DTHSPELL | `$02` | status/stat effect | implemented | 107 Spector | 6 | 0 | - |
| `$5C` THNDRBLAST | `$01` | damage | implemented | 123 XeAThoul (conditional:18) | 0 | 0 | 14 |
| `$5F` THNDHALBRT | `$01` | damage | implemented | 128 Lashiec | 0 | 0 | 16 |
| `$60` POSESSION | `$07` | status/stat effect | implemented | 128 Lashiec | 0 | 0 | 16 |
| `$61` ANOTHRGATE | `$01` | damage | implemented | 128 Lashiec | 0 | 0 | 16 |
| `$62` REINFORCE | `$2B` | status/stat effect | implemented | 128 Lashiec (conditional:17) | 0 | 0 | 16 |
| `$64` SHDWBREATH | `$01` | damage | implemented | 131 DarkForce2 | 0 | 0 | 17 |
| `$65` LIGHTSHOWR | `$01` | damage | implemented | 131 DarkForce2 | 0 | 0 | 17 |
| `$6A` WIND STORM | `$01` | damage | implemented | 143 Owltalon | 0 | 1 | - |

<!-- route_abilities:end -->
