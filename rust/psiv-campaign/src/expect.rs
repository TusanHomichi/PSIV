//! Assertions: the `expect` objective and a chapter's `closing` list.

use crate::driver::Driver;
use crate::halt::{Halt, HaltKind, Res};
use crate::menu::find_named;
use crate::route::{Expectation, FlagRef, NameOrId};

fn flag_text(flag: FlagRef) -> String {
    String::from(flag)
}

impl Driver {
    /// Halts unless every clause of `expect` holds. The game is read at rest:
    /// the party settles first, because a clause about where the party stands
    /// means where it stands once the scene is over.
    ///
    /// # Errors
    ///
    /// [`HaltKind::ExpectFailed`] naming every clause that does not hold.
    pub fn expect(&mut self, expect: &Expectation) -> Res {
        self.settle(false)?;
        let mut misses = Vec::new();
        let runtime = self.runtime();
        let game = runtime.game();
        for flag in &expect.flags_set {
            if !game.is_set(flag.0) {
                misses.push(format!("flag {} is clear, expected set", flag_text(*flag)));
            }
        }
        for flag in &expect.flags_clear {
            if !game.is_clear(flag.0) {
                misses.push(format!("flag {} is set, expected clear", flag_text(*flag)));
            }
        }
        let map = runtime.map_id().0;
        if let Some(want) = expect.map
            && map != want
        {
            misses.push(format!("map is {map:#x}, expected {want:#x}"));
        }
        if let Some(want) = expect.cell {
            let cell = crate::driver::standing_cell(runtime);
            if cell != want.cell() {
                misses.push(format!(
                    "cell is ({},{}), expected ({},{})",
                    cell.x, cell.y, want.0, want.1
                ));
            }
        }
        if let Some(want) = &expect.party {
            let party = runtime.camp_state().party;
            let names: Vec<&str> = party.iter().map(|m| m.name.as_str()).collect();
            let matches = want.len() == party.len()
                && want.iter().zip(&party).all(|(key, member)| {
                    find_named(
                        std::slice::from_ref(member),
                        key,
                        |m| m.name.as_str(),
                        |m| u32::from(m.id),
                    )
                    .is_some()
                });
            if !matches {
                misses.push(format!(
                    "party is [{}], expected [{}]",
                    names.join(", "),
                    want.iter()
                        .map(NameOrId::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        if let Some(want) = expect.money_at_least {
            let money = game.money();
            if money < want {
                misses.push(format!("money is {money}, expected at least {want}"));
            }
        }
        if let Some(want) = expect.vehicle {
            let riding = runtime.vehicle_index().unwrap_or(0);
            if riding != want {
                misses.push(format!("vehicle is {riding}, expected {want}"));
            }
        }
        if !expect.items_held.is_empty() || !expect.items_absent.is_empty() {
            let pack = runtime.camp_state().inventory;
            let holds = |key: &NameOrId| {
                find_named(&pack, key, |i| i.name.as_str(), |i| u32::from(i.id)).is_some()
            };
            for item in &expect.items_held {
                if !holds(item) {
                    misses.push(format!("the pack holds no {item}, expected one"));
                }
            }
            for item in &expect.items_absent {
                if holds(item) {
                    misses.push(format!("the pack holds {item}, expected none"));
                }
            }
        }
        if expect.status_clear {
            for member in runtime.camp_state().party {
                if member.status != 0 {
                    misses.push(format!(
                        "{} has status {:#04x}, expected 0",
                        member.name, member.status
                    ));
                }
            }
        }
        if misses.is_empty() {
            Ok(())
        } else {
            Err(Halt::new(HaltKind::ExpectFailed, misses.join("; ")))
        }
    }
}
