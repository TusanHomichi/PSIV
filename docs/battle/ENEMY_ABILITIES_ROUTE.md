# Enemy abilities on the Zelan-to-Kuran route

The route evidence behind [`ENEMY_ABILITIES.md`](ENEMY_ABILITIES.md): section 1 is
lane A3's damage routes (the route census, the chain corrections and the captures),
section 2 is lane A4's status and stat abilities. Split out of the inventory when
the two lanes merged (M34), because the inventory is a table and these are
evidence. A bare `§N` or "section N" in the text below that names no file is a
section of the inventory, [`ENEMY_ABILITIES.md`](ENEMY_ABILITIES.md); the counts,
classes and per-row status live there.

## 1. A3: Zelan through early Dezolis and Kuran (2026-10-04)

### Reproducible scope

The census includes **regular and conditional** ability lists, every position
cell's encounter group, vehicle groups, and literal scene-started event battles.
The map families cover Zelan/Zelan-space, Raja's temple, Ryuon, Tyler, the Hangar,
Dezolis/DezoSpaceport and every Kuran layer through F3. Scene records 42–48 add
sabotage battle 8 and DarkForce1 battle 9; scenes 43–46/48 add no other battle.
The static-record boundary is checked so a later scene cannot leak into scope.

```sh
python3 -m oracle.sweep.route_abilities --data-dir generated \
  --map-pattern '^(Zelan|Kuran|Dezolis$|DezoSpaceport|Hangar$|RajaTemple$|Ryuon|Tyler)' \
  --scene-doc docs/scenes/42_SpaceshipSabotage.md \
  --scene-doc docs/scenes/43_CrashLanding.md \
  --scene-doc docs/scenes/44_Landale.md \
  --scene-doc docs/scenes/45_KuranArrival.md \
  --scene-doc docs/scenes/46_NearDarkForce1.md \
  --scene-doc docs/scenes/47_DarkForce1.md \
  --scene-doc docs/scenes/48_DarkForce1Defeated.md \
  --out build/a3-evidence/route-set.json
```

Result: **31 maps, groups 11/12/13/44/45, 61 random formations, 21 random
enemy IDs, event enemy IDs 130/138, 24 ability IDs**. Group 13 is the Dezolis
vehicle table; omitting it or the conditional lists fails the synthetic census
controls (`tests/test_route_abilities.py`). The ignored JSON records every map,
formation, carrier, scene and input hash. It is an evidence output, not source data.

| class | derived ability IDs | scope carriers |
|---|---|---|
| damage | `$02` FLAME BOLT | 0 Helex |
| damage | `$04` LASRCANNON | 4 ProtectBit |
| damage | `$13` CELL SPLIT | 37 SnowSlug |
| damage, conditional | `$15` CHARGCNNON | 40 CRayTube |
| damage, conditional | `$18` EXPLOSION | 44 FloatMine, 50 FloatMine2 |
| damage | `$19` DETONATION | 45 CommndBall |
| damage | `$1C` FLARE SHOT | 48 Siren386, event 130 DarkForce1 |
| damage | `$1F` DBL SLASH | 145 RedMole |
| damage | `$20` PHONONMASR | event 130 DarkForce1 |
| damage | `$22` RAY BREATH | 118 LwAddmer |
| damage | `$23` SUPERSONIC | 142 Skytiara; conditional 69 BiterFly |
| damage | `$2B` NEEDLE | 68 Rajago, 69 BiterFly |
| damage | `$33` ACIDBREATH | 85 Piercer |
| damage | `$4F` HEWN | event 138 ChaosSorcr2 |
| damage | `$5A` FLAELI | event 138 ChaosSorcr2 |
| damage after intro | `$63` BURSTROC | event 130 DarkForce1 |
| damage | `$6A` WIND STORM | 143 Owltalon |
| status/stat, A4 (`enemy_effect`) | `$24` POISONMIST, `$25` SLEEP GAS, `$4B` SHADOWBIND; `$1D` BARRIER stays deferred | 57 Mistralgec, 63 GerotLux, event 138 ChaosSorcr2; 48 Siren386 |
| no-effect (A3 repairs SatMinion WAITING) | `$07` FISSION2, `$17` WAITING | 50 FloatMine2; 42 SatMinion/44 FloatMine/50 FloatMine2 |
| summon, outside damage/status lanes | `$14` WARNING | conditional 45 CommndBall; implemented by A4 as the refill (`resolve_fission`) |

### Chain corrections and implementation boundary

Zelan sabotage formation 8 seats **138 ChaosSorcr2 in enemy slot 1**, fighter
**6**. Retail `EnemyAttackOffs[138]` and `[111]` both point to `$00E762`;
`psiv_tools.battle_animations`' retail guard and pointer tables were used to
verify this (`build/a3-evidence/retail-pointers.json`, ROM hash recorded there).
All six FLAELI carriers have core coverage, and pack-backed tests load their
stats and skill through `psiv-data` and the existing runtime bridge. See
[`ENEMY_DAMAGE_ROUTES.md`](ENEMY_DAMAGE_ROUTES.md) §3 for the cited chains.

A3 adds 44 registry pairs, bringing `DAMAGE_SKILL_ROUTES` to **93**. The
per-carrier constructed controls use deliberately different mental, strength,
attack and resistance values, flipped target nibbles, dead/empty party slots,
and real-effect/unproven-pair negative controls. No retail stat or ability
record is copied into new committed fixtures.

- **DETONATION is one all-party request, then neighbor removal.** The old
  second hit at line 33700 belonged to EXPLOSION, outside DETONATION's reachable
  `$1C4` chain. `loc_18344` (`ps4.asm:33550`) branches back to `loc_182C8`,
  not forward to `loc_18478`. The actor survives; next/previous enemy objects
  clear without reward calls (`loc_17D4A`, lines 33154–33177).
- **EXPLOSION is one stored-target request, then actor removal**
  (`loc_18574`, lines 33694–33724). Cached HP/status remain unchanged;
  deactivation prevents further targeting/actions and earns no reward pool.
  The core uses its existing enemy-removal presentation cue (`Died`), not a
  fabricated HP-zero transition. State evidence does not certify that cue's
  animation or narration against retail.
- **CHARGCNNON's two beams request the same five routines in one frame**
  (`loc_18FA2`, lines 34439–34453): one 16-draw calculation per living member.
  Effect indices `$20`, `$23` and `$24` are also `AbilityEffect_None`
  (`AbilityEffectsOffs`, lines 9076/9079/9080); the effect gate now admits these
  proven no-op handlers, retaining refusal of real effects.
- **CHARGCNNON's accompanying WAITING turns clear their own ability byte**
  before the snapshot (`EnemyAttack_ArmDrone` / `loc_10468`, lines 22807–22813).
  The no-effect resolver now covers its 41/42 carriers for WAITING only;
  FISSION2 is a negative control. The extractor retains the observed zero
  and marks the complete clear-and-return signature. The comparator remains
  strict about damage, other ids and draws; see
  [the replay schema reading](../oracle/BATTLE_ORACLE_REPLAY.md#same-frame-ability-clearing-a3-2026-10-04).
- **BURSTROC is ordinary all-party damage after the intro**, via `$82C` /
  `loc_31AB6` (`ps4.asm:64247`), phase 3 to `loc_24A9E` (line 64263).
  DarkForce1 clears `$FFFFEE87` and its rolled ability once before loading
  animation-only `$818` (`EnemyAttack_DarkForce1`, lines 20031–20035;
  `loc_32344`, lines 64872–65000). Its other two arms then enter their normal
  damage routes. ProfoundDarkness1's different intro remains deferred (#62).

### Cartridge evidence

Captures use `python3 -m oracle.force`, `--require-ability`, durable HP, a
bounded round cap, and repeated byte-identical logs/traces. Only an ability
**actually used in an extracted round** counts. `oracle.sweep.capture_route`
rejects out-of-lane turns; the round-1 FLAELI miss is retained as a negative
capture. A forced preview's measured load frame now calibrates durable HP
patches; the initial mis-timed Helex captures remain retained as failures.

The 16 captures are committed fixtures under
`rust/psiv-core/src/battle/replay_fixtures/arc_zelan_damage/`, like every other
lane's (integration, 2026-10-04: lane A3 first kept them in an ignored local
directory behind a second loader, so a checkout without that directory
replayed fewer fixtures silently; the loader and `PSIV_REPLAY_FIXTURES` are
gone). Their enemy and skill records are in `motavia_pack.json`, regenerated by
`python3 -m oracle.sweep.replay_pack`.

Fixture paths below are relative to `rust/psiv-core/src/battle/replay_fixtures/arc_zelan_damage/`.
Raw logs/traces, host receipts and failed delays are retained under
`build/a3-evidence/captures/`; `ability-evidence.json` records the checked hashes.

| ability | fixture; formation; delay; captured carrier | round observed | generic replay |
|---|---|---|---|
| `$02` FLAME BOLT | `flame-bolt-fixed.json`; 94; 0; 0,0 | 1 | exact |
| `$04` LASRCANNON | `lasrcannon.json`; 127; 0; 4,4 | 1 | exact |
| `$13` CELL SPLIT | `cell-split.json`; 115; 2; 37 | 1 | exact |
| `$15` CHARGCNNON | `chargecannon.json`; 291; 0; 42,40,42 | 1 | exact |
| `$18` EXPLOSION | `explosion.json`; 264; 0; 44,44 | 2 | exact |
| `$19` DETONATION | `detonation.json`; 293; 0; 44,45,44 | 1 | exact |
| `$1C` FLARE SHOT | `flare-shot-restart.json`; 294; 0; 48,48 | 1 | exact |
| `$1F` DBL SLASH | `double-slash.json`; 124; 0; 145,145 | 1 | exact |
| `$20` PHONONMASR | `phononmaser.json`; 431; 0; 53 | 1 | exact |
| `$22` RAY BREATH | `ray-breath.json`; 130; 2; 118 | 1 | exact |
| `$23` SUPERSONIC | `supersonic.json`; 304; 2; 142 | 1 | exact |
| `$2B` NEEDLE | `needle.json`; 100; 5; 68 | 1 | exact |
| `$33` ACIDBREATH | `acid-breath.json`; 297; 0; 85,85 | 1 | exact |
| `$4F` HEWN | `hewn.json`; 368; 2; 111 | 1 | exact |
| `$5A` FLAELI | `flaeli.json`; 368; 1; 111 | 1 | exact |
| `$6A` WIND STORM | `wind-storm.json`; 133; 2; 143,143 | 1 | exact |
| `$63` BURSTROC | event battle 9, carrier 130; cited `$82C` chain and per-carrier/intro core tests | not capturable by `oracle.force` (#72) | event battle: chain + core; campaign proof remains open |

The generic test replayed **128 fixtures (112 committed + 16 local)** with
**zero divergence-manifest entries**. EXPLOSION's final fixture records victory
with positive cached enemy HP and excludes the later item-drop draw, using the
cartridge's logged victory declaration. Its original force report's HP-only
`withdrawal` label and the pre-repair extracts are retained, not rewritten.

The replay negative control changed only a copied FLAELI target's damage from
113 to 114: the generic test exited **101**, reporting the intended value
divergence at **f24988, round 1**. Original capture and manifest were untouched.

FLAELI and HEWN use random carrier 111's exact shared chain as evidence for
event carrier 138. PHONONMASR and FLARE SHOT have different random/event chains;
DarkForce1's event arms have their own cited registry rows and core controls.
Only the connected route result below can establish event-battle progress.

### Release campaign result

The prescribed route ran from New Game with a fresh, explicit save directory:

```sh
CARGO_BUILD_JOBS=2 cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
PSIV_SAVE_DIR=build/a3-route ./rust/target/release/psiv-campaign run \
  rust/psiv-campaign/routes/main.json --save-dir build/a3-route \
  --tape build/a3-route/run.tape --report build/a3-route/report.json
```

Build **exit 0**; run **exit 0**, all **36 chapters** completed in
**2,644,725 frames**, digest **`e6200509953185db`**. `zelan-sabotage` completed
its one scripted battle and the crash landing in 13,919 frames. There was no
status halt or balance loss. Final chapter `dezolis-tyler-grave` reaches the
Hangar; a read-only `psiv-campaign inspect` of its save confirms map `$15F`,
cell `(66,21)`, Chaz L27, Rika L29, Rune L27, Wren L20 and Raja L25.
Report/tape/snapshots are retained under `build/a3-route/`; command receipts,
binary/input/output hashes and the final-save reading are in
`build/a3-evidence/`. The main route's SHA-256 remains
`1ce3457606cb3a0df63780dfa5c4bb99f14d40addcd3ee3924d01308ae8318a1`.

This closes the connected sabotage-battle gate for carrier 138. Route lane C6
(2026-10-04, [`RUNNER_LOG_KURAN.md`](../campaign/RUNNER_LOG_KURAN.md)) then
extended the main route through Landale and Kuran and won event battle 9.
**BURSTROC's cartridge proof remains open.** The route fight is a connected
campaign result with FLARE SHOT, PHONONMASR and BURSTROC striking the party, but
it is not a cartridge comparison: there is no forced capture of event battle 9
(#72), and the animation-only first act is not checked against
`EnemyAttack_DarkForce1`. DarkForce1's FLARE SHOT/PHONONMASR event chains share
this gap despite their different random carriers' exact replays.

## 2. The Zelan-to-Kuran route (lane A4)

The campaign route crosses Dezolis to Kuran and Dark Force 1
([`RUNNER_LOG_ZELAN.md`](../campaign/RUNNER_LOG_ZELAN.md),
[`RUNNER_LOG_DEZOLIS.md`](../campaign/RUNNER_LOG_DEZOLIS.md)). The set of abilities it
will meet is **derived**, not listed, by

```
python3 -m oracle.sweep.route_abilities            # markdown; --json for the data
```

(`oracle/sweep/route_abilities.py`, the one census tool: lane A3's explicit
map-pattern scope in section 1 above is the same derivation with another scope reader,
and both scopes give identical abilities, carriers and classes on this route -
M34 compared them. An ability with no row in §2 or §3 is an error.) By default it
reads the maps the route file names from
the chapter `zelan-wren-canceller` to the last one (`rust/psiv-campaign/routes/main.json`),
the maps the scenes `SPACESHIP_SABOTAGE` to `DARK_FORCE_1_DEFEATED` load
(`rust/psiv-core/src/scenes/post_zio_cutscenes.rs`: docs/scenes 42-48), and Kuran's
eight encounter maps (the stretch ends there); resolves each through
`generated/encounters.json`, `formation_indexes.json` and `formations.json` to the
enemies its groups can draw; adds the event battles the scenes start
(`StartBattle` indexes to `boss_formations[].event_battle_index`); and lists every
regular and conditional ability of those enemies with the class this file's §2 and
§3 give it. A vehicle-only group (Dezolis group 13) is listed apart from the groups on
foot because the Ice Digger arrives with scene 48, after Kuran.

**Where it disagrees with section 4's list.** §4 names POISONMIST, STRNGLIGHT and
SLEEP GAS "on Dezolis and Hangar maps"; the derivation finds POISONMIST (Mistralgec,
Dezolis group 11) and SLEEP GAS (GerotLux, Hangar group 44) and **not** STRNGLIGHT: its
only carrier, 79 Shrieker, lives on the Island Cave maps. It also finds what §4 does
not list for these maps: the conditional WARNING (Kuran, 45 CommndBall) and BARRIER
(Kuran, 48 Siren386 by its arm-`$08` condition, which §2's carrier column leaves out),
the Chaos Sorcerer's SHADOWBIND, and `$63` BURSTROC on Dark Force 1 (a damage ability,
see its row).

| map | symbol | named by | groups on foot | vehicle groups |
|---|---|---|---|---|
| `001` | Dezolis | chapter dezolis-outside-raja-temple; chapter dezolis-tyler-grave; scene CRASH_LANDING; scene LANDALE | 11, 12 | 13 |
| `120` | Tyler | chapter dezolis-tyler-grave | - | - |
| `144` | Ryuon | chapter dezolis-gyuna; chapter dezolis-tyler-grave | - | - |
| `14A` | RyuonPub | chapter dezolis-gyuna | - | - |
| `14C` | RajaTemple | chapter zelan-sabotage; chapter dezolis-first-control; scene CRASH_LANDING | - | - |
| `15F` | Hangar | chapter dezolis-tyler-grave | 44 | - |
| `18C` | ZelanSpace | scene SPACESHIP_SABOTAGE | - | - |
| `18D` | Zelan | chapter zelan-wren-canceller; chapter zelan-sabotage | - | - |
| `18E` | Zelan_F1 | chapter zelan-wren-canceller; scene DARK_FORCE_1_DEFEATED | - | - |
| `190` | Kuran | Kuran interior (scene 45-47's map) | 45 | - |
| `191` | Kuran_F1 | Kuran interior (scene 45-47's map) | 45 | - |
| `192` | Kuran_F2 | Kuran interior (scene 45-47's map) | 45 | - |
| `193` | Kuran_F1_Part2 | Kuran interior (scene 45-47's map) | 45 | - |
| `194` | Kuran_F1_Part3 | Kuran interior (scene 45-47's map) | 45 | - |
| `195` | Kuran_F1_Part5 | Kuran interior (scene 45-47's map) | 45 | - |
| `196` | Kuran_F2_Part2 | Kuran interior (scene 45-47's map) | 45 | - |
| `197` | Kuran_F1_Part4 | Kuran interior (scene 45-47's map) | 45 | - |

| index | scenes | enemies |
|---|---|---|
| 8 | SPACESHIP_SABOTAGE | 138 ChaosSorcr2 |
| 9 | DARK_FORCE_1 | 130 DarkForce1 |

| ability | effect | class | ledger | carriers | foot maps | vehicle maps | events |
|---|---|---|---|---|---|---|---|
| `$02` FLAME BOLT | `$01` | — | implemented | 0 Helex | 2 | 0 | - |
| `$04` LASRCANNON | `$01` | damage | unsupported | 4 ProtectBit | 0 | 1 | - |
| `$07` FISSION | `$1E` | — | implemented | 50 FloatMine2 | 8 | 0 | - |
| `$13` CELL SPLIT | `$01` | damage | unsupported | 37 SnowSlug | 2 | 0 | - |
| `$14` WARNING | `$1E` | unknown | implemented | 45 CommndBall (conditional:1) | 8 | 0 | - |
| `$15` CHARGCNNON | `$20` | damage | unsupported | 40 CRayTube (conditional:5) | 8 | 0 | - |
| `$17` WAITING | `$22` | — | implemented | 42 SatMinion (conditional:5), 44 FloatMine, 50 FloatMine2 | 8 | 0 | - |
| `$18` EXPLOSION | `$23` | damage | unsupported | 44 FloatMine (conditional:7), 50 FloatMine2 (conditional:7) | 8 | 0 | - |
| `$19` DETONATION | `$24` | damage | unsupported | 45 CommndBall | 8 | 0 | - |
| `$1C` FLARE SHOT | `$01` | damage | unsupported | 48 Siren386, 130 DarkForce1 | 8 | 0 | 9 |
| `$1D` BARRIER | `$0B` | status/stat effect | unsupported | 48 Siren386 (conditional:8) | 8 | 0 | - |
| `$1F` DBL SLASH | `$01` | damage | unsupported | 145 RedMole | 1 | 0 | - |
| `$20` PHONONMASR | `$01` | damage | unsupported | 130 DarkForce1 | 0 | 0 | 9 |
| `$22` RAY BREATH | `$01` | damage | unsupported | 118 LwAddmer | 0 | 1 | - |
| `$23` SUPERSONIC | `$01` | damage | unsupported | 69 BiterFly (conditional:8), 142 Skytiara | 1 | 0 | - |
| `$24` POISONMIST | `$1B` | status/stat effect | implemented | 57 Mistralgec | 1 | 0 | - |
| `$25` SLEEP GAS | `$07` | status/stat effect | implemented | 63 GerotLux | 1 | 0 | - |
| `$2B` NEEDLE | `$01` | damage | unsupported | 68 Rajago, 69 BiterFly | 1 | 0 | - |
| `$33` ACIDBREATH | `$01` | — | implemented | 85 Piercer | 8 | 0 | - |
| `$4B` SHADOWBIND | `$06` | status/stat effect | partial | 138 ChaosSorcr2 | 0 | 0 | 8 |
| `$4F` HEWN | `$01` | damage | unsupported | 138 ChaosSorcr2 | 0 | 0 | 8 |
| `$5A` FLAELI | `$01` | damage | unsupported | 138 ChaosSorcr2 | 0 | 0 | 8 |
| `$63` BURSTROC | `$01` | damage | unsupported | 130 DarkForce1 | 0 | 0 | 9 |
| `$6A` WIND STORM | `$01` | damage | unsupported | 143 Owltalon | 0 | 1 | - |


Who owns what (the `class` column above is §2's object-chain rule): the `damage`
rows are the damage lane's (FLAELI first); the rest of the table is this lane's, with
the status in the last column of §2/§3 - `$14`, `$24`, `$25` and `$4B` implemented,
`$17`'s second carrier (42 SatMinion) read, `$1D` BARRIER deferred
([ENEMY_EFFECT_ABILITIES.md](ENEMY_EFFECT_ABILITIES.md) section 7), and Dark Force 1's
first action - not an ability id at all - modelled
([`battle/scripted_flag.rs`](../../rust/psiv-core/src/battle/scripted_flag.rs)): the
latch's object `$818` replaces the rolled ability and requests nothing, replayed
from the captured event battle 9. The captures, with the round each ability was seen
in, are [`BATTLE_ORACLE_ZELAN.md`](../oracle/BATTLE_ORACLE_ZELAN.md).
