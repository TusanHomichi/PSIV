//! From a landing or a confirm to a scene: the map's trigger list and the
//! map interaction-area probe.

use std::cell::RefCell;

use psiv_core::{
    Cell, EventIndex, Flag, GameState, PixelPos, TRIGGERS, TriggerContext, TriggerResult,
    evaluate_list,
};

use crate::{Runtime, RuntimeEvent};

/// The interaction events whose routine first reads the layout byte at the
/// leader's object position (`GetMapLayoutOffset` of `x >> 5`, `y >> 5`) and
/// returns at once unless it is the closed door: `(event, closed chunk)`.
/// `Event_ElevatorDoorOpening` (`cmpi.b #$4F, (a1)`, `ps4.asm:145601`) and the
/// Garuberk Tower's two door openings (`cmpi.b #$38, (a1)`, `ps4.asm:148657`
/// and `:148713`, ROM `$06F478` and `$06F52E`).
const CLOSED_DOOR_GUARDS: [(u16, u16); 3] = [(0x13, 0x4F), (0x35, 0x38), (0x36, 0x38)];

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
        let at = PixelPos::from_cell(cell);
        // The routines that read the layout use `curr_y_pos` plus or minus
        // `$10`, so resolve both live bytes here, once.
        let layout_below = self.map_chunk_at(PixelPos {
            x: at.x,
            y: at.y + 16,
        });
        let layout_above = self.map_chunk_at(PixelPos {
            x: at.x,
            y: at.y - 16,
        });
        // `UpdateRNGSeed` is the field stream `self.rng` holds. Scan with a
        // copy and store it back, so only routines that reach their draw
        // advance it.
        let rng = RefCell::new(self.rng.clone());
        let ctx = TriggerContext {
            state: &self.game,
            at,
            standing,
            previously_standing: self.prev_standing,
            layout_below,
            layout_above,
            rng: &rng,
        };
        let hit = evaluate_list(&TRIGGERS, &indices, &ctx);
        self.rng = rng.into_inner();
        self.prev_standing = standing;
        match hit {
            Some((index, TriggerResult::Fire(event))) => {
                if self.install_scene(event) {
                    events.push(RuntimeEvent::SceneStarted { trigger: index });
                } else {
                    events.push(RuntimeEvent::SceneMissing { event: event.0 });
                }
            }
            Some((_, TriggerResult::FireWithoutIndex | TriggerResult::NoEvent)) | None => {}
        }
        events
    }

    /// Runs the type-0/type-2 map interaction probe on a consumed confirm.
    ///
    /// The pack has already resolved the record's 8-pixel source and
    /// `XYRangeJmpTbl` selector into collision cells. Areas precede objects
    /// (`ps4.asm:118257-118264`); type 0's parameter is a dialogue byte, while
    /// type 2's parameter indexes the event table (`:118872-118878`).
    pub(crate) fn start_map_interaction(&mut self, events: &mut Vec<RuntimeEvent>) -> bool {
        let leader = self.party.leader();
        let Some(adjacent) = leader.cell().neighbor(leader.facing()) else {
            return false;
        };
        let Some(record) = self.data.map(psiv_data::MapId(self.map.id().0)) else {
            return false;
        };
        let Some(area) = record
            .interaction_areas
            .iter()
            .find(|area| {
                matches!(area.interaction_type, 0 | 2)
                    && interaction_flag_clear(&self.game, area)
                    && area.rect.is_some_and(|rect| {
                        rect.contains(psiv_data::CellPos::new(
                            u32::from(adjacent.x),
                            u32::from(adjacent.y),
                        ))
                    })
            })
            .cloned()
        else {
            return false;
        };
        // The retail gate tests and sets the selected bank before dispatch
        // (`ps4.asm:118845-118869`). Story flag zero is unconditional.
        match area.flag_type.id {
            0 if area.flag != 0 => {
                self.game
                    .set(Flag::event(u16::from(area.flag)))
                    .expect("interaction flag byte is in range");
            }
            1 => {
                self.game
                    .set(Flag::chest(u16::from(area.flag)))
                    .expect("interaction flag byte is in range");
            }
            2 => {
                self.game
                    .set(Flag::temp(u16::from(area.flag)))
                    .expect("interaction flag byte is in range");
            }
            _ => {}
        }
        if area.interaction_type == 0 {
            self.open_area_dialogue(area.parameter);
            return true;
        }
        let (area_index, parameter, event) = (area.index, area.parameter, area.event_index);
        let Some(event) = event else {
            events.push(RuntimeEvent::SceneMissing {
                event: u16::from(parameter),
            });
            return true;
        };
        // Door events whose first act tests the layout byte at the leader's
        // object position against the closed door and return when it differs.
        if let Some(&(_, closed)) = CLOSED_DOOR_GUARDS.iter().find(|(e, _)| *e == event)
            && self.map_chunk_at(PixelPos::from_cell(leader.cell())) != Some(closed)
        {
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
