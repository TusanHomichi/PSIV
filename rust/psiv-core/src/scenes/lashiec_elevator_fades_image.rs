//! #658: direct US calls, helper ownership, and controls for spurious fades.
use super::*;
use Fixed::{In, Init, Out};

fn lashiec_exit_matches(ops: &[SceneOp]) -> bool {
    let Some(resume) = ops
        .iter()
        .position(|op| matches!(op, SceneOp::RunDialogueResumeWithWindow { .. }))
    else {
        return false;
    };
    ops.get(resume + 1..resume + 7)
        == Some(
            &[
                SceneOp::FadeOut,
                SceneOp::AddItem { item: 0x8E },
                SceneOp::InitVramAndCram,
                SceneOp::SetWorldIndex { world: 1 },
                SceneOp::FlightPlanet,
                SceneOp::SetTextColour { colour: 0 },
            ][..],
        )
}

#[test]
fn lashiecs_out_precedes_the_item_and_the_shared_screen_precedes_the_map() {
    let scene = SCENES
        .iter()
        .find(|scene| scene.name == "Cutscene_LashiecDefeated")
        .unwrap();
    assert!(lashiec_exit_matches(scene.ops));
    let mut moved = scene.ops.to_vec();
    let item = moved
        .iter()
        .position(|op| *op == SceneOp::AddItem { item: 0x8E })
        .unwrap();
    moved.swap(item - 1, item);
    assert_eq!(
        native(&moved),
        native(scene.ops),
        "control preserves fade order"
    );
    assert!(
        !lashiec_exit_matches(&moved),
        "a fade across the item changes the boundary"
    );
}

#[test]
fn the_four_scenes_have_the_verified_us_calls_and_ranges() {
    let Some(rom) = rom() else { return };
    let ranges = documented_ranges();
    for (name, span, calls) in [
        (
            "Cutscene_LashiecDefeated",
            (0x077A68, 0x077BDA),
            &[
                (0x077A68, Init),
                (0x077A6E, In),
                (0x077AB4, Out),
                (0x077ABA, Init),
                (0x077AFC, In),
                (0x077B2E, Out),
                (0x077B52, Init),
                (0x077BAC, In),
            ][..],
        ),
        ("Event_DeVarsDefeated", (0x07175A, 0x071822), &[]),
        ("Event_SaLewsDefeated", (0x071822, 0x0718E6), &[]),
        (
            "Event_RidingElevator",
            (0x06C33E, 0x06C478),
            &[(0x06C346, Out), (0x06C398, In)][..],
        ),
    ] {
        assert!(ranges[name].iter().all(|&found| found == span), "{name}");
        assert_eq!(retail_calls(rom, &[span]), calls, "{name}");
        assert!(!KNOWN.iter().any(|(known, _)| *known == name));
        let scene = SCENES.iter().find(|scene| scene.name == name).unwrap();
        assert!(mismatches(rom, &ranges, &[*scene]).is_empty());
    }
    // EventPtrs points at the US entry, not the reference fork's build address.
    let pointer = 0x05A2B4 + 0x14 * 4;
    assert_eq!(
        u32::from_be_bytes(rom[pointer..pointer + 4].try_into().unwrap()),
        0x06C33E
    );
    assert_eq!(
        u16::from_be_bytes(rom[0x06C46C..0x06C46E].try_into().unwrap()),
        0x4E75
    );
    // Lashiec calls the same planet helper as the flights/DF1, with world 1.
    for (at, target) in [
        (0x077B68, 0x064C1E),
        (0x06C350, 0x0546EA),
        (0x06C356, 0x05AE98),
    ] {
        assert_eq!(
            u16::from_be_bytes(rom[at..at + 2].try_into().unwrap()),
            0x4EB9
        );
        assert_eq!(
            u32::from_be_bytes(rom[at + 2..at + 6].try_into().unwrap()),
            target
        );
    }
    assert_eq!(
        retail_calls(rom, &[(0x064C1E, 0x064D00)]),
        [(0x064C8E, In), (0x064CFA, Out)]
    );
    // Transition selection, map refresh and party overlap do not own a fixed fade.
    assert!(
        retail_calls(
            rom,
            &[
                (0x0546EA, 0x05479A),
                (0x05AE98, 0x05B0C0),
                (0x05A87A, 0x05A936)
            ]
        )
        .is_empty()
    );
}

#[test]
fn restored_guardian_fades_are_rejected() {
    let Some(rom) = rom() else { return };
    let ranges = documented_ranges();
    for name in ["Event_DeVarsDefeated", "Event_SaLewsDefeated"] {
        let scene = SCENES.iter().find(|scene| scene.name == name).unwrap();
        let mut ops = scene.ops.to_vec();
        ops.insert(1, SceneOp::FadeOut);
        let altered = crate::Scene {
            ops: Box::leak(ops.into_boxed_slice()),
            ..*scene
        };
        assert_eq!(mismatches(rom, &ranges, &[altered]).len(), 1, "{name}");
    }
}
