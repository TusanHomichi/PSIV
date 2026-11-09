//! Live enemy objects: advance the old objects before dispatching a new beat.
use super::AttackView;
use crate::BattleAnimationEvent;

#[derive(Default)]
pub(super) struct AttackTimeline {
    live: Vec<AttackView>,
    next_id: u64,
}

impl AttackTimeline {
    pub(super) fn advance(&mut self) {
        self.live.retain_mut(|attack| {
            attack.elapsed += 1;
            attack
                .event
                .total_frames
                .is_some_and(|total| attack.elapsed < total)
        });
    }

    pub(super) fn start(
        &mut self,
        event: &BattleAnimationEvent,
        target_x: Option<(psiv_core::battle::FighterId, i16)>,
    ) {
        // Each fighter owns one object; another dispatch replaces it, even
        // when its decoded art and event index are identical to the old one.
        self.live.retain(|attack| attack.event.actor != event.actor);
        if event.total_frames.is_some_and(|total| total > 0) {
            self.live.push(AttackView {
                id: self.next_id,
                event: event.clone(),
                elapsed: 0,
                // USA BattleObj_ZoranBultAtk waits for this first piece and
                // freezes it before its strike ($01528A..$0152C2).
                frozen_overlay: (event.enemy_id == 10).then_some((0, 2)),
                target_x,
            });
            self.next_id += 1;
        }
    }

    pub(super) fn views(&self) -> &[AttackView] {
        &self.live
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_core::battle::FighterId;

    fn event() -> BattleAnimationEvent {
        BattleAnimationEvent {
            event_index: 0,
            actor: FighterId::new(6).unwrap(),
            enemy_id: 10,
            sfx_id: 0xD8,
            frame_duration: Some(2),
            frame_count: Some(8),
            total_frames: Some(16),
            frame_durations: Some(vec![2; 8]),
            movement_proven: true,
            sprite_sheet_proven: true,
            flash_timing_proven: true,
        }
    }

    #[test]
    fn views_are_absolute_and_identical_dispatches_have_distinct_lifetimes() {
        let mut timeline = AttackTimeline::default();
        let event = event();
        timeline.start(&event, None);
        for age in 0..16 {
            let view = timeline.views().to_vec();
            assert_eq!(view.len(), 1);
            assert_eq!(view[0].elapsed, age);
            assert_eq!(view[0].id, 0);
            assert_eq!(
                timeline.views(),
                view,
                "reading a view cannot tick an attack"
            );
            timeline.advance();
        }
        assert!(timeline.views().is_empty());
        timeline.start(&event, None);
        assert_eq!(timeline.views()[0].id, 1);
        assert_eq!(timeline.views()[0].elapsed, 0);
        timeline.advance();
        timeline.start(&event, None);
        assert_eq!(timeline.views().len(), 1, "one object per fighter");
        assert_eq!(timeline.views()[0].id, 2);
        assert_eq!(timeline.views()[0].elapsed, 0);
    }

    #[test]
    fn unknown_or_zero_length_objects_are_never_live() {
        let mut timeline = AttackTimeline::default();
        let mut event = event();
        for total in [None, Some(0)] {
            event.total_frames = total;
            timeline.start(&event, None);
            timeline.advance();
            assert!(timeline.views().is_empty());
        }
    }
}
