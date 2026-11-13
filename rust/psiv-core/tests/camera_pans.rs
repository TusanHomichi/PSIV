//! Every scene camera pan, held to its retail `Event_MoveCamera` call.
//!
//! The routine (`ps4.asm:121468`) returns only on arrival, X leg then Y, so
//! every transcribed call is a blocking `SceneOp::PanCamera` (#267). The table
//! names each call's `jsr`/`jmp` line and the subject the cartridge loads into
//! `d0`/`d1` there: a literal, or an object's live `$30`/`$34`. A literal where
//! the cartridge reads a live object drifts as soon as that object is not
//! where the transcriber assumed (Alys at `$250`, not `$260`, in
//! `Event_AlysFound`). Calls the port does not transcribe yet are #565's.

use std::collections::BTreeMap;

use psiv_core::{ActorRef, CameraSubject, CharId, SCENES, SceneOp};

const LEADER: ActorRef = ActorRef::PartyMember(0);

const fn at(x: i32, y: i32) -> CameraSubject {
    CameraSubject::At { x, y }
}

const fn live(actor: ActorRef) -> CameraSubject {
    CameraSubject::Actor(actor)
}

/// `(scene, subject, speed, retail call)`, in each scene's op order.
const RETAIL_PANS: &[(&str, CameraSubject, u16, &str)] = &[
    (
        "Event_AlysFound",
        live(LEADER),
        2,
        "`Event_MoveCamera` (`ps4.asm:183729`)",
    ),
    // US retail, commented out under the reference fork's rewrite.
    (
        "Event_AfterIgglanova",
        live(LEADER),
        1,
        "`Event_MoveCamera` (`ps4.asm:147133`)",
    ),
    (
        "Cutscene_Dorin",
        live(LEADER),
        2,
        "`Event_MoveCamera` (`ps4.asm:153657`)",
    ),
    (
        "Event_RuneFlaeli",
        live(ActorRef::Character(CharId(3))),
        2,
        "`Event_MoveCamera` (`ps4.asm:183855`)",
    ),
    (
        "Event_RuneFlaeli",
        live(LEADER),
        2,
        "`Event_MoveCamera` (`ps4.asm:183868`)",
    ),
    (
        "Cutscene_Alshline",
        at(0x1E0, 0xA0),
        1,
        "`Event_MoveCamera` (`ps4.asm:153822`)",
    ),
    (
        "Cutscene_Alshline",
        at(0x1E0, 0xE0),
        2,
        "`Event_MoveCamera` (`ps4.asm:153832`)",
    ),
    (
        "Cutscene_AlysWounded",
        live(ActorRef::Character(CharId(0))),
        2,
        "`Event_MoveCamera` (`ps4.asm:154456`)",
    ),
    (
        "Cutscene_AlysWounded",
        live(LEADER),
        2,
        "`Event_MoveCamera` (`ps4.asm:154553`)",
    ),
    (
        "Event_GettingLandRover",
        at(0x1E0, 0x200),
        1,
        "`Event_MoveCamera` (`ps4.asm:147413`)",
    ),
    (
        "Event_MachineCenterAppearing",
        at(0x730, 0xB40),
        1,
        "`Event_MoveCamera` (`ps4.asm:144706`)",
    ),
    (
        "Event_MachineCenterAppearing",
        live(LEADER),
        2,
        "`Event_MoveCamera` (`ps4.asm:144728`)",
    ),
    (
        "Event_PsycoWandChest",
        CameraSubject::ActorX {
            actor: LEADER,
            y: 0x160,
        },
        1,
        "`Event_MoveCamera` (`ps4.asm:148073`)",
    ),
    (
        "Event_ZioFortBarrier",
        live(LEADER),
        1,
        "`Event_MoveCamera` (`ps4.asm:148249`)",
    ),
    (
        "Cutscene_CrashLaanding",
        at(0x240, 0x5D0),
        2,
        "`Event_MoveCamera` (`ps4.asm:155867`)",
    ),
    (
        "Cutscene_CrashLaanding",
        at(0x5F0, 0x120),
        1,
        "`Event_MoveCamera` (`ps4.asm:155987`)",
    ),
    (
        "Event_TylerGraveOpening",
        CameraSubject::ActorShifted {
            actor: LEADER,
            dx: 4,
        },
        4,
        "`Event_MoveCamera` (`ps4.asm:149263`)",
    ),
    (
        "Event_TylerGraveOpening",
        live(LEADER),
        4,
        "`Event_MoveCamera` (`ps4.asm:149266`)",
    ),
    (
        "Event_TylerGraveOpening",
        CameraSubject::ActorShifted {
            actor: LEADER,
            dx: 4,
        },
        4,
        "`Event_MoveCamera` (`ps4.asm:149324`)",
    ),
    (
        "Event_TylerGraveOpening",
        live(LEADER),
        4,
        "`Event_MoveCamera` (`ps4.asm:149327`)",
    ),
    (
        "Event_DominatorsDefeated",
        live(LEADER),
        2,
        "`Event_MoveCamera` (`ps4.asm:153102`)",
    ),
    (
        "Cutscene_Landale",
        at(0xC0, 0x480),
        1,
        "`Event_MoveCamera` (`ps4.asm:145487`)",
    ),
    (
        "Cutscene_Landale",
        live(LEADER),
        2,
        "`Event_MoveCamera` (`ps4.asm:145509`)",
    ),
    (
        "Event_EclipseTorchStolen",
        at(0x270, 0xF0),
        1,
        "`Event_MoveCamera` (`ps4.asm:149408`)",
    ),
    (
        "Event_EclipseTorchStolen",
        live(LEADER),
        1,
        "`Event_MoveCamera` (`ps4.asm:149516`)",
    ),
    (
        "Event_LashiecAppearance",
        at(0x1E0, 0x110),
        1,
        "`Event_MoveCamera` (`ps4.asm:150295`)",
    ),
    (
        "Event_StrengthTowerTop",
        at(0x1F0, 0x1C8),
        1,
        "`Event_MoveCamera` (`ps4.asm:150377`)",
    ),
    (
        "Event_StrengthTowerTop",
        at(0x1F0, 0x270),
        8,
        "`Event_MoveCamera` (`ps4.asm:150485`)",
    ),
    (
        "Event_CourageTowerTop",
        at(0x1F0, 0x1C0),
        1,
        "`Event_MoveCamera` (`ps4.asm:150526`)",
    ),
    (
        "Event_AngerTowerTop",
        at(0x1F0, 0x1D0),
        8,
        "`Event_MoveCamera` (`ps4.asm:151985`)",
    ),
    (
        "Event_AngerTowerTop",
        at(0x1C0, 0x260),
        2,
        "`Event_MoveCamera` (`ps4.asm:151993`)",
    ),
    (
        "Cutscene_RajaSick",
        at(0x1A0, 0x240),
        4,
        "`Event_MoveCamera` (`ps4.asm:156646`)",
    ),
    (
        "Cutscene_RajaSick",
        live(LEADER),
        4,
        "`Event_MoveCamera` (`ps4.asm:156692`)",
    ),
    (
        "Event_PickingFifthCharacter",
        live(LEADER),
        2,
        "`Event_MoveCamera` (`ps4.asm:151897`)",
    ),
    (
        "Event_StrippersAppearing",
        at(0x220, 0x1C0),
        1,
        "`Event_MoveCamera` (`ps4.asm:153178`)",
    ),
    (
        "Event_StrippersAppearing",
        live(LEADER),
        1,
        "`Event_MoveCamera` (`ps4.asm:153241`)",
    ),
    (
        "Event_WreckageEngine",
        CameraSubject::ActorX {
            actor: LEADER,
            y: 0x1F0,
        },
        1,
        "`Event_MoveCamera` (`ps4.asm:147244`)",
    ),
    (
        "Event_WreckageEngine",
        live(LEADER),
        1,
        "`Event_MoveCamera` (`ps4.asm:147347`)",
    ),
    (
        "Event_PlateSysEngine",
        live(LEADER),
        1,
        "`Event_MoveCamera` (`ps4.asm:147953`)",
    ),
    (
        "Event_MonsenEarthquake",
        live(LEADER),
        1,
        "`Event_MoveCamera` (`ps4.asm:147778`)",
    ),
];

type Pans = BTreeMap<&'static str, Vec<(CameraSubject, u16)>>;

#[test]
fn every_scene_pan_is_a_blocking_pan_on_its_retail_subject() {
    let mut shipped: Pans = BTreeMap::new();
    for scene in SCENES {
        for op in scene.ops {
            if let SceneOp::PanCamera { subject, speed } = *op {
                shipped
                    .entry(scene.name)
                    .or_default()
                    .push((subject, speed));
            }
        }
    }
    let mut retail: Pans = BTreeMap::new();
    for &(name, subject, speed, _call) in RETAIL_PANS {
        retail.entry(name).or_default().push((subject, speed));
    }
    for (name, pans) in &retail {
        assert_eq!(
            shipped.get(name),
            Some(pans),
            "{name}: its pans differ from the cartridge's Event_MoveCamera calls"
        );
    }
    for name in shipped.keys() {
        assert!(
            retail.contains_key(name),
            "{name} pans the camera; add its retail calls to RETAIL_PANS"
        );
    }
}

#[test]
fn the_table_cites_distinct_calls() {
    let mut lines: Vec<&str> = RETAIL_PANS.iter().map(|&(_, _, _, call)| call).collect();
    lines.sort_unstable();
    let count = lines.len();
    lines.dedup();
    assert_eq!(lines.len(), count, "two rows cite the same retail call");
}
