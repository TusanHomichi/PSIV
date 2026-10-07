//! Vahal Fort's platform and force-field barrier, through `Session::frame` with
//! pads only (issue #82).
//!
//! The campaign route does not reach Vahal Fort yet, so these start from a
//! constructed state (`Session::start(..).from_save(..)` with the story flags
//! set) and drive the ordinary input path: a held d-pad direction onto the
//! platform, a talk press at the barrier. The state that results is read back
//! from the runtime. The map-level cases (every platform, belt, terminal and
//! chest, with the live layout and collision) are
//! `rust/psiv-runtime/src/scene_vahal_tests.rs`; a pack built before the
//! platform chunks were added fails here by design (rebuild it, or set
//! `PSIV_RUNTIME_PACK`).

use std::path::Path;
use std::sync::OnceLock;

use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Button, Frame, Pad, Routed, Runtime, RuntimeEvent, Session};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

const VAHAL_FORT: u16 = 0x0C8;
const VAHAL_F2: u16 = 0x0CA;

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
                eprintln!("runtime pack absent; skipping Vahal Fort session tests");
                return None;
            }
            Some((
                GameData::load(path).expect("pack loads"),
                BattleFiles::load(path).expect("battle files load"),
            ))
        })
        .as_ref()
}

/// A player at `(map, x, y)` with Chaz, Rune, Rika, Wren and Demi.
struct Player {
    session: Session,
}

impl Player {
    fn at(map: u16, x: u16, y: u16, flags: &[u16]) -> Option<Player> {
        let (data, files) = pack()?;
        let initial = Session::start(data.clone())
            .with_battles(files.clone())
            .field()
            .expect("the pack boots");
        let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
        game.set_party([
            Some(CharId(0)),
            Some(CharId(3)),
            Some(CharId(5)),
            Some(CharId(7)),
            Some(CharId(6)),
        ]);
        for &flag in flags {
            game.set(Flag::event(flag)).expect("flag in range");
        }
        let mut session = Session::start(data.clone())
            .with_battles(files.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: map,
                    char_x: x * 16,
                    char_y: y * 16,
                },
            })
            .expect("the session starts");
        session.set_scene_dialogue_autoclose(true);
        Some(Player { session })
    }

    fn runtime(&self) -> &Runtime {
        self.session.runtime()
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

    /// Holds `button` until a scene is running; the frames it took.
    fn walk_into_scene(&mut self, button: Button) -> u32 {
        for frames in 0..200 {
            if self.runtime().scene_active() {
                self.tick(Pad::NEUTRAL);
                return frames;
            }
            self.tick(Pad::new(button));
        }
        panic!("no scene started");
    }

    /// Neutral frames until the scene is over, at most `limit`.
    fn finish_scene(&mut self, limit: u32) -> u32 {
        for frames in 0..limit {
            if !self.runtime().scene_active() {
                return frames;
            }
            self.tick(Pad::NEUTRAL);
        }
        panic!("the scene never finished");
    }

    /// A fresh press: down for a frame, up for the next.
    fn press(&mut self, button: Button) -> Frame {
        let frame = self.tick(Pad::new(button));
        self.tick(Pad::NEUTRAL);
        frame
    }
}

/// A held Down step onto the first platform starts the ride, which carries all
/// five party objects ten rows down and flips the platform's temp flag.
#[test]
fn walking_onto_the_first_platform_rides_it_down_with_the_whole_party() {
    let Some(mut player) = Player::at(VAHAL_F2, 45, 33, &[]) else {
        return;
    };
    assert_eq!(player.runtime().map_id().0, VAHAL_F2);
    assert!(player.runtime().game().is_clear(Flag::temp(0x09)));
    player.walk_into_scene(Button::Down);
    assert_eq!(player.runtime().scene_event().map(|e| e.0), Some(0x15));
    let frames = player.finish_scene(2_000);
    // The 60-frame settle, the 80-frame ride and a few frames of entry and exit.
    assert!((140..=180).contains(&frames), "{frames} frames");
    let cell = player.runtime().state().cell();
    assert_eq!((cell.x, cell.y), (45, 44));
    assert!(player.runtime().game().is_set(Flag::temp(0x09)));
    // Negative control: nothing fires again from standing on it.
    for _ in 0..60 {
        player.tick(Pad::NEUTRAL);
        assert!(!player.runtime().scene_active());
    }
    assert!(player.runtime().game().is_set(Flag::temp(0x09)));
}

/// Presses talk until a scene starts, then runs it out; the scene's event
/// index and the dialogue entries it opened, in order.
fn talk_through(player: &mut Player) -> (Option<u16>, Vec<u16>) {
    let mut entries = Vec::new();
    let mut event = None;
    let note = |frame: &Frame, entries: &mut Vec<u16>| {
        entries.extend(frame.routed.iter().filter_map(|r| match r {
            Routed::SceneDialogueSkipped { entry } => Some(*entry),
            _ => None,
        }));
    };
    for _ in 0..20 {
        let frame = player.press(Button::Speak);
        note(&frame, &mut entries);
        if player.runtime().scene_active() {
            event = player.runtime().scene_event().map(|e| e.0);
            break;
        }
    }
    for _ in 0..2_000 {
        if !player.runtime().scene_active() {
            break;
        }
        let frame = player.tick(Pad::NEUTRAL);
        note(&frame, &mut entries);
    }
    assert!(!player.runtime().scene_active(), "the scene never finished");
    (event, entries)
}

/// Talking to the force field's invisible blocks fires `$90`: the first talk
/// explains the barrier and sets `$B9`, every later one is the short refusal.
/// The barrier objects exist only until the Zema old man's `$B3` is set (the
/// map's own effect despawns objects 1 to 5 on it), so the state here is the
/// first visit, before him.
#[test]
fn the_force_field_barrier_explains_itself_once() {
    let Some(mut player) = Player::at(VAHAL_FORT, 46, 74, &[]) else {
        return;
    };
    // Face the blocks (the cell above is occupied, so the press only turns),
    // then talk.
    player.press(Button::Up);
    assert!(player.runtime().game().is_clear(Flag::event(0xB9)));
    let (event, entries) = talk_through(&mut player);
    assert_eq!(event, Some(0x90), "the talk fires Event_VahalFortBarrier");
    assert_eq!(entries, vec![1], "the long explanation");
    assert!(player.runtime().game().is_set(Flag::event(0xB9)));

    // The second talk is dialogue `$02` and writes nothing.
    let before = player.runtime().game().snapshot();
    let (event, entries) = talk_through(&mut player);
    assert_eq!(event, Some(0x90));
    assert_eq!(entries, vec![2], "the short refusal");
    assert_eq!(player.runtime().game().snapshot(), before);
    // Negative control: the party never got past the blocks.
    let cell = player.runtime().state().cell();
    assert_eq!((cell.x, cell.y), (46, 74));
}
