//! The Dezolis dismount scenes on the pack (#592): `Event_CarnivorousTrees`,
//! `Event_SavingKyra` and `Event_EclipseTorchUsed`, mounted on the Ice
//! Digger and on foot, frame by frame on Dezolis (`$001`), whose
//! `MapUpdate_DezoSnowstormPal` makes every `RunMapUpdates` call observable
//! as [`Runtime::map_updates`]' `calls`.
//!
//! A frame of map updates (`DoMapUpdateLoop`) moves the count; the bare
//! `VInt_Prepare` frames (`$0707A0`, `$070882`, `$0701A0`, `$0701B4`) and the
//! frames after the dismount (`$07080C`, `$0708EE`, `$070220`, whose
//! `Field_UpdateObjects` the port does not run: `docs/source-notes/README.md`,
//! "Dezolis dismount frames run no object pass") leave it.

use super::palette::{pack_present, runtime_at};
use crate::{Runtime, RuntimeEvent};
use psiv_core::{CharId, Flag, Input, PresentationOp, SceneOp};

/// One tick: the scene's sounds, DMA, dialogues and battle on it, then `+map`
/// when it ran map updates, and the vehicle after it (`foot` or `rover2`).
fn tick_line(runtime: &mut Runtime, calls: &mut u64) -> (String, bool) {
    let events = runtime.tick(Input::Neutral);
    let mut parts = Vec::new();
    let mut battle = false;
    for event in &events {
        match event {
            RuntimeEvent::ScenePresentation { op, .. } => match op {
                SceneOp::PlaySound { id } => parts.push(format!("sound {id:02X}")),
                SceneOp::SetSavedMusic { id } => parts.push(format!("saved {id:02X}")),
                SceneOp::DmaPlanes => parts.push("dma".to_owned()),
                SceneOp::Presentation {
                    op: PresentationOp::RebuildSprites,
                } => parts.push("sprites".to_owned()),
                _ => {}
            },
            RuntimeEvent::SceneDialogue { entry } => {
                parts.push(format!("dialogue {entry:02X}"));
                runtime.dialogue_closed();
            }
            RuntimeEvent::SceneDialogueResume => runtime.dialogue_closed(),
            // The scene's request frame; the battle starts after its entry.
            RuntimeEvent::EventBattleEntered { index } => {
                parts.push(format!("battle {index:02X}"));
                battle = true;
            }
            RuntimeEvent::SceneFaulted { .. } => panic!("{event:?}"),
            _ => {}
        }
    }
    let now = runtime.map_updates().calls;
    if now != *calls {
        assert_eq!(now, *calls + 1, "one map update a frame");
        parts.push("+map".to_owned());
    }
    *calls = now;
    parts.push(match runtime.vehicle_index() {
        None => "foot".to_owned(),
        Some(index) => format!("vehicle {index}"),
    });
    let done = battle || !runtime.scene_active();
    (parts.join(", "), done)
}

/// Runs `event` on Dezolis beside the Garuberk trees, mounted on the Ice
/// Digger or not, to its battle or its end (at most `limit` ticks): one line
/// a tick after the event starts, and the runtime.
fn dismount_run(event: u16, flags: &[Flag], mounted: bool, limit: usize) -> (Vec<String>, Runtime) {
    let party = [Some(CharId(0)), Some(CharId(1)), None, None, None];
    let mut runtime = runtime_at(0x001, (0xBA0, 0xD0), party, flags);
    let pack = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
    let files = psiv_data::BattleFiles::load(std::path::Path::new(pack)).expect("battle files");
    runtime.enable_battles(&files).expect("battles enable");
    if mounted {
        runtime.set_vehicle_index(2).expect("the Ice Digger mounts");
    }
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut calls = runtime.map_updates().calls;
    let mut lines = Vec::new();
    for _ in 0..limit {
        let (line, done) = tick_line(&mut runtime, &mut calls);
        lines.push(line);
        if done {
            break;
        }
    }
    (lines, runtime)
}

#[test]
fn the_tree_attacks_dismount_with_retail_frames_and_branches() {
    if !pack_present() {
        return;
    }
    // The first tick is the mode dispatcher's entry frame.
    let (trees, rt) = dismount_run(0x004C, &[], true, 20);
    assert_eq!(
        trees,
        [
            "vehicle 2",
            "saved 99, sound FD, +map, vehicle 2",
            "sound A9, sprites, vehicle 2",
            "foot",
            "dialogue 34, foot",
            "battle 0A, foot",
        ]
    );
    assert!(!rt.game().is_set(Flag::event(0x95)));
    // On foot, `beq.w loc_70826`: no dismount, no dialogue `$34`.
    let (trees, _) = dismount_run(0x004C, &[], false, 20);
    assert_eq!(trees, ["foot", "saved 99, battle 0A, foot"]);
}

/// `$070908..$07093B`: the panel DMA spends a frame without map updates
/// before dialogue `$26`, after the mounted branch has dismounted.
#[test]
fn saving_kyra_mounted_spends_the_panel_dma_frame() {
    if !pack_present() {
        return;
    }
    let raja_sick = [Flag::event(0x94)];
    let (kyra, rt) = dismount_run(0x004D, &raja_sick, true, 20);
    assert_eq!(
        kyra,
        [
            "vehicle 2",
            "saved 99, sound FD, +map, vehicle 2",
            "sound FB, sprites, vehicle 2",
            "foot",
            "dialogue 34, foot",
            "sound A9, +map, foot",
            "dma, foot",
            "dialogue 26, foot",
            "battle 0A, foot",
        ]
    );
    assert!(rt.game().is_set(Flag::event(0x95)));
}

/// The on-foot branch skips the dismount, but still runs the panel DMA.
#[test]
fn saving_kyra_on_foot_spends_the_panel_dma_frame() {
    if !pack_present() {
        return;
    }
    let raja_sick = [Flag::event(0x94)];
    let (kyra, rt) = dismount_run(0x004D, &raja_sick, false, 20);
    assert_eq!(
        kyra,
        [
            "foot",
            "sound A9, +map, foot",
            "dma, foot",
            "dialogue 26, foot",
            "battle 0A, foot",
        ]
    );
    assert!(rt.game().is_set(Flag::event(0x95)));
}

#[test]
fn the_eclipse_torch_dismount_plays_the_field_music_on_bare_frames() {
    if !pack_present() {
        return;
    }
    let lashiec = [Flag::event(0x9B)];
    let (mounted, rt) = dismount_run(0x0047, &lashiec, true, 2_000);
    assert_eq!(
        mounted[..4],
        [
            "vehicle 2",
            "saved 99, sound FD, vehicle 2",
            "sound 99, sprites, vehicle 2",
            "foot",
        ]
    );
    assert!(rt.game().is_set(Flag::event(0x9C)));
    let (on_foot, rt) = dismount_run(0x0047, &lashiec, false, 2_000);
    assert!(rt.game().is_set(Flag::event(0x9C)));
    // After the three dismount frames the runs are the same, map updates
    // included.
    assert_eq!(mounted[4..], on_foot[1..]);
}
