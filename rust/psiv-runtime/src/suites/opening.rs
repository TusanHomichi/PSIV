//! The opening-act golden path, headless: the FindingAlys trigger fires on
//! landing, Event_AlysFound runs through the full runtime loop (scene actors,
//! dialogue round-trip, party mutation, despawn), and Alys leads.

use std::path::Path;

use crate::{Runtime, RuntimeEvent};
use psiv_core::{Cell, CharId, Direction, Flag, Input, StepFrames};
use psiv_data::GameData;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn finding_alys_runs_the_scene_and_alys_leads() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let data = GameData::load(Path::new(PACK)).expect("pack loads");

    // PiataAcademy_F1; the trigger wants x == $260 (column 38) and
    // y >= $F0. Spawn one cell to the right on row 17 (the hallway entry of
    // tape 04) and step left onto (38, 17) so Chaz enters with camera (456, 168)
    // and NPC Alys takes MoveActorCommand(2) down from (37, 16) to (37, 17).
    let mut rt = Runtime::new(
        data,
        0x013,
        Cell::new(39, 17),
        Direction::Left,
        StepFrames::default(),
    )
    .expect("spawn");

    assert!(rt.game().is_set(Flag::event(7)), "game-start flag seeded");
    assert_eq!(
        rt.game().party_slot(0),
        Some(CharId(0)),
        "Chaz starts alone"
    );

    let mut log = Vec::new();
    let mut guard = 0;
    // Walk left one cell onto (38, 17); landing fires trigger 3.
    while !rt.scene_active() {
        let events = rt.tick(Input::Direction(Direction::Left));
        log.extend(events);
        guard += 1;
        assert!(guard < 40, "trigger never fired; log: {log:?}");
    }
    assert!(
        log.iter()
            .any(|e| matches!(e, RuntimeEvent::SceneStarted { trigger: 3 })),
        "expected trigger 3, got {log:?}"
    );

    // Drive the scene: answer every dialogue request, bounded ticks.
    let mut dialogues = 0;
    // (camera, party changed, scene ended) per frame, to separate the pan's
    // last movement from the scene's release and verify the 8-frame pan (#564).
    let mut trace = Vec::new();
    for _ in 0..4000 {
        let events = rt.tick(Input::Neutral);
        trace.push((
            rt.camera().effective(),
            events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::PartyChanged)),
            events.iter().any(|e| matches!(e, RuntimeEvent::SceneEnded)),
        ));
        for e in &events {
            if matches!(
                e,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                dialogues += 1;
                rt.dialogue_closed();
            }
        }
        log.extend(events);
        if !rt.scene_active() {
            break;
        }
    }
    assert!(
        !rt.scene_active(),
        "scene should finish; log tail: {:?}",
        &log[log.len().saturating_sub(6)..]
    );
    assert!(dialogues >= 1, "the scene talks");
    assert!(
        log.iter().any(|e| matches!(e, RuntimeEvent::PartyChanged)),
        "party changed"
    );
    assert!(
        log.iter()
            .any(|e| matches!(e, RuntimeEvent::NpcsDespawned { .. })),
        "NPC Alys despawned"
    );
    assert!(
        log.iter().any(|e| matches!(e, RuntimeEvent::SceneEnded)),
        "scene ended"
    );

    // The cartridge's outcome: Alys leads, Chaz second, the flag stops the
    // trigger from ever firing again.
    assert_eq!(rt.game().party_slot(0), Some(CharId(1)), "Alys leads");
    assert_eq!(rt.game().party_slot(1), Some(CharId(0)), "Chaz second");
    assert!(
        rt.game().is_set(Flag::event(0x08)),
        "EventFlag_AlysFound set"
    );

    // The scene's closing Event_MoveCamera frames the new leader's live
    // position minus the home offset ($98, $58), and returns only on arrival,
    // so the camera is there when the scene ends. NPC Alys stepped down via
    // MoveActorCommand(2) to ($250, $100), so the camera ends at (440, 168)
    // after an 8-frame pan (tape 04, f7478-f7486; #564).
    assert_eq!(
        rt.camera().effective(),
        (0x250 - 152, 0x100 - 88),
        "camera framed on Alys when the scene ends"
    );

    // The final pan frame is preserved: the scene ends a frame after the
    // camera last moved, and the hand-back replays no leader movement.
    let party_written = trace
        .iter()
        .position(|&(_, p, _)| p)
        .expect("party changed");
    let ended = trace.iter().position(|&(_, _, e)| e).expect("scene ended");
    assert_eq!(
        ended - party_written,
        8,
        "8-frame pan from party write to scene end (tape 04, f7478-f7486): {:?}",
        &trace[party_written..=ended]
    );
    assert!(ended >= 1, "scene cannot end on its first frame");
    assert_eq!(
        trace[ended - 1].0,
        (0x250 - 152, 0x100 - 88),
        "camera has already arrived the frame before the scene ends"
    );
    assert!(
        trace[ended - 2].0 != trace[ended - 1].0,
        "the last movement frame is separate from the release frame"
    );
    for _ in 0..12 {
        rt.tick(Input::Neutral);
        assert_eq!(
            rt.camera().effective(),
            (0x250 - 152, 0x100 - 88),
            "the follow driver was rebased, not replayed"
        );
    }

    // Trigger 3 must never re-fire; OTHER triggers may legitimately fire —
    // that's the story continuing (their conditions often require AlysFound
    // set). Record what does.
    let mut followups = Vec::new();
    'walk: for dir in [Direction::Down, Direction::Up] {
        for _ in 0..12 {
            let events = rt.tick(Input::Direction(dir));
            for e in &events {
                if let RuntimeEvent::SceneStarted { trigger } = e {
                    assert_ne!(*trigger, 3, "FindingAlys re-fired despite the flag");
                    followups.push(*trigger);
                    break 'walk;
                }
            }
        }
    }
    eprintln!("post-scene walk fired triggers: {followups:?}");
}
