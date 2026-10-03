//! The status strip's per-pane icon and ink line, decided the way
//! `Battle_DrawCommandIcons` (`ps4.asm:11056`) decides them every frame.
//!
//! Each of the five panes shows one 16x16 icon beside the name: the command
//! the fighter has chosen (`Battle_Command_Data`'s first byte of its four-byte
//! record), or a status icon that replaces it. The same routine picks the CRAM
//! line the pane's name and HP/TP text are drawn on, so a poisoned, asleep or
//! dead fighter's whole pane changes ink.

use super::view::{PaneView, PartyStatus};

/// The retail status bits the routine tests.
const POISONED: u8 = 0x01;
const PARALYZED: u8 = 0x02;
const DEAD: u8 = 0x04;
const ASLEEP: u8 = 0x08;
const ASLEEP_2: u8 = 0x20;
const ANDROID_DEAD: u8 = 0x40;

/// The icon index of a pane with no fighter (`loc_75F8`, `ps4.asm:11200`).
pub(super) const ICON_EMPTY: u8 = 9;
/// The icon a status writes into the command byte: paralysis, sleep, death
/// (`ps4.asm:11099-11110`).
const ICON_PARALYZED: u8 = 6;
const ICON_ASLEEP: u8 = 7;
const ICON_DEAD: u8 = 8;
/// The highest icon index that is a command (defend); above it the byte holds
/// a status icon.
const LAST_COMMAND_ICON: u8 = 5;

/// Runs one frame of `Battle_DrawCommandIcons` over the five command bytes and
/// the party strip, and returns what each pane draws.
///
/// The routine rewrites the command byte as it goes (a status icon replaces
/// the command, and a status icon whose status has gone reads back as no
/// command), so `bytes` is updated in place.
pub(super) fn refresh(bytes: &mut [u8; 5], party: &[PartyStatus]) -> Vec<PaneView> {
    (0..5u8)
        .map(|slot| {
            let fighter = slot + 1;
            let byte = &mut bytes[usize::from(slot)];
            let Some(member) = party.iter().find(|member| member.fighter == fighter) else {
                *byte = ICON_EMPTY;
                return PaneView {
                    fighter,
                    icon: ICON_EMPTY,
                    ink_line: 3,
                };
            };
            let status = member.status;
            // `andi.b #$6E`: paralyzed, dead, asleep, asleep 2, android dead.
            if status & (PARALYZED | DEAD | ASLEEP | ASLEEP_2 | ANDROID_DEAD) == 0
                && *byte > LAST_COMMAND_ICON
            {
                *byte = 0;
            }
            let ink_line = if status & (DEAD | ANDROID_DEAD) != 0 {
                *byte = ICON_DEAD;
                2
            } else if status & (PARALYZED | ASLEEP | ASLEEP_2) != 0 {
                if status & (ASLEEP | ASLEEP_2) != 0 {
                    *byte = ICON_ASLEEP;
                }
                if status & PARALYZED != 0 {
                    *byte = ICON_PARALYZED;
                }
                1
            } else if status & POISONED != 0 {
                0
            } else {
                3
            };
            PaneView {
                fighter,
                icon: *byte,
                ink_line,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(fighter: u8, status: u8) -> PartyStatus {
        PartyStatus {
            fighter,
            name: format!("m{fighter}"),
            hp: 10,
            max_hp: 10,
            tp: 0,
            status,
        }
    }

    #[test]
    fn a_healthy_member_shows_the_chosen_command_on_the_window_line() {
        let mut bytes = [2, 0, 0, 0, 0];
        let panes = refresh(&mut bytes, &[member(1, 0), member(2, 0)]);
        assert_eq!((panes[0].icon, panes[0].ink_line), (2, 3));
        assert_eq!((panes[1].icon, panes[1].ink_line), (0, 3));
    }

    #[test]
    fn empty_seats_show_the_empty_icon() {
        let mut bytes = [0; 5];
        let panes = refresh(&mut bytes, &[member(1, 0)]);
        assert_eq!(panes[4].icon, ICON_EMPTY);
        assert_eq!(bytes[4], ICON_EMPTY);
    }

    #[test]
    fn sleep_paralysis_and_death_replace_the_command_and_pick_their_line() {
        let mut bytes = [1; 5];
        let party = [
            member(1, ASLEEP),
            member(2, PARALYZED),
            member(3, DEAD),
            member(4, POISONED),
            member(5, TECH_SEALED),
        ];
        let panes = refresh(&mut bytes, &party);
        let got: Vec<_> = panes.iter().map(|p| (p.icon, p.ink_line)).collect();
        // Tech seal is not tested by the routine: that pane is a plain
        // attack-icon pane on the window line.
        assert_eq!(got, [(7, 1), (6, 1), (8, 2), (1, 0), (1, 3)]);
    }

    #[test]
    fn paralysis_outranks_sleep_and_a_cleared_status_reads_back_as_no_command() {
        let mut bytes = [3, 0, 0, 0, 0];
        let panes = refresh(&mut bytes, &[member(1, ASLEEP | PARALYZED)]);
        assert_eq!(panes[0].icon, ICON_PARALYZED);
        // The status wears off: the status icon is cleared, not the old
        // command restored.
        let panes = refresh(&mut bytes, &[member(1, 0)]);
        assert_eq!((panes[0].icon, bytes[0]), (0, 0));
    }

    const TECH_SEALED: u8 = 0x10;
}
