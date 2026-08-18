//! The opening-act golden path, headless: the FindingAlys trigger fires on
//! landing, Event_AlysFound runs through the full runtime loop (scene actors,
//! dialogue round-trip, party mutation, despawn), and Alys leads.

use std::path::Path;

use psiv_core::{Cell, CharId, Direction, Flag, Input, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn finding_alys_runs_the_scene_and_alys_leads() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let data = GameData::load(Path::new(PACK)).expect("pack loads");

    // PiataAcademy_F1; the trigger wants x == $260 (column 38) and
    // y >= $F0. Spawn one cell below a qualifying cell and step up so the
    // trigger evaluates on a landing, as RunEvents effectively does.
    let mut rt = Runtime::new(
        data,
        0x013,
        Cell::new(38, 18),
        Direction::Up,
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
    // Walk up one cell; the landing should fire trigger 3 and start the scene.
    while !rt.scene_active() {
        let events = rt.tick(Input::Direction(Direction::Up));
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
    for _ in 0..4000 {
        let events = rt.tick(Input::Neutral);
        for e in &events {
            if matches!(e, RuntimeEvent::SceneDialogue { .. }) {
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

    // The scene's closing Event_MoveCamera frames the new leader: subject
    // ($260, $F0) minus the home offset ($98, $58) = camera (456, 152). The
    // pan runs at 2 px/frame and may outlive the scene; give it room.
    for _ in 0..300 {
        rt.tick(Input::Neutral);
    }
    assert_eq!(
        rt.camera().effective(),
        (0x260 - 152, 0xF0 - 88),
        "camera framed on Alys after the scene"
    );

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
