//! The ship's destination menu, through `Session::frame` with pads only.
//!
//! The class: a scene that hands the frame to a list window (`loc_63BC4`,
//! `ps4.asm:133499-133726`), takes its answer from the pad, writes
//! `World_Index` on a confirm and flies, or does nothing on a Cancel. The
//! party walks onto the boarding row as a player would; nothing here calls a
//! runtime mutator, and the state that results is read back from the runtime.
//! The negative controls sit beside the cases they guard.

use std::path::Path;
use std::sync::OnceLock;

use psiv_core::{
    CharId, Flag, GameState, RetailLocation, RetailSave, SceneFault, WORLD_AIR_CASTLE,
    WORLD_DEZOLIS, WORLD_KURAN, WORLD_MOTAVIA, WORLD_ZELAN,
};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{
    Button, DestinationPhase, Frame, FrameMode, Pad, Routed, Runtime, RuntimeEvent, Session,
};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

const MOTA_SPACEPORT: u16 = 0x0BF;
const ZELAN: u16 = 0x18D;
const DEZO_SPACEPORT: u16 = 0x0D4;

fn pack() -> Option<&'static (GameData, BattleFiles)> {
    static PACK_FILES: OnceLock<Option<(GameData, BattleFiles)>> = OnceLock::new();
    PACK_FILES
        .get_or_init(|| {
            let selected = std::env::var_os("PSIV_RUNTIME_PACK");
            let path = selected
                .as_deref()
                .map(Path::new)
                .unwrap_or_else(|| Path::new(PACK));
            if !path.join("manifest.json").is_file() {
                eprintln!("runtime pack absent; skipping destination tests");
                return None;
            }
            Some((
                GameData::load(path).expect("pack loads"),
                BattleFiles::load(path).expect("battle files load"),
            ))
        })
        .as_ref()
}

/// A player in front of the ship's gangway, with a chosen story state.
struct Player {
    session: Session,
}

/// The story flags a state names: `AlysFound` (`$08`) is the post-Zio list.
struct Story<'a> {
    flags: &'a [u16],
    /// Town flags (`Flag::town`), the visited towns RYUKA lists.
    towns: &'a [u16],
    /// The saved `World_Index` word.
    world_word: u16,
}

impl Player {
    /// At `(map, x, y)` with a Chaz/Rika/Rune party and the story's flags.
    fn at(place: (u16, u16, u16), story: &Story) -> Option<Player> {
        let (data, files) = pack()?;
        let initial = Session::start(data.clone())
            .with_battles(files.clone())
            .field()
            .expect("the pack boots");
        let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
        game.set_party([
            Some(CharId(0)),
            Some(CharId(5)),
            Some(CharId(3)),
            None,
            None,
        ]);
        for &flag in story.flags {
            game.set(Flag::event(flag)).expect("flag in range");
        }
        for &town in story.towns {
            game.set(Flag::town(town)).expect("town flag in range");
        }
        let (map, x, y) = place;
        Some(Player {
            session: Session::start(data.clone())
                .with_battles(files.clone())
                .from_save(RetailSave {
                    snapshot: game.snapshot(),
                    location: RetailLocation {
                        world_index: story.world_word,
                        map_index_2: 0,
                        map_index: map,
                        char_x: x * 16,
                        char_y: y * 16,
                    },
                })
                .expect("the session starts"),
        })
    }

    fn tick(&mut self, pad: Pad) -> Frame {
        let frame = self.session.frame(pad);
        for event in &frame.events {
            assert!(
                !matches!(
                    event,
                    RuntimeEvent::SceneFaulted { .. }
                        | RuntimeEvent::MapRefreshFailed { .. }
                        | RuntimeEvent::UnpackedTarget { .. }
                        | RuntimeEvent::EncounterRolled { .. }
                ),
                "unexpected {event:?}"
            );
        }
        frame
    }

    /// A fresh press: down for a frame, up for the next.
    fn press(&mut self, button: Button) -> Frame {
        let frame = self.tick(Pad::new(button));
        self.tick(Pad::NEUTRAL);
        frame
    }

    fn runtime(&self) -> &Runtime {
        self.session.runtime()
    }

    /// Holds `button` until the destination menu is up; the frames it took.
    fn walk_into_menu(&mut self, button: Button) -> u32 {
        for frames in 0..400 {
            if self
                .session
                .destination_view()
                .is_some_and(|view| view.phase == DestinationPhase::Choosing)
            {
                // Let go, so the next press is a fresh edge.
                self.tick(Pad::NEUTRAL);
                return frames;
            }
            self.tick(Pad::new(button));
        }
        panic!("the menu never opened");
    }

    /// Frames with no pad until `done`, at most `limit`; how many it took.
    fn wait_until(&mut self, limit: u32, done: impl Fn(&Player) -> bool) -> u32 {
        for frames in 0..limit {
            if done(self) {
                return frames;
            }
            self.tick(Pad::NEUTRAL);
        }
        panic!("the wait never finished");
    }

    fn rows(&self) -> Vec<u8> {
        self.session
            .destination_view()
            .expect("menu up")
            .rows
            .clone()
    }

    /// Walks Up from the foot of the Mota Spaceport's gangway into the menu.
    fn board_at_mota(&mut self) {
        self.walk_into_menu(Button::Up);
    }
}

/// The foot of the Mota Spaceport's boarding row (column 30, row 20): one step
/// north fires `RunEvent_EnterSpaceship` (`ps4.asm:115743`).
const MOTA_GANGWAY: (u16, u16, u16) = (MOTA_SPACEPORT, 30, 20);

const ALYS_FOUND: u16 = 0x08;
const DEZO_SPACEPORT_FLAG: u16 = 0x82;
const AIR_CASTLE_FOUND: u16 = 0x99;

fn post_zio() -> Story<'static> {
    Story {
        flags: &[ALYS_FOUND, 0x66, 0x68],
        towns: &[],
        world_word: 0,
    }
}

/// At the Mota Spaceport the menu shows Zelan, and confirming flies to Zelan
/// with `World_Index` written (`ps4.asm:133677`), before the flight starts.
#[test]
fn the_mota_spaceport_menu_offers_zelan_and_a_confirm_flies_there() {
    let Some(mut player) = Player::at(MOTA_GANGWAY, &post_zio()) else {
        return;
    };
    assert_eq!(player.runtime().world_index(), WORLD_MOTAVIA);
    player.board_at_mota();
    let view = player.session.destination_view().unwrap().clone();
    assert_eq!(
        view.rows,
        vec![WORLD_ZELAN],
        "AlysFound's mask $90 minus Motavia"
    );
    assert_eq!(view.mask, 0x90);
    assert_eq!(view.phase, DestinationPhase::Choosing);
    // The pack's own words, when the pack carries the screen.
    if let Some(menu) = pack().and_then(|(data, _)| data.ship_menu()) {
        assert_eq!(menu.name(WORLD_ZELAN), Some("ZELAN"));
        assert_eq!(menu.prompt(), "Where do you want to go?");
    }
    // Nothing is written until the menu is answered.
    assert_eq!(player.runtime().world_index(), WORLD_MOTAVIA);
    assert_eq!(player.runtime().map_id().0, MOTA_SPACEPORT);

    let frame = player.tick(Pad::new(Button::Speak));
    assert_eq!(frame.sound, Some(0xF3), "SFXID_Selection");
    assert_eq!(
        player.session.destination_view().unwrap().phase,
        DestinationPhase::Confirmed
    );
    // The message types, the wait runs, and only then is the world written.
    let mut written_with_menu_up = false;
    for _ in 0..600 {
        if player.session.destination_view().is_none() {
            break;
        }
        written_with_menu_up |= player.runtime().world_index() != WORLD_MOTAVIA;
        player.tick(Pad::NEUTRAL);
    }
    assert!(
        !written_with_menu_up,
        "World_Index waits for the end of the message"
    );
    assert_eq!(player.runtime().world_index(), WORLD_ZELAN);
    // The flight runs the scene out: Motavia, Zelan Space, then Zelan.
    player.wait_until(6000, |p| {
        !p.runtime().scene_active() && p.runtime().map_id().0 == ZELAN
    });
    assert_eq!(player.runtime().map_id().0, ZELAN);
    assert_eq!(player.runtime().world_index(), WORLD_ZELAN);
    let cell = player.runtime().state().cell();
    assert_eq!(
        (cell.x, cell.y),
        (31, 46),
        "loc_64B5A's Zelan row ($3E,$5A)"
    );
}

/// Cancel returns to the spaceport and writes nothing (`loc_63E5E`).
#[test]
fn cancel_returns_to_the_spaceport_with_nothing_written() {
    let Some(mut player) = Player::at(MOTA_GANGWAY, &post_zio()) else {
        return;
    };
    player.board_at_mota();
    let before = player.runtime().game().snapshot();
    player.tick(Pad::new(Button::Cancel));
    assert_eq!(
        player.session.destination_view().map(|view| view.phase),
        Some(DestinationPhase::Closing),
        "Cancel starts the teardown"
    );
    // The cartridge reads no pad while it tears down: a Speak now is lost.
    player.tick(Pad::new(Button::Speak));
    player.wait_until(600, |p| !p.runtime().scene_active());
    assert!(player.session.destination_view().is_none());
    assert_eq!(player.runtime().map_id().0, MOTA_SPACEPORT);
    assert_eq!(player.runtime().world_index(), WORLD_MOTAVIA);
    assert_eq!(player.runtime().previous_map_id(), 0xFFFF);
    let cell = player.runtime().state().cell();
    assert_eq!(
        (cell.x, cell.y),
        (30, 21),
        "loc_64B5A's Mota Spaceport row ($3C,$28)"
    );
    assert_eq!(
        player.runtime().game().snapshot(),
        before,
        "no game state moved"
    );
}

/// A Cancel and a Speak in the same frame: `btst #4` tests Cancel first.
#[test]
fn cancel_wins_over_speak_pressed_with_it() {
    let Some(mut player) = Player::at(MOTA_GANGWAY, &post_zio()) else {
        return;
    };
    player.board_at_mota();
    player.tick(Pad::of(&[Button::Cancel, Button::Speak]));
    assert_eq!(
        player.session.destination_view().map(|view| view.phase),
        Some(DestinationPhase::Closing)
    );
    player.wait_until(600, |p| !p.runtime().scene_active());
    assert_eq!(player.runtime().world_index(), WORLD_MOTAVIA);
}

/// The list honours its flag table: the first flag set picks the mask, an extra
/// destination appears when its flag is set and is absent without it.
#[test]
fn the_list_honours_its_flag_table() {
    for (flags, expected, mask) in [
        // AlysFound only: Zelan.
        (vec![ALYS_FOUND], vec![WORLD_ZELAN], 0x90),
        // The Dezolis spaceport has appeared: $D8, Dezolis, Zelan and Kuran.
        (
            vec![ALYS_FOUND, DEZO_SPACEPORT_FLAG],
            vec![WORLD_DEZOLIS, WORLD_ZELAN, WORLD_KURAN],
            0xD8,
        ),
        // The Air Castle found: $DC adds its row, and beats the flags below it.
        (
            vec![ALYS_FOUND, DEZO_SPACEPORT_FLAG, AIR_CASTLE_FOUND],
            vec![WORLD_DEZOLIS, WORLD_ZELAN, WORLD_KURAN, WORLD_AIR_CASTLE],
            0xDC,
        ),
        // No table flag at all: the default $88, Kuran only (Motavia is here).
        (vec![], vec![WORLD_KURAN], 0x88),
    ] {
        let story = Story {
            flags: &flags,
            towns: &[],
            world_word: 0,
        };
        let Some(mut player) = Player::at(MOTA_GANGWAY, &story) else {
            return;
        };
        player.board_at_mota();
        let view = player.session.destination_view().unwrap();
        assert_eq!(
            (view.rows.clone(), view.mask),
            (expected, mask),
            "{flags:?}"
        );
    }
}

/// Up and Down wrap over the rows with `SFXID_MovingCursor`
/// (`Win_UpdateCursorUpDown`, `ps4.asm:141702`); Speak confirms the row under
/// the cursor, not the first.
#[test]
fn the_cursor_wraps_and_confirm_takes_the_row_under_it() {
    let story = Story {
        flags: &[ALYS_FOUND, DEZO_SPACEPORT_FLAG],
        towns: &[],
        world_word: 0,
    };
    let Some(mut player) = Player::at(MOTA_GANGWAY, &story) else {
        return;
    };
    player.board_at_mota();
    assert_eq!(player.rows(), vec![WORLD_DEZOLIS, WORLD_ZELAN, WORLD_KURAN]);
    let cursor = |p: &Player| p.session.destination_view().unwrap().cursor;
    assert_eq!(cursor(&player), 0);
    // Up from the first row wraps to the last.
    let frame = player.press(Button::Up);
    assert_eq!(frame.sound, Some(0xF2), "SFXID_MovingCursor");
    assert_eq!(cursor(&player), 2);
    // Down from the last wraps to the first.
    player.press(Button::Down);
    assert_eq!(cursor(&player), 0);
    player.press(Button::Down);
    assert_eq!(cursor(&player), 1);
    // Left and Right move nothing.
    player.press(Button::Right);
    player.press(Button::Left);
    assert_eq!(cursor(&player), 1);
    // Speak on the second row flies to Zelan, not Dezolis.
    player.press(Button::Speak);
    player.wait_until(600, |p| p.session.destination_view().is_none());
    assert_eq!(player.runtime().world_index(), WORLD_ZELAN);
}

/// H27, the reader: after a flight, RYUKA's town list reads the new world.
/// Before it, Motavia's visited town is listed; after the flight to Dezolis,
/// Dezolis's is (`Runtime::town_destinations`, `ps4.asm:128945-128975`).
#[test]
fn ryuka_reads_the_world_the_flight_wrote() {
    // Town flag 0 is the first Motavia town, flag 16 the first Dezolis town.
    let story = Story {
        flags: &[ALYS_FOUND, DEZO_SPACEPORT_FLAG],
        towns: &[0, 16],
        world_word: 0,
    };
    let Some(mut player) = Player::at(MOTA_GANGWAY, &story) else {
        return;
    };
    let listed = |p: &Player| {
        p.runtime()
            .town_destinations()
            .iter()
            .map(|town| town.world)
            .collect::<Vec<_>>()
    };
    assert_eq!(listed(&player), vec![0], "on Motavia the Motavian town");
    player.board_at_mota();
    // Dezolis is the first row.
    assert_eq!(
        player.session.destination_view().unwrap().selected_world(),
        Some(WORLD_DEZOLIS)
    );
    player.press(Button::Speak);
    player.wait_until(6000, |p| {
        !p.runtime().scene_active() && p.runtime().map_id().0 == DEZO_SPACEPORT
    });
    assert_eq!(player.runtime().world_index(), WORLD_DEZOLIS);
    assert_eq!(
        listed(&player),
        vec![1],
        "after the flight the Dezolan town"
    );
}

/// H27, the consequence the brief named: after flying to Zelan, the menu at
/// Zelan's boarding row no longer offers Zelan and does offer Motavia.
#[test]
fn zelans_boarding_row_offers_motavia_after_the_flight_there() {
    let Some(mut player) = Player::at(MOTA_GANGWAY, &post_zio()) else {
        return;
    };
    player.board_at_mota();
    player.press(Button::Speak);
    player.wait_until(6000, |p| {
        !p.runtime().scene_active() && p.runtime().map_id().0 == ZELAN
    });
    // Two steps down onto row 48.
    player.walk_into_menu(Button::Down);
    let view = player.session.destination_view().unwrap();
    assert_eq!(
        view.rows,
        vec![WORLD_MOTAVIA],
        "Zelan is where the party is"
    );
    assert_eq!(view.mask, 0x90);
    // Cancel at Zelan: back at the arrival cell, world unchanged.
    player.tick(Pad::new(Button::Cancel));
    player.wait_until(600, |p| !p.runtime().scene_active());
    assert_eq!(player.runtime().map_id().0, ZELAN);
    assert_eq!(player.runtime().world_index(), WORLD_ZELAN);
    let cell = player.runtime().state().cell();
    assert_eq!((cell.x, cell.y), (31, 46));
    // And a confirm flies home to the Mota Spaceport with the world written.
    player.walk_into_menu(Button::Down);
    player.press(Button::Speak);
    player.wait_until(6000, |p| {
        !p.runtime().scene_active() && p.runtime().map_id().0 == MOTA_SPACEPORT
    });
    assert_eq!(player.runtime().world_index(), WORLD_MOTAVIA);
    let cell = player.runtime().state().cell();
    assert_eq!(
        (cell.x, cell.y),
        (30, 21),
        "loc_64B5A's Motavia row ($3C,$28)"
    );
}

/// A request whose flight table has no row for the map the party is on is a
/// scene fault, never a silent no-op: the event started somewhere with no
/// spaceport row, and a Cancel asks the return table for it.
#[test]
fn a_flight_table_without_a_row_is_a_scene_fault() {
    let Some(mut player) = Player::at((0x13, 48, 19), &post_zio()) else {
        return;
    };
    assert!(player.session.debug_start_event(0x800D));
    // The menu opens wherever the scene runs; Cancel resumes on the return leg.
    player.wait_until(400, |p| {
        p.session
            .destination_view()
            .is_some_and(|view| view.phase == DestinationPhase::Choosing)
    });
    let mut fault = None;
    for pad in
        std::iter::once(Pad::new(Button::Cancel)).chain(std::iter::repeat_n(Pad::NEUTRAL, 600))
    {
        let frame = player.session.frame(pad);
        for event in frame.events {
            if let RuntimeEvent::SceneFaulted { fault: found } = event {
                fault = Some(found);
            }
        }
        if fault.is_some() {
            break;
        }
    }
    assert!(
        matches!(fault, Some(SceneFault::NoFlightTarget { map: 0x13, .. })),
        "{fault:?}"
    );
    // The scene is over, not parked.
    player.wait_until(600, |p| !p.runtime().scene_active());
}

/// The menu is the session's mode: the frame it owns says so, and the scene's
/// request is reported as routed.
#[test]
fn the_menu_frames_are_destination_frames() {
    let Some(mut player) = Player::at(MOTA_GANGWAY, &post_zio()) else {
        return;
    };
    let mut opened = false;
    for _ in 0..400 {
        let frame = player.tick(Pad::new(Button::Up));
        opened |= frame.routed.contains(&Routed::DestinationMenu);
        if opened {
            break;
        }
    }
    assert!(opened, "the scene's request opened the window");
    let frame = player.tick(Pad::NEUTRAL);
    assert_eq!(frame.mode, FrameMode::Destination);
}

/// The frame counts the oracle measured (`oracle/tapes/35_ship_destination_menu.tape`,
/// Speak at frame 7401): the Zelan row's takeoff map loads 173 frames after the
/// press, Motavia's 179 (two more typed characters at three frames each), and
/// a Cancel returns the scene 55 frames after its press.
#[test]
fn the_answers_take_the_frames_the_cartridge_took() {
    let Some(mut player) = Player::at(MOTA_GANGWAY, &post_zio()) else {
        return;
    };
    let map_change_after = |player: &mut Player, press: Button| {
        let mut offset = 0u32;
        let mut frame = player.tick(Pad::new(press));
        loop {
            let changed = frame
                .events
                .iter()
                .any(|event| matches!(event, RuntimeEvent::MapChanged { .. }));
            if changed {
                return offset;
            }
            offset += 1;
            assert!(offset < 1000, "no map change");
            frame = player.tick(Pad::NEUTRAL);
        }
    };
    player.board_at_mota();
    assert_eq!(map_change_after(&mut player, Button::Speak), 173, "ZELAN");
    player.wait_until(6000, |p| {
        !p.runtime().scene_active() && p.runtime().map_id().0 == ZELAN
    });
    player.walk_into_menu(Button::Down);
    assert_eq!(map_change_after(&mut player, Button::Speak), 179, "MOTAVIA");
    // A Cancel at the same menu: a fresh session, since the party is airborne.
    let Some(mut player) = Player::at(MOTA_GANGWAY, &post_zio()) else {
        return;
    };
    player.board_at_mota();
    assert_eq!(map_change_after(&mut player, Button::Cancel), 55, "Cancel");
}

/// Tape 35 confirm at 7401; field control is mode $0C/routine 0, after the
/// landing refresh, ordinary reload/fade and place-name window, not map write.
#[test]
fn both_flights_return_control_on_tape_35s_exact_frame() {
    for (origin, world, button, target, expected_maps, expected) in [
        (
            MOTA_GANGWAY,
            0,
            Button::Up,
            ZELAN,
            vec![(173, 0), (1216, 0x18C), (1652, ZELAN)],
            1861,
        ),
        (
            (ZELAN, 31, 46),
            0x0300,
            Button::Down,
            MOTA_SPACEPORT,
            vec![(179, 0x18C), (946, 0), (1406, MOTA_SPACEPORT)],
            1596,
        ),
    ] {
        let mut story = post_zio();
        story.world_word = world;
        let Some(mut player) = Player::at(origin, &story) else {
            return;
        };
        player.walk_into_menu(button);
        let clock = player.session.main_frame_count();
        let mut maps = Vec::new();
        let mut ops = Vec::new();
        let mut caption_seen = false;
        let mut caption_at = None;
        let mut control = None;
        for offset in 0..2200 {
            let frame = player.tick(if offset == 0 {
                Pad::new(Button::Speak)
            } else {
                Pad::NEUTRAL
            });
            for event in &frame.events {
                match event {
                    RuntimeEvent::MapChanged { map, .. } => maps.push((offset, map.0)),
                    RuntimeEvent::ScenePresentation { op } => ops.push((offset, *op)),
                    RuntimeEvent::SceneEnded => control = Some(offset),
                    _ => {}
                }
            }
            if let Some(view) = player.session.flight_view()
                && let Some(caption) = view.caption.as_deref()
            {
                let packed = player
                    .runtime()
                    .data()
                    .ship_menu()
                    .unwrap()
                    .flight_caption(player.runtime().world_index())
                    .expect("rebuilt pack captions");
                assert_eq!(caption, packed, "d4=1 types the whole caption at once");
                caption_seen = true;
                caption_at.get_or_insert(offset);
            }
            if control.is_some() {
                break;
            }
        }
        eprintln!(
            "tape35 {origin:?} -> {target:#x}: maps={maps:?}; control={control:?}; ops={ops:?}"
        );
        assert_eq!(maps, expected_maps, "tape 35 map-word write edges");
        assert_eq!(control, Some(expected), "confirm to ordinary field control");
        assert_eq!(player.runtime().map_id().0, target);
        assert_eq!(
            player.session.main_frame_count().wrapping_sub(clock),
            expected as u16 + 1,
            "every presentation/setup frame spends the field frame clock"
        );
        if player
            .runtime()
            .data()
            .ship_menu()
            .unwrap()
            .flight_caption(0)
            .is_some()
        {
            assert!(caption_seen);
            assert_eq!(
                caption_at,
                Some(if target == ZELAN { 826 } else { 556 }),
                "completed caption DMA follows setup, not the initial clear"
            );
        } else {
            eprintln!("older pack has no flight captions; caption assertion skipped");
        }
        assert!(
            player.session.flight_view().is_none(),
            "no flight overlay after control"
        );
    }
}

/// Window_Draw (tape 35 frames 7246..7257) never reads the pad. Edges spent
/// there stay lost, including a held Speak carried across the final upload.
#[test]
fn opening_discards_all_pad_edges_and_does_not_defer_a_held_accept() {
    let Some(mut player) = Player::at(MOTA_GANGWAY, &post_zio()) else {
        return;
    };
    for _ in 0..400 {
        if player.session.destination_view().is_some() {
            break;
        }
        player.tick(Pad::new(Button::Up));
    }
    assert_eq!(
        player.session.destination_view().unwrap().phase,
        DestinationPhase::Opening
    );
    let clock = player.session.main_frame_count();
    // Frame-end DMA widths from the fresh tape-35 CPU trace, not the port's
    // growth formula: prompt 7246..7252, list 7254..7257. The odd final list
    // column is uploaded with its text, after Window_Draw's skipped DMA.
    let prompt_columns = [2, 6, 10, 14, 18, 22, 26, 26, 26, 26, 26, 26];
    let list_columns = [0, 0, 0, 0, 0, 0, 0, 0, 2, 6, 10, 11];
    for opening in 1_u8..=12 {
        let buttons = if opening == 12 {
            vec![Button::Speak]
        } else if opening % 2 == 0 {
            vec![Button::Cancel, Button::Camp, Button::Up]
        } else {
            vec![Button::Speak, Button::Down]
        };
        let frame = player.tick(Pad::of(&buttons));
        let view = player
            .session
            .destination_view()
            .expect("opening presses cannot close the menu");
        assert_eq!(
            view.phase,
            if opening < 12 {
                DestinationPhase::Opening
            } else {
                DestinationPhase::Choosing
            }
        );
        assert_eq!(
            view.prompt_columns,
            prompt_columns[usize::from(opening - 1)]
        );
        assert_eq!(view.prompt_ready, opening >= 8);
        assert_eq!(view.list_columns, list_columns[usize::from(opening - 1)]);
        assert_eq!(view.cursor, 0);
        assert!(view.typed.is_empty());
        assert_eq!(frame.sound, None);
    }
    assert_eq!(player.session.main_frame_count().wrapping_sub(clock), 12);
    assert_eq!(
        player.tick(Pad::new(Button::Speak)).sound,
        None,
        "held edge was spent during opening"
    );
    assert_eq!(
        player.session.destination_view().unwrap().phase,
        DestinationPhase::Choosing
    );
    player.tick(Pad::NEUTRAL);
    // Positive control: the same pad press after opening is read immediately.
    assert_eq!(player.tick(Pad::new(Button::Speak)).sound, Some(0xF3));
    assert_eq!(
        player.session.destination_view().unwrap().phase,
        DestinationPhase::Confirmed
    );
}

/// Re-derive the clone pin from the shell's event fixture at tick 30. This
/// proves the view state only; X11 captures must still certify the fixed image.
#[test]
fn certified_tick_70_still_has_the_settled_hidden_cursor_state() {
    let Some((data, _)) = pack() else {
        return;
    };
    let mut session =
        psiv_runtime::scene_fixture(data.clone(), 0x800D, psiv_core::StepFrames::default())
            .unwrap()
            .unwrap();
    for tick in 1..=70 {
        if tick == 30 {
            assert!(session.debug_start_event(0x800D));
        }
        session.frame(Pad::NEUTRAL);
    }
    let view = session.destination_view().unwrap();
    assert_eq!(view.phase, DestinationPhase::Choosing);
    assert_eq!((view.prompt_columns, view.list_columns), (26, 11));
    assert!(view.prompt_ready);
    assert!(!view.cursor_visible);
    assert!(!view.blink);
    assert_eq!(view.rows, [WORLD_ZELAN]);
}
