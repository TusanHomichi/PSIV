//! From a landing or a confirm to a scene: the map's trigger list and the
//! type-2 interaction-area probe.

use psiv_core::{
    Cell, EventIndex, Flag, GameState, PixelPos, TRIGGERS, TriggerContext, TriggerResult,
};

use crate::{Runtime, RuntimeEvent};

/// Mirrors `Interaction_ChkMapAreas`'s already-processed gate. A story area
/// with flag zero is explicitly unconditional; nonzero selectors are clear
/// until the corresponding handler records them.
fn interaction_flag_clear(game: &GameState, area: &psiv_data::InteractionArea) -> bool {
    match area.flag_type.id {
        0 if area.flag == 0 => true,
        0 => game.is_clear(Flag::event(u16::from(area.flag))),
        1 => game.is_clear(Flag::chest(u16::from(area.flag))),
        2 => game.is_clear(Flag::temp(u16::from(area.flag))),
        _ => false,
    }
}

impl Runtime {
    /// Evaluates the map's trigger list at a landing.
    pub(crate) fn evaluate_triggers(&mut self, cell: Cell) -> Vec<RuntimeEvent> {
        let mut events = Vec::new();
        let Some(record) = self.data.map(psiv_data::MapId(self.map.id().0)) else {
            return events;
        };
        let indices: Vec<u8> = record.events.iter().map(|&e| e as u8).collect();
        let standing = self.map.collision_at(cell).map(|c| c.to_raw());
        let ctx = TriggerContext {
            state: &self.game,
            at: PixelPos::from_cell(cell),
            standing,
            previously_standing: self.prev_standing,
        };
        let mut unsupported = None;
        let mut hit = None;
        for &index in &indices {
            let Some(trigger) = TRIGGERS.get(usize::from(index)) else {
                continue;
            };
            let result = if index == 0x0D {
                self.elevator_trigger(cell)
            } else {
                trigger.evaluate(&ctx)
            };
            match result {
                TriggerResult::NoEvent => {}
                TriggerResult::Unsupported(..) => {
                    if unsupported.is_none() {
                        unsupported = Some((index, result));
                    }
                }
                _ => {
                    hit = Some((index, result));
                    break;
                }
            }
        }
        let hit = hit.or(unsupported);
        self.prev_standing = standing;
        match hit {
            Some((index, TriggerResult::Fire(event))) => {
                if self.install_scene(event) {
                    events.push(RuntimeEvent::SceneStarted { trigger: index });
                } else {
                    events.push(RuntimeEvent::SceneMissing { event: event.0 });
                }
            }
            Some((index, TriggerResult::Unsupported(..))) => {
                events.push(RuntimeEvent::TriggerUnsupported { trigger: index });
            }
            Some((_, TriggerResult::FireWithoutIndex))
            | Some((_, TriggerResult::NoEvent))
            | None => {}
        }
        events
    }

    /// Runs the type-2 map interaction area probe on a consumed confirm.
    ///
    /// The pack has already resolved the record's 8-pixel source and
    /// `XYRangeJmpTbl` selector into collision cells. Other interaction
    /// handlers remain deliberately outside this path: their parameters are
    /// dialogue/chest-specific, not event indexes.
    pub(crate) fn start_interaction_event(&mut self, events: &mut Vec<RuntimeEvent>) -> bool {
        let leader = self.party.leader();
        let Some(adjacent) = leader.cell().neighbor(leader.facing()) else {
            return false;
        };
        let Some(record) = self.data.map(psiv_data::MapId(self.map.id().0)) else {
            return false;
        };
        let Some((area_index, parameter, event)) = record
            .interaction_areas
            .iter()
            .find(|area| {
                area.interaction_type == 2
                    && interaction_flag_clear(&self.game, area)
                    && area.rect.is_some_and(|rect| {
                        rect.contains(psiv_data::CellPos::new(
                            u32::from(adjacent.x),
                            u32::from(adjacent.y),
                        ))
                    })
            })
            .map(|area| (area.index, area.parameter, area.event_index))
        else {
            return false;
        };
        let Some(event) = event else {
            events.push(RuntimeEvent::SceneMissing {
                event: u16::from(parameter),
            });
            return true;
        };
        // Event_ElevatorDoorOpening exits silently unless the chunk at the
        // leader's object position is the original closed-door tile.
        if event == 0x13 && self.map_chunk_at(PixelPos::from_cell(leader.cell())) != Some(0x4F) {
            return true;
        }
        if self.install_scene(EventIndex(event)) {
            events.push(RuntimeEvent::SceneStartedFromInteraction {
                area: area_index,
                event,
            });
        } else {
            events.push(RuntimeEvent::SceneMissing { event });
        }
        true
    }
}
