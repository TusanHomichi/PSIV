# Sound integration

Implemented 2026-08-16. This closes the runtime half of the sound slice; the
pack remains generated and gitignored under `runtime-pack/`.

## Runtime path

`psiv-data` loads `runtime-pack/sound/` as typed records. `psiv-godot` resolves
the records into `psiv_sound::SoundBank`, preserving the complete raw record
and its record-relative track start offset. `psiv-sound` then runs the PSIV
interpreter against:

- the extracted FM voices and exact volume-envelope byte streams;
- FM, PSG, and shared-subroutine track bytes;
- the extracted DAC bank/sample bytes;
- the Nuked-OPN2 YM2612 and SN76489 PSG cores.

DAC playback is live. `EE` sample commands select the extracted sample bank;
the driver renders PCM through YM `$2A`, enables DAC mode through `$2B`, and
applies the extracted direction, loop, pan, volume, and pitch controls. The
register log includes the DAC control/data writes, not just synthesized FM/PSG
events.

## Binding provenance

No sound binding was invented and no `psiv_tools` change was needed. The map
pack already emits `music.id`, `music.symbol`, and `music.changes_music` from
the ROM-derived map record. Piata is map `$013`; its pack binding is music
`$84`, `MotabiaTown`.

The music ID table is the retail `MusicPtrs` table at `$D1C40`, with IDs
`$81..$B4`. The corresponding symbols and dispatch rules are documented in
`docs/SOUND_SCOUT.md` and sourced from:

- `reference/ps4disasm/sound/ps4.sound_driver.asm` for pointer records and
  `PlaySoundID` dispatch;
- `reference/ps4disasm/ps4.constants.asm` for named music/SFX IDs;
- `reference/ps4disasm/ps4.asm` for map/event/battle call sites.

## Trigger coverage

| Retail route | Runtime/presentation route | Status |
|---|---|---|
| Map arrival `music.id`; zero means keep current | `Field::ready` and `MapChanged` call `play_map_music` | Wired |
| Normal field battle `$8F` `MeetThemHeadOn` | `EncounterRolled`; debug random battle uses the same ID | Wired |
| Event battle `EventBattleMusicData` | `SceneBattleStarted`, all 27 ROM table entries preserved | Wired |
| Victory `$8B` `Winners` | `service_battle_finish` on `Outcome::Victory` | Wired |
| Cursor `$F2` `MovingCursor` | Modal Godot directional actions | Wired for current field/battle/dialogue/shop/camp UI |
| Selection `$F3` `Selection` | Modal Godot accept/cancel actions | Wired for current field/battle/dialogue/shop/camp UI |
| Player physical attack | `BattleEvent::Attacked` -> `BattleSoundEvent` at the event index | Wired for all retail weapon classes |
| Miss effect `$B8` `AttackMiss` | `BattleEvent::Resolved { verdict: Miss }` | Wired |
| Enemy death `$B9` `EnemyKilled` | `BattleEvent::Died` | Wired |
| Enemy physical attack | `BattleEvent::Attacked` -> `BattleAnimationEvent`/exact pack SFX | Wired for all 153 retail enemy records; no generic `$BA` fallback |
| Technique/skill/item animation SFX | No Tier-1 core command/event carries the selected ability | Deferred: no honest runtime moment yet |
| Enemy ability/effect SFX | `UnsupportedAbility` records the roll but does not execute its animation | Deferred: no ability animation event surface yet |
| Sequence-internal `EB` | `commands.rs` queues the payload through the same driver priority path | Wired; register-log covered |
| Vehicle battle `$96` `CyberneticCarnival` | Mounted `EncounterRolled` and debug vehicle-battle entry dispatch `$96` through `Field::play_sound` | Wired 2026-08-16 |

Battle presentation now receives a `BattleTimeline`: the core event vector is
unchanged, and `BattleSoundEvent { event_index, id }` plus
`BattleAnimationEvent` are ordered sidecars. `psiv-runtime` captures weapon
state and the selected enemy record before round mutation, maps the retail
weapon-index table, and attaches requests to `Attacked`, miss, and death
events. `psiv-godot/src/battle/sfx.rs` pairs both sidecars with the screen
queue; `Field::drive_battle_if_active` drains sound IDs into the existing live
`Field::play_sound` path. The driver remains responsible for mixing SFX with
the active theme. The enemy records and their object/frame provenance live in
`battle/enemy_animations.json`; the scout and its explicit deferred surface
are recorded in `docs/BATTLE_ANIMATIONS.md`.

The current UI hooks intentionally cover input-owned `$F2`/`$F3` writes. They
do not claim that every internal menu write is a distinct new event: the
direct-write census below records those writes, while run/flee and level-up
confirmation have no additional retail SFX beyond the selection path.

`PSIV_DEBUG_TRACK=<id>` starts any extracted record through the Godot output
path. `PSIV_DEBUG_BATTLE=0x88` starts the oracle battle after boot and runs a
non-mutating attack/second-attack/miss audio probe through the same battle
screen queue. When both selectors are set, the debug track starts first and
the battle dispatch then selects its event music, matching the driver's single
music queue rather than mixing two BGM records.

## Retail battle `Sound_Index` census

This is the call-site census requested for the battle action/animation paths.
The source line numbers below are orientation anchors into
`reference/ps4disasm/ps4.asm`; the labels and opcode operands are the
provenance. The census was made over the battle UI block
`ps4.asm:1235-7476`, enemy/boss action blocks `ps4.asm:18424-69034`, and the
player/common battle-object blocks `ps4.asm:70876-81434` with:

```text
rg -n 'Sound_Index|Saved_Sound_Index|Battle_LoadSound' reference/ps4disasm/ps4.asm
```

### Menu, command, and results writes

These are direct `Sound_Index` writes in the battle UI state machine. The
existing input hook covers the live cursor/accept surface; the list is kept so
internal transitions do not disappear behind that abstraction.

| Retail path | Direct writes |
|---|---|
| Initial battle command/cursor setup | `1235` (`loc_BAE`) `$F3`; `1375` (`loc_CF8`) `$F3`; `1584`, `1592`, `1634`, `1642` (`Battle_UpdateRedCursor2`/cursor branches) `$F2` |
| Main and vehicle command windows | `1928` (`Battle_MainOptions`) `$F3`; `2043` (`Battle_VehMainOptions`) `$F3`; `2260` (`loc_1634`) `$F3`; `2294` (`loc_168C`) `$F3` |
| Defense, technique, skill, and item command acceptance | `2310` (`Battle_DefenseCommand`) `$F3`; `2331` (`Battle_TechCommand`) `$F3`; `2350` (`Battle_SkillCommand`) `$F3`; `2369` (`Battle_ItemCommand`) `$F3` |
| Submenu/target cursor movement | `2629` (`Battle_TechWindow`) `$F3`; `2736`, `2757` (`loc_1B0C`/`loc_1B58`) `$F2`; `2970` `$F3`; `3075`, `3096` `$F2`; `3385` `$F3`; `3500`, `3521` `$F2` |
| Later submenu/target branches | `5066`, `5102`, `5121`, `5125`, `5368`, `5372`, `5557`, `5653`, `5823`, `5850`, `6318`, `6341`, `6599`, `7324`, `7476` `$F3`; `5492`, `5504`, `5514`, `5523`, `5585`, `5592` `$F2` |
| Victory/results/level-up confirmation | `4709` (`Battle_VictoryMessage`) `$F3`; `6084` (`BattleResults_LevelUp`) `$F3` |
| Restore/stop paths | `6410-6411` restores `Saved_Sound_Index`; `6422` (`Battle_OpenDefeatedMsg`, revision-0 branch) stops music; `6642` (`loc_4806`) stops music |

`Battle_ProcessRUN` (`ps4.asm:7672-7718`) and `Battle_RunFailMsg`
(`ps4.asm:6654-6697`) contain no additional SFX write: a successful escape
ends the battle, and a failed escape displays `Cannot escape!`. The input
selection write is the only retail menu sound on that path. Likewise, the
level-up routine writes `$F3` on confirmation; there is no separate
level-up-jingle `Sound_Index` write in the results block.

### Action and animation writes

The direct animation census has three layers:

1. **Immediate writes** from enemy, boss, technique, and effect objects. The
   IDs observed are `AttackMiss`, `AndroidSkillImplant`, `AnotherGate`,
   `BlackWave`, `Brose`, `BuffCast`, `Claw`, `Deban`, `Efess`, `EnemyAttack1`,
   `EnemyAttack3`, `EnemyAttack4`, `EnemyAttack5`, `EnemyKilled`,
   `EnemySpellCast`, `FireBreath`, `Foi`, `Fusion`, `Gra`, `GraveOpening`,
   `HealTechCast`, `LaserAttack`, `Legeon`, `Lightning`, `MechEnemyAlarm`,
   `Megid`, `MoleAttack`, `Moonshad`, `Phonon`, `Recovery`, `Res`, `Rifle`,
   `Rimit`, `Saner`, `Shot`, `Slasher`, `SleepGas`, `Spark`, `Sword`,
   `Tandle`, `TechCast`, `Vol`, `WarCry`, and `Zan`.
2. **Player attack selection**: `BattleObj_SlasherTrace`
   (`ps4.asm:70876-70882`) writes `$F1`/`Slasher`; `CloseRangeObjs_DoDamage`
   (`71002-71063`) writes `$F5`/`Sword` on a successful close-range hit and
   `$B8`/`AttackMiss` on a miss. `LoadCloseRangeAttackObj` (`71123-71152`)
   selects `CloseRangeWpns_SoundIndexes` (`71157-71179`): weapon classes
   `1-8` and `$13-$15` use `Sword` `$F5`, `9-$E` use `Rod` `$B5`, and
   `$F-$12` use `Claw` `$E8`. Gun classes `$16-$1C` use `Shot` `$B6` through
   `LoadShotFiredObj` (`71334-71354`). Critical close-range attacks load the
   selected sound four times at delays `0,4,8,$C` (`71231-71244`).
3. **Delayed object routes**: `Battle_LoadSoundObj_CloseRangeWpn`,
   `Battle_LoadSoundObj`, and `Battle_LoadSoundExtendedObj` are the object
   constructors at `71606-71639`. Their shared consumers are
   `BattleObj_Sound` (`74698-74709`) and `BattleObj_SoundExtended`
   (`74714-74745`); they write the object-local ID to `Sound_Index` only when
   its timer/extended routine reaches the playback moment. Call sites include
   `71231-71244`, `71354`, `71460-71471`, `71732`, `71866`, `72344`,
   `72949-72974`, `73224`, `73295`, `73447`, `74083`, `74416-74420`,
   `74496`, `75259`, `75410`, `75767-75782`, `75868-75875`, `76622`,
   `76706-76724`, `77188-77190`, `77265`, `77390`, `78564`, `78698-78700`,
   `78966`, `79334`, `79445`, `79700`, `80106-80114`, `80231`, `80477`,
   `81003`, and `81434`.

The exact direct-write occurrence index is retained here so a future scout can
diff it mechanically rather than relying on a prose summary. All numbers are
`ps4.asm` source lines in the three battle ranges above:

```text
AttackMiss: 52423,53873,63269,69018,71062,77534
AndroidSkillImplant: 27541,52253,75689,75902,76056,78055
AnotherGate: 52949,59859,61335,61880
BlackWave: 67066
Brose: 38440,38450,45457,45576,45658,54755,55866,56189,57154,57673,58188,58404,60566,62978,63421,67174,74010,74226,74244,79140
BuffCast: 38395,40132,41378,53239,54717,56112,56246,74139
Claw: 28340,28519,28704
Deban: 32816,33940,40182,55802,74301,79798
Efess: 27498,34391,45516,46914,47199,55052,55117,67955,72611,75480,79607
EnemyAttack1: 29314,30407,32154,32190,35285,40324,52422,53872,54625,55082,60740,61641,63268,67795,68039,68075,69007,69012,69029,69034,71727
EnemyAttack3: 29663,30145,30221,30285,31481,35264,36088,68152
EnemyAttack4: 30071,35955,36310,38590,46234,46366,47316,57996
EnemyAttack5: 36209,40805,42262,68409
EnemyKilled: 18424
EnemySpellCast: 53250,58370,60537,63364
FireBreath: 30321,31351,42144,47564,55372,57531,58269,62484,63043,66616,66996,67277,75735
Foi: 45188,45381,52061,54190,55012,57283,72505,75976
Fusion: 34997,35749,41906,45761,52465,52525,62304,62427,64001,64272,64428,64967
Gra: 73690
GraveOpening: 47917,47954,47961,48012,67567
HealTechCast: 44733,46640,55695,74351
LaserAttack: 26987,27170,28105,30641,30874,31297,31745,31961,32853,34559,45563,53372,55287,57774,60735,62034,64623,73052,73121,73178
Legeon: 43801,44373,55597,56762
Lightning: 62575,63772
MechEnemyAlarm: 26830,27081,27322,27685,27842,28213,28412,29084,30370,30486,30602,30747,31621,32041,32073,32238,32654,32919,33055,33751,33916,34034,34644,34709,52237,71676,71831,72013
Megid: 33268,33342,33397,33468,33539,33591,33669,53616,60177,73959,81388
MoleAttack: 29465,29599,31229,31414,31546,34969,35040,35727,35802,37988,38646,38718,38831,39317,39546,39973,40118,40350,40516,40766,40903,41364,41666,41739,41870,42004,42236,44501,44557,44663,44710,44846,45022,45056,45096,45238,45451,45570,45652,46002,46220,46343,46429,46598,46744,46987,47159,47297,47424,47540,47671,47800,48228,48361,52406,53133,53334,54133,54235,54272,54335,54460,54586,55195,55276,55358,55463,55535,55672,55768,55828,55881,55958,56089,56220,56342,56381,56445,56485,56910,57023,57131,57253,57390,57511,57642,57966,61620,62365,63224,63751,64610,67665,67787,67945,68145,68293,68304,68396,68598,68788,68839
Moonshad: 38330,39917,46138,67369,72731,72897,72944
MovingCursor: 1584,1592,1634,1642,2736,2757,3075,3096,3500,3521,5492,5504,5514,5523,5585,5592
Nothing: 60853
Phonon: 31072,31171,31796,31904,40589,40694,64538,67677
Recovery: 26903,31291,32389,32736,34134,34879,43779,43876,53152,53355,53509,54164,54699,54797,54872,54915,54958,58054,58155,58237,58353,60078,60317,60513,60682,61232,61296,61422,62149,62357,62962,63028,63142,63352,63490,63763,66686,67383,74494,75122,76363,79698
Res: 44677,44759,46674,55731,74387
Rifle: 64340,66772,66782,66794,72154
Rimit: 41236,44026,60405,63158,74203,76842
Saner: 39664,41411,47475,55999,74277
Selection: 1235,1375,1928,2043,2260,2294,2310,2331,2350,2369,2629,2970,3385,4709,5066,5102,5121,5125,5368,5372,5557,5653,5823,5850,6084,6318,6341,6599,7324,7476
Shot: 27334,28829,28872,28986,29023,30465,30579,32993,71698
Slasher: 28235,28431,28598,29088,29206,29381,33253,33573,33641,33867,34815,45444,52077,53772,64086,68611,70879,72320
SleepGas: 37871,37881,37891,37898,41818,57873
Spark: 54613,60374
Sword: 71056,77551
Tandle: 30933,35607,54053,54516,55206,56634,61635,63728,66811,80831
TechCast: 38053,38234,38923,39987,45261,46016,46790,53552,54147,54349,54371,54484,54813,54890,54933,54991,55558,56509,56916,57029,57139,57261,57398,57519,57650
Vol: 39832,46858,56312
WarCry: 78919
Zan: 67133
Sound_StopMusic: 6422,6642
```

The object-local writes at `Saved_Sound_Index_2/3/4` are also part of the
route: `30579`, `52422-52423`, `53872-53873`, `63268-63269`, and
`69012/69034` save delayed IDs; `67103`, `68905`, `68983-68994` restore them
into the live index. The new enemy-object scout retains those writes and
their owning object IDs. The Tier-1 runtime still exposes one ordered attack
event rather than every child/effect tick, so those child writes remain
provenance and are not falsely emitted as extra timeline events.

## Dispatch coverage and honest limits

| Retail census family | Current route | Coverage decision |
|---|---|---|
| Weapon attack (`Sword`, `Rod`, `Claw`, `Shot`) | Pre-round raw equipment -> exact retail index table -> `Attacked` | Wired for Tier-1 physical turns; multi-target attacks emit one action sound, matching the object load point |
| Hit/miss | `Resolved` miss -> `$B8`; ordinary hit is already the weapon action sound | Wired |
| Enemy death | `Died` -> `$B9` | Wired |
| Per-enemy physical animation (`EnemyAttack1/3/4/5`, `MoleAttack`, etc.) | Retail `EnemyAttackOffs` object graph -> exact SFX on enemy `Attacked` | Wired for all 153 records; fixed timing is presented where proven, movement/sprite sheets remain explicit |
| Technique/skill/item effects | `TechCast`, `Foi`, `Megid`, healing/buff/effect families | Deferred; core has no command/event carrying the retail technique/skill/effect at its animation moment |
| Battle menus | `$F2`/`$F3` modal input hooks; direct internal writes remain censused | Wired for live input, internal transition parity deferred |
| Run/flee | `Battle_ProcessRUN` and `Battle_RunFailMsg` have no distinct SFX write | No additional sound to dispatch; selection is the only retail menu sound |
| Victory and level-up results | `$8B` victory music plus `$F3` result confirmation | Wired; no separate level-up jingle exists in the census |
| EB sequence routes | Driver `EB` queues the byte and applies sound priority before the next tick | Wired; fixture proves SFX starts over active music |
| Vehicle battle music `$96` | Mounted encounter/debug battle dispatches `$96` through the existing music queue | Wired; vehicle battle surface is recorded in `docs/VEHICLES.md` |

## EB route and channel-stealing proof

Retail `cfEB_PlaySound` (`reference/ps4disasm/sound/ps4.sound_driver.asm:1624-1626`)
copies the following byte into the sound latch. The extracted music records
contain `FF EB <id>` routes in addition to game-code writes. `psiv-sound` now
uses the same priority arbitration for EB requests as external `play(id)`
requests; if several tracks raise sound in one driver tick, the highest
priority ID survives instead of whichever track happened to run last.

The existing driver channel policy was already the right mixer shape:
`start_sfx` steals only overlapping FM/PSG channels, `update_suppression`
issues music note-off writes while the SFX is live, and resumes those music
tracks when the SFX ends. No new mixer workaround was added. The fixture
`eb_sequence_sound_dispatch_overlays_music_deterministically` loads a music
track whose EB raises `$B5`, a `$B5` SFX on the same FM channel, runs three
driver ticks twice, and asserts identical register logs containing both the
music note-off (`$28`) and SFX voice writes.
The companion fixture `battle_action_sfx_dispatch_order_is_deterministic_over_theme`
queues a theme, `$F5` weapon attack, and `$B8` miss in that order and asserts
the same register trace twice, with compressed FM voice algorithms
`[theme, attack, miss]`. `distinct_enemy_attack_sfx_register_log_is_ordered_and_deterministic`
does the same for retail `$D8` Zoran Bult followed by `$D6` Gunner Bit and
asserts compressed algorithms `[4, 5]` on both runs.

## Verification

The final local checks for the sound slice were:

```text
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=2 cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets --all-features -- -D warnings
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml --workspace
timeout 30s env PSIV_DEBUG_BATTLE=0x88 godot --headless --audio-driver Dummy --path godot --quit-after 300
```

Results for this slice: Rust workspace green, clippy zero, and the sound
register-log fixture plus runtime battle-SFX ordering tests are green. The
intended Godot headless proof prints the battle theme dispatch followed by the
exact enemy attack IDs `0xd8` and `0xd6`, then miss effect `0xb8`; this
workspace currently has no `godot` or `godot4` executable, so that last live
engine command is environment-blocked and is not claimed green here. The
real Piata track's existing first-three-tick fixture remains deterministic and
includes YM DAC `$2A` data plus `$2B` enable writes.

Headless Godot boots exit successfully and demonstrate normal Piata routing,
debug-track routing, and combined debug-battle routing. Godot 4.7.1 reports
one engine-owned `AudioStreamGeneratorPlayback` ObjectDB warning during
process teardown; the safe `stop`/stream-detach path is in place, but the
headless engine does not retire that playback before its final leak audit.
