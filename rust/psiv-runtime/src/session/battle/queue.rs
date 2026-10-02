//! The battle's event queue: one runtime timeline, paired with the retail
//! sound and animation cues keyed to each event.

use std::collections::{BTreeMap, VecDeque};

use psiv_core::battle::BattleEvent;

use crate::events::{BattleAnimationEvent, BattleSoundEvent, BattleTimeline};

/// The events still to play, each with its cues.
pub(crate) type QueuedQueue = VecDeque<QueuedEvent>;

/// One queued event with the retail cues keyed to it.
#[derive(Debug, Clone)]
pub(crate) struct QueuedEvent {
    pub(crate) event: BattleEvent,
    pub(crate) sounds: Vec<u8>,
    pub(crate) animations: Vec<BattleAnimationEvent>,
}

/// Pairs a runtime timeline with its event-indexed cues, in event order.
pub(crate) fn queue_timeline(timeline: BattleTimeline) -> QueuedQueue {
    let mut by_event = BTreeMap::<usize, Vec<u8>>::new();
    let mut animations_by_event = BTreeMap::<usize, Vec<BattleAnimationEvent>>::new();
    for BattleSoundEvent { event_index, id } in timeline.sounds {
        if event_index < timeline.events.len() {
            by_event.entry(event_index).or_default().push(id);
        } else {
            debug_assert!(
                false,
                "battle sound event index {event_index} exceeds timeline length {}",
                timeline.events.len()
            );
        }
    }
    for animation in timeline.animations {
        if animation.event_index < timeline.events.len() {
            animations_by_event
                .entry(animation.event_index)
                .or_default()
                .push(animation);
        } else {
            debug_assert!(
                false,
                "battle animation event index {} exceeds timeline length {}",
                animation.event_index,
                timeline.events.len()
            );
        }
    }
    timeline
        .events
        .into_iter()
        .enumerate()
        .map(|(event_index, event)| QueuedEvent {
            event,
            sounds: by_event.remove(&event_index).unwrap_or_default(),
            animations: animations_by_event.remove(&event_index).unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_core::battle::FighterId;

    /// The sound and animation sidecars stay keyed to the event that raises
    /// them, in event order. Moved here from `psiv-godot/src/battle/sfx.rs`.
    #[test]
    fn queued_events_keep_their_sound_and_animation_sidecars_in_order() {
        let party = FighterId::new(1).expect("party fighter");
        let zoran = FighterId::new(6).expect("zoran fighter");
        let gunner = FighterId::new(7).expect("gunner fighter");
        let queued = queue_timeline(BattleTimeline {
            events: vec![
                BattleEvent::Attacked {
                    actor: zoran,
                    targets: vec![party],
                },
                BattleEvent::Attacked {
                    actor: gunner,
                    targets: vec![party],
                },
            ],
            sounds: vec![
                BattleSoundEvent {
                    event_index: 0,
                    id: 0xD8,
                },
                BattleSoundEvent {
                    event_index: 1,
                    id: 0xD6,
                },
            ],
            animations: vec![BattleAnimationEvent {
                event_index: 1,
                actor: gunner,
                enemy_id: 2,
                sfx_id: 0xD6,
                frame_duration: None,
                frame_count: None,
                total_frames: None,
                frame_durations: None,
                movement_proven: false,
                sprite_sheet_proven: false,
                flash_timing_proven: false,
            }],
        });
        let queued: Vec<_> = queued.into_iter().collect();
        assert_eq!(
            queued.iter().map(|e| e.sounds.clone()).collect::<Vec<_>>(),
            vec![vec![0xD8], vec![0xD6]]
        );
        assert!(queued[0].animations.is_empty());
        assert_eq!(queued[1].animations[0].sfx_id, 0xD6);
    }
}
