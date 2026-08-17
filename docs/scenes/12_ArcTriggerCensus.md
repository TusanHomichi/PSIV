# Retail trigger census: Piata gate through the Dezo endgame boundary

Scouted from the retail per-map event lists in `generated/maps.json`, the
packed map records and the ROM. Trigger ids below are `RunEventsJmpTbl`
indices; the event on the right is the `Event_Index` written by the routine.
The pointer table authority is retail `EventPtrs` at `$05A2B4` (161 entries,
`$00..$A0`) and `CutscenePtrs` at `$05A580` (34 entries). The rows after
MeetingRika are retail dispatch order, not a story-wiki ordering.

## Map lists

| Map | id | tree | per-map events | dispatch |
|---|---:|---:|---|---|
| BioPlant Part2 | `$A3` / 163 | 12 | `$0C` | `$0012` / BioPlant alarm |
| BioPlant B4 Part2 | `$AC` / 172 | 34 | `$1A` | `$8007` / Meeting Rika |
| Aiedo | `$54` / 84 | 5 | `$27` | `$003C` / leaving ChazHouse |
| ChazHouse | `$5E` / 94 | 10 | `$26` | `$003B` / ChazHouse rest |
| Zema | `$24` / 36 | 4 | `$15,$17,$74` | `$8005`, `$8006`, `$008A` |
| BirthValley | `$2B` / 43 | 3 | `$00` | null; no scene |
| BirthValley B1 | `$2C` / 44 | 3 | `$00` | null; no scene |
| KrupKindergarten | `$3A` / 58 | 5 | `$16` | `$000D` / Saya |
| TonoeStorageRoom | `$42` / 66 | 7 | `$00` | null; direct door control below |
| TonoeGryzHouse | `$43` / 67 | 7 | `$14` | `$8004` after Dorin flag |
| TonoeBasement B3 | `$4A` / 74 | 7 | `$18` | `$0028` / Alshline found |
| ValleyMaze parts | `$9B..$A1` / 155..161 | 7 | `$00` | null |
| ValleyMazeOutside | `$D8` / 216 | 7 | `$30` | `$0027` / Rune Flaeli |
| ValleyMazeOutside2 | `$D9` / 217 | 7 | `$00` | null |
| MachineCenter B1 Part2 | `$B9` / 185 | 7 | `$1B` | `$002B` / Getting Land Rover |
| Motavia overworld | `$00` / 0 | 5 | `$06,$3A,$3B,$3C,$5B` | `$0006`; house/old Piata controls |
| ZioFort F4 | `$8B` / 139 | 3 | `$1C,$1D` | `$8008`, `$8009` |
| LadeaTower F2 | `$8E` / 142 | 5 | `$19` | `$002E` / Rune reunion |
| LadeaTower F5 | `$91` / 145 | 3 | `$2A,$1E` | `$002F`, `$800A` |
| Nurvus B4 Part2 | `$D3` / 211 | 3 | `$1F,$20` | `$0034`, `$800B` |
| Molcum | `$40` / 64 | 1 | `$00` | null; no retail scene body |
| Mota Spaceport | `$BF` / 191 | 3 | `$12,$21,$00` | `$800D` / spaceship |
| Zelan Space | `$18C` / 396 | 1 | `$29` | `$800F` / crash |
| Zelan | `$18D` / 397 | 3 | `$0D,$28,$23` | `$800E`; spaceship |
| Zelan F1 | `$18E` / 398 | 3 | `$0D,$55,$00` | dialogue/Wren handoff |
| Kuran Space | `$18F` / 399 | 1 | `$00` | null |
| Kuran | `$190` / 400 | 3 | `$0D,$23,$2C` | `$003D` / Kuran arrival |
| Dezolis | `$001` / 1 | 3 | `$31,$35,$36` | later Dezo controls |
| Raja Temple | `$14C` / 332 | 1 | `$00` | null; cutscene landing target |
| Hangar | `$15F` / 351 | 1 | `$2B` | `$8010` / Landale |

The list entries are not scene ids. For example, Zema's `$15` calls
`RunEvent_UsingAlshline`, which writes cutscene `$8005`; treating `$15` as an
event routine would dispatch the wrong pointer. Birth Valley's `$00` and
Molcum's `$00` are the retail null routine and are intentionally not
represented by a scene.

## Dezo campaign map lists

These are the retail map surfaces touched by the `$33..$54` dispatch delta.
The side entries are included so the pointer census does not imply that every
retail body is part of the fixed chain.

| Map | id | retail map events / route role |
|---|---:|---|
| Dezolis overworld | `$001` | `$35/$36`; Le Roof, Carnivorous Trees, Saving Kyra, Eclipse Torch |
| Inner Sanctuary | `$16E` | entrance/guard dialogue; direct permission surface |
| Inner Sanctuary B1 | `$16F` | Lutz, Elsydeon cave and Elsydeon landing |
| Gumbious F1 | `$162` | Lashiec defeat landing and Bishop handoff |
| Air Castle floors | `$170..$17A` | arrival, Xe A Thoul, fake chest and Lashiec |
| Mota Spaceport | `$0BF` | Gumbious/late Reunion map landing |
| Strength Tower | `$0F2..$0F6` | `$5E` top event |
| Courage Tower | `$0F7..$0FB` | `$5F` top event and DeVars/Sa Lews route |
| Anger Tower | `$0FC..$0FE` | `$69/$6A`; Profound Darkness approach |
| Motavia / Rykros route | `$000/$002` | Seth/Aero/Rykros player-controlled side of the chain |

## Retail `RunEventsJmpTbl[$33..$54]`

This is the exact dispatch delta audited by
`the_dezo_campaign_dispatch_delta_matches_the_retail_census` in
`rust/psiv-core/src/trigger_table.rs`.

| index | retail routine | required gate / location summary | result |
|---:|---|---|---|
| `$33` | `RunEvent_MeetingLeRoof` | Le Roof `$D1` clear | `EventPtrs[$48]` / `$0048` |
| `$34` | `RunEvent_LeRoofAgain` | `$D5/$D3` set, `$D6` clear | `CutscenePtrs[$1C]` / `$801C` |
| `$35` | `RunEvent_CarnivorousTrees` | custom Raja Sick/Saving Kyra arms | `$004C`, `$004D`, or `$8013` |
| `$36` | `RunEvent_EclipseTorchUsed` | `$9B` set, `$9C` clear, `x=$B80..$BB0`, `y<=$E0` | `EventPtrs[$47]` / `$0047` |
| `$37` | `RunEvent_FindDarkForce2` | `$9E` clear, `y=$100` | `EventPtrs[$4E]` / `$004E` |
| `$38` | `RunEvent_DarkForce2Defeated` | `$9E` set, `$A1` clear | `CutscenePtrs[$17]` / `$8017` |
| `$39` | `RunEvent_LutzRevelation` | `$97` clear, `x=$1E0..$210`, `y=$1D0..$1F0` | `CutscenePtrs[$14]` / `$8014` |
| `$3A` | `RunEvent_MeetingSeth` | `$C1` clear, `($770,$9B0)` | `CutscenePtrs[$19]` / `$8019` |
| `$3B` | `RunEvent_AeroPrism` | chest `$10D` set, `$C5` clear | `CutscenePtrs[$1A]` / `$801A` |
| `$3C` | `RunEvent_DarkForce3Defeated` | `$C5` set, `$C6` clear | `EventPtrs[$50]` / `$0050` |
| `$3D` | `RunEvent_ReshelBattle` | `$8B` clear | `EventPtrs[$53]` / `$0053` |
| `$3E` | `RunEvent_ClmCenterForcedBattle` | `$92/$9E` clear | `EventPtrs[$54]` / `$0054` |
| `$3F` | `RunEvent_ClmCenterAfterBattle` | `$92` set, `$A4/$9E` clear | `EventPtrs[$55]` / `$0055` |
| `$40` | `RunEvent_FightingDElmLars` | `$93/$9E` clear, `y=$0F0` | `EventPtrs[$56]` / `$0056` |
| `$41` | `RunEvent_DElmLarsDefeated` | `$93` set, `$A5/$9E` clear | `EventPtrs[$57]` / `$0057` |
| `$42` | `RunEvent_FindingAirCastle` | `$98` set, `$99` clear | `CutscenePtrs[$15]` / `$8015` |
| `$43` | `RunEvent_EnterAirCastle` | `$9F` clear | `EventPtrs[$58]` / `$0058` |
| `$44` | `RunEvent_FindXeAThoul` | `$9A` clear, Air Castle rectangle | `EventPtrs[$59]` / `$0059` |
| `$45` | `RunEvent_AirCstlFakeChest` | chest `$10C` set, `$A6` clear | `EventPtrs[$5A]` / `$005A` |
| `$46` | `RunEvent_LashiecAppearing` | `$A6` set, `$9B` clear | `EventPtrs[$5D]` / `$005D` |
| `$47` | `RunEvent_LashiecDefeated` | `$9B` set | `CutscenePtrs[$16]` / `$8016` |
| `$48` | `RunEvent_GumbiousBishop` | Hydrofoil `$9D` clear, Gumbious rectangle | `CutscenePtrs[$18]` / `$8018` |
| `$49` | `RunEvent_StrengthTowerTop` | `$E2` clear | `EventPtrs[$5E]` / `$005E` |
| `$4A` | `RunEvent_CourageTowerTop` | `$E3` clear | `EventPtrs[$5F]` / `$005F` |
| `$4B` | `RunEvent_DeVarsDefeated` | `$D4` set, `$E5` clear | `EventPtrs[$64]` / `$0064` |
| `$4C` | `RunEvent_SaLewsDefeated` | `$D2` set, `$E6` clear | `EventPtrs[$65]` / `$0065` |
| `$4D` | `RunEvent_Null4D` | unconditional `moveq #1,d7`, no index | `FireWithoutIndex` |
| `$4E` | `RunEvent_ReFaze` | `$E4` set, `$E1` clear | `EventPtrs[$63]` / `$0063` |
| `$4F` | `RunEvent_BeforeElsydeonCave` | `$D6` set, `$D8` clear | `CutscenePtrs[$1D]` / `$801D` |
| `$50` | `RunEvent_Reunion` | `$D9` set, `$DA` clear | `CutscenePtrs[$1F]` / `$801F` |
| `$51` | `RunEvent_AngerTowerTop` | `$E4` clear, `x=$1C0..$1D0`, `y=$250..$260` | `EventPtrs[$69]` / `$0069` |
| `$52` | `RunEvent_AngerTowerExitTop` | `$E7` clear, `x=$1A0..$1B0`, `y=$260` | `EventPtrs[$6A]` / `$006A` |
| `$53` | `RunEvent_ProfoundDarkness` | `$E8` clear, `y=$160` | `CutscenePtrs[$20]` / `$8020` |
| `$54` | `RunEvent_Ending` | `$E8` set | `CutscenePtrs[$21]` / `$8021` (boundary) |

## Retail pointer body ranges for the Dezo census

`EventPtrs` ranges below are ROM half-open ranges shown as inclusive bytes in
the scene documents. This table includes direct side bodies so they cannot be
mistaken for missing pointer data.

| EventPtrs | retail body | ROM range | scene record |
|---:|---|---|---|
| `$47` | `Event_EclipseTorchUsed` | `$07018A..$070482` | [56](56_EclipseTorchUsed.md) |
| `$48` | `Event_MeetingLeRoof` | `$070482..$07069E` | [51](51_MeetingLeRoof.md) |
| `$49` | `Event_MuskCatsGuarding` | `$07069E..$0706C0` | boundary [88](88_RetailBoundaries.md) |
| `$4A` | `Event_MuskCatElder` | `$0706C0..$070702` | boundary [88](88_RetailBoundaries.md) |
| `$4B` | `Event_PenguinOwner` | `$070702..$070774` | boundary [88](88_RetailBoundaries.md) |
| `$4C` | `Event_CarnivorousTrees` | `$070774..$070856` | [53](53_CarnivorousTrees.md) |
| `$4D` | `Event_SavingKyra` | `$070856..$070976` | [54](54_SavingKyra.md) |
| `$4E` | `Event_DarkForce2` | `$070976..$0709A2` | [57](57_DarkForce2.md) |
| `$4F` | `Event_EsperGuardPermission` | `$0709A2..$070A2A` | boundary [88](88_RetailBoundaries.md) |
| `$50` | `Event_DarkForce3Defeated` | `$070A2A..$070A4E` | [62](62_DarkForce3Defeated.md) |
| `$51` | `Event_InnerSanctGuard` | `$070A4E..$070A76` | boundary [88](88_RetailBoundaries.md) |
| `$52` | `Event_InnerSanctGuardBeforeElsydeon` | `$070A76..$070A9E` | boundary [88](88_RetailBoundaries.md) |
| `$53` | `Event_ReshelBattle` | `$070A9E..$070ABC` | [63](63_ReshelBattle.md) |
| `$54` | `Event_ClmCenterForcedBattle` | `$070ABC..$070ADA` | [64](64_ClmCenterForcedBattle.md) |
| `$55` | `Event_ClmCenterAfterBattle` | `$070ADA..$070AEC` | [65](65_ClmCenterAfterBattle.md) |
| `$56` | `Event_DElmLars` | `$070AEC..$070B0C` | [66](66_DElmLars.md) |
| `$57` | `Event_AfterDElmLarsBattle` | `$070B0C..$070B1E` | [67](67_AfterDElmLarsBattle.md) |
| `$58` | `Event_AirCastleArrival` | `$070B1E..$070B30` | [69](69_AirCastleArrival.md) |
| `$59` | `Event_XeAThoulBeforeBattle` | `$070B30..$070B56` | [70](70_XeAThoulBeforeBattle.md) |
| `$5A` | `Event_AirCastleFakeChest` | `$070B56..$070C30` | [71](71_AirCastleFakeChest.md) |
| `$5B` | `Event_RajaSick` | `$070C30..$070C82` | boundary [88](88_RetailBoundaries.md) |
| `$5C` | `Event_Gyuna` | `$070C82..$070CB4` | boundary [88](88_RetailBoundaries.md) |
| `$5D` | `Event_LashiecAppearance` | `$070CB4..$070E4E` | [72](72_LashiecAppearance.md) |
| `$5E` | `Event_StrengthTowerTop` | `$070E4E..$071102` | [75](75_StrengthTowerTop.md) |
| `$5F` | `Event_CourageTowerTop` | `$071102..$07144A` | [76](76_CourageTowerTop.md) |
| `$60` | `Event_DeVars` | `$07144A..$07146A` | [77](77_DeVars.md) |
| `$61` | `Event_SaLews` | `$07146A..$07148A` | [78](78_SaLews.md) |
| `$62` | `Event_AngerTowerAlys` | `$07148A..$07157A` | boundary [88](88_RetailBoundaries.md) |
| `$63` | `Event_ReFaze` | `$07157A..$07175A` | [79](79_ReFaze.md) |
| `$64` | `Event_DeVarsDefeated` | `$07175A..$071822` | [80](80_DeVarsDefeated.md) |
| `$65` | `Event_SaLewsDefeated` | `$071822..$0718E6` | [81](81_SaLewsDefeated.md) |
| `$66` | `Event_HuntersGuild` | `$0718E6..$071E56` | boundary [88](88_RetailBoundaries.md) |
| `$67` | `Event_RuneHealingChaz` | `$071E56..$071E66` | boundary [88](88_RetailBoundaries.md) |
| `$68` | `Event_PickingFifthCharacter` | `$071E66..$0721CC` | boundary [88](88_RetailBoundaries.md) |
| `$69` | `Event_AngerTowerTop` | `$0721CC..$072262` | [85](85_AngerTowerTop.md) |
| `$6A` | `Event_AngerTowerExitTop` | `$072262..$0722D2` | [86](86_AngerTowerExitTop.md) |

The corresponding cutscene bodies are:

| CutscenePtrs | body | ROM range | scene record |
|---:|---|---|---|
| `$12` / `$8012` | `Cutscene_RajaSick` | `$07734C..$077788` | boundary [88](88_RetailBoundaries.md) |
| `$13` / `$8013` | `Cutscene_MeetingKyra` | `$077788..$077896` | [55](55_MeetingKyra.md) |
| `$14` / `$8014` | `Cutscene_LutzRevelation` | `$077896..$077A2E` | [58](58_LutzRevelation.md) |
| `$15` / `$8015` | `Cutscene_FindingAirCastle` | `$077A2E..$077A68` | [68](68_FindingAirCastle.md) |
| `$16` / `$8016` | `Cutscene_LashiecDefeated` | `$077A68..$077BDA` | [73](73_LashiecDefeated.md) |
| `$17` / `$8017` | `Cutscene_DarkForce2Defeated` | `$077BDA..$077DC6` | [59](59_DarkForce2Defeated.md) |
| `$18` / `$8018` | `Cutscene_GumbiousBishop` | `$077DC6..$077EAC` | [74](74_GumbiousBishop.md) |
| `$19` / `$8019` | `Cutscene_MeetingSeth` | `$077EAC..$077F2E` | [60](60_MeetingSeth.md) |
| `$1A` / `$801A` | `Cutscene_AeroPrism` | `$077F2E..$07818E` | [61](61_AeroPrism.md) |
| `$1B` / `$801B` | `Cutscene_Rykros` | `$07818E..$078346` | boundary [88](88_RetailBoundaries.md) |
| `$1C` / `$801C` | `Cutscene_LeRoofAgain` | `$078346..$0784B6` | [52](52_LeRoofAgain.md) |
| `$1D` / `$801D` | `Cutscene_BeforeElsydeonCave` | `$0784B6..$078584` | [82](82_BeforeElsydeonCave.md) |
| `$1E` / `$801E` | `Cutscene_Elsydeon` | `$078584..$078A7C` | [83](83_Elsydeon.md) |
| `$1F` / `$801F` | `Cutscene_Reunion` | `$078A7C..$078D30` | [84](84_Reunion.md) |
| `$20` / `$8020` | `Cutscene_ProfoundDarkness` | `$078D30..$078F3E` | [87](87_ProfoundDarkness.md) |
| `$21` / `$8021` | `Cutscene_Ending` | `$078F3E..EOF` | boundary [88](88_RetailBoundaries.md) |

## Direct dialogue controls on the same route

The map lists only cover `RunEvents`. Dialogue controls are a second dispatch
surface and are part of the playable arc:

| map tree | entry | control | event / scene |
|---|---:|---|---|
| tree 3 | `$6A` (106) | flag `$10`, then F6 | `$8002` `Cutscene_ProfHolt` |
| tree 7 | `$02` | F6 | `$8003` `Cutscene_MeetingRune` |
| tree 7 | `$17` (23) | flag `$65`, then F6 | `$0032` `Event_MeetingDorin` |
| tree 7 | `$29` (41) | flag `$65`, then F6 | `$0033` `Event_TonoeBasementDoor` |
| tree 4 | `$4C` (76) | F6 | `$008B` `Event_ZemaOldMan` |
| tree 4 | `$4E` (78) | F6 | `$008C` `Event_ZemaOldManAfterMission` |

The Aiedo supermarket is not in this table because its inn/shop routine calls
`Event_GirlsSneakingOut` directly when selector `$06` is active and both Zio
and GirlsCaught are clear. It is a pointer-table event `$23`, not a
`RunEventsJmpTbl` dispatch.

## Trigger formulas transcribed in `trigger_table`

| trigger | retail routine | required state | event |
|---:|---|---|---|
| `$14` | `RunEvent_Dorin` | Dorin `$36` set, Gryz `$30` clear | `$8004` |
| `$0C` | `RunEvent_BioPlantAlarm` | temp `$08` clear, leader Y `$180` | `$0012` |
| `$15` | `RunEvent_UsingAlshline` | Alshline `$32` set, Zema Igglanova `$33` clear | `$8005` |
| `$16` | `RunEvent_MeetingSaya` | Saya `$12` clear, standing y `<=$280` | `$000D` |
| `$17` | `RunEvent_ZemaIgglanovaDefeated` | Zema Igglanova `$33` set, after-beat `$37` clear | `$8006` |
| `$18` | `RunEvent_FindingAlshline` | chest `$08` set, Alshline `$32` clear | `$0028` |
| `$30` | `RunEvent_RuneFlaeli` | Rune `$11` set, Tonoe path `$13` clear | `$0027` |
| `$1A` | `RunEvent_MeetingRika` | BioPlant escape `$34` clear, leader Y `$1A0` | `$8007` |
| `$26` | `RunEvent_ChazHouseRest` | temp `$18` clear | `$003B` |
| `$27` | `RunEvent_ClrChazHouseRest` | temp `$18` set | `$003C` |
| `$74` | `RunEvent_ZemaServantBattle` | Silver Soldier `$B1` set, servants `$B2` clear | `$008A` |
| `$1B` | `RunEvent_GettingLandRover` | Control Key chest `$10A` set, Land Rover `$44` clear, leader Y `>=$1C0` | `$002B` |
| `$06` | `RunEvent_MachineCenter` | Zio `$42` set, Machine Center `$43` clear, X `$710..$750`, Y `$AD0` | `$0006` |
| `$1C` | `RunEvent_SavingDemi` | Zio `$42` clear, leader Y `$170` | `$8008` |
| `$1D` | `RunEvent_AlysWounded` | Zio `$42` set, Demi Joined `$47` clear | `$8009` |
| `$19` | `RunEvent_RuneLadeaTower` | Rune Joined Again `$62` clear, leader X `>=$3C0` | `$002E` |
| `$2A` | `RunEvent_FindingPsycoWand` | Gy Laguiah `$69` clear, leader X `$1E0..$1F0`, Y `$1B0` | `$002F` |
| `$1E` | `RunEvent_PsycoWandFound` | Psycho Wand chest `$109` set, After Alys Death 2 `$67` clear | `$800A` |
| `$1F` | `RunEvent_ZioNurvus` | Zio Nurvus `$65` clear, leader Y `$1E0` | `$0034` |
| `$20` | `RunEvent_ZioDefeated` | Gryz Gone `$68` clear, Zio Nurvus `$65` set | `$800B` |
| `$21` | `RunEvent_EnterSpaceship` | Mota Spaceport rectangle `x=$1E0..$1F0,y=$120` | `$800D` |
| `$22` | `RunEvent_EnterGrbkTwDoor` | map-layout custom check | `$0038` (not in this slice) |
| `$23` | `RunEvent_KuranEnterSpaceship` | leader Y `$2F0` | `$800D` |
| `$24` | `RunEvent_AirCstlEnterSpaceship` | leader Y `$370` | `$800D` |
| `$25` | `RunEvent_SilenceTmEnterSpaceship` | leader Y `$120` | `$800D` |
| `$26` | `RunEvent_ChazHouseRest` | temp `$18` clear | `$003B` (already covered) |
| `$27` | `RunEvent_ClrChazHouseRest` | temp `$18` set | `$003C` (already covered) |
| `$28` | `RunEvent_SpaceshipSabotage` | Wren `$70` + Canceller `$72` set; Chaos `$71` clear; Y `$2F0` | `$800E` |
| `$29` | `RunEvent_CrashLanding` | Chaos `$71` set | `$800F` |
| `$2A` | `RunEvent_FindingPsycoWand` | Gy Laguiah `$69` clear; `x=$1E0..$1F0,y=$1B0` | `$002F` (already covered) |
| `$2B` | `RunEvent_FindingLandale` | Tyler Grave `$84` set; Dezo Spaceport `$82` clear; `x=$1A0..$1B0,y=$520` | `$8010` |
| `$2C` | `RunEvent_EnterKuran` | Kuran `$86` clear | `$003D` |
| `$2D` | `RunEvent_NearDarkForce` | Near Dark Force `$87` clear; leader Y `$200` | `$003E` |
| `$2E` | `RunEvent_FindDarkForce` | Dark Force 1 `$83` clear; leader Y `$0D0` | `$003F` |
| `$2F` | `RunEvent_JuzaDefeated` | Juza `$41` set; Juza Defeated `$48` clear | `$0041` |
| `$30` | `RunEvent_RuneFlaeli` | Rune `$11` set; Tonoe path `$13` clear | `$0027` (already covered) |
| `$31` | `RunEvent_OutsideRajaTemple` | Snowstorm `$80` clear | `$0043` (later) |
| `$32` | `RunEvent_DarkForce1Defeated` | Dark Force 1 `$83` set; Ice Digger `$89` clear | `$8011` |

The formulas are covered by the focused census test in
`rust/psiv-core/src/trigger_table.rs`. No `trigger_custom` addition was
needed: every trigger in these maps is already a flags/position condition.

`$2A` and `$1E` deliberately have different prerequisites and write different
event indices: `$2A` is the position-gated live Psycho Wand chest/battle body
(`$2F`), while `$1E` is the opened-chest gate that dispatches cutscene `$800A`
after the map event loop is evaluated again. That distinction is where a flat
story summary tends to lose the intermediate battle.

## Retail pointer and dispatch audit after `$8007`

`EventPtrs` is a 161-entry retail table, `$00..$A0`; the next relevant
non-null bodies after the MeetingRika hand-off are:

| EventPtrs index | Retail body | ROM bytes | How it is reached |
|---:|---|---|---|
| `$06` | `Event_MachineCenterAppearing` | `$06B4B2..$06B6F3` | `RunEventsJmpTbl[$06]` on Motavia |
| `$2B` | `Event_GettingLandRover` | `$06DEBE..$06E0E9` | `RunEventsJmpTbl[$1B]` on Machine Center |
| `$2E` | `Event_RuneLadaeTower` | `$06E930..$06EA15` | `RunEventsJmpTbl[$19]` on Ladea F2 |
| `$2F` | `Event_PsycoWandChest` | `$06EA16..$06EC61` | `RunEventsJmpTbl[$2A]` on Ladea F5 |
| `$30` | `Event_ZioFortBarrier` | `$06EC62..$06EE3F` | direct Zio Fort map-data path; no RunEvent writer |
| `$31` | `Event_ZioFanatic` | `$06EE40..$06EEBD` | direct/map dialogue path; not the `$8007` chain writer |
| `$34` | `Event_ZioNurvus` | `$06F2EA..$06F439` | `RunEventsJmpTbl[$1F]` on Nurvus |

The corresponding cutscene table entries are contiguous after MeetingRika:

| CutscenePtrs index | Event | ROM bytes | RunEvent writer |
|---:|---|---|---|
| `$07` | `Cutscene_MeetingRika` `$8007` | `$0745DE..$074A7D` | `$1A` |
| `$08` | `Cutscene_DemiRescue` `$8008` | `$074A7E..$074B71` | `$1C` |
| `$09` | `Cutscene_AlysWounded` `$8009` | `$074B72..$0751FF` | `$1D` |
| `$0A` | `Cutscene_PsycoWand` `$800A` | `$075200..$075A11` | `$1E` |
| `$0B` | `Cutscene_ZioDefeated` `$800B` | `$075A12..$075FC7` | `$20` |
| `$0C` | `Cutscene_MeetingWren` `$800C` | `$075FC8..$07606B` | dialogue/scene handoff |
| `$0D` | `Cutscene_InsideSpaceship` `$800D` | `$07606C..$07607D` | `$21/$23/$24/$25` |
| `$0E` | `Cutscene_SpaceshipSabotage` `$800E` | `$07607E..$076589` | `$28` |
| `$0F` | `Cutscene_CrashLaanding` `$800F` | `$07658A..$07714F` | `$29` |
| `$10` | `Cutscene_Landale` `$8010` | `$077150..$0771D1` | `$2B` |
| `$11` | `Cutscene_DarkForce1Defeated` `$8011` | `$0771D2..$07734B` | `$32` |

The ordinary EventPtrs bodies reached by the same post-Zio census are:

| EventPtrs index | Retail body | ROM bytes | Dispatch |
|---:|---|---|---|
| `$3D` | `Event_KuranArrival` | `$06FAD4..$06FAE5` | `$2C` |
| `$3E` | `Event_NearDarkForce1` | `$06FAE6..$06FAF7` | `$2D` |
| `$3F` | `Event_DarkForce1` | `$06FAF8..$06FB1D` | `$2E` |
| `$40` | `Event_Juza` | `$06FB1E..$06FB5D` | interaction path |
| `$41` | `Event_JuzaDefeated` | `$06FB5E..$06FBED` | `$2F` |

The post-Zio dispatch delta is **18 RunEvents entries**, `$21..$32`, including
the generic spaceship destinations, two already-covered detours and later
Raja/Dezo controls. The newly transcribed fixed chain is
`$800C → $800D → $800E → $800F → $8010 → $003D → $003E → $003F → $8011`,
with `$0040/$0041` recorded as the adjacent Kuran/Juza interaction path.
Map travel and player dialogue separate those dispatches; the table, not story
memory, decides their identity and order. `EventPtrs[$A1]` and `[$A2]` do not
exist in retail: FortuneTeller and AfterFortuneTeller are Grand Cross-only
labels and are excluded.

## Proven boundary

The new typed chain runs from the Dark Force 1 post-battle handoff through
`$8020`, including the Reunion gate: `$50`, `RunEvent_Reunion`, requires
Elsydeon `$D9` set and Reunion `$DA` clear before writing `$801F`. The chain
stops at the `$8020` Profound Darkness battle request. `$54` then dispatches
`Cutscene_Ending` `$8021`, whose body is the final credits/presentation
surface and is intentionally outside the current typed scene contract. The
direct Raja Sick, Rykros, Alys, guild and fifth-character controls are
enumerated in [88](88_RetailBoundaries.md), but are not hidden in the fixed
RunEvents chain. Molcum `$40` still has only `[0]`; `$00` dispatches
`RunEvent_Null00`, which returns without an event body.

## Fork and byte authority

`reference/ps4disasm/ps4.asm` is the Grand Cross clone, not a retail oracle.
For the retail-only scene includes (`MeetingSaya`, `RuneFlaeli`, `Alshline`,
`ZemaIgglanovaDefeated`, `BioPlantAlarm`, `GirlsSneakingOut`, `ChazHouse`,
`LeavingChazHouse`, `MeetingRika`, and the other conditional scene bodies), the
`grand_cross=1` branch is an absent `script/scenes/...` include; the retail
bytes below are the only executable authority. The surviving `grand_cross=0`
branches were checked against the retail pointer ranges before they were
expressed as `SceneOp` data. No clone executable is checked in, so “byte diff”
means retail disassembly versus the clone's surviving source branch, never a
pretend binary comparison. Every scene in docs 31-87 cites its retail pointer
range and the source branch; direct side bodies are listed but not
misclassified as `RunEvent` writers.

FortuneTeller and AfterFortuneTeller are the hard boundary: the disassembly
has `if grand_cross=1` at `ps4.asm:116594` and `:184013`, no retail body and no
retail pointer slots. They are excluded from both the pointer census and the
scene registry.
