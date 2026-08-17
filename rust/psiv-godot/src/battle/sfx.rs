use std::collections::{BTreeMap, VecDeque};

use psiv_core::battle::BattleEvent;
use psiv_runtime::{BattleAnimationEvent, BattleTimeline};

/// One presentation event plus the retail SFX raised when it starts.
pub(super) struct QueuedBattleEvent {
    pub(super) event: BattleEvent,
    pub(super) sounds: Vec<u8>,
    pub(super) animations: Vec<BattleAnimationEvent>,
}

/// Pairs a runtime timeline with its event-indexed sound sidecar.
pub(super) fn queue_timeline(timeline: BattleTimeline) -> VecDeque<QueuedBattleEvent> {
    let mut by_event = BTreeMap::<usize, Vec<u8>>::new();
    let mut animations_by_event = BTreeMap::<usize, Vec<BattleAnimationEvent>>::new();
    for sound in timeline.sounds {
        if sound.event_index < timeline.events.len() {
            by_event
                .entry(sound.event_index)
                .or_default()
                .push(sound.id);
        } else {
            debug_assert!(
                false,
                "battle sound event index {} exceeds timeline length {}",
                sound.event_index,
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
        .map(|(event_index, event)| QueuedBattleEvent {
            event,
            sounds: by_event.remove(&event_index).unwrap_or_default(),
            animations: animations_by_event.remove(&event_index).unwrap_or_default(),
        })
        .collect()
}

/// A small queue owned by the battle screen so audio dispatch remains ordered
/// with visual event playback and does not become an input-side effect.
#[derive(Default)]
pub(super) struct BattleSoundRequests {
    requests: VecDeque<u8>,
}

impl BattleSoundRequests {
    pub(super) fn clear(&mut self) {
        self.requests.clear();
    }

    pub(super) fn extend(&mut self, sounds: impl IntoIterator<Item = u8>) {
        self.requests.extend(sounds);
    }

    pub(super) fn take(&mut self) -> Vec<u8> {
        self.requests.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_core::battle::{BattleEvent, FighterId};

    #[test]
    fn queue_keeps_enemy_sfx_and_animation_sidecars_in_event_order() {
        let party = FighterId::new(1).expect("party fighter");
        let zoran = FighterId::new(6).expect("zoran fighter");
        let gunner = FighterId::new(7).expect("gunner fighter");
        let events = vec![
            BattleEvent::Attacked {
                actor: zoran,
                targets: vec![party],
            },
            BattleEvent::Attacked {
                actor: gunner,
                targets: vec![party],
            },
        ];
        let queued = queue_timeline(BattleTimeline {
            events,
            sounds: vec![
                psiv_runtime::BattleSoundEvent {
                    event_index: 0,
                    id: 0xD8,
                },
                psiv_runtime::BattleSoundEvent {
                    event_index: 1,
                    id: 0xD6,
                },
            ],
            animations: vec![
                BattleAnimationEvent {
                    event_index: 0,
                    actor: zoran,
                    enemy_id: 10,
                    sfx_id: 0xD8,
                    frame_duration: Some(2),
                    frame_count: Some(8),
                    total_frames: Some(16),
                    movement_proven: false,
                    flash_timing_proven: true,
                },
                BattleAnimationEvent {
                    event_index: 1,
                    actor: gunner,
                    enemy_id: 2,
                    sfx_id: 0xD6,
                    frame_duration: None,
                    frame_count: None,
                    total_frames: None,
                    movement_proven: false,
                    flash_timing_proven: false,
                },
            ],
        });
        let queued: Vec<_> = queued.into_iter().collect();
        assert_eq!(
            queued
                .iter()
                .map(|event| event.sounds.clone())
                .collect::<Vec<_>>(),
            vec![vec![0xD8], vec![0xD6]]
        );
        assert_eq!(
            queued
                .iter()
                .map(|event| event.animations[0].sfx_id)
                .collect::<Vec<_>>(),
            vec![0xD8, 0xD6]
        );
    }
}
