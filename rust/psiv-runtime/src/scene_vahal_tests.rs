//! Vahal Fort and the Weapon Plant on the real maps (issue #82): the platforms
//! and belts fired by the ordinary landing trigger, the terminals and chests
//! fired by the interaction probe, and the story scenes fired by their flags.
//!
//! Every case starts from a constructed state (`Runtime::from_save` with the
//! story flags set), because the campaign route does not reach Vahal Fort yet:
//! these are state tests, not campaign evidence. They read the pack
//! `PSIV_RUNTIME_PACK` names, else the repository's, and a pack built before
//! the platform chunks were added fails here by design (rebuild it).
use super::*;
use psiv_core::{
    CharId, CollisionType, GameState, Input, PixelPos, RetailLocation, RetailSave, SceneOp,
};
use psiv_data::BattleFiles;

/// The pack, loaded once for the whole module.
fn pack() -> GameData {
    static DATA: std::sync::OnceLock<GameData> = std::sync::OnceLock::new();
    DATA.get_or_init(|| GameData::load(&pack_dir()).unwrap())
        .clone()
}

/// A runtime on `map` at `cell` with a party of `members` (character ids in
/// slot order) and the given flags.
fn rt_at(map: u16, cell: Cell, members: &[u8], flags: &[Flag]) -> Runtime {
    let data = pack();
    let mut game = GameState::new();
    let mut slots = [None; psiv_core::PARTY_SLOTS];
    for (slot, id) in members.iter().enumerate() {
        slots[slot] = Some(CharId(*id));
    }
    game.set_party(slots);
    for flag in flags {
        game.set(*flag).unwrap();
    }
    Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: cell.x * 16,
                char_y: cell.y * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap()
}

/// Chaz, Rune, Rika, Wren and Demi: the party that reaches Vahal Fort.
const PARTY: [u8; 5] = [0, 3, 5, 7, 6];

/// What a scene did, tick by tick.
#[derive(Default)]
struct Run {
    /// The event index of the scene that started, if one did.
    event: Option<u16>,
    /// `(tick, live chunk patches)` at every `MapRefreshed`.
    writes: Vec<ChunkFrame>,
    /// Ticks from the scene's first to its last frame.
    ticks: usize,
    /// Every sound the scene played, in order.
    sounds: Vec<u8>,
    /// The scene's dialogue entries.
    dialogues: Vec<u16>,
    /// Every tick's events.
    events: Vec<RuntimeEvent>,
    /// The cast's cells at each dialogue the scene opened.
    at_dialogue: Vec<Vec<Cell>>,
}

/// Presses `input` until a scene starts, then runs it out, closing dialogue as
/// it opens. Panics on any fault.
fn run_scene(rt: &mut Runtime, input: Input) -> Run {
    let mut run = Run::default();
    for _ in 0..40 {
        let events = checked_tick(rt, input);
        if let Some(event) = rt.scene_event() {
            run.event = Some(event.0);
            collect(rt, &events, 0, &mut run);
            break;
        }
    }
    assert!(run.event.is_some(), "no scene started on {input:?}");
    run_out(rt, &mut run);
    run
}

fn collect(rt: &mut Runtime, events: &[RuntimeEvent], tick: usize, run: &mut Run) {
    for event in events {
        match event {
            RuntimeEvent::MapRefreshed => run.writes.push((tick, rt.effects.chunk_patches.clone())),
            RuntimeEvent::ScenePresentation {
                op: SceneOp::PlaySound { id },
            } => run.sounds.push(*id),
            RuntimeEvent::SceneDialogue { entry } => {
                run.dialogues.push(*entry);
                run.at_dialogue
                    .push(rt.scene_actors().iter().map(|a| a.cell).collect());
                rt.dialogue_closed();
            }
            RuntimeEvent::SceneDialogueResume => {
                run.at_dialogue
                    .push(rt.scene_actors().iter().map(|a| a.cell).collect());
                rt.dialogue_closed();
            }
            _ => {}
        }
    }
    run.events.extend(events.iter().cloned());
}

/// Ticks a started scene to its end.
fn run_out(rt: &mut Runtime, run: &mut Run) {
    for tick in 1..20_000 {
        if !rt.scene_active() {
            run.ticks = tick;
            return;
        }
        let events = checked_tick(rt, Input::Neutral);
        collect(rt, &events, tick, run);
    }
    panic!("the scene never finished");
}

fn leader(rt: &Runtime) -> Cell {
    rt.state().cell()
}

/// Every party member's cell: the party is stacked after a ride or a carry.
fn party_cells(rt: &Runtime) -> Vec<Cell> {
    rt.party.members().iter().map(|m| m.cell).collect()
}

fn chunk(rt: &Runtime, x: i32, y: i32) -> Option<u16> {
    rt.map_chunk_at(PixelPos { x, y })
}

/// The collision of the four cells of chunk `(cx, cy)`.
fn chunk_collision(rt: &Runtime, cx: u16, cy: u16) -> Vec<Option<CollisionType>> {
    (0..4)
        .map(|i| {
            rt.map
                .collision_at(Cell::new(cx * 2 + i % 2, cy * 2 + i / 2))
        })
        .collect()
}

fn solid(rt: &Runtime, cx: u16, cy: u16) -> bool {
    chunk_collision(rt, cx, cy)
        .iter()
        .all(|c| *c == Some(CollisionType::Solid))
}

fn recovery(rt: &Runtime, cx: u16, cy: u16) -> bool {
    chunk_collision(rt, cx, cy)
        .iter()
        .all(|c| c.map(CollisionType::to_raw) == Some(2))
}

const VAHAL_F2: u16 = 0x0CA;
const WEAPON_PLANT_F1: u16 = 0x0C5;

#[test]
fn the_first_vahal_platform_rides_down_and_back_through_the_ordinary_trigger() {
    let mut rt = rt_at(VAHAL_F2, Cell::new(45, 33), &PARTY, &[]);
    // At rest, flag clear: the platform sits at chunk row 17 and the shaft
    // below it is solid (the map's own load effect wrote `$CC/$CD` there).
    assert_eq!(chunk(&rt, 22 * 32, 17 * 32), Some(0xCC));
    assert!(recovery(&rt, 22, 17));
    assert!(solid(&rt, 22, 22));

    // One step down onto it fires `RunEvent_VahFortMovingPlatform` ($0E).
    let down = run_scene(&mut rt, Input::Direction(Direction::Down));
    assert_eq!(down.event, Some(0x15));
    assert!(
        down.events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneStarted { trigger: 0x0E }))
    );
    // Start pair (the vacated platform), 61 frames later the end pair: the
    // 60-frame settle plus the ride (80 frames), the end write one frame on.
    assert_eq!(down.writes.len(), 2, "{:?}", down.writes);
    assert!(down.writes[0].1.contains(&(22, 17, 0xC2)));
    assert!(down.writes[0].1.contains(&(23, 17, 0xC3)));
    assert!(down.writes[1].1.contains(&(22, 22, 0xCC)));
    assert!(down.writes[1].1.contains(&(23, 22, 0xCD)));
    let gap = down.writes[1].0 - down.writes[0].0;
    assert!(
        (140..=144).contains(&gap),
        "settle and ride took {gap} ticks"
    );
    assert_eq!(down.sounds, vec![0xE7, 0xFC]);

    // The whole party arrived ten rows down, in one stack.
    assert_eq!(leader(&rt), Cell::new(45, 44));
    assert!(party_cells(&rt).iter().all(|c| *c == Cell::new(45, 44)));
    assert!(rt.game.is_set(Flag::temp(0x09)));
    // The map followed: the platform stands at row 22 and the old spot is
    // the solid shaft again.
    assert!(recovery(&rt, 22, 22));
    assert!(solid(&rt, 22, 17));
    assert_eq!(chunk(&rt, 22 * 32, 22 * 32), Some(0xCC));

    // A fresh load of the same state (the map's own `MapDataManager` effect,
    // gated on the temp flag the ride toggled) puts the platform where the live
    // writes left it.
    let reloaded = rt_at(VAHAL_F2, Cell::new(45, 44), &PARTY, &[Flag::temp(0x09)]);
    assert!(recovery(&reloaded, 22, 22));
    assert!(solid(&reloaded, 22, 17));
    assert_eq!(chunk(&reloaded, 22 * 32, 22 * 32), Some(0xCC));
    assert_eq!(
        chunk_collision(&reloaded, 22, 17),
        chunk_collision(&rt, 22, 17)
    );

    // Negative control: arriving on the platform's far end does not fire it
    // again (the previous cell's collision was the platform's own type 2).
    for _ in 0..30 {
        let events = checked_tick(&mut rt, Input::Neutral);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneStarted { .. }))
        );
    }
    assert_eq!(leader(&rt), Cell::new(45, 44));
    // Walking along the platform does not fire it either...
    for _ in 0..8 {
        checked_tick(&mut rt, Input::Direction(Direction::Left));
    }
    assert_eq!(leader(&rt), Cell::new(44, 44));
    assert!(!rt.scene_active());
    // ...but stepping off and on again does, now riding up (flag set).
    for _ in 0..8 {
        checked_tick(&mut rt, Input::Direction(Direction::Left));
    }
    assert_eq!(leader(&rt), Cell::new(43, 44));
    assert!(!rt.scene_active());
    let up = run_scene(&mut rt, Input::Direction(Direction::Right));
    assert_eq!(up.event, Some(0x15));
    assert_eq!(leader(&rt), Cell::new(44, 34));
    assert!(party_cells(&rt).iter().all(|c| *c == Cell::new(44, 34)));
    assert!(rt.game.is_clear(Flag::temp(0x09)), "toggled back");
    assert!(up.writes[0].1.contains(&(22, 22, 0xC0)));
    assert!(up.writes[1].1.contains(&(22, 17, 0xCC)));
    assert!(recovery(&rt, 22, 17));
    assert!(solid(&rt, 22, 22));
}

#[test]
fn the_second_vahal_platform_rises_from_the_bottom_when_its_flag_is_clear() {
    // The corridor below the shaft; one step up onto the platform's chunk.
    let mut rt = rt_at(VAHAL_F2, Cell::new(45, 60), &PARTY, &[]);
    assert!(recovery(&rt, 22, 29));
    let up = run_scene(&mut rt, Input::Direction(Direction::Up));
    assert_eq!(up.event, Some(0x16));
    assert_eq!(leader(&rt), Cell::new(45, 49));
    assert!(party_cells(&rt).iter().all(|c| *c == Cell::new(45, 49)));
    assert!(rt.game.is_set(Flag::temp(0x0A)));
    assert!(up.writes[0].1.contains(&(22, 29, 0xC0)));
    assert!(up.writes[1].1.contains(&(22, 24, 0xCC)));
    assert!(recovery(&rt, 22, 24));
    assert!(solid(&rt, 22, 29));
    // Negative control: platform 1's flag is its own; this ride left it alone.
    assert!(rt.game.is_clear(Flag::temp(0x09)));
}

/// One Weapon Plant F1 platform: the cell a player steps down from, the
/// event it fires, the row it ends on and the chunk pair it ends at.
struct WeaponPlantCase {
    from: Cell,
    event: u16,
    flag: u16,
    to_row: u16,
    end: (u32, u32),
}

#[test]
fn the_four_weapon_plant_platforms_carry_the_party_fourteen_rows_down() {
    let cases = [
        WeaponPlantCase {
            from: Cell::new(29, 41),
            event: 0x17,
            flag: 0x0D,
            to_row: 56,
            end: (14, 28),
        },
        WeaponPlantCase {
            from: Cell::new(35, 21),
            event: 0x18,
            flag: 0x0E,
            to_row: 36,
            end: (17, 18),
        },
        WeaponPlantCase {
            from: Cell::new(57, 21),
            event: 0x19,
            flag: 0x0F,
            to_row: 36,
            end: (28, 18),
        },
        WeaponPlantCase {
            from: Cell::new(63, 41),
            event: 0x1A,
            flag: 0x10,
            to_row: 56,
            end: (31, 28),
        },
    ];
    for case in cases {
        let mut rt = rt_at(WEAPON_PLANT_F1, case.from, &PARTY, &[]);
        let ride = run_scene(&mut rt, Input::Direction(Direction::Down));
        let label = format!("event ${:02X}", case.event);
        assert_eq!(ride.event, Some(case.event), "{label}");
        assert!(
            ride.events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneStarted { trigger: 0x0F })),
            "{label}"
        );
        let to = Cell::new(case.from.x, case.to_row);
        assert_eq!(leader(&rt), to, "{label}");
        assert!(party_cells(&rt).iter().all(|c| *c == to), "{label}");
        assert!(rt.game.is_set(Flag::temp(case.flag)), "{label}");
        assert!(
            ride.writes[1].1.contains(&(case.end.0, case.end.1, 0xCC)),
            "{label}"
        );
        assert!(
            recovery(&rt, case.end.0 as u16, case.end.1 as u16),
            "{label}"
        );
        // 60 settle frames and 112 ride frames separate the two writes.
        let gap = ride.writes[1].0 - ride.writes[0].0;
        assert!((172..=176).contains(&gap), "{label}: {gap} ticks");
        // The other three platforms' flags are untouched.
        for other in [0x0D, 0x0E, 0x0F, 0x10] {
            assert_eq!(
                rt.game.is_set(Flag::temp(other)),
                other == case.flag,
                "{label}: flag ${other:02X}"
            );
        }
    }
}

const VAHAL_F3: u16 = 0x0CB;
const VAHAL_FORT: u16 = 0x0C8;
const WEAPON_PLANT: u16 = 0x0C4;
const WEAPON_PLANT_F2: u16 = 0x0C6;
const WEAPON_PLANT_F3: u16 = 0x0C7;

/// A belt case: where the player stands, the pad direction that lands on the
/// belt's entry box, the event the trigger picks, and where the carry ends.
struct BeltCase {
    map: u16,
    start: Cell,
    press: Direction,
    flags: Vec<Flag>,
    trigger: u8,
    event: u16,
    end: Cell,
}

#[test]
fn belts_carry_the_party_to_the_first_cell_off_the_belt_and_the_terminal_flag_swaps_the_event() {
    let cases = [
        // Vahal Fort F2's vertical belt, chunk column 12, chunk rows 17-21:
        // the entry box is the first group of `RunEvent_VahFortConveyorBelt`,
        // whose event is `$1D` whatever the flags say. Ten cells down to the
        // first row of chunk row 22.
        BeltCase {
            map: VAHAL_F2,
            start: Cell::new(24, 33),
            press: Direction::Down,
            flags: vec![],
            trigger: 0x10,
            event: 0x1D,
            end: Cell::new(24, 44),
        },
        BeltCase {
            map: VAHAL_F2,
            start: Cell::new(24, 33),
            press: Direction::Down,
            flags: vec![Flag::temp(0x0B), Flag::temp(0x0C)],
            trigger: 0x10,
            event: 0x1D,
            end: Cell::new(24, 44),
        },
        // The third group's entry at row 50: `$1D` down with `$0C` clear (ten
        // cells, chunk rows 25-29), `$1E` up with it set (the chunk above is
        // not a belt, so one cell).
        BeltCase {
            map: VAHAL_F2,
            start: Cell::new(24, 49),
            press: Direction::Down,
            flags: vec![],
            trigger: 0x10,
            event: 0x1D,
            end: Cell::new(24, 60),
        },
        BeltCase {
            map: VAHAL_F2,
            start: Cell::new(24, 49),
            press: Direction::Down,
            flags: vec![Flag::temp(0x0C)],
            trigger: 0x10,
            event: 0x1E,
            end: Cell::new(24, 49),
        },
        // The horizontal belt (chunk row 31, chunk columns 14-17): `$1F` right
        // with `$0C` clear, `$20` left with it set.
        BeltCase {
            map: VAHAL_F2,
            start: Cell::new(27, 62),
            press: Direction::Right,
            flags: vec![],
            trigger: 0x10,
            event: 0x1F,
            end: Cell::new(36, 62),
        },
        BeltCase {
            map: VAHAL_F2,
            start: Cell::new(27, 62),
            press: Direction::Right,
            flags: vec![Flag::temp(0x0C)],
            trigger: 0x10,
            event: 0x20,
            end: Cell::new(27, 62),
        },
        // Weapon Plant F3's belt: its own trigger slot (`$11`), `$1D` down
        // while `$11` is clear.
        BeltCase {
            map: WEAPON_PLANT_F3,
            start: Cell::new(18, 19),
            press: Direction::Down,
            flags: vec![],
            trigger: 0x11,
            event: 0x1D,
            end: Cell::new(18, 32),
        },
    ];
    for case in cases {
        let mut rt = rt_at(case.map, case.start, &PARTY, &case.flags);
        let ride = run_scene(&mut rt, Input::Direction(case.press));
        let label = format!(
            "map ${:03X} from {:?} flags {:?}",
            case.map,
            case.start,
            case.flags.len()
        );
        assert_eq!(ride.event, Some(case.event), "{label}");
        assert!(
            ride.events.iter().any(
                |e| matches!(e, RuntimeEvent::SceneStarted { trigger } if *trigger == case.trigger)
            ),
            "{label}"
        );
        assert_eq!(leader(&rt), case.end, "{label}");
        assert!(party_cells(&rt).iter().all(|c| *c == case.end), "{label}");
        assert_eq!(ride.sounds, vec![0xE7, 0xFC], "{label}");
        // 16 frames a cell, not the party's 8.
        let travelled = u32::from(case.end.x.abs_diff(case.start.x))
            + u32::from(case.end.y.abs_diff(case.start.y));
        let cells = if travelled == 0 { 0 } else { travelled - 1 };
        assert!(
            ride.ticks as u32 >= cells.max(1) * 16,
            "{label}: {} ticks for {cells} cells",
            ride.ticks
        );
        // The belt flags are the terminals' to flip, not the belt's.
        for flag in [Flag::temp(0x0B), Flag::temp(0x0C)] {
            assert_eq!(rt.game.is_set(flag), case.flags.contains(&flag), "{label}");
        }
    }
}

#[test]
fn walking_the_belt_off_its_entry_box_starts_nothing() {
    // Negative control: the carry is fired by the entry boxes alone (rows 34-35
    // and 42-43 of this column). A player already on the belt column between
    // them walks it as ordinary ground.
    let mut rt = rt_at(VAHAL_F2, Cell::new(24, 36), &PARTY, &[]);
    for _ in 0..4 {
        for _ in 0..8 {
            let events = checked_tick(&mut rt, Input::Direction(Direction::Down));
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, RuntimeEvent::SceneStarted { .. })),
                "a belt carry fired at {:?}",
                rt.state().cell()
            );
        }
    }
    assert!(!rt.scene_active());
    assert_eq!(leader(&rt), Cell::new(24, 40));
}

/// Walks into a terminal's face and presses the confirm button.
fn use_terminal(rt: &mut Runtime, facing: Direction) -> Run {
    checked_tick(rt, Input::Neutral);
    checked_tick(rt, Input::Direction(facing));
    checked_tick(rt, Input::Neutral);
    let events = checked_tick(rt, Input::Action);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneStartedFromInteraction { .. })),
        "terminal at {:?}: {events:?}",
        rt.state().cell()
    );
    let mut run = Run {
        event: rt.scene_event().map(|e| e.0),
        ..Run::default()
    };
    run_out(rt, &mut run);
    run
}

#[test]
fn the_terminals_flip_the_belt_flag_of_the_half_the_leader_stands_in() {
    // (map, the cell in front of the terminal, the way it faces, the event,
    // the temp flag the leader's X selects). Vahal Fort splits at x = $1F0
    // (cell 31), the Weapon Plant at $300 (cell 48).
    let cases = [
        (VAHAL_F2, Cell::new(38, 61), Direction::Up, 0x1B, 0x0B),
        (VAHAL_F2, Cell::new(64, 45), Direction::Up, 0x1B, 0x0B),
        (VAHAL_F2, Cell::new(29, 44), Direction::Left, 0x1B, 0x0C),
        (
            WEAPON_PLANT_F3,
            Cell::new(32, 23),
            Direction::Up,
            0x1C,
            0x11,
        ),
        (
            WEAPON_PLANT_F3,
            Cell::new(60, 23),
            Direction::Up,
            0x1C,
            0x12,
        ),
    ];
    for (map, cell, facing, event, flag) in cases {
        let mut rt = rt_at(map, cell, &PARTY, &[]);
        let used = use_terminal(&mut rt, facing);
        let label = format!("map ${map:03X} at {cell:?}");
        assert_eq!(used.event, Some(event), "{label}");
        assert!(rt.game.is_set(Flag::temp(flag)), "{label}");
        for other in [0x0B, 0x0C, 0x11, 0x12] {
            assert_eq!(rt.game.is_set(Flag::temp(other)), other == flag, "{label}");
        }
        assert_eq!(used.sounds, vec![0xDB, 0xDB], "{label}");
        // Using it again restores the original direction.
        let again = use_terminal(&mut rt, facing);
        assert_eq!(again.event, Some(event), "{label}");
        assert!(rt.game.is_clear(Flag::temp(flag)), "{label}");
    }
}

/// Walks to a chest's face and presses the confirm button.
fn open_chest(rt: &mut Runtime) -> Run {
    checked_tick(rt, Input::Neutral);
    checked_tick(rt, Input::Direction(Direction::Up));
    checked_tick(rt, Input::Neutral);
    let events = checked_tick(rt, Input::Action);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneStartedFromInteraction { .. })),
        "chest at {:?}: {events:?}",
        rt.state().cell()
    );
    let mut run = Run {
        event: rt.scene_event().map(|e| e.0),
        ..Run::default()
    };
    run_out(rt, &mut run);
    run
}

#[test]
fn wren_installs_the_burst_roc_and_the_positron_bolt_from_their_chests() {
    // (map, the cell below the chest, the event, the chest's object index, the
    // dialogue, the skill slot and id Wren gains, the event flag, the chest
    // flag the interaction gate sets).
    let cases = [
        (
            WEAPON_PLANT_F2,
            Cell::new(51, 30),
            0x6E,
            7,
            8,
            5,
            0x0E,
            0x74,
            0x74,
        ),
        (VAHAL_F3, Cell::new(76, 62), 0x6F, 8, 6, 6, 0x0F, 0x90, 0x90),
    ];
    for (map, cell, event, chest, entry, slot, skill, event_flag, chest_flag) in cases {
        let mut rt = rt_at(map, cell, &PARTY, &[]);
        // Seat the roster the way `InitializeCharStats` does, from the pack.
        let files = BattleFiles::load(&pack_dir()).unwrap();
        rt.enable_battles(&files).unwrap();
        let before = rt.game.roster().get(CharId(7)).cloned();
        let label = format!("map ${map:03X}");
        assert!(rt.game.is_clear(Flag::chest(chest_flag)), "{label}");
        let run = open_chest(&mut rt);
        assert_eq!(run.event, Some(event), "{label}");
        assert_eq!(run.dialogues, vec![entry], "{label}");
        assert_eq!(run.sounds, vec![0xE1, 0xCF, 0xCF, 0xCF], "{label}");
        // The lid is up, Wren knows the skill, and both banks' flags are set:
        // the gate's chest flag and the scene's own event flag.
        assert_eq!(rt.map.npcs()[chest].facing, Direction::Up, "{label}");
        let wren = rt.game.roster().get(CharId(7)).unwrap();
        assert_eq!(wren.skills[slot], skill, "{label}");
        if let Some(before) = before {
            let mut expected = before;
            expected.skills[slot] = skill;
            assert_eq!(*wren, expected, "{label}: only the skill byte changed");
        }
        assert!(rt.game.is_set(Flag::event(event_flag)), "{label}");
        assert!(rt.game.is_set(Flag::chest(chest_flag)), "{label}");
        // Negative control: the gate has recorded the chest, so a second
        // confirm in front of it starts nothing.
        checked_tick(&mut rt, Input::Neutral);
        let again = checked_tick(&mut rt, Input::Action);
        assert!(
            !again
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneStartedFromInteraction { .. })),
            "{label}"
        );
        assert!(!rt.scene_active(), "{label}");
    }
}

/// Starts on the first landing and runs the story scene out.
fn story(map: u16, cell: Cell, members: &[u8], flags: &[Flag]) -> (Runtime, Run) {
    let mut rt = rt_at(map, cell, members, flags);
    let run = run_scene(&mut rt, Input::Neutral);
    (rt, run)
}

#[test]
fn the_story_triggers_fire_once_on_their_flags_and_say_their_lines() {
    // (map, flags that arm it, the trigger slot, the event, the dialogue, the
    // flag it sets).
    let cases = [
        (VAHAL_FORT, Cell::new(46, 76), 0xB3, 0x75, 0x8D, 0, 0xB4),
        (VAHAL_F2, Cell::new(46, 62), 0xB4, 0x76, 0x8E, 3, 0xB5),
        (WEAPON_PLANT, Cell::new(30, 22), 0xFFFF, 0x78, 0x92, 7, 0xC7),
    ];
    for (map, cell, arm, trigger, event, entry, sets) in cases {
        let arming: Vec<Flag> = if arm == 0xFFFF {
            vec![]
        } else {
            vec![Flag::event(arm)]
        };
        let label = format!("map ${map:03X} event ${event:02X}");
        // Negative control: the same map without its arming flag is quiet
        // (the Weapon Plant's trigger has none, but a set `$C7` silences it).
        let quiet_flags = if arm == 0xFFFF {
            vec![Flag::event(sets)]
        } else {
            vec![]
        };
        let mut quiet = rt_at(map, cell, &PARTY, &quiet_flags);
        for _ in 0..30 {
            let events = checked_tick(&mut quiet, Input::Neutral);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, RuntimeEvent::SceneStarted { .. })),
                "{label}"
            );
        }
        let (mut rt, run) = story(map, cell, &PARTY, &arming);
        assert_eq!(run.event, Some(event), "{label}");
        assert!(run.events.iter().any(
            |e| matches!(e, RuntimeEvent::SceneStarted { trigger: t } if usize::from(*t) == trigger)
        ));
        assert_eq!(run.dialogues, vec![entry], "{label}");
        assert!(rt.game.is_set(Flag::event(sets)), "{label}");
        // It fires once: the flag it sets disarms it.
        for _ in 0..30 {
            let events = checked_tick(&mut rt, Input::Neutral);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, RuntimeEvent::SceneStarted { .. })),
                "{label}"
            );
        }
    }
}

#[test]
fn the_dominators_aftermath_runs_from_its_trigger_with_the_real_party() {
    for fifth in [6, 4, 2, 8, 9, 10] {
        let members = [0, 3, 5, 7, fifth];
        let (rt, run) = story(VAHAL_F3, Cell::new(44, 45), &members, &[Flag::event(0xB6)]);
        let label = format!("fifth {fifth}");
        assert_eq!(run.event, Some(0x91), "{label}");
        assert!(
            run.events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneStarted { trigger: 0x77 }))
        );
        // Dialogue 5 to its yield, then the rest after the barrier palette.
        assert_eq!(run.dialogues, vec![5], "{label}");
        assert_eq!(run.at_dialogue.len(), 2, "{label}");
        assert!(rt.game.is_set(Flag::event(0xBB)), "{label}");
        // Step offset restored, leader at ($2E0,$2F0) = cell (46,48).
        assert_eq!(leader(&rt), Cell::new(46, 48), "{label}");
        // The ring at the first window: Chaz, Rune, Rika, Wren and the fifth.
        let ring = &run.at_dialogue[0];
        assert_eq!(
            ring[..5],
            [
                Cell::new(44, 47),
                Cell::new(46, 47),
                Cell::new(48, 47),
                Cell::new(46, 44),
                Cell::new(47, 47)
            ],
            "{label}"
        );
    }
    // Negative control: with `DaughterShutDown` ($BB) set the trigger is silent.
    let mut quiet = rt_at(
        VAHAL_F3,
        Cell::new(44, 45),
        &PARTY,
        &[Flag::event(0xB6), Flag::event(0xBB)],
    );
    for _ in 0..30 {
        let events = checked_tick(&mut quiet, Input::Neutral);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneStarted { .. }))
        );
    }
}
