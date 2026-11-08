use super::*;
use crate::scene::SceneOp;
use crate::scene_presentation::PresentationOp;

/// Op counts as the transcription docs state them.
///
/// Two scenes diverge from their doc's number, both for stated reasons.
///
/// `Event_GameStart`'s doc says "~78" and means it — the tilde is theirs.
/// The count depends on how the prologue's two `dbra` colour ramps are
/// expanded; here each page is 8 ops (4 text draws, one fade-up loop, the
/// 900-frame hold, one fade-down loop, the closing pause), matching the
/// asm's loop structure rather than unrolling 20 iterations each.
///
/// `Event_MeetingHahn` is the other: its doc writes the
/// command-selecting branch as a single op that computes a *value*, and
/// this vocabulary has no value-selecting branch — so it becomes control
/// flow over two `MoveActorCommand` ops plus a `Jump`, two ops more.
#[test]
fn every_scene_matches_its_docs_op_count() {
    let expected: &[(&str, usize)] = &[
        ("Event_GameStart", 80),
        ("Event_PiataChazAlone", 2),
        ("Event_AlysFound", 12),
        ("Cutscene_PiataPrincipal", 10),
        ("Event_SuspicionOnPrincipal", 2),
        ("Event_MeetingHahn", 16),
        ("Event_BasementContainers", 7),
        ("Event_IgglanovaBattle", 4),
        ("Event_AfterIgglanova", 15),
        ("Event_PrincipalConfession", 6),
        ("Event_PiataGuardsReprimand", 10),
        ("Cutscene_ProfHolt", 13),
        ("Cutscene_MeetingRune", 13),
        ("Event_MeetingDorin", 36),
        ("Cutscene_Dorin", 17),
        ("Event_RuneFlaeli", 45),
        ("Event_AlshlineFound", 2),
        ("Cutscene_Alshline", 86),
        ("Cutscene_ZemaIgglanovaDefeated", 11),
        ("Event_ZemaServantBattle", 4),
        ("Event_ZemaOldMan", 3),
        ("Event_ZemaOldManAfterMission", 3),
        ("Event_MeetingSaya", 12),
        ("Event_TonoeBasementDoor", 19),
        ("Event_BioPlantAlarm", 6),
        ("Event_BioPlantDoorOpening", 8),
        ("Event_ElevatorDoorOpening", 9),
        ("Event_RidingElevator", 19),
        ("Event_GirlsSneakingOut", 27),
        ("Event_ChazHouse", 13),
        ("Event_LeavingChazHouse", 1),
        ("Cutscene_MeetingRika", 107),
        ("Cutscene_DemiRescue", 26),
        ("Cutscene_AlysWounded", 57),
        ("Event_GettingLandRover", 31),
        ("Event_MachineCenterAppearing", 14),
        ("Event_RuneLadaeTower", 9),
        ("Event_PsycoWandChest", 20),
        ("Cutscene_PsycoWand", 110),
        ("Event_ZioFortBarrier", 35),
        ("Event_ZioNurvus", 18),
        ("Cutscene_ZioDefeated", 46),
        ("Cutscene_MeetingWren", 16),
        ("Cutscene_InsideSpaceship", 44),
        ("Cutscene_SpaceshipSabotage", 36),
        ("Cutscene_CrashLaanding", 131),
        ("Event_OutsideRajaTemple", 3),
        ("Event_TylerGraveOpening", 19),
        ("Event_Gyuna", 7),
        ("Event_CancellerReminder", 2),
        ("Event_MileSandWormBattle", 5),
        ("Event_FractOozeFound", 5),
        ("Event_KingRappy", 7),
        ("Event_DaughterTerminal", 5),
        ("Event_VahalFortEntrance", 2),
        ("Event_VahalFortMidway", 2),
        ("Event_VahalFortBarrier", 5),
        ("Event_VahFortMovingPlatform1", 22),
        ("Event_VahFortMovingPlatform2", 22),
        ("Event_VahalFortTerminal", 8),
        ("Event_DominatorsDefeated", 49),
        ("Event_PosiBolt", 14),
        ("Event_WeaponPlantArrival", 2),
        ("Event_WpnPlntMovingPlatform1", 22),
        ("Event_WpnPlntMovingPlatform2", 22),
        ("Event_WpnPlntMovingPlatform3", 22),
        ("Event_WpnPlntMovingPlatform4", 22),
        ("Event_WeaponPlantTerminal", 8),
        ("Event_Burstroc", 14),
        ("Event_ConveyorBeltDown", 6),
        ("Event_ConveyorBeltUp", 6),
        ("Event_ConveyorBeltRight", 6),
        ("Event_ConveyorBeltLeft", 6),
        ("Event_Recovery", 16),
        ("Event_MuskCatsGuarding", 4),
        ("Event_MuskCatElder", 6),
        ("Event_PenguinOwner", 12),
        ("Event_RajaSick", 11),
        ("Event_RuneHealingChaz", 17),
        ("Event_Hijammer", 14),
        ("Event_PickingFifthCharacter", 15),
        ("Event_GaruberkTwDoorOpening1", 19),
        ("Event_GaruberkTwDoorOpening2", 19),
        ("Event_GaruberkTwDoorEntered1", 21),
        ("Event_GaruberkTwDoorEntered2", 21),
        ("Event_GaruberkTwEyeAction1", 8),
        ("Event_GaruberkTwEyeAction2", 4),
        ("Event_SoldiersTempleCaveDialogue1", 2),
        ("Event_SoldiersTempleCaveDialogue2", 2),
        ("Event_SoldiersTempleReached", 2),
        ("Event_AeroPrismFound", 3),
        ("Cutscene_Landale", 24),
        ("Event_KuranArrival", 2),
        ("Event_NearDarkForce1", 2),
        ("Event_DarkForce1", 5),
        ("Cutscene_DarkForce1Defeated", 28),
        ("Event_Juza", 5),
        ("Event_JuzaDefeated", 3),
        ("Event_MeetingLeRoof", 17),
        ("Cutscene_LeRoofAgain", 67),
        ("Event_CarnivorousTrees", 13),
        ("Event_SavingKyra", 19),
        ("Cutscene_MeetingKyra", 24),
        ("Event_EsperGuardPermission", 14),
        ("Event_PersistentEsperGuards", 6),
        ("Event_InnerSanctGuard", 3),
        ("Event_InnerSanctGuardBeforeElsydeon", 3),
        ("Event_EclipseTorchUsed", 28),
        ("Event_DarkForce2", 6),
        ("Cutscene_LutzRevelation", 30),
        ("Event_EclipseTorchStolen", 58),
        ("Cutscene_DarkForce2Defeated", 33),
        ("Cutscene_MeetingSeth", 11),
        ("Cutscene_AeroPrism", 27),
        ("Event_DarkForce3Defeated", 2),
        ("Event_ReshelBattle", 4),
        ("Event_ClmCenterForcedBattle", 4),
        ("Event_ClmCenterAfterBattle", 2),
        ("Event_DElmLars", 4),
        ("Event_AfterDElmLarsBattle", 2),
        ("Cutscene_FindingAirCastle", 50),
        ("Event_AirCastleArrival", 2),
        ("Event_XeAThoulBeforeBattle", 5),
        ("Event_AirCastleFakeChest", 11),
        ("Event_LashiecAppearance", 12),
        ("Cutscene_LashiecDefeated", 28),
        ("Cutscene_GumbiousBishop", 18),
        ("Event_StrengthTowerTop", 15),
        ("Event_CourageTowerTop", 18),
        ("Event_DeVars", 4),
        ("Event_SaLews", 4),
        ("Event_ReFaze", 27),
        ("Event_DeVarsDefeated", 6),
        ("Event_SaLewsDefeated", 6),
        ("Cutscene_BeforeElsydeonCave", 16),
        ("Cutscene_Elsydeon", 47),
        ("Cutscene_Reunion", 36),
        ("Event_SilenceTmDoorOpening", 11),
        ("Event_EnterAngerTower", 16),
        ("Event_AngerTowerTop", 12),
        ("Event_AngerTowerAlys", 11),
        ("Event_AngerTowerExitTop", 10),
        ("Cutscene_ProfoundDarkness", 20),
        ("Cutscene_RajaSick", 54),
        ("Cutscene_Rykros", 28),
        ("Cutscene_Ending", 371),
        ("Event_RockyOwner", 3),
        ("Event_GirlPrison", 3),
        ("Event_GirlsBail", 3),
        ("Event_PayingGirlsBail", 9),
        ("Event_GirlsBailedOut", 8),
        ("Event_AiedoManSavings", 4),
        ("Event_AiedoManCrushOnAlys", 6),
        ("Event_HuntersGuild", 56),
        ("Event_StrippersAppearing", 33),
        ("Event_MonsenEarthquake", 117),
        ("Event_RanchOwner", 13),
        ("Event_RanchOwnerAfterBattle", 3),
        ("Event_RockyFound", 3),
        ("Event_CatchingRocky", 58),
        ("Event_TallasMom", 3),
        ("Event_TallasMomAfterRescue", 3),
        ("Event_InsideMonsenHole", 2),
        ("Event_TallasRescued", 14),
        ("Event_ExitingStripClub", 1),
        ("Event_DyingBoy", 53),
        ("Event_WreckageEngine", 67),
        ("Event_LostGirlsMother", 3),
        ("Event_KingRappyDefeated", 18),
        ("Event_MeeseClinicSickWoman", 20),
        ("Event_Phonon", 33),
        ("Event_PlateSysEngine", 33),
        ("Event_PenguinFeedStolen", 58),
        ("Event_FaintingPriest", 12),
        ("Event_ZioFanatic", 7),
        ("Event_Pennant", 7),
        ("Event_WoodCarving", 7),
        ("Event_PiataDormOwner", 3),
        ("Event_MissingStudentFound", 29),
        ("Event_OldManNearMissingStudent", 6),
        ("Event_MissingStudentInBed", 14),
        ("Event_DyingBoyFather", 3),
        ("Event_FatherAfterBoyRecovery", 3),
        ("Event_MeetingSekreas", 3),
        ("Event_BoardingLandRover", 7),
        ("Event_BoardingIceDigger", 7),
        ("Event_BoardingHydrofoil", 7),
        ("Event_GettingOffVehicle", 9),
    ];
    assert_eq!(
        SCENES.len(),
        expected.len(),
        "every act scene is registered"
    );
    // Every disagreement at once, so one change that moves several counts
    // is fixed in one pass.
    let wrong: Vec<String> = expected
        .iter()
        .filter_map(|(name, ops)| {
            let scene = SCENES
                .iter()
                .find(|s| s.name == *name)
                .unwrap_or_else(|| panic!("{name} is missing from the registry"));
            (scene.ops.len() != *ops)
                .then(|| format!("{name}: {} ops, the docs say {ops}", scene.ops.len()))
        })
        .collect();
    assert!(wrong.is_empty(), "op counts: {wrong:#?}");
}

/// The op indices of `ops` whose `WalkObject` order disagrees with the
/// `Char_Move_Flags` bit 1 the scene's last `SetFollowMode` left, read in
/// op order from a clear bit.
fn walk_order_mismatches(ops: &[SceneOp]) -> Vec<usize> {
    let mut y_first_now = false;
    let mut wrong = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        match op {
            SceneOp::SetFollowMode { bits } => y_first_now = bits & 0b10 != 0,
            SceneOp::Presentation {
                op: PresentationOp::WalkObject { y_first, .. },
            } if *y_first != y_first_now => wrong.push(index),
            _ => {}
        }
    }
    wrong
}

/// `FieldObj_GetAutoInput` reads bit 1 for scene-created objects too, so
/// every temporary-object walk carries the order the scene has set.
#[test]
fn every_object_walk_takes_the_axis_order_in_effect() {
    for scene in SCENES {
        assert_eq!(
            walk_order_mismatches(scene.ops),
            [0usize; 0],
            "{}",
            scene.name
        );
    }
}

/// The check's negative controls: a Y-first walk before the bit is set,
/// an X-first one after it, and one left Y-first after it is cleared.
#[test]
fn a_walk_against_the_follow_mode_is_caught() {
    let walk = |y_first| SceneOp::Presentation {
        op: PresentationOp::WalkObject {
            slot: 8,
            x: 0x230,
            y: 0x220,
            y_first,
        },
    };
    let mode = |bits| SceneOp::SetFollowMode { bits };
    assert_eq!(walk_order_mismatches(&[walk(true)]), [0]);
    assert_eq!(walk_order_mismatches(&[mode(0b111), walk(false)]), [1]);
    assert_eq!(
        walk_order_mismatches(&[mode(0b111), walk(true), mode(0b101), walk(true)]),
        [3]
    );
    assert!(walk_order_mismatches(&[mode(0b111), walk(true), mode(0b101), walk(false)]).is_empty());
}

#[test]
fn every_jump_target_is_in_range() {
    for scene in SCENES {
        let len = scene.ops.len();
        for (index, op) in scene.ops.iter().enumerate() {
            let targets: Vec<usize> = match op {
                SceneOp::Jump { to } => vec![*to],
                SceneOp::BranchDialogueEnd { if_ended } => vec![*if_ended],
                SceneOp::BranchFlag {
                    if_set, if_clear, ..
                } => vec![*if_set, *if_clear],
                SceneOp::BranchChoice { if_yes, if_no } => vec![*if_yes, *if_no],
                SceneOp::BranchIfAligned {
                    if_aligned, if_not, ..
                } => vec![*if_aligned, *if_not],
                SceneOp::BranchIfActorCoord {
                    if_true, if_false, ..
                } => vec![*if_true, *if_false],
                SceneOp::BranchIfSavedDialogueByte {
                    if_equal, if_not, ..
                } => vec![*if_equal, *if_not],
                SceneOp::BranchIfVehicle {
                    if_mounted,
                    if_on_foot,
                } => vec![*if_mounted, *if_on_foot],
                SceneOp::BranchIfMoneyBelow {
                    if_below, if_not, ..
                } => vec![*if_below, *if_not],
                SceneOp::BranchIfActorGreater {
                    if_greater, if_not, ..
                } => vec![*if_greater, *if_not],
                SceneOp::BranchIfPartyMember {
                    if_present,
                    if_absent,
                    ..
                } => vec![*if_present, *if_absent],
                _ => vec![],
            };
            for target in targets {
                assert!(
                    target <= len,
                    "{} op {index} jumps to {target}, past {len}",
                    scene.name
                );
            }
        }
    }
}

#[test]
fn every_scene_loads_the_trees_its_retail_bytes_load() {
    // `DialogueTreesToRAM` calls per transcribed scene, counted in the US
    // retail image's own byte range for that scene
    // (`docs/scenes/REVISION_AUDIT.md`; `tests/test_scene_trees.py`
    // re-derives this column from the ROM, so the table cannot rot in
    // silence). A scene absent from this list loads no tree.
    //
    // This is the H17 guard. `Cutscene_AlysWounded` and `Cutscene_PsycoWand`
    // reached dialogue `$2C`/`$2D`/`$2E` on Krup Inn F1's bound tree 5
    // because the four `move.l #DialogueTree6, d0` calls before them sit
    // under `if revision>0` and were dropped together. Op counts and map
    // state both looked right; only a per-scene tree-load census sees it.
    const RETAIL_TREE_LOADS: &[(&str, usize)] = &[
        ("Event_GameStart", 4),
        ("Event_PiataGuardsReprimand", 2),
        ("Cutscene_Alshline", 3),
        ("Cutscene_AlysWounded", 3),
        ("Cutscene_ZemaIgglanovaDefeated", 1),
        ("Cutscene_MeetingRika", 1),
        ("Cutscene_PsycoWand", 3),
        ("Cutscene_ZioDefeated", 1),
        ("Cutscene_CrashLaanding", 2),
        ("Event_OutsideRajaTemple", 1),
        ("Cutscene_LashiecDefeated", 2),
        ("Cutscene_GumbiousBishop", 1),
        ("Cutscene_MeetingSeth", 1),
        ("Cutscene_Rykros", 1),
        ("Cutscene_Ending", 1),
        ("Cutscene_MeetingKyra", 1),
        ("Event_CarnivorousTrees", 1),
        ("Event_SavingKyra", 2),
        ("Event_PersistentEsperGuards", 2),
        ("Event_EclipseTorchStolen", 2),
        ("Event_EclipseTorchUsed", 1),
        ("Event_HuntersGuild", 10),
    ];
    for scene in SCENES {
        let ported = scene
            .ops
            .iter()
            .filter(|op| matches!(op, SceneOp::SetDialogueTree { .. }))
            .count();
        let retail = RETAIL_TREE_LOADS
            .iter()
            .find(|(name, _)| *name == scene.name)
            .map_or(0, |(_, loads)| *loads);
        assert_eq!(
            ported, retail,
            "{} loads {retail} dialogue tree(s) in the cartridge",
            scene.name
        );
    }
    for (name, _) in RETAIL_TREE_LOADS {
        assert!(
            SCENES.iter().any(|scene| scene.name == *name),
            "{name} is missing from the registry"
        );
    }
}

/// The H22 shape: `RunDialogueResume` reopens `Saved_Dialogue_Addr`, so a
/// scene may only resume after it has run a dialogue itself. A scene whose
/// first dialogue op is a resume reads whatever ran last in play, or
/// nothing after a load.
#[test]
fn no_scene_resumes_before_it_opens_a_dialogue() {
    for scene in SCENES {
        let first = scene.ops.iter().find(|op| {
            matches!(
                op,
                SceneOp::RunDialogue { .. }
                    | SceneOp::RunDialogueResume
                    | SceneOp::RunDialogueResumeWithWindow { .. }
            )
        });
        assert!(
            !matches!(
                first,
                Some(SceneOp::RunDialogueResume | SceneOp::RunDialogueResumeWithWindow { .. })
            ),
            "{} resumes a dialogue it never opened",
            scene.name
        );
    }
}

/// The frames a palette-tone loop's presentation record stands for, when
/// the op is one: `Pal_IncreaseTone` (`$58608`) and the tone flashes each
/// share their `dbra` loop with a `VInt_Prepare`, so the record says what
/// the palette does and the `WaitFrames` after it is the loop's frames.
fn tone_loop_frames(op: &SceneOp) -> Option<u16> {
    match op {
        SceneOp::Presentation {
            op:
                crate::scene_presentation::PresentationOp::PaletteIncreaseTone { frames, .. }
                | crate::scene_presentation::PresentationOp::PaletteRestoreTone { frames }
                | crate::scene_presentation::PresentationOp::PaletteToneFlash { frames, .. },
        } => Some(*frames),
        _ => None,
    }
}

/// Every op index whose tone loop is not followed by its own frames.
fn tone_loops_without_their_frames(ops: &[SceneOp]) -> Vec<usize> {
    ops.iter()
        .enumerate()
        .filter_map(|(i, op)| {
            let frames = tone_loop_frames(op)?;
            match ops.get(i + 1) {
                Some(SceneOp::WaitFrames { frames: waited }) if *waited == frames => None,
                _ => Some(i),
            }
        })
        .collect()
}

/// The ending carried two `Pal_IncreaseTone` loops with no frames and one
/// with frames but no record, 120 frames short headless.
#[test]
fn every_palette_tone_loop_is_followed_by_its_frames() {
    for scene in SCENES {
        assert_eq!(
            tone_loops_without_their_frames(scene.ops),
            Vec::<usize>::new(),
            "{}: a palette-tone loop without its WaitFrames",
            scene.name
        );
    }
}

/// Negative control: a tone loop alone, or followed by the wrong count,
/// is reported at its index.
#[test]
fn a_tone_loop_without_its_frames_is_reported() {
    let tone = SceneOp::Presentation {
        op: PresentationOp::PaletteIncreaseTone {
            frames: 60,
            hold: None,
        },
    };
    assert_eq!(tone_loops_without_their_frames(&[tone]), vec![0]);
    assert_eq!(
        tone_loops_without_their_frames(&[SceneOp::End, tone, SceneOp::WaitFrames { frames: 59 }]),
        vec![1]
    );
    assert!(
        tone_loops_without_their_frames(&[tone, SceneOp::WaitFrames { frames: 60 }]).is_empty()
    );
}

/// Every op index whose `ObjectOp` spawn is not drawn before the scene
/// does anything else.
///
/// The renderer creates map-object sprites only on a sprite rebuild, and
/// a spawned slot is not drawn by the spawn itself: the cartridge draws
/// it with the `Field_BuildSprites` after it, which the port records as
/// `PresentationOp::RebuildSprites`. Only the same-frame state writes
/// retail makes between the two (another spawn, a clear, a dialogue or
/// routine word) may come first; a wait, a line or the scene's end
/// before the rebuild leaves the slot undrawn for that long.
fn spawns_not_drawn(ops: &[SceneOp]) -> Vec<usize> {
    ops.iter()
        .enumerate()
        .filter(|(_, op)| matches!(op, SceneOp::Object(_)))
        .filter_map(|(i, _)| {
            for op in &ops[i + 1..] {
                match op {
                    SceneOp::Presentation {
                        op: PresentationOp::RebuildSprites,
                    } => return None,
                    SceneOp::Object(_)
                    | SceneOp::DespawnNpc { .. }
                    | SceneOp::SetNpcDialogue { .. }
                    | SceneOp::SetObjectRoutine { .. } => {}
                    _ => return Some(i),
                }
            }
            Some(i)
        })
        .collect()
}

/// `Event_ZioFanatic` spawned the writhing believer and waited 30 frames
/// with no rebuild, so he was not on screen until the scene ended.
#[test]
fn every_spawned_object_is_drawn_before_the_scene_goes_on() {
    for scene in SCENES {
        assert_eq!(
            spawns_not_drawn(scene.ops),
            Vec::<usize>::new(),
            "{}: an ObjectOp spawn without its RebuildSprites",
            scene.name
        );
    }
}

/// Negative controls: a spawn followed by a wait, by a line, by a wait
/// before its rebuild, or by nothing is reported at its index; one whose
/// rebuild follows a clear and a dialogue word is not.
#[test]
fn a_spawn_without_its_rebuild_is_reported() {
    let spawn = SceneOp::Object(crate::scene::ObjectOp::SpawnAt {
        slot: 9,
        routine: 0x148,
        at: crate::scene::ActorRef::Npc(4),
        solid: true,
    });
    let rebuild = SceneOp::Presentation {
        op: PresentationOp::RebuildSprites,
    };
    let wait = SceneOp::WaitFrames { frames: 30 };
    let clear = SceneOp::DespawnNpc {
        npc_index: 4,
        count: 1,
    };
    assert_eq!(spawns_not_drawn(&[spawn, wait, rebuild]), vec![0]);
    assert_eq!(spawns_not_drawn(&[spawn, clear, wait]), vec![0]);
    assert_eq!(
        spawns_not_drawn(&[spawn, aiedo_events::standard(0x1F), rebuild]),
        vec![0]
    );
    assert_eq!(spawns_not_drawn(&[rebuild, wait, spawn]), vec![2]);
    assert_eq!(spawns_not_drawn(&[spawn]), vec![0]);
    assert!(
        spawns_not_drawn(&[
            spawn,
            clear,
            SceneOp::SetNpcDialogue {
                npc_index: 9,
                dialogue_id: 0x38,
            },
            rebuild,
            wait,
        ])
        .is_empty()
    );
}

/// Why a palette record cannot be what the cartridge writes: a word
/// offset past the 64-word buffer, a destination outside
/// `Palette_Table_Buffer`, or a literal with bits outside a CRAM word's
/// `----bbb-ggg-rrr-` (a byte offset or a VDP register write recorded
/// as palette words).
fn palette_record_fault(op: &PresentationOp) -> Option<&'static str> {
    const CRAM: u16 = 0x0EEE;
    let buffer = |ram: u32, words: u32| {
        (0xFFFF_FB00..0xFFFF_FB80).contains(&ram) && ram + 2 * words <= 0xFFFF_FB80
    };
    match *op {
        PresentationOp::SetPaletteWords {
            offset,
            first,
            second,
        } => {
            if offset > 62 {
                Some("offset past the buffer")
            } else if (first | second) & !CRAM != 0 {
                Some("not CRAM words")
            } else {
                None
            }
        }
        PresentationOp::AddPaletteWord { offset, .. } => {
            (offset > 63).then_some("offset past the buffer")
        }
        PresentationOp::PaletteCopy {
            start,
            words,
            source,
            ..
        } => {
            if usize::from(start) + usize::from(words) > 64 {
                Some("copy past the buffer")
            } else if matches!(source, crate::PaletteSource::Fill(v) if v & !CRAM != 0) {
                Some("not a CRAM word")
            } else {
                None
            }
        }
        PresentationOp::PaletteIncreaseTone {
            hold: Some(hold), ..
        } => (!buffer(hold.destination_ram, u32::from(hold.words))).then_some("hold outside"),
        PresentationOp::RykrosPaletteCycle {
            destination_ram,
            words_per_frame,
            ..
        } => (!buffer(destination_ram, u32::from(words_per_frame))).then_some("cycle outside"),
        PresentationOp::PaletteRampFromTable {
            destination_ram, ..
        } => (!buffer(destination_ram + 2, 3)).then_some("ramp outside"),
        PresentationOp::PaletteCycleFromTable { offset, .. } => {
            (offset > 62).then_some("offset past the buffer")
        }
        _ => None,
    }
}

/// Every palette record in the registry is one the 64-word buffer can
/// take. `SetPaletteWords` once held byte offsets (`$44`, `$5C`) and a
/// VDP register write (`$8C89`) as palette words.
#[test]
fn every_palette_record_fits_the_palette_buffer() {
    for scene in SCENES {
        for (index, op) in scene.ops.iter().enumerate() {
            if let SceneOp::Presentation { op } = op {
                assert_eq!(
                    palette_record_fault(op),
                    None,
                    "{} op {index}: {op:?}",
                    scene.name
                );
            }
            if let SceneOp::LoadPalette { words, start, .. } = op {
                assert!(usize::from(*start) + usize::from(*words) <= 64, "{} op {index}", scene.name);
            }
        }
    }
}

/// Negative control: each mistake the check names is caught.
#[test]
fn a_palette_record_that_cannot_fit_is_reported() {
    let words = |offset, first, second| PresentationOp::SetPaletteWords {
        offset,
        first,
        second,
    };
    assert_eq!(palette_record_fault(&words(0x5C, 0x0620, 0x0EEE)), Some("offset past the buffer"));
    assert_eq!(palette_record_fault(&words(0x18, 0x8C89, 0x8C81)), Some("not CRAM words"));
    assert_eq!(palette_record_fault(&words(0x2E, 0x0620, 0x0EEE)), None);
    assert_eq!(
        palette_record_fault(&PresentationOp::PaletteCopy {
            source: crate::PaletteSource::Fill(0),
            destination: crate::PaletteBuffer::Live,
            start: 32,
            words: 64,
        }),
        Some("copy past the buffer")
    );
    assert_eq!(
        palette_record_fault(&PresentationOp::PaletteIncreaseTone {
            frames: 1,
            hold: Some(crate::PaletteHold {
                source_rom_addr: 0,
                destination_ram: 0xFFFF_FB78,
                words: 16,
            }),
        }),
        Some("hold outside")
    );
}

#[test]
fn event_indexes_are_unique_and_findable() {
    for scene in SCENES {
        assert_eq!(
            scene_for(scene.event).map(|s| s.name),
            Some(scene.name),
            "{} should be findable by its event index",
            scene.name
        );
    }
    let mut seen: Vec<_> = SCENES.iter().map(|s| s.event).collect();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), before, "two scenes share an event index");
}

#[test]
fn the_cutscene_is_the_only_one_with_bit_15_set() {
    let cutscenes: Vec<_> = SCENES
        .iter()
        .filter(|s| s.event.is_cutscene())
        .map(|s| s.name)
        .collect();
    assert_eq!(
        cutscenes,
        vec![
            "Cutscene_PiataPrincipal",
            "Cutscene_ProfHolt",
            "Cutscene_MeetingRune",
            "Cutscene_Dorin",
            "Cutscene_Alshline",
            "Cutscene_ZemaIgglanovaDefeated",
            "Cutscene_MeetingRika",
            "Cutscene_DemiRescue",
            "Cutscene_AlysWounded",
            "Cutscene_PsycoWand",
            "Cutscene_ZioDefeated",
            "Cutscene_MeetingWren",
            "Cutscene_InsideSpaceship",
            "Cutscene_SpaceshipSabotage",
            "Cutscene_CrashLaanding",
            "Cutscene_Landale",
            "Cutscene_DarkForce1Defeated",
            "Cutscene_LeRoofAgain",
            "Cutscene_MeetingKyra",
            "Cutscene_LutzRevelation",
            "Cutscene_DarkForce2Defeated",
            "Cutscene_MeetingSeth",
            "Cutscene_AeroPrism",
            "Cutscene_FindingAirCastle",
            "Cutscene_LashiecDefeated",
            "Cutscene_GumbiousBishop",
            "Cutscene_BeforeElsydeonCave",
            "Cutscene_Elsydeon",
            "Cutscene_Reunion",
            "Cutscene_ProfoundDarkness",
            "Cutscene_RajaSick",
            "Cutscene_Rykros",
            "Cutscene_Ending",
        ]
    );
}
