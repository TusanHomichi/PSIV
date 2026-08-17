# Next-arc trigger census: Piata gate through the post-Zio retail handoff

Scouted from the retail per-map event lists in `generated/maps.json` and the
packed map records. Trigger ids below are `RunEventsJmpTbl` indices; the event
on the right is the `Event_Index` written by the routine. The pointer table
authority is retail `EventPtrs` at `$05A2B4` (161 entries, `$00..$A0`) and
`CutscenePtrs` at `$05A580` (34 entries). The rows after MeetingRika are the
retail order of the player-controlled chain, not a story-wiki ordering.

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

After the Dark Force 1 defeat handoff, the fixed next major story gate remains
`$50`, `RunEvent_Reunion`, which requires Elsydeon `$D9` set and Reunion `$DA`
clear before writing `$801F`. The current slice stops at the first Dark Force
post-battle inventory/map handoff because the next surface is the larger
player-controlled Dezo/Kuran campaign, not an automatic continuation of the
same scene. Molcum `$40` still has only `[0]`; `$00` dispatches
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
pretend binary comparison. Every scene in docs 31-50 cites its retail pointer
range and the source branch; the two direct Zio Fort bodies are listed but not
misclassified as `RunEvent` writers.

FortuneTeller and AfterFortuneTeller are the hard boundary: the disassembly
has `if grand_cross=1` at `ps4.asm:116594` and `:184013`, no retail body and no
retail pointer slots. They are excluded from both the pointer census and the
scene registry.
