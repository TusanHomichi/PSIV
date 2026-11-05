//! `Event_PickingFifthCharacter`'s reads and writes ([`FifthOp`]) on the row in
//! `Window_Option_Index_2`. The table is `scenes/fifth_character.rs`.

use super::{Blocked, SceneRunner, pixel_cell};
use crate::geom::Cell;
use crate::scene::{ActorRef, DialogueId, FifthOp, SceneEffect, SceneFault, ScriptedActor};
use crate::scenes::fifth_character::{FIRST_OBJECT, FifthRow, FifthTable};
use crate::state::{CharId, Flag, GameState, PARTY_SLOTS};

/// `Current_Party_Slot_5`.
const FIFTH_SLOT: usize = PARTY_SLOTS - 1;

fn home_cell(row: &FifthRow) -> Cell {
    pixel_cell(row.x, row.y)
}

/// `$071EE6..$071F0A`: the row whose stand is one cell ahead of the leader.
fn row_ahead(table: &FifthTable, ahead: Cell) -> Option<usize> {
    table.rows.iter().position(|row| home_cell(row) == ahead)
}

/// `loc_72156`: the first party slot holding someone in the table, with their
/// row.
fn leaver(table: &FifthTable, party: [Option<CharId>; PARTY_SLOTS]) -> Option<(usize, usize)> {
    party.iter().enumerate().find_map(|(slot, who)| {
        let row = table.rows.iter().position(|row| Some(row.who) == *who)?;
        Some((slot, row))
    })
}

impl SceneRunner {
    pub(super) fn fifth_op(
        &mut self,
        op: FifthOp,
        state: &mut GameState,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        let Some(table) = self.fifth_table else {
            return Some(SceneFault::NoFifthTable);
        };
        match op {
            FifthOp::WaitParty => {
                self.pc += 1;
                if self.party_walking() {
                    self.blocked = Blocked::PartyAtRest;
                }
            }
            FifthOp::RefillRecords => {
                for who in table.records {
                    if let Some(record) = state.roster_mut().get_mut(who) {
                        record.curr_hp = record.max_hp;
                        record.curr_tp = record.max_tp;
                        record.status = 0;
                    }
                    effects.push(SceneEffect::RosterChanged { who });
                }
                self.pc += 1;
            }
            FifthOp::PickRow => {
                let leader = ActorRef::PartyMember(0);
                let Some(walker) = self.actor(leader) else {
                    return Some(SceneFault::UnknownActor { actor: leader });
                };
                let facing = walker.facing;
                let (dx, dy) = facing.delta();
                let x = i32::from(walker.cell.x) + dx;
                let y = i32::from(walker.cell.y) + dy;
                let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
                    return Some(SceneFault::NoFifthRow);
                };
                let Some(row) = row_ahead(&table, Cell::new(x, y)) else {
                    return Some(SceneFault::NoFifthRow);
                };
                self.fifth_row = row;
                let actor = ActorRef::Npc(FIRST_OBJECT + row);
                let turned = facing.opposite();
                let Some(friend) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                friend.facing = turned;
                effects.push(SceneEffect::ActorFaced {
                    actor,
                    facing: turned,
                });
                self.pc += 1;
            }
            FifthOp::RunLine { offset } => {
                let entry = table.rows[self.fifth_row].first_line + offset;
                self.open_line(entry, effects);
            }
            FifthOp::BranchFifthSlotFree { if_free, if_taken } => {
                self.pc = if state.party_slot(FIFTH_SLOT).is_none() {
                    if_free
                } else {
                    if_taken
                };
            }
            FifthOp::PickLeaver => {
                let Some(found) = leaver(&table, state.party()) else {
                    return Some(SceneFault::NoFifthRow);
                };
                self.fifth_leaver = found;
                self.pc += 1;
            }
            FifthOp::RunLeaverLine => {
                let entry = table.rows[self.fifth_leaver.1].first_line + 3;
                self.open_line(entry, effects);
            }
            FifthOp::Exchange => return self.exchange(&table, state, effects),
            FifthOp::TakeFifthSlot => return self.take_fifth_slot(&table, state, effects),
            FifthOp::ClearPicked => {
                for row in table.rows {
                    let flag = Flag::event(row.picked);
                    if state.write(flag, false).is_err() {
                        return Some(SceneFault::BadWrite);
                    }
                    effects.push(SceneEffect::FlagChanged { flag, value: false });
                }
                self.pc += 1;
            }
            FifthOp::SetPicked => {
                let flag = Flag::event(table.rows[self.fifth_row].picked);
                if state.set(flag).is_err() {
                    return Some(SceneFault::BadWrite);
                }
                effects.push(SceneEffect::FlagChanged { flag, value: true });
                self.pc += 1;
            }
        }
        None
    }

    /// `Event_GetAndRunDialogue` on a row's line.
    fn open_line(&mut self, entry: u16, effects: &mut Vec<SceneEffect>) {
        effects.push(SceneEffect::DialogueOpen(DialogueId(entry)));
        self.dialogue_window = crate::scene::DialogueWindow::Standard;
        self.blocked = Blocked::Dialogue;
        self.pc += 1;
    }

    /// Whether any party object is still moving: the `$28(a4)` / `$2A(a4)` test
    /// of `loc_71E74`.
    pub(super) fn party_walking(&self) -> bool {
        self.actors
            .iter()
            .any(|a| matches!(a.actor, ActorRef::PartyMember(_)) && a.is_walking())
    }

    /// `$071F8C..$07207C`: the friend along steps back to their own stand, the
    /// friend picked takes their party slot from where they stood, and the
    /// macros follow the two.
    fn exchange(
        &mut self,
        table: &FifthTable,
        state: &mut GameState,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        let (slot, leaving_row) = self.fifth_leaver;
        let row = table.rows[self.fifth_row];
        let leaving = table.rows[leaving_row];
        // `jsr Event_RemoveCharFromMacros`.
        state.remove_from_macros(leaving.who);
        // The party object's struct over the leaver's home object, which then
        // takes dialogue `$19` and the home stand (`Event_MoveObjectStatic`).
        let party_actor = ActorRef::PartyMember(slot);
        let Some(facing) = self.actor(party_actor).map(|actor| actor.facing) else {
            return Some(SceneFault::UnknownActor { actor: party_actor });
        };
        let home = ActorRef::Npc(FIRST_OBJECT + leaving_row);
        let Some(object) = self.actor_mut(home) else {
            return Some(SceneFault::UnknownActor { actor: home });
        };
        object.park(home_cell(&leaving), facing);
        effects.push(SceneEffect::NpcRestored {
            npc_index: FIRST_OBJECT + leaving_row,
            facing,
        });
        // The picked friend's object struct over the party object, then cleared.
        if let Some(fault) = self.seat_picked(slot, row, effects) {
            return Some(fault);
        }
        if state.set_party_slot(slot, Some(row.who)).is_err() {
            return Some(SceneFault::BadWrite);
        }
        // `jsr Event_AddMacro`.
        state.add_to_macros(row.who);
        effects.push(SceneEffect::PartyChanged);
        self.pc += 1;
        None
    }

    /// `loc_72080`: the picked friend's object struct moves to `Character_5`,
    /// and `Current_Party_Slot_5` takes them.
    fn take_fifth_slot(
        &mut self,
        table: &FifthTable,
        state: &mut GameState,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        let row = table.rows[self.fifth_row];
        if let Some(fault) = self.seat_picked(FIFTH_SLOT, row, effects) {
            return Some(fault);
        }
        if state.set_party_slot(FIFTH_SLOT, Some(row.who)).is_err() {
            return Some(SceneFault::BadWrite);
        }
        state.add_to_macros(row.who);
        effects.push(SceneEffect::PartyChanged);
        self.pc += 1;
        None
    }

    /// The party object `slot` becomes the picked friend: at the friend's cell,
    /// with the friend's facing, its destination its own position
    /// (`move.w $30(a3), $38(a4)`, and the same on Y), so it stands there until
    /// the leader walks. The friend's own object is cleared.
    fn seat_picked(
        &mut self,
        slot: usize,
        row: FifthRow,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        let object = ActorRef::Npc(FIRST_OBJECT + self.fifth_row);
        let Some(friend) = self.actor(object).copied() else {
            return Some(SceneFault::UnknownActor { actor: object });
        };
        let cell = home_cell(&row);
        let actor = ActorRef::PartyMember(slot);
        if let Some(seat) = self.actor_mut(actor) {
            seat.park(cell, friend.facing);
        } else {
            self.actors
                .push(ScriptedActor::new(actor, cell, friend.facing));
        }
        effects.push(SceneEffect::NpcDespawned {
            npc_index: FIRST_OBJECT + self.fifth_row,
            count: 1,
        });
        effects.push(SceneEffect::ActorPlaced { actor, at: cell });
        effects.push(SceneEffect::PartyMemberWalksIn { slot });
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenes::{CHAZ, RIKA, RUNE, WREN};

    /// A made-up table: five friends on a diagonal, ids 20 to 24.
    fn synthetic() -> FifthTable {
        let row = |n: u8| FifthRow {
            x: 0x40 + 0x10 * i32::from(n),
            y: 0x40 + 0x20 * i32::from(n),
            who: CharId(20 + n),
            first_line: 0x30 + 4 * u16::from(n),
            picked: 0x60 + u16::from(n),
        };
        FifthTable {
            rows: [row(0), row(1), row(2), row(3), row(4)],
            records: [CharId(20), CharId(21), CharId(22), CharId(23), CharId(24)],
        }
    }

    /// `loc_72156` scans the slots in order, whichever slot the friend holds.
    #[test]
    fn the_leaver_is_the_first_slot_in_the_table() {
        let table = synthetic();
        let party = |slot: usize, who: CharId| {
            let mut slots = [Some(CHAZ), Some(RUNE), Some(RIKA), Some(WREN), Some(WREN)];
            slots[slot] = Some(who);
            slots
        };
        assert_eq!(leaver(&table, party(4, CharId(24))), Some((4, 4)));
        assert_eq!(leaver(&table, party(1, CharId(21))), Some((1, 1)));
        assert_eq!(leaver(&table, party(0, CharId(20))), Some((0, 0)));
        // Negative control: nobody in the table.
        assert_eq!(
            leaver(
                &table,
                [Some(CHAZ), Some(RUNE), Some(RIKA), Some(WREN), None]
            ),
            None
        );
    }

    #[test]
    fn a_stand_ahead_names_its_row_and_nothing_else_does() {
        let table = synthetic();
        for (index, row) in table.rows.iter().enumerate() {
            assert_eq!(row_ahead(&table, home_cell(row)), Some(index));
        }
        assert_eq!(row_ahead(&table, Cell::new(1, 1)), None);
    }
}
