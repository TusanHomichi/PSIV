# Enemy abilities from the Air Castle to the Ending (lane A6)

The evidence behind the A6 rows of [`ENEMY_ABILITIES.md`](ENEMY_ABILITIES.md):
every enemy ability the game can meet after the Air Castle - the Climate Center,
D.Elm Lars, Garuberk Tower and Dark Force 2, Seth's Aero Prism and Dark Force 3,
Reshel, Rykros, Vahal Fort and the Weapon Plant, the three towers and their
guardians, Re Faze, Elsydeon, The Edge and Profound Darkness - read out of the
disassembly, implemented in `psiv-core` and captured on the cartridge. With this
lane every regular and reachable conditional ability is implemented, for every
carrier: the per-pair census (section 6) has no open pair. The census that set
the work is section 9, generated.

## 1. The stretch

`python3 -m oracle.sweep.route_abilities --stretch air-castle-ending` derives the
work list. The route file does not reach these maps, so the stretch is the
explicit kind (`oracle/sweep/route_abilities.py`, `STRETCHES`):

- **Scene documents 57-89** (`ENDGAME_SCENE_DOCS`): each document's **Data** row
  names the scene record whose `LoadMap`s and `StartBattle`s join the scope. A
  record may live in a child module its file re-exports
  (`#[path = "dezo_campaign_late.rs"] mod ...; pub use ...::*;`), and
  `scene_record` follows that export. `88_RetailBoundaries.md` is a census with
  no Data row; the battle it names is below.
- **Map families**, each cited: the Air Castle (`RunEventsJmpTbl` `$43`-`$47`),
  Garuberk Tower (Dark Force 2, `$37`/`$38` at its seventh part), the Climate
  Center (`$3E`-`$41`), Reshel (`$3D`), Island Cave and the Soldiers' Temple (the
  Aero Prism, `$57`-`$5A`), Rykros (`Cutscene_Rykros` once Dark Force 3 falls),
  Vahal Fort and the Weapon Plant (named by the A6 scope), the three towers
  (`$49`-`$4C`, `$4E`, `$51`, `$52`, `$56`), the Inner Sanctuary and Elsydeon
  Cave (`$39`, `$4F`) and The Edge (`$53`, `$54`). Myst Vale (group 46) is left
  out: its only events are the Musk Cats' interactions, no story scene.
- **Battle 24**: `Event_AngerTowerAlys` (`ps4.asm:150697`) writes `$18` to
  `Event_Battle_Index` (line 150758) - Ryre, the Anger Tower's guardian.
- **Enemies a battle seats beyond its formation**: battle 26's form changes write
  ProfoundDarkness2 (`loc_2F8D8`, `ps4.asm:61916`) and ProfoundDarkness3
  (`loc_2EDD8`, 61119) over `Enemy_Positions`.

`tests/test_route_abilities.py` holds the stretch to every boss battle (Dark
Force 2 and 3, Gy-Laguiah, D.Elm Lars, De Vars and Sa Lews at the tower tops,
Ryre, Re Faze, Profound Darkness's three forms) and to every map with an
encounter table whose event list holds an event of these scenes; dropping the
form changes drops CANCELING from the list (the negative control).

## 2. What runs now

Damage arms (`enemy_damage/routes/endgame.rs`, `endgame_bosses.rs`); every arm,
object, request tail and call count is cited in the module:

| ability | carriers | request | object calls |
|---|---|---|---|
| `$03` RAIL-GUN | 2 GunnerBit | single | - |
| `$05` LIGHTNING | 8 Sweeper | single | - |
| `$09` MOTRCANNON | 18 Servant | all party | 1 (`$AC`, line 28770) |
| `$0A` TWIN CLAW | 20 Silvalt, 21 Goldine (arm 2) | single (two `$C8` write one request) | - |
| `$0E` MICROMISSL | 26 LifeDeletr; 28 DragerDuel, 29 JurafaDuel | all party | 8 for LifeDeletr (`$810`, line 52107) |
| `$0F` FLAMLAUNCH | 29 JurafaDuel | single | - |
| `$21` FIREBREATH, `$22` RAY BREATH, `$64` SHDWBREATH | 133 ProfoundDarkness1 | single | - |
| `$23` SUPERSONIC | 61 BlindHeads (arm 9) | all party | - |
| `$30` DISTORTION, `$65` LIGHTSHOWR | 134 ProfoundDarkness2 | all party | - |
| `$61` ANOTHRGATE | 134 ProfoundDarkness2 | all party | 236 (Lashiec's `$7F8`) |
| `$3C` BLADESHINE / `$3D` HAKEN BOLT | 87 TwinArms, 88 SoldrFiend | all party / single | - |
| `$42` RAY-SPEAR / `$43` THROWLANCR | 97 KingSaber, 98 DarkRider / 98 | single / all party | - |
| `$48` GIFOI | 101 DarkWitch, 124 LeFawGan | single, sealable | - |
| `$4A` STAR DUST | 105 ShadMirage | all party | - |
| `$52` TANDLE | 111-113 the ChaosSorcr family, 121 SaLews | all party | - |
| `$55` LEGEON | 113 ImagioMage, 121 SaLews (arm 7) | all party | - |
| `$58` LGHTBREATH | 119 CulaBellr | single | - |
| `$59` DISRUPTARM | 120 DeVars (arm 8) | all party | - |
| `$5D` TANDIL | 124 LeFawGan, 125 GiLeFarg | all party, no seal test | - |
| `$5E` MEGID | 127 ReFaze; 135 ProfoundDarkness3 | all party | 112 for PD3 (`$8AC`) |

Status and stat arms (`enemy_effect_routes.rs`):

| ability | carriers | handler, range | notes |
|---|---|---|---|
| `$16` FLASH | 43 StarDrone | `$21` DexterityDown, 9 | no request: `dexterity_battle = dexterity_mod - STR`, at least 1 |
| `$1A` CYANICBOMB | 46 VopalSphre (arm 7) | `$02` Death, 8 | the kill, then the caster leaves (`$1C8` state 8) |
| `$1E` SPARK | 49 Browren486 (arm 8) | `$02` Death, 8 | the arm picks a living android (one draw for two or more); none, and the turn is a swing |
| `$24` POISONMIST | 58 FlameNewt | `$1B` Poison, 8 | Mistralgec's arm and object |
| `$26` SHIFT | 64, 65, 66, 72 (conditional), 116 Radhin | `$09` AttackUp, 3 (self) | sealable; the conditional carriers' arms swing once attack is raised; Radhin's `$75C` first draws a random enemy for its animation |
| `$27` SANER | 116 Radhin; 65 DeathBearr (arm 2) | `$0C` AgilityUp, 2 | DeathBearr's arm compares words and always swings while its agility stands |
| `$28` DORAN | 64 DarkMaraud (arm `$0B`) | `$06`, 9 | sealable |
| `$29` SEALS | 65, 66 (arm `$0A`), 116 | `$08`, 9 | sealable |
| `$2A` RIMIT | 66 ChaosBrngr (arm `$0B`) | `$07`, 9, then `loc_25074` | sealable |
| `$2D` DEBAN | 116 Radhin | `$0A` DefenseUp, 2 | no "already raised" guard, sealable |
| `$36` STRNGLIGHT | 79 Shrieker | `$07`, 8 | `loc_24D76` |
| `$4B` SHADOWBIND | 132 DarkForce3, 105 ShadMirage | `$06`, 9 | |
| `$4C` EVIL EYE | 135 ProfoundDarkness3 | `$07`, 8 | one call (`loc_250A2`), not DarkForce2's two |
| `$4E` DTHSPELL | 112 Illusionst, 113 ImagioMage | `$02`, 8 | `$72C`, then the kill |
| `$50` BAD SMELL | 110 Ghoul | `$07`, 9 | `loc_24D76` |
| `$51` MINDBLST | 112, 113, 132 | `$07`, 9 | no request: a sleep, not damage |
| `$69` CANCELING | 135 ProfoundDarkness3 | `$27` RestoreStats, 9 | no request, no roll |

The bosses' ability-0 arms - De Vars's `$78C`, Ryre's `$7CC`, Dark Force 3's
`$854`, Profound Darkness 2's `$88C` and 3's `$8A0` - are single swings: each
object flinches `$38` and makes one request, after `loc_B6A2`'s ordinary hit roll
and with no call of its own, which is the port's physical attack; the boss
fixtures replay them.

Heals (`enemy_skill::resolve_res`, one `HEALS` row per arm): SAR `$46` (100
TechMaster) and GISAR `$49` (101 DarkWitch, 116 Radhin) heal every living enemy
in slot order (`$3D8`, `$774`); SoldrFiend's GIRES `$3E` heals its caster (`$378`
moves `Current_Actor_Index` into `Current_Target_Index`, `ps4.asm:46698`). Every
one tests the caster's seal first.

Reloads (`enemy_fusion`, the pack's inline formations): the WorkerPods' COMBINE
(`$0C`/`$0D`, `loc_1308C`: one LifeDeletr), FractOoze's FISSION (`$1B`,
`loc_1987E`: four JR.OOZE; effect `$25` is `AbilityEffect_None`) and
InfantWorm's NOTHING (`$41`, `loc_224E8`: one SandWorm). The WorkerPods' and
FractOoze's arms keep `$24(a4)`; InfantWorm's clears it.

## 3. `$FFFFEE87`, `$FFFFEE98` and Profound Darkness (#62)

`$FFFFEE98` (the Zio phase counter) was already modelled from all its writers and
readers (`battle/zio.rs`): `EnemyInit_Zio` clears it (`ps4.asm:17896`) and the
three Zio routines read and advance it (19411-19523); no endgame routine touches
it. Zio2's CORRSION, the issue's second carrier, has been a route since lane F3.

`$FFFFEE87` is a latch. Every access (`battle/scripted_flag.rs`):

| access | where |
|---|---|
| set | `EnemyInit_Zio` (17898); `loc_BAEC` (17947), called only by `EnemyInit_DarkForce1` (18086); `EnemyInit_ProfoundDarkness1` (18009); `EnemyInit_DarkForce2` (18067, its own `st`); `EnemyInit_CarnivorousTree` (18108, behind the `$FFFF4660` guard, so only the first tree) |
| read | `loc_B62A` (17448): the opening priority becomes an enemy ambush, and every enemy slot's reaction byte takes the Ambush bit (17456-17463) |
| read, clear | the first action of `EnemyAttack_ProfoundDarkness1` (19824-19828), `_DarkForce2` (19971-19975), `_DarkForce1` (20031-20035), `_CarnivorousTree` (20102-20105) |

All four first actions are modelled now (`first_action`):

- **Profound Darkness 1** loads `$864` (`loc_30376`, 62647): a palette fade and
  the `$868` raster wave; no request, no effect call, no call to the generator.
  `FirstZioAction::ProfoundDarknessRise`. Its later turns take the ordinary roll
  into the damage registry (FIREBREATH, RAY BREATH, SHDWBREATH).
- **The carnivorous trees** load `$808` (`loc_27706`, 52501), which redraws the
  trees and sets `StatusParalyzed` on the stats of `Fighter_Enemy_1`..`3`
  (52551-52555). `FirstZioAction::TreesTakeRoot` and the three
  `StatusInflicted` events. Before this lane the port made that turn a physical
  swing.

The forms (`battle/enemy_form.rs`): PD1's else arm (`$67`, written by its arm 2 at
half HP) loads `$888`, which writes ProfoundDarkness2 over `Enemy_Positions`
(61916) and calls `loc_7F22`; PD2's (`$68`) loads `$89C`, ProfoundDarkness3
(61119). The new form starts at its record's full HP with its status cleared; no
init routine runs, so the fighter keeps its reaction byte - the Ambush bit the
latch set at the start is still there when PD3 first acts, and its arm `$0B`
makes that turn MEGID. Nothing floors the forms' HP: PD1 or PD2 dealt their whole
HP between their turns die an ordinary boss death (`$840`).

## 4. Corrections found on the way

- **MINDBLST, FLASH and CANCELING make no damage request**; the inventory's class
  is `status/stat effect` for all three. FLASH's handler is
  `AbilityEffect_DexterityDown`; MINDBLST is a sleep (`loc_25074`); CANCELING is
  `AbilityEffect_RestoreStats`.
- **The BalDuels' MICROMISSL is all party** (`$E8`'s five-slot write at 72120);
  the routes ledger's "mixed chain" read a child id as `BattleObj_AbeFrogAtk`.
  **FLAMLAUNCH is `$E4`** (`loc_37704`, request at 71974), not the zero arm's
  `$E0`. **LEGEON's retail request is line 57589** (`loc_2C03A`); 56959 is the
  revision-0 (Japanese) arm, and Rune's `BattleObj_Legeon` is not on the chain.
- **SoldrFiend's GIRES heals itself**, not the lowest-HP enemy; **SAR and GISAR
  heal every living enemy** through another object (`$3D8`) than RES's `$3D4`.
- **DeathBearr's SANER is never cast**: its guard compares the words at `$1E` and
  `$20`, and an enemy's base agility byte is zero.
- **SHIFT's buff lands on the caster** (range 3). Radhin's animation picks a
  random enemy first; the pick moves the seed and nothing else.
- **The WorkerPods' COMBINE keeps `$24(a4)`**, unlike the Air Castle's, and
  **FractOoze's FISSION is a reload**, not the `loc_14CBE` refill.
- **`loc_BAEC` is DarkForce1's alone**; DarkForce2's init raises the latch itself.
  **The trees' first action paralyzes the trees**, so it is not presentation
  only.
- **PD3 reads no latch**: the inventory's `†` on its EVIL EYE was DarkForce2's.

## 5. Captures

`python3 -m oracle.sweep.endgame --capture --extract` reproduces the set (one case
at a time, `--only NAME`; `--list` names them). The machinery is
`oracle.sweep.forced_cases`, shared with `oracle.sweep.air_castle`. Every capture
ran twice byte-identical and passed `python3 -m oracle.rng_trace check`; the
fixtures are in `rust/psiv-core/src/battle/replay_fixtures/endgame/` and replay
exactly through `every_fixture_replays_as_recorded`, with `divergences.json`
still empty.

<!-- captures:begin -->
| fixture | battle | enemy abilities observed (rounds) | rounds kept | outcome | trace sha256 |
|---|---|---|---|---|---|
| `lightning` | `$151` | `$04` (1, 3, 4, 5), `$05` (1, 2, 4) | 5 | truncated | 9e31a86fceb6 |
| `motrcannon` | `$154` | `$09` (1, 2, 3) | 4 | defeat | ee0d004b4a0b |
| `micromissl_lifedeletr` | `$1A6` | `$0B` (4, 5, 6), `$0E` (2) | 6 | truncated | 87548735ed1f |
| `micromissl_drager` | `$13F` | `$0E` (2, 3, 4) | 6 | truncated | b3487b6174e3 |
| `flamlaunch` | `$1A8` | `$0E` (1, 2, 3, 4), `$0F` (2, 3) | 4 | defeat | 88ae7c7294cc |
| `flash` | `$158` | `$16` (4, 5, 6) | 6 | truncated | 1b54d05bf777 |
| `poisonmist_newt` | `$199` | `$21` (1, 2, 3, 4, 5, 6), `$24` (1, 2, 5) | 6 | truncated | 99f0cb05646e |
| `strnglight` | `$19C` | `$36` (2) | 3 | - | 83dc09d33771 |
| `twinarms` | `$1C9` | `$3C` (1, 3, 4, 5, 6), `$3D` (1, 2, 3, 4, 5) | 6 | defeat | 5962f83c4187 |
| `soldrfiend` | `$1E2` | `$2F` (1, 3), `$3C` (3), `$3D` (1, 2, 4) | 4 | defeat | e3e2ce6e79a0 |
| `kingsaber` | `$183` | `$42` (1, 2, 3, 4, 5) | 6 | defeat | dd68fc947961 |
| `darkrider` | `$1E5` | `$42` (1), `$43` (1, 2, 3) | 4 | defeat | c86bc6c6fc1e |
| `darkwitch` | `$1CC` | `$2E` (1, 2, 3, 5, 6), `$35` (4), `$48` (1, 2, 4, 5, 6) | 6 | truncated | ee279adfd275 |
| `ghoul` | `$186` | `$50` (1) | 2 | - | a7f6163957dd |
| `chaossorcr` | `$171` | `$4B` (1, 4), `$4D` (4), `$4F` (1, 2, 5, 6), `$52` (3), `$5A` (2) | 6 | truncated | 374b52c5e637 |
| `illusionst` | `$1BE` | `$4D` (1, 2, 4), `$4F` (1, 2, 3), `$51` (1), `$52` (2, 3, 4), `$5A` (3) | 4 | defeat | ba9b70160486 |
| `imagiomage` | `$1EA` | `$4D` (4), `$4E` (3), `$4F` (5, 6), `$51` (4, 6), `$55` (3, 5), `$5A` (1, 2) | 6 | truncated | 8ab146218acc |
| `radhin` | `$189` | `$26` (1, 4, 6), `$27` (2, 3, 5), `$29` (3, 4, 6), `$2D` (1, 2, 3), `$56` (2, 3, 4, 5, 6) | 6 | truncated | 7ba18da93097 |
| `culabellr` | `$1C0` | `$58` (1, 3, 4) | 6 | truncated | c9ad20b4a842 |
| `lefawgan` | `$1C3` | `$35` (1, 2, 3, 4), `$5D` (2) | 4 | defeat | 36734a0f5170 |
| `lefawgan_gifoi` | `$1C3` | `$35` (1, 2, 3, 4, 6, 7, 8), `$48` (1, 2, 3, 4), `$5D` (6, 8) | 8 | truncated | 9acb4577089a |
| `gilefarg` | `$1EE` | `$35` (1), `$5A` (2, 4), `$5D` (2, 3) | 4 | defeat | 7ff0a93329b6 |
| `railgun` | `$E8` | `$03` (1, 2, 3, 4) | 4 | truncated | 59cb8afbe941 |
| `cyanicbomb` | `$1A9` | `$1A` (4, 5) | 5 | victory | 3da298f0b46c |
| `twin_claw_silvalt` | `$155` | `$0A` (4, 5) | 5 | victory | 1a936b467741 |
| `twin_claw_goldine` | `$1A3` | `$0A` (4, 5), `$0B` (1) | 5 | victory | 96ab7be5a032 |
| `fission3` | `$197` | `$13` (1, 3, 4), `$1B` (5) | 6 | truncated | fd112cbf2e37 |
| `supersonic_alone` | `$1DA` | `$23` (2, 3, 4) | 4 | truncated | fd254eabfdd0 |
| `darkmaraud_shift` | `$17C` | `$26` (4), `$28` (1) | 5 | truncated | e18cacb62662 |
| `deathbearr` | `$1B8` | `$26` (1), `$29` (3) | 4 | truncated | d129420ff6f0 |
| `chaosbrngr` | `$1DC` | `$26` (4), `$29` (3), `$2A` (1) | 5 | truncated | 38aa949f4f12 |
| `bloodsaber_shift` | `$1C6` | `$26` (2) | 4 | truncated | 7705132bfc32 |
| `soldrfiend_gires` | `$1E1` | `$2F` (1, 2), `$3C` (6), `$3E` (3, 4, 5) | 6 | truncated | 2d87cbb19323 |
| `infantworm` | `$3C` | `$38` (4) | 4 | truncated | a1c96f6a1ba2 |
| `darkwitch_gisar` | `$1CB` | `$2E` (1, 2, 5), `$35` (1, 4), `$48` (2, 3, 5), `$49` (3, 4) | 5 | truncated | 496a03918ff0 |
| `techmaster_sar` | `$110` | `$3E` (1, 2, 4, 5), `$40` (5), `$44` (3, 4), `$46` (2, 3), `$47` (1) | 5 | truncated | 06267e64e709 |
| `radhin_gisar` | `$187` | `$26` (2), `$27` (1, 2), `$2D` (3, 5), `$49` (4), `$56` (1, 3, 5) | 5 | truncated | b857bb72b78d |
| `dthspell_illusionst` | `$1BC` | `$4E` (6), `$4F` (1), `$51` (2), `$52` (5), `$5A` (3, 7) | 8 | truncated | e0e550d111c0 |
| `dthspell_imagiomage` | `$1E9` | `$4D` (7), `$4E` (1), `$4F` (3), `$51` (5), `$52` (2), `$5A` (4, 6) | 8 | truncated | 8c68a30356e2 |
| `spark` | `$1AC` | `$1C` (3), `$1E` (3) | 4 | truncated | c68f3eb0ec81 |
| `spark_two` | `$1AC` | `$1D` (4), `$1E` (3, 4) | 4 | truncated | 001b1560ab60 |
| `combine_workerpods` | `$149` | `$0B` (4), `$0C` (3) | 4 | truncated | 70023e28554e |
| `combine_wiredine` | `$149` | `$0D` (3) | 4 | truncated | 106ccd35027b |
| `deathbearr_saner` | `$1B8` | `$26` (1) | 3 | victory | 963803b4f77a |
| `trees` | event 10 | - | 3 | truncated | 13998108dd27 |
| `ryre` | event 24 | - | 3 | truncated | 7037c0a49c47 |
| `dark_force_3` | event 18 | `$4B` (9), `$4D` (3, 5, 8, 10, 12, 16, 20), `$51` (11, 14, 17) | 20 | truncated | 386be31b4188 |
| `de_vars` | event 22 | `$59` (2, 3, 4) | 4 | truncated | 5e06442058db |
| `sa_lews_tandle` | event 23 | `$4F` (2, 5), `$52` (4, 8, 10), `$5A` (1, 3, 6, 7, 9) | 10 | truncated | fb6b94cf6eef |
| `sa_lews` | event 23 | `$55` (2, 3, 4, 5, 6), `$5A` (1) | 6 | truncated | cfeabc489c1e |
| `re_faze` | event 25 | `$5E` (1, 2, 3) | 3 | truncated | 1795acad305f |
| `profound_darkness_slow` | event 26 | `$21` (2, 7, 11), `$22` (5, 9), `$30` (23), `$4C` (26, 33, 43, 45, 50, 52, 58), `$5E` (25, 28, 32, 35, 37, 41, 46, 48, 49, 51, 53, 56), `$61` (19), `$64` (3, 4, 6, 8, 10), `$65` (13, 16, 18, 22), `$69` (29, 31, 42) | 58 | victory | a3edace91555 |
| `profound_darkness_long` | event 26 | `$21` (2), `$22` (5, 7), `$30` (13, 14), `$4C` (17, 30, 33), `$5E` (16, 19, 21, 24, 26, 27, 28, 34), `$64` (3, 4, 6), `$65` (10) | 34 | victory | 4798732f98bb |
| `profound_darkness` | event 26 | `$21` (2), `$30` (7), `$4C` (12), `$5E` (9, 11, 14), `$64` (3, 4), `$65` (6) | 16 | victory | 8aaa46df1856 |

`python3 -m oracle.sweep.endgame --table` prints this table from the committed
fixtures and the work directory's reports. The party is tape 07's (Alys, Chaz,
Hahn; `spark` and `spark_two` seat Wren, and Wren and Demi, through the
`characters` fixture), with `--durable` HP; scripted cases patch the stats their
`script` names (`oracle.sweep.player_capture.patches`). Notes:

- Rounds where a turn is filed as an attack resolving no slot - COMBINE's
  reloads that clear `$24(a4)`, InfantWorm's NOTHING, the form changes, the
  latch's first actions - are checked by what the round's end state seats
  (`infantworm` is a `seated` case) and by the exact replay.
- `--delay` moves the rolls of a forced *formation*; it moved nothing in an event
  battle (`dark_force_3` gave the same frames at delays 1-5), so the event cases
  are lengthened instead (`repeats` lets a scripted battle run past tape 07's own
  input blocks).
- `profound_darkness` is the whole fight: the latch's rise (round 1), FIREBREATH
  and SHDWBREATH, the change to the second form (round 5), LIGHTSHOWR and
  DISTORTION, the change to the third (round 8), MEGID on its first turn from the
  latch's Ambush bit (round 9), EVIL EYE, and the third form's death (round 16):
  zero HP, no death bit, no experience. `profound_darkness_slow` adds ANOTHRGATE
  and CANCELING, `profound_darkness_long` RAY BREATH.
- Uncaptured, core tests only: 105 ShadMirage's STAR DUST and SHADOWBIND (no US
  formation seats it).
<!-- captures:end -->

## 6. The per-pair census

`psiv-runtime`'s `suites/enemy_dispatch_census.rs` loads the pack, takes every
enemy record's eight regular ids and its conditional ids (skipping an
`EnemyAI_Nothing` slot, which never writes its ability - 140 Zio2's `$53`), and
asks `psiv_core::battle::dispatch_owner` which resolver runs each pair. Every pair
has one. The question is the resolvers' own gates (`enemy_dispatch.rs`), not a
second table.

## 7. Negative controls

<!-- controls:begin -->
Each control below changed one thing, ran `every_fixture_replays_as_recorded`
(or the census) and was restored; the log is the lane's
`build/a6-evidence/negative-controls.log`.

| control | result |
|---|---|
| LIGHTNING's route removed | `endgame/lightning` diverges at f24868, round 1: the port reports `UnsupportedAbility` |
| MOTRCANNON's object call 1 -> 0 | `endgame/motrcannon` diverges at f24884: damage 157 against 161 |
| LifeDeletr MICROMISSL's calls 8 -> 7 | `endgame/micromissl_lifedeletr` diverges at f25285: 141 against 144 |
| MEGID's calls 112 -> 111 | `endgame/profound_darkness`, round 9: PD3's HP 9689 against 9665 |
| PD3's death pays and marks | `endgame/profound_darkness`, round 16: status 4 against 0 |
| a form change clears the reaction byte | passes on `profound_darkness` (its first PD3 roll was MEGID anyway); fails on `profound_darkness_long`, round 16 (HP 993 against 992) - the Ambush bit's survival is the cartridge's |
| the trees' first action without the paralysis | `endgame/trees` diverges at f24869: 14 rolls against 51 |
| SPARK's pick bit reversed | `endgame/spark_two`, round 3: the wrong android dies |
| CYANICBOMB keeps its caster | `endgame/cyanicbomb` diverges at f26053: the removed caster acts |
| DeathBearr's SANER without its guard | `endgame/deathbearr_saner`, round 3: the swing's damage is missing |
| SHIFT without its guard | `endgame/bloodsaber_shift` diverges at f25944 |
| CulaBellr's LGHTBREATH route removed | the census lists `(119, 0x58)` |
<!-- controls:end -->

## 8. The route

`routes/main.json` from power-on with a release `psiv-campaign` built from this
lane's tree completes its 50 chapters in 3,852,548 frames with digest
`c73dc6fd2919f8ef`, not the base's `29c81ae27664b15b`. The first chapter that
differs is `dezolis-saving-kyra` (7,799 frames against 7,547): the carnivorous
trees' battle (event 10), whose first tree to act now spends the latch on `$808`
and paralyzes the three trees instead of swinging. That is the cartridge's turn:
`trees.json` replays exactly with it and diverges without it (section 7). The
control run - the same tree with only `first_action(0x81)` returned to `None` -
completes the 50 chapters with the base's digest, `29c81ae27664b15b`, in
3,852,511 frames, so no other change touches the route.

## 9. The census

The tables below are `python3 -m oracle.sweep.route_abilities --stretch
air-castle-ending`'s output, regenerated with `--update-doc` (a test fails when
they drift).

<!-- route_abilities:begin -->

Maps and the groups they draw

| map | symbol | named by | groups on foot | vehicle groups |
|---|---|---|---|---|
| `000` | Motavia | scene MEETING_SETH | 0, 1, 2, 3, 4, 5, 6, 7 | 9, 10, 8 |
| `001` | Dezolis | scene DARK_FORCE_2_DEFEATED | 11, 12 | 13 |
| `002` | Rykros | Rykros: Cutscene_Rykros once Dark Force 3 falls (loc_64C1E, scene census 88) | 60 | - |
| `07B` | Termi | scene ENDING | - | - |
| `092` | IslandCave | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | 57 | - |
| `093` | IslandCave_F1 | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | 57 | - |
| `094` | IslandCave_F1_Part2 | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | 57 | - |
| `095` | IslandCave_Part2 | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | 57 | - |
| `096` | IslandCave_B1 | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | 57 | - |
| `097` | IslandCave_F2 | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | 57 | - |
| `098` | IslandCave_F3 | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | 57 | - |
| `099` | SoldiersTempleOutside | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | - | - |
| `09A` | SoldiersTemple | Island Cave and the Soldiers' Temple: the Aero Prism ($57-$5A, scene 61) | - | - |
| `0BF` | MotaSpaceport | scene GUMBIOUS_BISHOP; scene REUNION | - | - |
| `0C0` | ClimCenter | Climate Center: Gy-Laguiah and D.Elm Lars ($3E-$41, scenes 64-67) | 47 | - |
| `0C1` | ClimCenter_F1 | Climate Center: Gy-Laguiah and D.Elm Lars ($3E-$41, scenes 64-67) | 47 | - |
| `0C2` | ClimCenter_F2 | Climate Center: Gy-Laguiah and D.Elm Lars ($3E-$41, scenes 64-67) | 48 | - |
| `0C3` | ClimCenter_F3 | Climate Center: Gy-Laguiah and D.Elm Lars ($3E-$41, scenes 64-67) | 48 | - |
| `0C4` | WeaponPlant | Vahal Fort and the Weapon Plant (named by the A6 scope) | 50 | - |
| `0C5` | WeaponPlant_F1 | Vahal Fort and the Weapon Plant (named by the A6 scope) | 50 | - |
| `0C6` | WeaponPlant_F2 | Vahal Fort and the Weapon Plant (named by the A6 scope) | 51 | - |
| `0C7` | WeaponPlant_F3 | Vahal Fort and the Weapon Plant (named by the A6 scope) | 51 | - |
| `0C8` | VahalFort | Vahal Fort and the Weapon Plant (named by the A6 scope) | 58 | - |
| `0C9` | VahalFort_F1 | Vahal Fort and the Weapon Plant (named by the A6 scope) | 58 | - |
| `0CA` | VahalFort_F2 | Vahal Fort and the Weapon Plant (named by the A6 scope) | 59 | - |
| `0CB` | VahalFort_F3 | Vahal Fort and the Weapon Plant (named by the A6 scope) | 59 | - |
| `0F2` | StrengthTower | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 61 | - |
| `0F3` | StrengthTower_F1 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 61 | - |
| `0F4` | StrengthTower_F2 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 62 | - |
| `0F5` | StrengthTower_F3 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 62 | - |
| `0F6` | StrengthTower_F4 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | - | - |
| `0F7` | CourageTower | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 61 | - |
| `0F8` | CourageTower_F1 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 61 | - |
| `0F9` | CourageTower_F2 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 62 | - |
| `0FA` | CourageTower_F3 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 62 | - |
| `0FB` | CourageTower_F4 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | - | - |
| `0FC` | AngerTower | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86) | 61 | - |
| `0FD` | AngerTower_F1 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86); scene ANGER_TOWER_EXIT_TOP | 61 | - |
| `0FE` | AngerTower_F2 | the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86); scene ANGER_TOWER_TOP | - | - |
| `100` | TheEdge | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | 64 | - |
| `101` | TheEdge_Part2 | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | 64 | - |
| `102` | TheEdge_Part3 | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | 65 | - |
| `103` | TheEdge_Part4 | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | 65 | - |
| `104` | TheEdge_Part5 | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | 65 | - |
| `105` | TheEdge_Part6 | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | 66 | - |
| `106` | TheEdge_Part7 | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | 66 | - |
| `107` | TheEdge_Part8 | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | 66 | - |
| `108` | TheEdge_Part9 | The Edge: Profound Darkness and the Ending ($53, $54; scenes 87 and 89) | - | - |
| `14D` | Reshel1 | Reshel: the zombie battle ($3D, scene 63) | 49 | - |
| `14E` | Reshel2 | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `14F` | Reshel3 | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `150` | Reshel2House | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `151` | Reshel2WeaponShop | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `152` | Reshel3House1 | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `153` | Reshel3ItemShop | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `154` | Reshel3House2 | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `155` | Reshel3WeaponShop | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `156` | Reshel3Inn | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `157` | Reshel3House3 | Reshel: the zombie battle ($3D, scene 63) | - | - |
| `15D` | ElsydeonCave | Elsydeon ($39, $4F; scenes 58, 82-83) | 63 | - |
| `15E` | ElsydeonCave_B1 | Elsydeon ($39, $4F; scenes 58, 82-83) | 63 | - |
| `162` | Gumbious_F1 | scene LASHIEC_DEFEATED | - | - |
| `16E` | InnerSanctuary | Elsydeon ($39, $4F; scenes 58, 82-83) | - | - |
| `16F` | InnerSanctuary_B1 | Elsydeon ($39, $4F; scenes 58, 82-83); scene LUTZ_REVELATION; scene BEFORE_ELSYDEON_CAVE; scene ELSYDEON | - | - |
| `170` | AirCastle_Part6 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | - | - |
| `171` | AirCastle | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | - | - |
| `172` | AirCastle_Part2 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `173` | AirCastle_Part3 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `174` | AirCastle_Part4 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `175` | AirCastle_Part5 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `176` | AirCastle_F1_Part9 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `177` | AirCastle_F1_Part5 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `178` | AirCastle_F1_Part2 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `179` | AirCastle_F1_Part10 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `17A` | AirCastleInner | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `17B` | AirCastle_F1_Part11 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `17C` | AirCastle_F1_Part12 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `17D` | AirCastle_F1_Part13 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `17E` | AirCastle_Part8 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `17F` | AirCastle_Part7 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `180` | AirCastle_F1_Part4 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `181` | AirCastle_F1 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `182` | AirCastle_F1_Part3 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `183` | AirCastle_F2 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `184` | AirCastleXeAThoulRoom | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 52 | - |
| `185` | AirCastleInner_B1 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 53 | - |
| `186` | AirCastleInner_B1_Part2 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 53 | - |
| `187` | AirCastleInner_B1_Part3 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | - | - |
| `188` | AirCastleInner_B2 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 53 | - |
| `189` | AirCastleInner_B3 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 53 | - |
| `18A` | AirCastleInner_B4 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 53 | - |
| `18B` | AirCastleInner_B5 | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | 53 | - |
| `199` | GaruberkTower | Garuberk Tower: Dark Force 2 at its seventh part ($37/$38, scenes 57 and 59) | 54 | - |
| `19A` | GaruberkTower_Part2 | Garuberk Tower: Dark Force 2 at its seventh part ($37/$38, scenes 57 and 59) | 54 | - |
| `19B` | GaruberkTower_Part3 | Garuberk Tower: Dark Force 2 at its seventh part ($37/$38, scenes 57 and 59) | 55 | - |
| `19C` | GaruberkTower_Part4 | Garuberk Tower: Dark Force 2 at its seventh part ($37/$38, scenes 57 and 59) | 55 | - |
| `19D` | GaruberkTower_Part5 | Garuberk Tower: Dark Force 2 at its seventh part ($37/$38, scenes 57 and 59) | 56 | - |
| `19E` | GaruberkTower_Part6 | Garuberk Tower: Dark Force 2 at its seventh part ($37/$38, scenes 57 and 59) | 56 | - |
| `19F` | GaruberkTower_Part7 | Garuberk Tower: Dark Force 2 at its seventh part ($37/$38, scenes 57 and 59) | 56 | - |
| `1A0` | AirCastleSpace | Air Castle: Xe-A-Thoul, the fake chest and Lashiec (RunEventsJmpTbl $43-$47, scenes 69-73) | - | - |

Event battles

| index | scenes | enemies |
|---|---|---|
| 11 | CLM_CENTER_FORCED_BATTLE | 117 GyLaguiah |
| 12 | D_ELM_LARS | 122 DElmLars |
| 13 | RESHEL_BATTLE | 109 ? |
| 14 | XE_ATHOUL_BEFORE_BATTLE | 123 XeAThoul |
| 15 | AIR_CASTLE_FAKE_CHEST | 107 Spector |
| 16 | LASHIEC_APPEARANCE | 128 Lashiec |
| 17 | DARK_FORCE_2 | 131 DarkForce2 |
| 18 | AERO_PRISM | 132 DarkForce3 |
| 22 | DE_VARS | 120 DeVars |
| 23 | SA_LEWS | 121 SaLews |
| 24 | Event_AngerTowerAlys (ps4.asm:150697): battle $18 | 126 ? |
| 25 | REFAZE | 127 ReFaze |
| 26 | PROFOUND_DARKNESS, loc_2F8D8 (ps4.asm:61916): Profound Darkness's second form, loc_2EDD8 (ps4.asm:61119): Profound Darkness's third form | 133 ProfoundDarkness1, 134 ProfoundDarkness2, 135 ProfoundDarkness3 |

Abilities the scope can meet

| ability | effect | class | ledger | carriers | foot maps | vehicle maps | events |
|---|---|---|---|---|---|---|---|
| `$02` FLAME BOLT | `$01` | damage | implemented | 0 Helex, 5 ForcedFly | 1 | 1 | - |
| `$04` LASRCANNON | `$01` | damage | implemented | 4 ProtectBit, 8 Sweeper, 52 Debugger | 8 | 1 | - |
| `$05` LIGHTNING | `$01` | damage | implemented | 8 Sweeper | 4 | 0 | - |
| `$08` SPIRAL BLD | `$01` | damage | implemented | 15 Fanbite | 1 | 0 | - |
| `$09` MOTRCANNON | `$01` | damage | implemented | 18 Servant | 4 | 0 | - |
| `$0A` TWIN CLAW | `$01` | damage | implemented | 20 Silvalt (conditional:2), 21 Goldine (conditional:2) | 6 | 0 | - |
| `$0B` STASISBALL | `$1C` | status/stat effect | implemented | 21 Goldine, 26 LifeDeletr | 4 | 0 | - |
| `$0C` COMBINE | `$1F` | scripted/custom | implemented | 23 ArthroPod (conditional:3) | 8 | 0 | - |
| `$0D` COMBINE | `$1F` | scripted/custom | implemented | 25 Wiredine (conditional:4) | 8 | 0 | - |
| `$0E` MICROMISSL | `$01` | damage | implemented | 26 LifeDeletr, 28 DragerDuel, 29 JurafaDuel | 8 | 0 | - |
| `$0F` FLAMLAUNCH | `$01` | damage | implemented | 29 JurafaDuel | 4 | 0 | - |
| `$11` POISON | `$1B` | status/stat effect | implemented | 32 Caterpillr | 1 | 0 | - |
| `$13` CELL SPLIT | `$01` | damage | implemented | 37 SnowSlug, 38 FractOoze | 8 | 0 | - |
| `$16` FLASH | `$21` | status/stat effect | implemented | 43 StarDrone | 4 | 0 | - |
| `$17` WAITING | `$22` | no effect | implemented | 46 VopalSphre | 4 | 0 | - |
| `$1A` CYANICBOMB | `$02` | status/stat effect | implemented | 46 VopalSphre (conditional:7) | 4 | 0 | - |
| `$1B` FISSION | `$25` | scripted/custom | implemented | 38 FractOoze (conditional:2) | 7 | 0 | - |
| `$1C` FLARE SHOT | `$01` | damage | implemented | 49 Browren486 | 4 | 0 | - |
| `$1D` BARRIER | `$0B` | status/stat effect | implemented | 49 Browren486 | 4 | 0 | - |
| `$1E` SPARK | `$02` | status/stat effect | implemented | 49 Browren486 (conditional:8) | 4 | 0 | - |
| `$1F` DBL SLASH | `$01` | damage | implemented | 52 Debugger, 53 Dominator, 145 RedMole | 5 | 0 | - |
| `$20` PHONONMASR | `$01` | damage | implemented | 53 Dominator | 2 | 0 | - |
| `$21` FIREBREATH | `$01` | damage | implemented | 58 FlameNewt, 59 StoneHeads, 84 BladeRight, 117 GyLaguiah, 133 ProfoundDarkness1 | 40 | 0 | 11, 26 |
| `$22` RAY BREATH | `$01` | damage | implemented | 60 CrminHeads, 61 BlindHeads, 118 LwAddmer, 133 ProfoundDarkness1 | 16 | 1 | 26 |
| `$23` SUPERSONIC | `$01` | damage | implemented | 61 BlindHeads (conditional:9), 69 BiterFly (conditional:8), 142 Skytiara | 6 | 0 | - |
| `$24` POISONMIST | `$1B` | status/stat effect | implemented | 57 Mistralgec, 58 FlameNewt | 8 | 0 | - |
| `$26` SHIFT | `$09` | status/stat effect | implemented | 64 DarkMaraud (conditional:2), 65 DeathBearr (conditional:9), 66 ChaosBrngr (conditional:2), 72 BloodSaber (conditional:9), 116 Radhin | 22 | 0 | - |
| `$27` SANER | `$0C` | status/stat effect | implemented | 65 DeathBearr (conditional:2), 116 Radhin | 16 | 0 | - |
| `$28` DORAN | `$06` | status/stat effect | implemented | 64 DarkMaraud (conditional:11) | 5 | 0 | - |
| `$29` SEALS | `$08` | status/stat effect | implemented | 65 DeathBearr (conditional:10), 66 ChaosBrngr (conditional:10), 116 Radhin | 22 | 0 | - |
| `$2A` RIMIT | `$07` | status/stat effect | implemented | 66 ChaosBrngr (conditional:11) | 6 | 0 | - |
| `$2B` NEEDLE | `$01` | damage | implemented | 68 Rajago, 69 BiterFly | 1 | 0 | - |
| `$2C` AIRSLASH | `$01` | damage | implemented | 71 FrostSaber (conditional:2), 72 BloodSaber (conditional:2) | 35 | 0 | - |
| `$2D` DEBAN | `$0A` | status/stat effect | implemented | 71 FrostSaber (conditional:7), 72 BloodSaber (conditional:7), 116 Radhin | 38 | 0 | - |
| `$2E` GIWAT | `$01` | damage | implemented | 71 FrostSaber, 91 HewGilla, 101 DarkWitch, 122 DElmLars, 123 XeAThoul | 38 | 1 | 12, 14 |
| `$2F` VOL | `$02` | status/stat effect | implemented | 72 BloodSaber, 88 SoldrFiend | 14 | 0 | - |
| `$30` DISTORTION | `$01` | damage | implemented | 73 DimensWorm (conditional:12), 74 OuterBeast (conditional:12), 134 ProfoundDarkness2 | 30 | 0 | 26 |
| `$31` GRA | `$01` | damage | implemented | 73 DimensWorm, 74 OuterBeast | 30 | 0 | - |
| `$32` GIGRA | `$01` | damage | implemented | 74 OuterBeast | 5 | 0 | - |
| `$33` ACIDBREATH | `$01` | damage | implemented | 86 HakenLeft | 13 | 0 | - |
| `$35` GIZAN | `$01` | damage | implemented | 101 DarkWitch, 122 DElmLars, 123 XeAThoul, 124 LeFawGan, 125 GiLeFarg | 21 | 0 | 12, 14 |
| `$36` STRNGLIGHT | `$07` | status/stat effect | implemented | 79 Shrieker | 7 | 0 | - |
| `$37` SAND STORM | `$01` | damage | implemented | 81 DesrtLeach | 0 | 1 | - |
| `$38` EARTHQUAKE | `$01` | damage | implemented | 80 SandWorm | 1 | 0 | - |
| `$39` MAELSTROM | `$01` | damage | implemented | 82 Leviathan | 0 | 1 | - |
| `$3A` COMBINE | `$1F` | unknown | implemented | 84 BladeRight (conditional:13) | 31 | 0 | - |
| `$3B` COMBINE | `$1F` | unknown | implemented | 86 HakenLeft (conditional:14) | 13 | 0 | - |
| `$3C` BLADESHINE | `$01` | damage | implemented | 87 TwinArms, 88 SoldrFiend | 18 | 0 | - |
| `$3D` HAKEN BOLT | `$01` | damage | implemented | 87 TwinArms, 88 SoldrFiend | 18 | 0 | - |
| `$3E` GIRES | `$12` | status/stat effect | implemented | 88 SoldrFiend (conditional:2) | 8 | 0 | - |
| `$3F` FLODBREATH | `$01` | damage | implemented | 90 Depcen, 91 HewGilla, 92 Elmelew | 1 | 1 | - |
| `$40` WAT | `$01` | damage | implemented | 91 HewGilla, 92 Elmelew, 99 TechUser | 1 | 1 | - |
| `$41` NOTHING | `$1F` | scripted/custom | implemented | 94 InfantWorm (conditional:9) | 1 | 0 | - |
| `$42` RAY-SPEAR | `$01` | damage | implemented | 97 KingSaber, 98 DarkRider | 11 | 0 | - |
| `$43` THROWLANCR | `$01` | damage | implemented | 98 DarkRider | 6 | 0 | - |
| `$44` FOI | `$01` | damage | implemented | 99 TechUser | 1 | 0 | - |
| `$45` RES | `$12` | status/stat effect | implemented | 99 TechUser (conditional:15) | 1 | 0 | - |
| `$48` GIFOI | `$01` | damage | implemented | 101 DarkWitch, 124 LeFawGan | 10 | 0 | - |
| `$49` GISAR | `$12` | status/stat effect | implemented | 101 DarkWitch (conditional:15), 116 Radhin (conditional:15) | 11 | 0 | - |
| `$4B` SHADOWBIND | `$06` | status/stat effect | implemented | 111 ChaosSorcr, 132 DarkForce3 | 10 | 0 | 18 |
| `$4C` EVIL EYE | `$07` | status/stat effect | implemented | 107 Spector, 131 DarkForce2, 135 ProfoundDarkness3 | 25 | 0 | 15, 17, 26 |
| `$4D` CORRSION | `$01` | damage | implemented | 107 Spector, 111 ChaosSorcr, 112 Illusionst, 113 ImagioMage, 132 DarkForce3 | 47 | 0 | 15, 18 |
| `$4E` DTHSPELL | `$02` | status/stat effect | implemented | 107 Spector, 112 Illusionst, 113 ImagioMage | 43 | 0 | 15 |
| `$4F` HEWN | `$01` | damage | implemented | 108 Phantom, 111 ChaosSorcr, 112 Illusionst, 113 ImagioMage, 121 SaLews | 30 | 0 | 23 |
| `$50` BAD SMELL | `$07` | status/stat effect | implemented | 110 Ghoul | 4 | 0 | - |
| `$51` MINDBLST | `$07` | status/stat effect | implemented | 112 Illusionst, 113 ImagioMage, 132 DarkForce3 | 18 | 0 | 18 |
| `$52` TANDLE | `$01` | damage | implemented | 111 ChaosSorcr, 112 Illusionst, 113 ImagioMage, 121 SaLews | 28 | 0 | 23 |
| `$55` LEGEON | `$01` | damage | implemented | 113 ImagioMage, 121 SaLews (conditional:7) | 8 | 0 | 23 |
| `$56` FORCEFLASH | `$01` | damage | implemented | 116 Radhin | 5 | 0 | - |
| `$58` LGHTBREATH | `$01` | damage | implemented | 119 CulaBellr | 1 | 0 | - |
| `$59` DISRUPTARM | `$01` | damage | implemented | 120 DeVars (conditional:8) | 0 | 0 | 22 |
| `$5A` FLAELI | `$01` | damage | implemented | 111 ChaosSorcr, 112 Illusionst, 113 ImagioMage, 121 SaLews, 125 GiLeFarg | 28 | 0 | 23 |
| `$5C` THNDRBLAST | `$01` | damage | implemented | 123 XeAThoul (conditional:18) | 0 | 0 | 14 |
| `$5D` TANDIL | `$01` | damage | implemented | 124 LeFawGan, 125 GiLeFarg | 10 | 0 | - |
| `$5E` MEGID | `$01` | damage | implemented | 127 ReFaze, 135 ProfoundDarkness3 | 0 | 0 | 25, 26 |
| `$5F` THNDHALBRT | `$01` | damage | implemented | 128 Lashiec | 0 | 0 | 16 |
| `$60` POSESSION | `$07` | status/stat effect | implemented | 128 Lashiec | 0 | 0 | 16 |
| `$61` ANOTHRGATE | `$01` | damage | implemented | 128 Lashiec, 134 ProfoundDarkness2 | 0 | 0 | 16, 26 |
| `$62` REINFORCE | `$2B` | status/stat effect | implemented | 128 Lashiec (conditional:17) | 0 | 0 | 16 |
| `$64` SHDWBREATH | `$01` | damage | implemented | 131 DarkForce2, 133 ProfoundDarkness1 | 0 | 0 | 17, 26 |
| `$65` LIGHTSHOWR | `$01` | damage | implemented | 131 DarkForce2, 134 ProfoundDarkness2 | 0 | 0 | 17, 26 |
| `$67` NOTHING | `$1F` | scripted/custom | implemented | 133 ProfoundDarkness1 (conditional:2) | 0 | 0 | 26 |
| `$68` NOTHING | `$1F` | scripted/custom | implemented | 134 ProfoundDarkness2 (conditional:2) | 0 | 0 | 26 |
| `$69` CANCELING | `$27` | status/stat effect | implemented | 135 ProfoundDarkness3 | 0 | 0 | 26 |
| `$6A` WIND STORM | `$01` | damage | implemented | 143 Owltalon | 0 | 1 | - |
| `$6D` ROUND EYES | `$01` | damage | implemented | 147 Rappy | 1 | 0 | - |
| `$6E` LOVEL EYES | `$01` | damage | implemented | 148 BlueRappy | 1 | 0 | - |

<!-- route_abilities:end -->
