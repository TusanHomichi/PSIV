# Event coverage census

Every event index the cartridge can fire, derived from the US image, and what
the port does with each. The class this closes: the campaign runner kept
halting on events the port never transcribed (`$71` sandworm, the Garuberk
doors, the penguin feed, `$2A` Canceller reminder, four
`Event_GetAndRunDialogue2` callers, then H28's `$43` on every arrival outside
Raja Temple), each found one at a time by a halt.

**The guard.** [`tests/test_event_coverage.py`](../../tests/test_event_coverage.py)
(derivation in [`tests/event_census.py`](../../tests/event_census.py)) and the
paired Rust test `rust/psiv-core/tests/event_census.rs` (`cargo test -p
psiv-core --test event_census`). Every fired event must be a registered scene,
a null routine, only called from inside a registered scene, or in the
allowlist with a tracking issue. The allowlist only shrinks: a scene for an
allowlisted event fails both tests until its entry is removed, and the entry
count may never exceed `ALLOWLIST_CEILING` (lowered by hand when a scene
lands). Negative controls in both languages: unregister a scene and the guard
fails naming it. Regenerate the tables below with
`python3 -m tests.event_census --write`.

## How each source is read

An event reaches the field dispatcher (`FieldRoutine_Event`, `$5A266`) only by
a write to `Event_Index` (`$FFFFECA8`) or by a direct call to an event
routine. Everything below is read from the US image; the clone's labels are
navigation only (names in the tables come from its `EventPtrs`/`CutscenePtrs`
lists, which the retail pointer tables at `$05A2B4` (161 entries) and `$05A580`
(34 entries) fix by index).

| Source | How it is read |
|---|---|
| **All writers** | every operand `$ECA8` below `$80000` is classified by the instruction before it: `move.w #imm` (`31FC`), the two register writers (`31C4` at `$058A5C` in `Interaction_GetEvent`, `31C0` at `$058CA8` in `GetEventFromDialogue`), or a read (`3038`, `btst #7`). Any other shape, and any long-form `$FFFFECA8`, fails the census naming its address, so a writer cannot hide. 140 operands below `$80000`. |
| **Map triggers** | the immediates inside the 128 `RunEventsJmpTbl` routines (table at `$0560E8`, `bra.w` entries, located through the `lea` at `$0560CA`; `psiv_tools.newgame.run_events_jmp_tbl`). A routine counts when some map's event list (`psiv_tools.maps.extract_maps`, the per-map `$FF`-terminated id list) names a slot that reaches it. Slots no map lists (`$01`, `$02`, `$05`, `$4D`, ...) fire nothing and are not rows. The custom triggers (`RidingElevator`, the conveyors, the sandworm, the penguin feed, `EnterGrbkTwDoor`) are routines in the same table, so their writes are the same scan. |
| **Interaction areas** | `InteractionRoutines[2]` (`$058836`+4) is `Interaction_GetEvent` (`$058A4C`); its first instruction is the `lea` that names `Interaction_EventIndexes`, and it writes `table[d4]`. `d4` is byte 9 of a map interaction area whose routine byte is 2 (`psiv_tools.maps` interaction areas). The table is read from the image. |
| **Dialogue** | `GetEventFromDialogue` (`$058C9C`) writes the big-endian word after a `$F6` control byte. Every `$F6` in every entry of the 43 decoded trees (`psiv_tools.text.extract_dialogue`). `$F6 $00 $00` fires `Event_NoEvent`, a null routine. |
| **Field input and item actions** | the remaining immediate sites, named in `OTHER_SITES`: the title screen's Start (`$9F`), leaving a vehicle (`$10`, `loc_56084`) and the ITEM-menu actions (Land Rover, Ice Digger and Hydrofoil `$09..$0B`; Pennant and Wood Carving `$97`, `$98`). |
| **Direct calls** | an absolute `jsr`/`jmp`, a `bsr`/`bra` or a PC-relative `jsr`/`jmp` landing exactly on an `EventPtrs`/`CutscenePtrs` routine start. From inside another event's byte range it is a *chain* (`$07` `Event_MotaSpaceportAppearing` from `Cutscene_ZioDefeated`, `$11` `Event_DezoSpaceportAppearing` from `Cutscene_Landale`, `$21` `Event_Recovery` from `Event_RuneHealingChaz`, the `Cutscene_InsideSpaceship` tail jumps): the callee is part of the caller's scene. Elsewhere it is named in `DIRECT_SITES`: the Aiedo inn (`$23`, #39) and the ship-arrival tail jump to `Cutscene_Rykros`. A new call anywhere else fails the census. |
| **Null** | a routine whose first word is `rts` (`4E75`): firing it does nothing, so it needs no scene. |

**Limit.** The scan sees writes that spell the address `$ECA8`. A write through
a pointer register to `$FFFFECA8` would not be seen; no `lea` or long form of
the address exists in the code (the census fails if one appears), and the
cartridge keeps no other route to the field event dispatcher.

## Census

<!-- census:begin -->
| Event | Routine | Reached by | Map or tree | Status |
|---|---|---|---|---|
| `$0000` | `Event_NoEvent` | dialogue `$F6` | tree 7 entry 40, tree 13 entry 70 | null routine (`rts`), no scene needed |
| `$0003` | `Event_AlysFound` | map trigger $03 | PiataAcademy_F1 | scene `Event_AlysFound` |
| `$0004` | `Event_MeetingHahn` | dialogue `$F6` | tree 33 entry 20 | scene `Event_MeetingHahn` |
| `$0006` | `Event_MachineCenterAppearing` | map trigger $06 | Motavia | scene `Event_MachineCenterAppearing` |
| `$0007` | `Event_MotaSpaceportAppearing` | called from $800B |  | inside the caller's scene (`$800B`) |
| `$0008` | `Event_BioPlantDoorOpening` | interaction area $08 | BirthValley_B1 | scene `Event_BioPlantDoorOpening` |
| `$0009` | `Event_BoardingLandRover` | ItemAction_LandRover: the ITEM menu boards the Land Rover |  | scene `Event_BoardingLandRover` |
| `$000A` | `Event_BoardingIceDigger` | ItemAction_IceDigger: the ITEM menu boards the Ice Digger |  | scene `Event_BoardingIceDigger` |
| `$000B` | `Event_BoardingHydrofoil` | ItemAction_Hydrofoil: the ITEM menu boards the Hydrofoil |  | scene `Event_BoardingHydrofoil` |
| `$000C` | `Event_BasementContainers` | map trigger $08 | AcademyBasement_B2 | scene `Event_BasementContainers` |
| `$000D` | `Event_MeetingSaya` | map trigger $16 | KrupKindergarten | scene `Event_MeetingSaya` |
| `$000E` | `Event_AiedoManCrushOnAlys` | dialogue `$F6` | tree 11 entry 7 | allowlisted, #81 |
| `$000F` | `Event_SuspicionOnPrincipal` | map trigger $0A | PiataAcademy_F1 | scene `Event_SuspicionOnPrincipal` |
| `$0010` | `Event_GettingOffVehicle` | field input: Cancel/Start-class press while on a vehicle (`loc_56084`) |  | allowlisted, #81 |
| `$0011` | `Event_DezoSpaceportAppearing` | called from $8010 |  | inside the caller's scene (`$8010`) |
| `$0012` | `Event_BioPlantAlarm` | map trigger $0C | BioPlant_Part2 | scene `Event_BioPlantAlarm` |
| `$0013` | `Event_ElevatorDoorOpening` | interaction area $13 | BioPlant_Part3, BioPlant_B1, BioPlant_B2 (+42 more) | scene `Event_ElevatorDoorOpening` |
| `$0014` | `Event_RidingElevator` | map trigger $0D | BioPlant_B1, BioPlant_B2, BioPlant_B3 (+42 more) | scene `Event_RidingElevator` |
| `$0015` | `Event_VahFortMovingPlatform1` | map trigger $0E | VahalFort_F2 | allowlisted, #82 |
| `$0016` | `Event_VahFortMovingPlatform2` | map trigger $0E | VahalFort_F2 | allowlisted, #82 |
| `$0017` | `Event_WpnPlntMovingPlatform1` | map trigger $0F | WeaponPlant_F1 | allowlisted, #82 |
| `$0018` | `Event_WpnPlntMovingPlatform2` | map trigger $0F | WeaponPlant_F1 | allowlisted, #82 |
| `$0019` | `Event_WpnPlntMovingPlatform3` | map trigger $0F | WeaponPlant_F1 | allowlisted, #82 |
| `$001A` | `Event_WpnPlntMovingPlatform4` | map trigger $0F | WeaponPlant_F1 | allowlisted, #82 |
| `$001B` | `Event_VahalFortTerminal` | interaction area $1B | VahalFort_F2 | allowlisted, #82 |
| `$001C` | `Event_WeaponPlantTerminal` | interaction area $1C | WeaponPlant_F3 | allowlisted, #82 |
| `$001D` | `Event_ConveyorBeltDown` | map trigger $10; map trigger $11 | VahalFort_F2, WeaponPlant_F3 | allowlisted, #82 |
| `$001E` | `Event_ConveyorBeltUp` | map trigger $10; map trigger $11 | VahalFort_F2, WeaponPlant_F3 | allowlisted, #82 |
| `$001F` | `Event_ConveyorBeltRight` | map trigger $10; map trigger $11 | VahalFort_F2, WeaponPlant_F3 | allowlisted, #82 |
| `$0020` | `Event_ConveyorBeltLeft` | map trigger $10; map trigger $11 | VahalFort_F2, WeaponPlant_F3 | allowlisted, #82 |
| `$0021` | `Event_Recovery` | map trigger $12; called from $0067 | AirCastleXeAThoulRoom, AirCastle_F1_Part9, DezoSpaceport (+3 more) | allowlisted, #83 |
| `$0022` | `Event_AiedoManSavings` | dialogue `$F6` | tree 11 entry 24 | allowlisted, #81 |
| `$0023` | `Event_GirlsSneakingOut` | direct call: the Aiedo shop/inn routine (`Event_GirlsSneakingOut`, `ps4.asm` shop code) |  | scene `Event_GirlsSneakingOut` |
| `$0024` | `Event_FaintingPriest` | dialogue `$F6` | tree 9 entry 55 | allowlisted, #81 |
| `$0025` | `Event_AfterIgglanova` | map trigger $13 | AcademyBasement_B2 | scene `Event_AfterIgglanova` |
| `$0026` | `Event_PrincipalConfession` | dialogue `$F6` | tree 33 entry 22 | scene `Event_PrincipalConfession` |
| `$0027` | `Event_RuneFlaeli` | map trigger $30 | ValleyMazeOutside | scene `Event_RuneFlaeli` |
| `$0028` | `Event_AlshlineFound` | map trigger $18 | TonoeBasement_B3 | scene `Event_AlshlineFound` |
| `$0029` | `Event_WreckageEngine` | interaction area $01; dialogue `$F6` | Wreckage_F2_Part4, tree 8 entry 70 | allowlisted, #81 |
| `$002A` | `Event_CancellerReminder` | map trigger $55 | Zelan_F1 | scene `Event_CancellerReminder` |
| `$002B` | `Event_GettingLandRover` | map trigger $1B | MachineCenter_B1_Part2 | scene `Event_GettingLandRover` |
| `$002C` | `Event_MonsenEarthquake` | dialogue `$F6` | tree 12 entry 10 | allowlisted, #81 |
| `$002D` | `Event_PlateSysEngine` | interaction area $02 | PlateSystem_F4 | allowlisted, #81 |
| `$002E` | `Event_RuneLadaeTower` | map trigger $19 | LadeaTower_F2 | scene `Event_RuneLadaeTower` |
| `$002F` | `Event_PsycoWandChest` | map trigger $2A | LadeaTower_F5 | scene `Event_PsycoWandChest` |
| `$0030` | `Event_ZioFortBarrier` | dialogue `$F6` | tree 13 entry 73 | scene `Event_ZioFortBarrier` |
| `$0031` | `Event_ZioFanatic` | dialogue `$F6` | tree 9 entry 32 | allowlisted, #81 |
| `$0032` | `Event_MeetingDorin` | dialogue `$F6` | tree 7 entry 23 | scene `Event_MeetingDorin` |
| `$0033` | `Event_TonoeBasementDoor` | dialogue `$F6` | tree 7 entry 41 | scene `Event_TonoeBasementDoor` |
| `$0034` | `Event_ZioNurvus` | map trigger $1F | Nurvus_B4_Part2 | scene `Event_ZioNurvus` |
| `$0035` | `Event_GaruberkTwDoorOpening1` | interaction area $03 | GaruberkTower, GaruberkTower_Part2, GaruberkTower_Part3 (+3 more) | allowlisted, #83 |
| `$0036` | `Event_GaruberkTwDoorOpening2` | interaction area $04 | GaruberkTower_Part2, GaruberkTower_Part3, GaruberkTower_Part4 (+3 more) | allowlisted, #83 |
| `$0037` | `Event_GaruberkTwDoorEntered1` | map trigger $22 | GaruberkTower, GaruberkTower_Part2, GaruberkTower_Part3 (+4 more) | allowlisted, #56 |
| `$0038` | `Event_GaruberkTwDoorEntered2` | map trigger $22 | GaruberkTower, GaruberkTower_Part2, GaruberkTower_Part3 (+4 more) | allowlisted, #56 |
| `$0039` | `Event_GaruberkTwEyeAction1` | interaction area $05 | GaruberkTower_Part2 | allowlisted, #83 |
| `$003A` | `Event_GaruberkTwEyeAction2` | interaction area $06 | GaruberkTower_Part5 | allowlisted, #83 |
| `$003B` | `Event_ChazHouse` | map trigger $26 | ChazHouse | scene `Event_ChazHouse` |
| `$003C` | `Event_LeavingChazHouse` | map trigger $27 | Aiedo | scene `Event_LeavingChazHouse` |
| `$003D` | `Event_KuranArrival` | map trigger $2C | Kuran | scene `Event_KuranArrival` |
| `$003E` | `Event_NearDarkForce1` | map trigger $2D | Kuran_F3 | scene `Event_NearDarkForce1` |
| `$003F` | `Event_DarkForce1` | map trigger $2E | Kuran_F3 | scene `Event_DarkForce1` |
| `$0040` | `Event_Juza` | dialogue `$F6` | tree 13 entry 71 | scene `Event_Juza` |
| `$0041` | `Event_JuzaDefeated` | map trigger $2F | ZioFortJuzaRoom | scene `Event_JuzaDefeated` |
| `$0042` | `Event_SilenceTmDoorOpening` | interaction area $07 | LeRoofRoom | allowlisted, #83 |
| `$0043` | `Event_OutsideRajaTemple` | map trigger $31 | Dezolis | scene `Event_OutsideRajaTemple` |
| `$0044` | `Event_TylerGraveOpening` | dialogue `$F6` | tree 14 entry 35 | scene `Event_TylerGraveOpening` |
| `$0045` | `Event_PersistentEsperGuards` | dialogue `$F6` | tree 20 entry 52, tree 21 entry 52 | scene `Event_PersistentEsperGuards` |
| `$0046` | `Event_EclipseTorchStolen` | dialogue `$F6` | tree 38 entry 53 | scene `Event_EclipseTorchStolen` |
| `$0047` | `Event_EclipseTorchUsed` | map trigger $36 | Dezolis | scene `Event_EclipseTorchUsed` |
| `$0048` | `Event_MeetingLeRoof` | map trigger $33 | LeRoofRoom | scene `Event_MeetingLeRoof` |
| `$0049` | `Event_MuskCatsGuarding` | dialogue `$F6` | tree 17 entry 48 | allowlisted, #83 |
| `$004A` | `Event_MuskCatElder` | dialogue `$F6` | tree 17 entry 44 | allowlisted, #83 |
| `$004B` | `Event_PenguinOwner` | dialogue `$F6` | tree 17 entry 17 | allowlisted, #83 |
| `$004C` | `Event_CarnivorousTrees` | map trigger $35 | Dezolis | scene `Event_CarnivorousTrees` |
| `$004D` | `Event_SavingKyra` | map trigger $35 | Dezolis | scene `Event_SavingKyra` |
| `$004E` | `Event_DarkForce2` | map trigger $37 | GaruberkTower_Part7 | scene `Event_DarkForce2` |
| `$004F` | `Event_EsperGuardPermission` | dialogue `$F6` | tree 20 entry 51, tree 21 entry 51 | scene `Event_EsperGuardPermission` |
| `$0050` | `Event_DarkForce3Defeated` | map trigger $3C | Motavia, SoldiersTempleOutside | scene `Event_DarkForce3Defeated` |
| `$0051` | `Event_InnerSanctGuard` | dialogue `$F6` | tree 20 entry 36 | allowlisted, #83 |
| `$0052` | `Event_InnerSanctGuardBeforeElsydeon` | dialogue `$F6` | tree 20 entry 39 | allowlisted, #83 |
| `$0053` | `Event_ReshelBattle` | map trigger $3D | Reshel1 | scene `Event_ReshelBattle` |
| `$0054` | `Event_ClmCenterForcedBattle` | map trigger $3E | ClimCenter | scene `Event_ClmCenterForcedBattle` |
| `$0055` | `Event_ClmCenterAfterBattle` | map trigger $3F | ClimCenter | scene `Event_ClmCenterAfterBattle` |
| `$0056` | `Event_DElmLars` | map trigger $40 | ClimCenter_F3 | scene `Event_DElmLars` |
| `$0057` | `Event_AfterDElmLarsBattle` | map trigger $41 | ClimCenter_F3 | scene `Event_AfterDElmLarsBattle` |
| `$0058` | `Event_AirCastleArrival` | map trigger $43 | AirCastle | scene `Event_AirCastleArrival` |
| `$0059` | `Event_XeAThoulBeforeBattle` | map trigger $44 | AirCastleXeAThoulRoom | scene `Event_XeAThoulBeforeBattle` |
| `$005A` | `Event_AirCastleFakeChest` | map trigger $45 | AirCastleInner_B1_Part3 | scene `Event_AirCastleFakeChest` |
| `$005B` | `Event_RajaSick` | dialogue `$F6` | tree 18 entry 101 | allowlisted, #83 |
| `$005C` | `Event_Gyuna` | dialogue `$F6` | tree 16 entry 48 | scene `Event_Gyuna` |
| `$005D` | `Event_LashiecAppearance` | map trigger $46 | AirCastleInner_B1_Part3 | scene `Event_LashiecAppearance` |
| `$005E` | `Event_StrengthTowerTop` | map trigger $49 | StrengthTower_F4 | scene `Event_StrengthTowerTop` |
| `$005F` | `Event_CourageTowerTop` | map trigger $4A | CourageTower_F4 | scene `Event_CourageTowerTop` |
| `$0060` | `Event_DeVars` | dialogue `$F6` | tree 41 entry 13 | scene `Event_DeVars` |
| `$0061` | `Event_SaLews` | dialogue `$F6` | tree 41 entry 14 | scene `Event_SaLews` |
| `$0062` | `Event_AngerTowerAlys` | dialogue `$F6` | tree 41 entry 15 | allowlisted, #71 |
| `$0063` | `Event_ReFaze` | map trigger $4E | AngerTower_F2 | scene `Event_ReFaze` |
| `$0064` | `Event_DeVarsDefeated` | map trigger $4B | StrengthTower_F4 | scene `Event_DeVarsDefeated` |
| `$0065` | `Event_SaLewsDefeated` | map trigger $4C | CourageTower_F4 | scene `Event_SaLewsDefeated` |
| `$0066` | `Event_HuntersGuild` | interaction area $09 | HuntersGuild | allowlisted, #81 |
| `$0067` | `Event_RuneHealingChaz` | dialogue `$F6` | tree 20 entry 55 | allowlisted, #83 |
| `$0068` | `Event_PickingFifthCharacter` | dialogue `$F6` | tree 39 entry 25 | allowlisted, #83 |
| `$0069` | `Event_AngerTowerTop` | map trigger $51 | AngerTower_F1 | scene `Event_AngerTowerTop` |
| `$006A` | `Event_AngerTowerExitTop` | map trigger $52 | AngerTower_F2 | scene `Event_AngerTowerExitTop` |
| `$006B` | `Event_IgglanovaBattle` | interaction area $0B | AcademyBasement_B2 | scene `Event_IgglanovaBattle` |
| `$006C` | `Event_Phonon` | interaction area $0C | PlateSystem_F3 | allowlisted, #81 |
| `$006D` | `Event_Hijammer` | interaction area $0D | Kuran_F1_Part2 | allowlisted, #83 |
| `$006E` | `Event_Burstroc` | interaction area $0E | WeaponPlant_F2 | allowlisted, #82 |
| `$006F` | `Event_PosiBolt` | interaction area $0F | VahalFort_F3 | allowlisted, #82 |
| `$0070` | `Event_RanchOwner` | dialogue `$F6` | tree 3 entry 19 | allowlisted, #81 |
| `$0071` | `Event_MileSandWormBattle` | map trigger $70 | Mile | scene `Event_MileSandWormBattle` |
| `$0072` | `Event_RanchOwnerAfterBattle` | dialogue `$F6` | tree 3 entry 21 | allowlisted, #81 |
| `$0073` | `Event_RockyFound` | dialogue `$F6` | tree 12 entry 40 | allowlisted, #81 |
| `$0074` | `Event_CatchingRocky` | dialogue `$F6` | tree 5 entry 52, tree 6 entry 52, tree 12 entry 41 (+2 more) | allowlisted, #81 |
| `$0075` | `Event_RockyOwner` | dialogue `$F6` | tree 11 entry 68 | allowlisted, #81 |
| `$0076` | `Event_PiataDormOwner` | dialogue `$F6` | tree 2 entry 89 | allowlisted, #81 |
| `$0077` | `Event_MissingStudentFound` | dialogue `$F6` | tree 9 entry 75 | allowlisted, #81 |
| `$0078` | `Event_OldManNearMissingStudent` | dialogue `$F6` | tree 9 entry 78 | allowlisted, #81 |
| `$0079` | `Event_MissingStudentInBed` | dialogue `$F6` | tree 9 entry 81 | allowlisted, #81 |
| `$007A` | `Event_TallasMom` | dialogue `$F6` | tree 12 entry 28 | allowlisted, #81 |
| `$007B` | `Event_TallasMomAfterRescue` | dialogue `$F6` | tree 12 entry 30 | allowlisted, #81 |
| `$007C` | `Event_InsideMonsenHole` | map trigger $71 | MonsenCave | allowlisted, #81 |
| `$007D` | `Event_FractOozeFound` | dialogue `$F6` | tree 12 entry 38 | scene `Event_FractOozeFound` |
| `$007E` | `Event_TallasRescued` | map trigger $72 | MonsenCave | allowlisted, #81 |
| `$007F` | `Event_LostGirlsMother` | dialogue `$F6` | tree 25 entry 13 | allowlisted, #81 |
| `$0080` | `Event_GirlPrison` | dialogue `$F6` | tree 11 entry 89 | allowlisted, #81 |
| `$0081` | `Event_GirlsBail` | dialogue `$F6` | tree 11 entry 79 | allowlisted, #81 |
| `$0082` | `Event_PayingGirlsBail` | dialogue `$F6` | tree 11 entry 96 | allowlisted, #81 |
| `$0083` | `Event_GirlsBailedOut` | dialogue `$F6` | tree 11 entry 91 | allowlisted, #81 |
| `$0084` | `Event_DyingBoyFather` | dialogue `$F6` | tree 24 entry 41 | allowlisted, #81 |
| `$0085` | `Event_FatherAfterBoyRecovery` | dialogue `$F6` | tree 24 entry 43 | allowlisted, #81 |
| `$0086` | `Event_DyingBoy` | dialogue `$F6` | tree 24 entry 50 | allowlisted, #81 |
| `$0087` | `Event_MeetingSekreas` | dialogue `$F6` | tree 24 entry 37 | allowlisted, #81 |
| `$0088` | `Event_KingRappy` | dialogue `$F6` | tree 24 entry 61, tree 25 entry 61 | scene `Event_KingRappy` |
| `$0089` | `Event_KingRappyDefeated` | map trigger $73 | RappyCave | allowlisted, #81 |
| `$008A` | `Event_ZemaServantBattle` | map trigger $74 | Zema | scene `Event_ZemaServantBattle` |
| `$008B` | `Event_ZemaOldMan` | dialogue `$F6` | tree 4 entry 76 | scene `Event_ZemaOldMan` |
| `$008C` | `Event_ZemaOldManAfterMission` | dialogue `$F6` | tree 4 entry 78 | scene `Event_ZemaOldManAfterMission` |
| `$008D` | `Event_VahalFortEntrance` | map trigger $75 | VahalFort | allowlisted, #82 |
| `$008E` | `Event_VahalFortMidway` | map trigger $76 | VahalFort_F2 | allowlisted, #82 |
| `$008F` | `Event_DaughterTerminal` | dialogue `$F6` | tree 43 entry 10 | scene `Event_DaughterTerminal` |
| `$0090` | `Event_VahalFortBarrier` | dialogue `$F6` | tree 43 entry 9 | allowlisted, #82 |
| `$0091` | `Event_DominatorsDefeated` | map trigger $77 | VahalFort_F3 | allowlisted, #82 |
| `$0092` | `Event_WeaponPlantArrival` | map trigger $78 | WeaponPlant | allowlisted, #82 |
| `$0093` | `Event_MeeseClinicSickWoman` | dialogue `$F6` | tree 18 entry 102 | allowlisted, #81 |
| `$0094` | `Event_StrippersAppearing` | map trigger $79 | StripClub | allowlisted, #81 |
| `$0095` | `Event_ExitingStripClub` | map trigger $7A | HuntersGuild | allowlisted, #81 |
| `$0096` | `Event_PenguinFeedStolen` | map trigger $7B | ZosaItemShop | allowlisted, #56 |
| `$0097` | `Event_Pennant` | ItemAction_Pennant: ITEM menu, Chaz's house only |  | allowlisted, #81 |
| `$0098` | `Event_WoodCarving` | ItemAction_WoodCarvin: ITEM menu, Chaz's house only |  | allowlisted, #81 |
| `$0099` | `Event_EnterAngerTower` | map trigger $56 | AngerTower | allowlisted, #83 |
| `$009A` | `Event_SoldiersTempleCaveDialogue1` | map trigger $57 | IslandCave_F1 | allowlisted, #81 |
| `$009B` | `Event_SoldiersTempleCaveDialogue2` | map trigger $58 | IslandCave_F3 | allowlisted, #81 |
| `$009C` | `Event_SoldiersTempleReached` | map trigger $59 | SoldiersTempleOutside | allowlisted, #81 |
| `$009D` | `Event_AeroPrismFound` | map trigger $5A | SoldiersTemple | allowlisted, #81 |
| `$009E` | `Event_PiataGuardsReprimand` | map trigger $5B | Motavia | scene `Event_PiataGuardsReprimand` |
| `$009F` | `Event_GameStart` | Title_StartOption: Start on the title screen |  | scene `Event_GameStart` |
| `$00A0` | `Event_PiataChazAlone` | map trigger $7C | PiataAcademy_F1 | scene `Event_PiataChazAlone` |
| `$8001` | `Cutscene_PiataPrincipal` | dialogue `$F6` | tree 33 entry 21 | scene `Cutscene_PiataPrincipal` |
| `$8002` | `Cutscene_ProfHolt` | dialogue `$F6` | tree 3 entry 106 | scene `Cutscene_ProfHolt` |
| `$8003` | `Cutscene_MeetingRune` | dialogue `$F6` | tree 7 entry 2 | scene `Cutscene_MeetingRune` |
| `$8004` | `Cutscene_Dorin` | map trigger $14 | TonoeGryzHouse | scene `Cutscene_Dorin` |
| `$8005` | `Cutscene_Alshline` | map trigger $15 | Zema | scene `Cutscene_Alshline` |
| `$8006` | `Cutscene_ZemaIgglanovaDefeated` | map trigger $17 | Zema | scene `Cutscene_ZemaIgglanovaDefeated` |
| `$8007` | `Cutscene_MeetingRika` | map trigger $1A | BioPlant_B4_Part2 | scene `Cutscene_MeetingRika` |
| `$8008` | `Cutscene_DemiRescue` | map trigger $1C | ZioFort_F4 | scene `Cutscene_DemiRescue` |
| `$8009` | `Cutscene_AlysWounded` | map trigger $1D | ZioFort_F4 | scene `Cutscene_AlysWounded` |
| `$800A` | `Cutscene_PsycoWand` | map trigger $1E | LadeaTower_F5 | scene `Cutscene_PsycoWand` |
| `$800B` | `Cutscene_ZioDefeated` | map trigger $20 | Nurvus_B4_Part2 | scene `Cutscene_ZioDefeated` |
| `$800C` | `Cutscene_MeetingWren` | dialogue `$F6` | tree 35 entry 9 | scene `Cutscene_MeetingWren` |
| `$800D` | `Cutscene_InsideSpaceship` | map trigger $21; map trigger $23; map trigger $24; map trigger $25; called from $8015; called from $801C | DezoSpaceport, MotaSpaceport, Kuran (+3 more) | scene `Cutscene_InsideSpaceship` |
| `$800E` | `Cutscene_SpaceshipSabotage` | map trigger $28 | Zelan | scene `Cutscene_SpaceshipSabotage` |
| `$800F` | `Cutscene_CrashLaanding` | map trigger $29 | ZelanSpace | scene `Cutscene_CrashLaanding` |
| `$8010` | `Cutscene_Landale` | map trigger $2B | Hangar | scene `Cutscene_Landale` |
| `$8011` | `Cutscene_DarkForce1Defeated` | map trigger $32 | Kuran_F3 | scene `Cutscene_DarkForce1Defeated` |
| `$8012` | `Cutscene_RajaSick` | dialogue `$F6` | tree 18 entry 100 | scene `Cutscene_RajaSick` |
| `$8013` | `Cutscene_MeetingKyra` | map trigger $35 | Dezolis | scene `Cutscene_MeetingKyra` |
| `$8014` | `Cutscene_LutzRevelation` | map trigger $39 | InnerSanctuary | scene `Cutscene_LutzRevelation` |
| `$8015` | `Cutscene_FindingAirCastle` | map trigger $42 | DezoSpaceport | scene `Cutscene_FindingAirCastle` |
| `$8016` | `Cutscene_LashiecDefeated` | map trigger $47 | AirCastleInner_B1_Part3 | scene `Cutscene_LashiecDefeated` |
| `$8017` | `Cutscene_DarkForce2Defeated` | map trigger $38 | GaruberkTower_Part7 | scene `Cutscene_DarkForce2Defeated` |
| `$8018` | `Cutscene_GumbiousBishop` | map trigger $48 | Gumbious_B2_Part2 | scene `Cutscene_GumbiousBishop` |
| `$8019` | `Cutscene_MeetingSeth` | map trigger $3A | Motavia | scene `Cutscene_MeetingSeth` |
| `$801A` | `Cutscene_AeroPrism` | map trigger $3B | Motavia, SoldiersTempleOutside | scene `Cutscene_AeroPrism` |
| `$801B` | `Cutscene_Rykros` | direct call: the ship-arrival routine's tail jump to `Cutscene_Rykros` |  | scene `Cutscene_Rykros` |
| `$801C` | `Cutscene_LeRoofAgain` | map trigger $34 | LeRoofRoom | scene `Cutscene_LeRoofAgain` |
| `$801D` | `Cutscene_BeforeElsydeonCave` | map trigger $4F | InnerSanctuary_B1 | scene `Cutscene_BeforeElsydeonCave` |
| `$801E` | `Cutscene_Elsydeon` | interaction area $0A | ElsydeonCave_B1 | scene `Cutscene_Elsydeon` |
| `$801F` | `Cutscene_Reunion` | map trigger $50 | DezoSpaceport | scene `Cutscene_Reunion` |
| `$8020` | `Cutscene_ProfoundDarkness` | map trigger $53 | TheEdge_Part9 | scene `Cutscene_ProfoundDarkness` |
| `$8021` | `Cutscene_Ending` | map trigger $54 | TheEdge_Part9 | scene `Cutscene_Ending` |
<!-- census:end -->

## Allowlist by tracking issue

Each allowlisted event has exactly one of these five issues, so a route-chapter
lane that reaches an area has its own worklist. A scene that lands removes its
entry and lowers `ALLOWLIST_CEILING` in `tests/event_census.py` and
`rust/psiv-core/tests/event_census.rs`.

<!-- counts:begin -->
| Issue | Area | Allowlisted events |
|---:|---|---:|
| #56 | trigger-fired events | 3 |
| #71 | `Event_GetAndRunDialogue2` callers | 1 |
| #81 | Motavia side content | 42 |
| #82 | Vahal Fort and Weapon Plant | 19 |
| #83 | Dezolis late arc | 16 |
| | total | 81 |
<!-- counts:end -->

- [#56](https://github.com/TusanHomichi/PSIV/issues/56): trigger-fired events
  (`$71`, `$37`, `$38`, `$96`, `$2A`).
- [#71](https://github.com/TusanHomichi/PSIV/issues/71): the four
  `Event_GetAndRunDialogue2` callers (`$62`, `$7D`, `$88`, `$8F`).
- [#81](https://github.com/TusanHomichi/PSIV/issues/81): Motavia side content.
- [#82](https://github.com/TusanHomichi/PSIV/issues/82): Vahal Fort and Weapon Plant.
- [#83](https://github.com/TusanHomichi/PSIV/issues/83): Dezolis late arc,
  including the Dezolis dialogue controls.

## Why the remaining #56 and #71 entries are not transcribed

Each needs something the scene vocabulary does not have, and each is off the route to Kuran:

| Event | Blocker |
|---|---|
| `$37`, `$38` `Event_GaruberkTwDoorEntered1/2` (`$06F5A4`, `$06F724`, #56) | The body resolves layout cells from the leader's live position (`GetMapLayoutOffset` with `Character_1.x >> 5`, `y - $10`), writes a chunk pair, then plays a table-driven BG animation with `RefreshPlaneBG` per frame and a `tst.w $2A(a4)` walk loop. `WriteMapChunks` takes literal coordinates; the live-position write, the table walk and the map-update loop need new ops (Garuberk Tower, [#83](https://github.com/TusanHomichi/PSIV/issues/83)'s area). |
| `$96` `Event_PenguinFeedStolen` (`$0735F6`, #56) | Builds a temporary thief object in slot `$C380` (map object 2 of Zosa's item shop, which has two objects), steps it with `Event_MoveSingleObject` toward the leader's live X, then clears it. The scene cast holds party members and map objects by index; a temporary object has no `ActorRef`. |
| `$62` `Event_AngerTowerAlys` (`$07148A`, #71) | Steps `Character_1` (the leader) and a map object with `Event_StepObject` along a direction table. `DriftNpcs` moves map objects only; the leader's position is the party driver's. |

Two entries sit in an area only by closest fit: `$10` (`Event_GettingOffVehicle`,
a field-input event with no map) is under #81, and `$21` (`Event_Recovery`, the
generic recovery tile, whose maps are Dezolis late-arc rooms and the
spaceports) is under #83.
