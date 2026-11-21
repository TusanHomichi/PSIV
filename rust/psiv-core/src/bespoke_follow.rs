//! Follow-target direction selection for bespoke field routines.

use crate::map::FieldMap;

use super::{BespokeContext, FollowTarget};

pub(super) fn follow_command(
    map: &FieldMap,
    npc_index: usize,
    target: FollowTarget,
    follow_goal: Option<(i32, i32)>,
    context: BespokeContext<'_>,
) -> u8 {
    let Some(npc) = map.npcs().get(npc_index) else {
        return 0;
    };
    let target_pixels = match target {
        FollowTarget::LeaderGuard | FollowTarget::LeaderRanch => context.driver_pixels,
        FollowTarget::PartyTail => context.party.last().map_or(context.driver_pixels, |cell| {
            (i32::from(cell.x) * 16, (i32::from(cell.y) - 1) * 16)
        }),
        FollowTarget::PreviousObject | FollowTarget::ObjectAt(_) => {
            let target_index = match target {
                FollowTarget::PreviousObject => npc_index.checked_sub(1),
                FollowTarget::ObjectAt(index) => Some(index),
                _ => None,
            };
            let Some(target_index) = target_index else {
                return 0;
            };
            let Some(target_npc) = map.npcs().get(target_index) else {
                return 0;
            };
            let (dx, dy) = target_npc.facing.opposite().delta();
            (
                (i32::from(target_npc.cell.x) + dx) * 16,
                (i32::from(target_npc.cell.y) - 1 + dy) * 16,
            )
        }
        FollowTarget::ObjectCell(_) => {
            let Some(goal) = follow_goal else {
                return 0;
            };
            goal
        }
    };
    let object_pixels = (
        i32::from(npc.cell.x) * 16 + i32::from(npc.offset.x),
        (i32::from(npc.cell.y) - 1) * 16 + i32::from(npc.offset.y),
    );
    let dy = object_pixels.1 - target_pixels.1;
    let dx = object_pixels.0 - target_pixels.0;
    let distance = |value: i32| value.unsigned_abs() <= 16;
    match target {
        FollowTarget::LeaderGuard => {
            if !(0..=16).contains(&dy) || dx == 0 || !distance(dx) {
                1
            } else if dx > 0 {
                3
            } else {
                4
            }
        }
        FollowTarget::LeaderRanch => {
            if dy.unsigned_abs() > 16 || dx == 0 || !distance(dx) {
                2
            } else if dx > 0 {
                3
            } else {
                4
            }
        }
        FollowTarget::PartyTail
        | FollowTarget::PreviousObject
        | FollowTarget::ObjectAt(_)
        | FollowTarget::ObjectCell(_) => {
            if dx != 0 {
                if horizontal_step(map, object_pixels.0, target_pixels.0) < 0 {
                    3
                } else {
                    4
                }
            } else if vertical_step(map, object_pixels.1, target_pixels.1) > 0 {
                2
            } else if vertical_step(map, object_pixels.1, target_pixels.1) < 0 {
                1
            } else {
                0
            }
        }
    }
}

fn horizontal_step(map: &FieldMap, current: i32, target: i32) -> i32 {
    axis_step(map, current, target, i32::from(map.width()) * 16)
}

fn vertical_step(map: &FieldMap, current: i32, target: i32) -> i32 {
    axis_step(map, current, target, i32::from(map.height()) * 16)
}

fn axis_step(map: &FieldMap, current: i32, target: i32, period: i32) -> i32 {
    let direct = target - current;
    if direct == 0 {
        return 0;
    }
    if !map.wraps() {
        return direct.signum();
    }
    let wrapped = period - direct.abs();
    if wrapped <= direct.abs() {
        -direct.signum()
    } else {
        direct.signum()
    }
}
