use std::collections::{BTreeMap, VecDeque};

use psiv_core::battle::BattleEvent;
use psiv_runtime::BattleTimeline;

/// One presentation event plus the retail SFX raised when it starts.
pub(super) struct QueuedBattleEvent {
    pub(super) event: BattleEvent,
    pub(super) sounds: Vec<u8>,
}

/// Pairs a runtime timeline with its event-indexed sound sidecar.
pub(super) fn queue_timeline(timeline: BattleTimeline) -> VecDeque<QueuedBattleEvent> {
    let mut by_event = BTreeMap::<usize, Vec<u8>>::new();
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
    timeline
        .events
        .into_iter()
        .enumerate()
        .map(|(event_index, event)| QueuedBattleEvent {
            event,
            sounds: by_event.remove(&event_index).unwrap_or_default(),
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
