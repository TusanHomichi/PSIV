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
