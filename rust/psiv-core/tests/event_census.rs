//! The event census against the scene registry.
//!
//! `tests/event_census.py` derives, from the US image, every event index the
//! cartridge can fire (`Event_Index` writes, interaction areas, dialogue `$F6`
//! controls, direct calls) and `tests/test_event_coverage.py` holds the table
//! below equal to that derivation. This test holds the same table against the
//! live registry (`scene_for`, `SCENES`), so the three stay one list:
//!
//! - a `Scene` row must be registered;
//! - a row with a scene registered may not stay allowlisted (the allowlist
//!   only shrinks);
//! - an `InsideScene` row (an event only ever called from inside another
//!   event's routine) needs that caller registered;
//! - a fired event with neither scene nor allowlist entry has no valid row: the
//!   generator writes `Disposition::Missing`, which does not compile;
//! - a registered scene the census never finds is a failure too: the census,
//!   not the registry, is the list of what the cartridge can fire.
//!
//! The table is generated: `python3 -m tests.event_census --rust`. See
//! `docs/scenes/EVENT_COVERAGE.md` for how each source is read.

use psiv_core::{EventIndex, SCENES, scene_for};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Disposition {
    /// A scene is registered for the event.
    Scene,
    /// The routine is a bare `rts`; firing it does nothing.
    Null,
    /// Only ever called from inside the scene of this other event.
    InsideScene(u16),
    /// No scene yet; tracked by this GitHub issue.
    Allowlisted(u32),
}

use Disposition::{Allowlisted, InsideScene, Null, Scene};

/// The tracking issues an allowlist entry may name: #56 trigger-fired events,
/// #71 `Event_GetAndRunDialogue2` callers, #81 Motavia side content, #82 Vahal
/// Fort and Weapon Plant, #83 Dezolis late arc.
const ISSUES: [u32; 5] = [56, 71, 81, 82, 83];

/// The most the allowlist may hold. Lower it when a scene lands; never raise it.
const ALLOWLIST_CEILING: usize = 51;

#[rustfmt::skip]
const CENSUS: &[(u16, &str, Disposition)] = &[
    // census:begin
    (0x0000, "Event_NoEvent", Disposition::Null),
    (0x0003, "Event_AlysFound", Disposition::Scene),
    (0x0004, "Event_MeetingHahn", Disposition::Scene),
    (0x0006, "Event_MachineCenterAppearing", Disposition::Scene),
    (0x0007, "Event_MotaSpaceportAppearing", Disposition::InsideScene(0x800B)),
    (0x0008, "Event_BioPlantDoorOpening", Disposition::Scene),
    (0x0009, "Event_BoardingLandRover", Disposition::Scene),
    (0x000A, "Event_BoardingIceDigger", Disposition::Scene),
    (0x000B, "Event_BoardingHydrofoil", Disposition::Scene),
    (0x000C, "Event_BasementContainers", Disposition::Scene),
    (0x000D, "Event_MeetingSaya", Disposition::Scene),
    (0x000E, "Event_AiedoManCrushOnAlys", Disposition::Allowlisted(81)),
    (0x000F, "Event_SuspicionOnPrincipal", Disposition::Scene),
    (0x0010, "Event_GettingOffVehicle", Disposition::Allowlisted(81)),
    (0x0011, "Event_DezoSpaceportAppearing", Disposition::InsideScene(0x8010)),
    (0x0012, "Event_BioPlantAlarm", Disposition::Scene),
    (0x0013, "Event_ElevatorDoorOpening", Disposition::Scene),
    (0x0014, "Event_RidingElevator", Disposition::Scene),
    (0x0015, "Event_VahFortMovingPlatform1", Disposition::Scene),
    (0x0016, "Event_VahFortMovingPlatform2", Disposition::Scene),
    (0x0017, "Event_WpnPlntMovingPlatform1", Disposition::Scene),
    (0x0018, "Event_WpnPlntMovingPlatform2", Disposition::Scene),
    (0x0019, "Event_WpnPlntMovingPlatform3", Disposition::Scene),
    (0x001A, "Event_WpnPlntMovingPlatform4", Disposition::Scene),
    (0x001B, "Event_VahalFortTerminal", Disposition::Scene),
    (0x001C, "Event_WeaponPlantTerminal", Disposition::Scene),
    (0x001D, "Event_ConveyorBeltDown", Disposition::Scene),
    (0x001E, "Event_ConveyorBeltUp", Disposition::Scene),
    (0x001F, "Event_ConveyorBeltRight", Disposition::Scene),
    (0x0020, "Event_ConveyorBeltLeft", Disposition::Scene),
    (0x0021, "Event_Recovery", Disposition::Scene),
    (0x0022, "Event_AiedoManSavings", Disposition::Allowlisted(81)),
    (0x0023, "Event_GirlsSneakingOut", Disposition::Scene),
    (0x0024, "Event_FaintingPriest", Disposition::Allowlisted(81)),
    (0x0025, "Event_AfterIgglanova", Disposition::Scene),
    (0x0026, "Event_PrincipalConfession", Disposition::Scene),
    (0x0027, "Event_RuneFlaeli", Disposition::Scene),
    (0x0028, "Event_AlshlineFound", Disposition::Scene),
    (0x0029, "Event_WreckageEngine", Disposition::Allowlisted(81)),
    (0x002A, "Event_CancellerReminder", Disposition::Scene),
    (0x002B, "Event_GettingLandRover", Disposition::Scene),
    (0x002C, "Event_MonsenEarthquake", Disposition::Allowlisted(81)),
    (0x002D, "Event_PlateSysEngine", Disposition::Allowlisted(81)),
    (0x002E, "Event_RuneLadaeTower", Disposition::Scene),
    (0x002F, "Event_PsycoWandChest", Disposition::Scene),
    (0x0030, "Event_ZioFortBarrier", Disposition::Scene),
    (0x0031, "Event_ZioFanatic", Disposition::Allowlisted(81)),
    (0x0032, "Event_MeetingDorin", Disposition::Scene),
    (0x0033, "Event_TonoeBasementDoor", Disposition::Scene),
    (0x0034, "Event_ZioNurvus", Disposition::Scene),
    (0x0035, "Event_GaruberkTwDoorOpening1", Disposition::Scene),
    (0x0036, "Event_GaruberkTwDoorOpening2", Disposition::Scene),
    (0x0037, "Event_GaruberkTwDoorEntered1", Disposition::Scene),
    (0x0038, "Event_GaruberkTwDoorEntered2", Disposition::Scene),
    (0x0039, "Event_GaruberkTwEyeAction1", Disposition::Scene),
    (0x003A, "Event_GaruberkTwEyeAction2", Disposition::Scene),
    (0x003B, "Event_ChazHouse", Disposition::Scene),
    (0x003C, "Event_LeavingChazHouse", Disposition::Scene),
    (0x003D, "Event_KuranArrival", Disposition::Scene),
    (0x003E, "Event_NearDarkForce1", Disposition::Scene),
    (0x003F, "Event_DarkForce1", Disposition::Scene),
    (0x0040, "Event_Juza", Disposition::Scene),
    (0x0041, "Event_JuzaDefeated", Disposition::Scene),
    (0x0042, "Event_SilenceTmDoorOpening", Disposition::Allowlisted(83)),
    (0x0043, "Event_OutsideRajaTemple", Disposition::Scene),
    (0x0044, "Event_TylerGraveOpening", Disposition::Scene),
    (0x0045, "Event_PersistentEsperGuards", Disposition::Scene),
    (0x0046, "Event_EclipseTorchStolen", Disposition::Scene),
    (0x0047, "Event_EclipseTorchUsed", Disposition::Scene),
    (0x0048, "Event_MeetingLeRoof", Disposition::Scene),
    (0x0049, "Event_MuskCatsGuarding", Disposition::Allowlisted(83)),
    (0x004A, "Event_MuskCatElder", Disposition::Allowlisted(83)),
    (0x004B, "Event_PenguinOwner", Disposition::Allowlisted(83)),
    (0x004C, "Event_CarnivorousTrees", Disposition::Scene),
    (0x004D, "Event_SavingKyra", Disposition::Scene),
    (0x004E, "Event_DarkForce2", Disposition::Scene),
    (0x004F, "Event_EsperGuardPermission", Disposition::Scene),
    (0x0050, "Event_DarkForce3Defeated", Disposition::Scene),
    (0x0051, "Event_InnerSanctGuard", Disposition::Allowlisted(83)),
    (0x0052, "Event_InnerSanctGuardBeforeElsydeon", Disposition::Allowlisted(83)),
    (0x0053, "Event_ReshelBattle", Disposition::Scene),
    (0x0054, "Event_ClmCenterForcedBattle", Disposition::Scene),
    (0x0055, "Event_ClmCenterAfterBattle", Disposition::Scene),
    (0x0056, "Event_DElmLars", Disposition::Scene),
    (0x0057, "Event_AfterDElmLarsBattle", Disposition::Scene),
    (0x0058, "Event_AirCastleArrival", Disposition::Scene),
    (0x0059, "Event_XeAThoulBeforeBattle", Disposition::Scene),
    (0x005A, "Event_AirCastleFakeChest", Disposition::Scene),
    (0x005B, "Event_RajaSick", Disposition::Allowlisted(83)),
    (0x005C, "Event_Gyuna", Disposition::Scene),
    (0x005D, "Event_LashiecAppearance", Disposition::Scene),
    (0x005E, "Event_StrengthTowerTop", Disposition::Scene),
    (0x005F, "Event_CourageTowerTop", Disposition::Scene),
    (0x0060, "Event_DeVars", Disposition::Scene),
    (0x0061, "Event_SaLews", Disposition::Scene),
    (0x0062, "Event_AngerTowerAlys", Disposition::Allowlisted(71)),
    (0x0063, "Event_ReFaze", Disposition::Scene),
    (0x0064, "Event_DeVarsDefeated", Disposition::Scene),
    (0x0065, "Event_SaLewsDefeated", Disposition::Scene),
    (0x0066, "Event_HuntersGuild", Disposition::Allowlisted(81)),
    (0x0067, "Event_RuneHealingChaz", Disposition::Allowlisted(83)),
    (0x0068, "Event_PickingFifthCharacter", Disposition::Allowlisted(83)),
    (0x0069, "Event_AngerTowerTop", Disposition::Scene),
    (0x006A, "Event_AngerTowerExitTop", Disposition::Scene),
    (0x006B, "Event_IgglanovaBattle", Disposition::Scene),
    (0x006C, "Event_Phonon", Disposition::Allowlisted(81)),
    (0x006D, "Event_Hijammer", Disposition::Allowlisted(83)),
    (0x006E, "Event_Burstroc", Disposition::Scene),
    (0x006F, "Event_PosiBolt", Disposition::Scene),
    (0x0070, "Event_RanchOwner", Disposition::Allowlisted(81)),
    (0x0071, "Event_MileSandWormBattle", Disposition::Scene),
    (0x0072, "Event_RanchOwnerAfterBattle", Disposition::Allowlisted(81)),
    (0x0073, "Event_RockyFound", Disposition::Allowlisted(81)),
    (0x0074, "Event_CatchingRocky", Disposition::Allowlisted(81)),
    (0x0075, "Event_RockyOwner", Disposition::Allowlisted(81)),
    (0x0076, "Event_PiataDormOwner", Disposition::Allowlisted(81)),
    (0x0077, "Event_MissingStudentFound", Disposition::Allowlisted(81)),
    (0x0078, "Event_OldManNearMissingStudent", Disposition::Allowlisted(81)),
    (0x0079, "Event_MissingStudentInBed", Disposition::Allowlisted(81)),
    (0x007A, "Event_TallasMom", Disposition::Allowlisted(81)),
    (0x007B, "Event_TallasMomAfterRescue", Disposition::Allowlisted(81)),
    (0x007C, "Event_InsideMonsenHole", Disposition::Allowlisted(81)),
    (0x007D, "Event_FractOozeFound", Disposition::Scene),
    (0x007E, "Event_TallasRescued", Disposition::Allowlisted(81)),
    (0x007F, "Event_LostGirlsMother", Disposition::Allowlisted(81)),
    (0x0080, "Event_GirlPrison", Disposition::Allowlisted(81)),
    (0x0081, "Event_GirlsBail", Disposition::Allowlisted(81)),
    (0x0082, "Event_PayingGirlsBail", Disposition::Allowlisted(81)),
    (0x0083, "Event_GirlsBailedOut", Disposition::Allowlisted(81)),
    (0x0084, "Event_DyingBoyFather", Disposition::Allowlisted(81)),
    (0x0085, "Event_FatherAfterBoyRecovery", Disposition::Allowlisted(81)),
    (0x0086, "Event_DyingBoy", Disposition::Allowlisted(81)),
    (0x0087, "Event_MeetingSekreas", Disposition::Allowlisted(81)),
    (0x0088, "Event_KingRappy", Disposition::Scene),
    (0x0089, "Event_KingRappyDefeated", Disposition::Allowlisted(81)),
    (0x008A, "Event_ZemaServantBattle", Disposition::Scene),
    (0x008B, "Event_ZemaOldMan", Disposition::Scene),
    (0x008C, "Event_ZemaOldManAfterMission", Disposition::Scene),
    (0x008D, "Event_VahalFortEntrance", Disposition::Scene),
    (0x008E, "Event_VahalFortMidway", Disposition::Scene),
    (0x008F, "Event_DaughterTerminal", Disposition::Scene),
    (0x0090, "Event_VahalFortBarrier", Disposition::Scene),
    (0x0091, "Event_DominatorsDefeated", Disposition::Scene),
    (0x0092, "Event_WeaponPlantArrival", Disposition::Scene),
    (0x0093, "Event_MeeseClinicSickWoman", Disposition::Allowlisted(81)),
    (0x0094, "Event_StrippersAppearing", Disposition::Allowlisted(81)),
    (0x0095, "Event_ExitingStripClub", Disposition::Allowlisted(81)),
    (0x0096, "Event_PenguinFeedStolen", Disposition::Allowlisted(56)),
    (0x0097, "Event_Pennant", Disposition::Allowlisted(81)),
    (0x0098, "Event_WoodCarving", Disposition::Allowlisted(81)),
    (0x0099, "Event_EnterAngerTower", Disposition::Allowlisted(83)),
    (0x009A, "Event_SoldiersTempleCaveDialogue1", Disposition::Scene),
    (0x009B, "Event_SoldiersTempleCaveDialogue2", Disposition::Scene),
    (0x009C, "Event_SoldiersTempleReached", Disposition::Scene),
    (0x009D, "Event_AeroPrismFound", Disposition::Scene),
    (0x009E, "Event_PiataGuardsReprimand", Disposition::Scene),
    (0x009F, "Event_GameStart", Disposition::Scene),
    (0x00A0, "Event_PiataChazAlone", Disposition::Scene),
    (0x8001, "Cutscene_PiataPrincipal", Disposition::Scene),
    (0x8002, "Cutscene_ProfHolt", Disposition::Scene),
    (0x8003, "Cutscene_MeetingRune", Disposition::Scene),
    (0x8004, "Cutscene_Dorin", Disposition::Scene),
    (0x8005, "Cutscene_Alshline", Disposition::Scene),
    (0x8006, "Cutscene_ZemaIgglanovaDefeated", Disposition::Scene),
    (0x8007, "Cutscene_MeetingRika", Disposition::Scene),
    (0x8008, "Cutscene_DemiRescue", Disposition::Scene),
    (0x8009, "Cutscene_AlysWounded", Disposition::Scene),
    (0x800A, "Cutscene_PsycoWand", Disposition::Scene),
    (0x800B, "Cutscene_ZioDefeated", Disposition::Scene),
    (0x800C, "Cutscene_MeetingWren", Disposition::Scene),
    (0x800D, "Cutscene_InsideSpaceship", Disposition::Scene),
    (0x800E, "Cutscene_SpaceshipSabotage", Disposition::Scene),
    (0x800F, "Cutscene_CrashLaanding", Disposition::Scene),
    (0x8010, "Cutscene_Landale", Disposition::Scene),
    (0x8011, "Cutscene_DarkForce1Defeated", Disposition::Scene),
    (0x8012, "Cutscene_RajaSick", Disposition::Scene),
    (0x8013, "Cutscene_MeetingKyra", Disposition::Scene),
    (0x8014, "Cutscene_LutzRevelation", Disposition::Scene),
    (0x8015, "Cutscene_FindingAirCastle", Disposition::Scene),
    (0x8016, "Cutscene_LashiecDefeated", Disposition::Scene),
    (0x8017, "Cutscene_DarkForce2Defeated", Disposition::Scene),
    (0x8018, "Cutscene_GumbiousBishop", Disposition::Scene),
    (0x8019, "Cutscene_MeetingSeth", Disposition::Scene),
    (0x801A, "Cutscene_AeroPrism", Disposition::Scene),
    (0x801B, "Cutscene_Rykros", Disposition::Scene),
    (0x801C, "Cutscene_LeRoofAgain", Disposition::Scene),
    (0x801D, "Cutscene_BeforeElsydeonCave", Disposition::Scene),
    (0x801E, "Cutscene_Elsydeon", Disposition::Scene),
    (0x801F, "Cutscene_Reunion", Disposition::Scene),
    (0x8020, "Cutscene_ProfoundDarkness", Disposition::Scene),
    (0x8021, "Cutscene_Ending", Disposition::Scene),
    // census:end
];

fn registered(event: u16) -> Option<&'static str> {
    scene_for(EventIndex(event)).map(|scene| scene.name)
}

/// Every way the table and a registry can disagree, each naming the event.
fn violations(registry: &dyn Fn(u16) -> Option<&'static str>) -> Vec<String> {
    let mut found = Vec::new();
    let mut allowlisted = 0;
    for &(event, label, disposition) in CENSUS {
        match disposition {
            Scene => {
                if registry(event).is_none() {
                    found.push(format!("${event:04X} {label} has no registered scene"));
                }
            }
            Null => {}
            InsideScene(caller) => {
                if registry(caller).is_none() {
                    found.push(format!(
                        "${event:04X} {label} is only called from ${caller:04X}, which has no scene"
                    ));
                }
            }
            Allowlisted(issue) => {
                allowlisted += 1;
                if !ISSUES.contains(&issue) {
                    found.push(format!(
                        "${event:04X} {label} names untracked issue #{issue}"
                    ));
                }
                if let Some(name) = registry(event) {
                    found.push(format!(
                        "${event:04X} {label} is registered as {name} but still allowlisted"
                    ));
                }
            }
        }
    }
    if allowlisted > ALLOWLIST_CEILING {
        found.push(format!(
            "the allowlist holds {allowlisted} entries, over its ceiling of {ALLOWLIST_CEILING}"
        ));
    }
    found
}

#[test]
fn every_event_the_cartridge_can_fire_has_a_scene_or_a_listed_reason() {
    let found = violations(&registered);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn every_registered_scene_is_an_event_the_cartridge_fires() {
    for scene in SCENES {
        assert!(
            CENSUS
                .iter()
                .any(|&(event, _, disposition)| event == scene.event.0 && disposition == Scene),
            "{} (${:04X}) is registered but the census does not find the cartridge firing it",
            scene.name,
            scene.event.0
        );
    }
}

#[test]
fn the_table_is_sorted_and_has_no_duplicates() {
    for pair in CENSUS.windows(2) {
        assert!(pair[0].0 < pair[1].0, "${:04X} is out of order", pair[1].0);
    }
}

/// Negative control: unregister a scene and the guard fails, naming it.
#[test]
fn unregistering_a_scene_fails_the_guard_and_names_it() {
    let hide = |event: u16| {
        if event == 0x43 {
            None
        } else {
            registered(event)
        }
    };
    let found = violations(&hide);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("Event_OutsideRajaTemple"), "{found:?}");
}

/// Negative control: an allowlisted event whose scene lands must leave the
/// list, so the list only shrinks.
#[test]
fn a_scene_for_an_allowlisted_event_fails_until_the_entry_is_removed() {
    let Some(&(event, label, _)) = CENSUS
        .iter()
        .find(|&&(_, _, d)| matches!(d, Allowlisted(_)))
    else {
        return; // an empty allowlist has nothing to shrink
    };
    let fake = |asked: u16| {
        if asked == event {
            Some("Event_Fake")
        } else {
            registered(asked)
        }
    };
    let found = violations(&fake);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains(label) && found[0].contains("still allowlisted"));
}
