//! Counter, gate and ordered palette classes through pad-only sessions.
//! Cartridge rules/citations: docs/field/MAP_UPDATES.md.

use std::path::PathBuf;
use std::sync::OnceLock;

use psiv_core::{Cell, CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_data::{GameData, MapId, UpdateProgram};
use psiv_runtime::{Pad, Session};

fn data() -> &'static GameData {
    static DATA: OnceLock<GameData> = OnceLock::new();
    DATA.get_or_init(|| {
        let root = std::env::var_os("PSIV_RUNTIME_PACK")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack")
            });
        GameData::load(&root).unwrap()
    })
}

fn session(map: u16, flags: &[(u16, bool)]) -> Session {
    let start = Session::start(data().clone()).new_game().unwrap();
    let mut game = GameState::from_snapshot(&start.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), None, None, None, None]);
    for &(id, set) in flags {
        let _ = game.write(Flag::event(id), set);
    }
    let record = data().map(MapId(map)).unwrap();
    let at = (1..record.dimensions.height_cells)
        .find_map(|y| {
            (0..record.dimensions.width_cells).find_map(|x| {
                let pos = psiv_data::CellPos::new(x, y);
                (record.collision_at(pos) == Some(psiv_data::CollisionType::Normal)
                    && record.warps_at(pos).next().is_none()
                    && !record.npcs.iter().any(|n| n.x_cell == x && n.y_cell == y))
                .then_some(Cell::new(x as u16, y as u16))
            })
        })
        .unwrap();
    Session::start(data().clone())
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                map_index: map,
                char_x: at.x * 16,
                char_y: at.y * 16,
                ..RetailLocation::default()
            },
        })
        .unwrap()
}

#[test]
fn palette_counters_are_independent_and_wrap_at_their_own_table_bounds() {
    let mut session = session(162, &[]);
    for frame in 1..=384 {
        session.frame(Pad::NEUTRAL);
        let counter = session.runtime().map_updates().counters;
        let steps = frame / 8;
        let phase = |length| {
            if steps == 0 {
                0
            } else {
                ((steps - 1) % length + 1) as u8
            }
        };
        assert_eq!(counter[..2], [phase(6), phase(8)], "frame {frame}");
    }
}

#[test]
fn strip_lights_use_the_local_duration_byte_and_park_it_in_a_menu() {
    let mut session = session(90, &[]);
    let UpdateProgram::Palette { cycles, .. } =
        &data().map(MapId(90)).unwrap().map_updates.as_ref().unwrap()[0].program
    else {
        panic!("strip recipe")
    };
    let frames = &cycles[0].frames;
    for frame in 1..=256 {
        session.frame(Pad::NEUTRAL);
        let view = session.runtime().map_updates();
        assert_eq!(
            view.counters[0], frame as u8,
            "byte wraps independently of palette phase"
        );
        if frame % 8 == 0 {
            assert_eq!(
                view.palette[26],
                frames[((frame / 8 - 1) % 6) as usize][0].word
            );
        }
    }
    session.frame(Pad::new(psiv_runtime::Button::Camp));
    let before = session.runtime().map_updates().clone();
    for _ in 0..16 {
        session.frame(Pad::NEUTRAL);
    }
    assert_eq!(session.runtime().map_updates(), &before);
}

#[test]
fn shutdown_gates_stop_counters_and_reunion_bypasses_both() {
    for (flags, active) in [
        (vec![(0x65, false), (0x61, false), (0xDA, false)], true),
        (vec![(0x65, true), (0x61, false), (0xDA, false)], false),
        (vec![(0x65, false), (0x61, true), (0xDA, false)], false),
        (vec![(0x65, true), (0x61, true), (0xDA, true)], true),
    ] {
        let mut session = session(187, &flags);
        for _ in 0..16 {
            session.frame(Pad::NEUTRAL);
        }
        let view = session.runtime().map_updates();
        assert_eq!(view.counters[..2], if active { [2, 2] } else { [0, 0] });
        if !active {
            let UpdateProgram::Palette { stopped, .. } = &data()
                .map(MapId(187))
                .unwrap()
                .map_updates
                .as_ref()
                .unwrap()
                .iter()
                .find(|e| e.index == 0x17)
                .unwrap()
                .program
            else {
                panic!("shutdown recipe")
            };
            for write in stopped {
                assert_eq!(view.palette[usize::from(write.slot)], write.word);
            }
        }
    }
}

#[test]
fn edge_clear_rotate_and_blob_copies_follow_the_map_list_order() {
    let mut session = session(256, &[]);
    let record = data().map(MapId(256)).unwrap();
    let updates = record.map_updates.as_ref().unwrap();
    let UpdateProgram::Edge { colors } = &updates[0].program else {
        panic!("edge recipe")
    };
    let UpdateProgram::EdgeLine { frames, .. } = &updates[1].program else {
        panic!("line recipe")
    };
    let mut expected = session.runtime().map_updates().palette.clone();
    for frame in 1..=116 {
        session.frame(Pad::NEUTRAL);
        expected[..14].fill(0);
        if frame % 2 == 0 {
            let phase = (frame / 2 - 1) % 13;
            for (i, &word) in colors.iter().enumerate() {
                expected[1 + (phase + i) % 13] = word;
            }
        }
        if frame % 4 == 0 {
            for write in &frames[(frame / 4 - 1) % 28] {
                expected[usize::from(write.slot)] = write.word;
            }
        }
        assert_eq!(
            session.runtime().map_updates().palette,
            expected,
            "ordered writes frame {frame}"
        );
    }
}

#[test]
fn edge_sliding_blob_and_signed_scroll_keep_separate_counters() {
    let mut session = session(257, &[]);
    let updates = data()
        .map(MapId(257))
        .unwrap()
        .map_updates
        .as_ref()
        .unwrap();
    let UpdateProgram::EdgeLine {
        frames,
        sliding: true,
    } = &updates[1].program
    else {
        panic!("sliding recipe")
    };
    let UpdateProgram::Scroll { samples, .. } = &updates[2].program else {
        panic!("scroll recipe")
    };
    for frame in 1..=260 {
        session.frame(Pad::NEUTRAL);
        let view = session.runtime().map_updates();
        assert_eq!(view.counters[2], frame as u8);
        for row in [0, 1, 63, 255] {
            let sample = samples[((frame as usize) + row * 4) & 255] as i8;
            assert_eq!(
                view.scroll_writes[row * 2 + 1].word,
                ((i16::from(sample) >> 4) - 16) as u16
            );
        }
        if frame % 4 == 0 {
            for write in frames[((frame / 4 - 1) % 14) as usize]
                .iter()
                .filter(|w| w.slot >= 16)
            {
                assert_eq!(view.palette[usize::from(write.slot)], write.word);
            }
        }
    }
}

#[test]
fn no_update_entry_has_no_palette_or_state_side_effects() {
    let mut session = session(17, &[]);
    let before = session.runtime().map_updates().palette.clone();
    let state = session.runtime().game().snapshot();
    for _ in 0..32 {
        session.frame(Pad::NEUTRAL);
    }
    assert_eq!(session.runtime().map_updates().palette, before);
    assert_eq!(session.runtime().map_updates().palette_revision, 0);
    assert_eq!(session.runtime().map_updates().counters, [0; 3]);
    assert_eq!(session.runtime().game().snapshot(), state);
    assert_eq!(session.runtime().map_updates().calls, 32);
}
