//! Fixed fades at the Dorin/Saya retail boundaries (#655), beyond the
//! image census's call order: moving a fade across state writes still
//! matches that census but changes what the player sees.

use super::{ALYS, DEMI, GRYZ, HAHN, next_arc::DORIN, post_rika_cutscenes::ALYS_WOUNDED};
use crate::{DialogueId, DialogueSource, Flag, SceneOp};

fn entry(op: &SceneOp, id: u16) -> bool {
    matches!(op, SceneOp::RunDialogue { source: DialogueSource::Entry(DialogueId(found)), .. } if *found == id)
}

fn at(ops: &[SceneOp], pick: impl Fn(&SceneOp) -> bool) -> usize {
    ops.iter().position(pick).expect("boundary op exists")
}

#[test]
fn dorin_fades_after_refresh_and_replacement_before_departure() {
    let ops = DORIN.ops;
    // $074034/$07403A, then Get5($25) at $074048.
    assert_eq!(&ops[..2], &[SceneOp::InitVramAndCram, SceneOp::FadeIn]);
    let text = at(ops, |op| entry(op, 0x25));
    let map = at(ops, |op| matches!(op, SceneOp::LoadMap { map: 0x43, .. }));
    let gryz = at(
        ops,
        |op| matches!(op, SceneOp::PromoteNpcToChar { char_id, .. } if *char_id == GRYZ),
    );
    let despawn = at(ops, |op| {
        matches!(
            op,
            SceneOp::DespawnNpc {
                npc_index: 2,
                count: 1
            }
        )
    });
    assert!(text < map && map < gryz && gryz < despawn);
    // $07411C: after Gryz's build and NPC removal, before $13+1 map ticks
    // and Music_TonoeDePon ($81), never before RefreshMap/Get5's close.
    assert_eq!(
        &ops[despawn + 1..despawn + 4],
        &[
            SceneOp::FadeIn,
            SceneOp::Wait { ticks: 20 },
            SceneOp::PlaySound { id: 0x81 }
        ]
    );
}

/// $074E40's last continuation, $074EBC's fade, $074EC4's Saya flag,
/// $074ECA's common music/party tail. The repeat visit jumps to the tail.
fn saya_exit(ops: &[SceneOp]) -> bool {
    let flag = at(ops, |op| {
        *op == SceneOp::SetFlag {
            flag: Flag::event(0x12),
            value: true,
        }
    });
    flag >= 2
        && ops[flag - 2] == SceneOp::RunDialogueResume
        && ops[flag - 1] == SceneOp::FadeOut
        && ops[flag + 1] == SceneOp::PlaySound { id: 0xA5 }
}

#[test]
fn alys_wounded_fades_at_both_inns_and_before_the_common_party_tail() {
    let ops = ALYS_WOUNDED.ops;
    // $074B72/$074B78 before raw $43; $074C14/$074C1A before panel $4A.
    assert_eq!(&ops[..2], &[SceneOp::InitVramAndCram, SceneOp::FadeIn]);
    assert!(entry(&ops[3], 0x43));
    assert_eq!(
        &ops[8..12],
        &[
            SceneOp::Wait { ticks: 60 },
            SceneOp::InitVramAndCram,
            SceneOp::FadeIn,
            SceneOp::PanelCreate { id: 0x4A },
        ]
    );
    let branch = at(
        ops,
        |op| matches!(op, SceneOp::BranchFlag { flag, .. } if *flag == Flag::event(0x12)),
    );
    let SceneOp::BranchFlag {
        if_set, if_clear, ..
    } = ops[branch]
    else {
        unreachable!()
    };
    assert_eq!(ops[if_set], SceneOp::PlaySound { id: 0xA5 });
    assert!(matches!(
        ops[if_clear],
        SceneOp::LoadMap {
            map: 0x3F,
            prev_map: 0,
            ..
        }
    ));
    let heal = at(ops, |op| {
        matches!(
            op,
            SceneOp::ObjectAnimation {
                object_id: 0x8194,
                ..
            }
        )
    });
    let saya = at(ops, |op| entry(op, 0x2C));
    assert!(if_clear < heal && heal < saya);
    // $074D12, between Saya's construction and tree-6 dialogue $2C.
    assert_eq!(ops[heal + 1], SceneOp::FadeIn);
    assert_eq!(
        ops[saya - 2],
        SceneOp::SetDialogueTree { rom_addr: 0x1E32A0 }
    );
    assert!(saya_exit(ops));
    assert_eq!(
        &ops[if_set + 1..if_set + 6],
        &[
            SceneOp::RemovePartyMember { who: HAHN },
            SceneOp::ClearCharacterStatus { who: HAHN },
            SceneOp::RemovePartyMember { who: ALYS },
            SceneOp::ClearCharacterStatus { who: ALYS },
            SceneOp::JoinParty { slot: 3, who: DEMI },
        ]
    );
    let inn = at(ops, |op| {
        matches!(
            op,
            SceneOp::LoadMap {
                map: 0x3F,
                prev_map: 0x3E,
                ..
            }
        )
    });
    assert!(if_set + 6 < inn);
    // $07500E, after the common map/party build, before camera and $2D.
    assert_eq!(ops[inn + 1], SceneOp::FadeIn);
    assert!(matches!(ops[inn + 2], SceneOp::PanCamera { .. }));
    assert!(entry(&ops[inn + 4], 0x2D));
}

#[test]
fn dropping_the_repaired_fade_or_moving_it_past_the_flag_breaks_the_boundary() {
    let mut ops = ALYS_WOUNDED.ops.to_vec();
    let out = at(&ops, |op| *op == SceneOp::FadeOut);
    ops.remove(out);
    assert!(!saya_exit(&ops), "reverting the repair must fail");
    ops.insert(out + 1, SceneOp::FadeOut);
    assert!(
        !saya_exit(&ops),
        "the call order alone cannot guard this move"
    );
}
