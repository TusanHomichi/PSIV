use super::*;
use crate::battle::SliceRolls;
use crate::collision::CollisionGrid;
use crate::map::{MapId, Npc, NpcId};

fn map_with_npc(cell: Cell) -> FieldMap {
    let grid = CollisionGrid::filled(8, 8, 0).unwrap();
    FieldMap::new(
        MapId(0),
        grid,
        vec![],
        vec![Npc::new(NpcId(0), cell, Direction::Down)],
    )
    .unwrap()
}

fn map_with_object_pair() -> FieldMap {
    let grid = CollisionGrid::filled(8, 8, 0).unwrap();
    FieldMap::new(
        MapId(0),
        grid,
        vec![],
        vec![
            Npc::new(NpcId(0), Cell::new(2, 2), Direction::Right),
            Npc::new(NpcId(1), Cell::new(4, 5), Direction::Down),
        ],
    )
    .unwrap()
}

fn context<'a>(party: &'a [Cell], flags: BespokeFlags, frame: u16) -> BespokeContext<'a> {
    BespokeContext {
        frame,
        driver_pixels: (32, 16),
        party,
        flags,
    }
}

#[test]
fn a_fixed_attempt_keeps_the_facing_without_spending_rng() {
    let mut map = map_with_npc(Cell::new(2, 2));
    let kind = BespokeKind::Fixed {
        command: 4,
        speed: WanderSpeed::Selector0,
        leash: Leash {
            x_max: 2,
            y_max: 16,
            x: 2,
            y: 8,
        },
    };
    let mut set = BespokeSet::build(&map, &[(0, kind)]).unwrap();
    let mut rolls = SliceRolls::new(&[0x1234]);
    set.tick_one_for_npc(
        0,
        &mut map,
        &mut rolls,
        context(&[], BespokeFlags::default(), 0),
    );

    assert_eq!(map.npcs()[0].facing, Direction::Right);
    assert_eq!(map.npcs()[0].cell, Cell::new(2, 2));
    assert_eq!(rolls.drawn(), 0);
}

#[test]
fn mouse_uses_one_shared_roll_when_its_timer_expires() {
    let mut map = map_with_npc(Cell::new(2, 2));
    let kind = BespokeKind::Random {
        kind: BespokeRandom::Mouse,
        speed: WanderSpeed::Selector0,
        leash: Leash {
            x_max: 16,
            y_max: 16,
            x: 8,
            y: 8,
        },
    };
    let mut set = BespokeSet::build(&map, &[(0, kind)]).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    set.tick_one_for_npc(
        0,
        &mut map,
        &mut rolls,
        context(&[], BespokeFlags::default(), 0),
    );

    assert_eq!(map.npcs()[0].facing, Direction::Up);
    assert_eq!(map.npcs()[0].cell, Cell::new(2, 1));
    assert_eq!(rolls.drawn(), 1);
    assert_eq!(set.get(0).unwrap().timer(), 0x20);
}

#[test]
fn a_flagged_random_routine_is_quiet_until_its_event_gate() {
    let mut map = map_with_npc(Cell::new(2, 2));
    let kind = BespokeKind::FlaggedRandom {
        flag: BespokeFlag::IgglanovaZema,
        kind: BespokeRandom::Igglanova,
        speed: WanderSpeed::Selector0,
        leash: Leash {
            x_max: 16,
            y_max: 16,
            x: 8,
            y: 8,
        },
    };
    let mut set = BespokeSet::build(&map, &[(0, kind)]).unwrap();
    let mut rolls = SliceRolls::new(&[2]);
    set.tick_one_for_npc(
        0,
        &mut map,
        &mut rolls,
        context(&[], BespokeFlags::default(), 0),
    );
    assert_eq!(rolls.drawn(), 0);
    assert_eq!(map.npcs()[0].cell, Cell::new(2, 2));

    let flags = BespokeFlags {
        igglanova_zema: true,
        ..BespokeFlags::default()
    };
    set.tick_one_for_npc(0, &mut map, &mut rolls, context(&[], flags, 0));
    assert_eq!(rolls.drawn(), 1);
    assert_eq!(map.npcs()[0].facing, Direction::Down);
}

#[test]
fn object_follow_uses_the_retail_horizontal_first_target() {
    let mut map = map_with_object_pair();
    let kind = BespokeKind::Follow {
        target: FollowTarget::PreviousObject,
        speed: WanderSpeed::Selector0,
        leash: Leash {
            x_max: 32,
            y_max: 32,
            x: 16,
            y: 16,
        },
    };
    let mut set = BespokeSet::build(&map, &[(1, kind)]).unwrap();
    let mut rolls = SliceRolls::new(&[]);

    set.tick_one_for_npc(
        1,
        &mut map,
        &mut rolls,
        context(&[], BespokeFlags::default(), 0),
    );

    assert_eq!(map.npcs()[1].facing, Direction::Left);
    assert_eq!(map.npcs()[1].cell, Cell::new(3, 5));
    assert_eq!(rolls.drawn(), 0);
}

#[test]
fn object_follow_uses_the_retail_vertical_direction_when_aligned() {
    let mut map = map_with_object_pair();
    map.set_npc_facing(0, Direction::Down).unwrap();
    map.set_npc_cell(1, Cell::new(2, 5)).unwrap();
    let kind = BespokeKind::Follow {
        target: FollowTarget::ObjectAt(0),
        speed: WanderSpeed::Selector0,
        leash: Leash {
            x_max: 32,
            y_max: 32,
            x: 16,
            y: 16,
        },
    };
    let mut set = BespokeSet::build(&map, &[(1, kind)]).unwrap();
    let mut rolls = SliceRolls::new(&[]);

    set.tick_one_for_npc(
        1,
        &mut map,
        &mut rolls,
        context(&[], BespokeFlags::default(), 0),
    );

    assert_eq!(map.npcs()[1].facing, Direction::Up);
    assert_eq!(map.npcs()[1].cell, Cell::new(2, 4));
    assert_eq!(rolls.drawn(), 0);
}

fn guard_pair() -> FieldMap {
    let grid = CollisionGrid::filled(12, 8, 0).unwrap();
    FieldMap::new(
        MapId(0),
        grid,
        vec![],
        vec![
            Npc::new(NpcId(0), Cell::new(5, 3), Direction::Down),
            Npc::new(NpcId(0), Cell::new(6, 3), Direction::Down),
        ],
    )
    .unwrap()
}

fn door_guard(after: &'static [u8]) -> BespokeKind {
    BespokeKind::FlaggedPattern {
        flag: BespokeFlag::EspMansionGuards,
        before: 2,
        after,
        speed: WanderSpeed::Selector0,
        // `FieldObj_EsperGuard`'s init: `$3C = $10`, `$3D = 0`, `$3E = 8`, `$3F = 0`.
        leash: Leash {
            x_max: 16,
            y_max: 0,
            x: 8,
            y: 0,
        },
    }
}

/// Runs `ticks` frames with the permission flag set and returns the cells.
fn run_guards(kinds: [BespokeKind; 2], ticks: u16) -> (Cell, Cell) {
    let mut map = guard_pair();
    let mut set = BespokeSet::build(&map, &[(0, kinds[0]), (1, kinds[1])]).unwrap();
    let mut rolls = SliceRolls::new(&[]);
    let flags = BespokeFlags {
        esp_mansion_guards: true,
        ..BespokeFlags::default()
    };
    for frame in 0..ticks {
        for npc in 0..2 {
            set.tick_one_for_npc(npc, &mut map, &mut rolls, context(&[], flags, frame));
        }
    }
    (map.npcs()[0].cell, map.npcs()[1].cell)
}

/// `loc_4A0A4` with the tables `loc_4A0EC` (`03 02 FF`) and `loc_4A0F0`
/// (`04 02 FF`): the first guard steps left once, the second right once, the
/// vertical command is refused by the `$3D = 0` leash, and the `$FF` latches
/// (`move.w #2, $1C(a4)`), so neither moves again however long the frames run.
#[test]
fn the_esper_door_guards_step_aside_once_and_stay() {
    let kinds = [
        door_guard(PATTERN_ESPER_GUARD),
        door_guard(PATTERN_ESPER_GUARD_SECOND),
    ];
    let (left, right) = run_guards(kinds, 600);
    assert_eq!(left, Cell::new(4, 3), "the first guard stepped left once");
    assert_eq!(
        right,
        Cell::new(7, 3),
        "the second guard stepped right once"
    );
    let (left_later, right_later) = run_guards(kinds, 6000);
    assert_eq!((left, right), (left_later, right_later), "the latch holds");
}

/// `loc_4A022` with the table `loc_4A05A` (`03 03 02 FF`) and
/// `move.w #3, $1C(a4)`: two steps left, the refused down, then idle for good.
#[test]
fn the_musk_cat_guard_steps_aside_twice_and_stays() {
    let kinds = [door_guard(PATTERN_MUSK_GUARD), BespokeKind::StaticAnimation];
    let (guard, _) = run_guards(kinds, 600);
    assert_eq!(guard, Cell::new(3, 3));
    let (later, _) = run_guards(kinds, 6000);
    assert_eq!(guard, later);
}

/// The looping routines (`loc_49FCE`: `clr.w $1C(a4)`) restart on `$FF`; the
/// latch is the guards' alone. A looped pattern still comes round again.
#[test]
fn a_looping_pattern_restarts_on_its_terminator() {
    let mut map = map_with_npc(Cell::new(2, 2));
    let kind = BespokeKind::Pattern {
        commands: &[4, 3, IDLE],
        speed: WanderSpeed::Selector0,
        leash: Leash {
            x_max: 16,
            y_max: 16,
            x: 8,
            y: 8,
        },
    };
    let mut set = BespokeSet::build(&map, &[(0, kind)]).unwrap();
    let mut rolls = SliceRolls::new(&[]);
    let mut visits = vec![map.npcs()[0].cell.x];
    for frame in 0..600 {
        set.tick_one_for_npc(
            0,
            &mut map,
            &mut rolls,
            context(&[], BespokeFlags::default(), frame),
        );
        let x = map.npcs()[0].cell.x;
        if visits.last() != Some(&x) {
            visits.push(x);
        }
    }
    assert!(
        visits.len() >= 6 && visits.starts_with(&[2, 3, 2, 3, 2]),
        "right, left, and round again: {visits:?}"
    );
}
