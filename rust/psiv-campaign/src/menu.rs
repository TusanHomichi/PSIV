//! Menu navigation shared by the shop, inn and camp controllers: move a cursor
//! the view reports onto the row the objective wants, one press at a time.

use psiv_runtime::Button;

use crate::driver::Driver;
use crate::halt::{Halt, HaltKind, Res};
use crate::route::NameOrId;

/// Presses a menu may need to bring a cursor home: more rows than any list.
const CURSOR_PRESSES: usize = 64;

impl Driver {
    /// Presses Up or Down until `read` reports `wanted`.
    ///
    /// `rows` is the list's length when the window wraps (the camp's root, the
    /// shop's BUY/SELL, the item lists), and the press takes the shorter way
    /// round; `None` for a window that stops at its ends.
    ///
    /// # Errors
    ///
    /// [`HaltKind::MenuEntryMissing`] when the cursor never arrives, or a halt
    /// from a frame.
    pub fn cursor_to(
        &mut self,
        what: &str,
        read: impl Fn(&Driver) -> Option<usize>,
        wanted: usize,
        rows: Option<usize>,
    ) -> Res {
        for _ in 0..CURSOR_PRESSES {
            let Some(current) = read(self) else {
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    format!("the {what} window is not open"),
                ));
            };
            if current == wanted {
                return Ok(());
            }
            let down = match rows {
                Some(rows) if rows > 0 => (wanted + rows - current % rows) % rows <= rows / 2,
                _ => wanted > current,
            };
            self.tap(if down { Button::Down } else { Button::Up })?;
        }
        Err(Halt::new(
            HaltKind::MenuEntryMissing,
            format!("the {what} cursor never reached row {wanted}"),
        ))
    }
}

/// The index of the entry `key` names: by number, or by name compared without
/// case.
pub fn find_named<T>(
    entries: &[T],
    key: &NameOrId,
    name: impl Fn(&T) -> &str,
    id: impl Fn(&T) -> u32,
) -> Option<usize> {
    entries.iter().position(|entry| match key {
        NameOrId::Id(wanted) => id(entry) == u32::from(*wanted),
        NameOrId::Name(wanted) => name(entry).eq_ignore_ascii_case(wanted),
    })
}

/// "name, name, name" for a halt that lists what a menu did have.
pub fn listing<T>(entries: &[T], name: impl Fn(&T) -> &str) -> String {
    entries.iter().map(name).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_without_case_and_numbers_match_ids() {
        let names = ["Chaz".to_owned(), "Alys".to_owned()];
        let by = |key| find_named(&names, &key, |n| n.as_str(), |_| 7);
        assert_eq!(by(NameOrId::Name("alys".into())), Some(1));
        assert_eq!(by(NameOrId::Name("Rune".into())), None);
        assert_eq!(by(NameOrId::Id(7)), Some(0));
        assert_eq!(by(NameOrId::Id(8)), None);
    }
}
