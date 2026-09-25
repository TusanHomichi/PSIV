//! The eleven character records.
//!
//! `Character_Stats` is at `$FFFFF500` with a stride of `$80`, and the RAM map
//! names all eleven individually — Chaz `$F500` through Seth `$FA00`
//! (`ps4.constants.asm:2398-2409`). Not five: the party holds five at a time,
//! but the cartridge keeps a record for every character whether or not they are
//! in it, and benched characters keep gaining experience. `FillBattleStats`
//! (`ps4.asm:11272`) walks the same eleven with `moveq #$A, d7`.
//!
//! # The record is [`Stats`], not a projection of it
//!
//! There is one copy of each character in RAM and battles read and write it in
//! place — `FillBattleStats` walks `Character_Stats` itself, and the experience
//! award writes `exp` straight into the same records. Nothing anywhere copies a
//! character into a battle-local structure and back.
//!
//! So the persistent record and the battle record are the same structure, and
//! this module stores [`Stats`] directly. A conversion layer would be inventing
//! a seam the hardware does not have, and every seam is somewhere to drift.
//! `docs/field/FIELD_STATE.md` carries the adjudication and the round-trip contract.
//!
//! # What survives a battle
//!
//! Everything except the `_battle` tier, which `FillBattleStats` overwrites
//! from `_mod` at the start of every battle. [`CharacterRoster::absorb`] writes
//! records back whole rather than picking fields, which is what makes that
//! true by construction instead of by maintenance.
//!
//! The `_battle` values are *saved* — the save routine copies `$F100`..`$FB00`
//! whole, character records included — and simply never read while stale.
//!
//! # `gain_exp_flag` and the two award passes
//!
//! `Battle_VictoryMessage`'s reward pass (`ps4.asm:4735-4782`) runs twice.
//!
//! **In the party.** For each occupied slot, `st gain_exp_flag(a0)` sets the
//! flag **unconditionally, before any other check**. Then `status & $44` masks
//! the two dead bits; if either is set the character gains no experience. So a
//! character who died in the battle still earns the flag, just not the reward.
//!
//! **Out of the party.** Gated on `EventFlag_Reunion` (`$DA`) being **clear**.
//! While it is, a second pass walks all eleven records, skips anyone already
//! paid as a party member, and gives the same per-head share to every character
//! whose `gain_exp_flag` is set. Once Reunion is set, benched characters stop
//! gaining — a hard stop, not a taper.
//!
//! Both passes clamp experience to [`EXPERIENCE_CAP`].
//!
//! `battle::split_rewards` documents the second pass as out of scope because
//! "that needs the roster of everyone recruited so far, which battle does not
//! own". This module owns it.

use crate::battle::{PartyMember, Stats};
use crate::error::MapError;
use crate::state::CharId;

/// Records in `Character_Stats`: `$F500` to `$FA80`, stride `$80`.
pub const CHARACTER_COUNT: usize = 11;

/// The ceiling both award passes clamp to — `cmpi.l #9999999, exp(a0)`.
pub const EXPERIENCE_CAP: u32 = 9_999_999;

/// `EventFlag_Reunion` (`ps4.constants.asm:1646`): once set, characters outside
/// the party stop gaining experience.
pub const REUNION_FLAG: u16 = 0xDA;

/// The dead bits in `status`, masked as `andi.b #$44, d0` — `StatusDead` (bit
/// 2) and the android dead bit (6). A character with either gains no
/// experience.
pub const DEAD_STATUS_MASK: u8 = 0x44;

/// The eleven character records.
///
/// A seat is `None` before the pack has been read; the new-game initialiser
/// fills all eleven at once, so in a running game every seat is occupied.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CharacterRoster {
    seats: Vec<Option<Stats>>,
}

impl CharacterRoster {
    /// An empty roster: eleven unseated slots.
    #[must_use]
    pub fn new() -> CharacterRoster {
        CharacterRoster {
            seats: vec![None; CHARACTER_COUNT],
        }
    }

    /// The record for `id`, or `None` if that seat is empty.
    #[must_use]
    pub fn get(&self, id: CharId) -> Option<&Stats> {
        self.seats.get(usize::from(id.0))?.as_ref()
    }

    /// The record for `id`, mutably.
    pub fn get_mut(&mut self, id: CharId) -> Option<&mut Stats> {
        self.seats.get_mut(usize::from(id.0))?.as_mut()
    }

    /// Seats a character.
    ///
    /// # Errors
    ///
    /// [`MapError::UnknownCharacter`] for an id outside `0..11`.
    pub fn seat(&mut self, id: CharId, stats: Stats) -> Result<(), MapError> {
        let seat = self
            .seats
            .get_mut(usize::from(id.0))
            .ok_or(MapError::UnknownCharacter { id: id.0 })?;
        *seat = Some(stats);
        Ok(())
    }

    /// Every seated character, in record order.
    pub fn seated(&self) -> impl Iterator<Item = (CharId, &Stats)> {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(index, seat)| Some((CharId(index as u8), seat.as_ref()?)))
    }

    /// How many seats are filled.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seats.iter().filter(|seat| seat.is_some()).count()
    }

    /// Whether no character has been seated yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Writes a finished battle's party back over the roster.
    ///
    /// [`crate::battle::Battle::into_party`] hands back the same records that
    /// went in, so this replaces them whole. Nothing is picked out and nothing
    /// is merged: a field added to [`Stats`] later round-trips without anyone
    /// remembering to add it here.
    ///
    /// Returns how many records were written. A member whose id names no seat
    /// is skipped rather than seated, because a battle cannot recruit.
    pub fn absorb(&mut self, party: &[PartyMember]) -> usize {
        let mut written = 0;
        for member in party {
            let Some(seat) = self.seats.get_mut(usize::from(member.character)) else {
                continue;
            };
            if seat.is_some() {
                *seat = Some(member.stats.clone());
                written += 1;
            }
        }
        written
    }

    /// The in-party half of the award: sets `gain_exp_flag` on every occupied
    /// slot and pays the living ones.
    ///
    /// `st gain_exp_flag(a0)` runs **before** the status check, so a character
    /// who died still earns the flag. Returns the ids that were actually paid.
    ///
    /// `battle::award` already does this for the fighters it holds; this exists
    /// for the roster-level path and for the dead-member case, which the battle
    /// side skips because its recipient list is the living only.
    pub fn award_party(&mut self, party: &[CharId], each: u16) -> Vec<CharId> {
        let mut paid = Vec::new();
        for id in party {
            let Some(stats) = self.get_mut(*id) else {
                continue;
            };
            stats.gain_exp_flag = true;
            if stats.status & DEAD_STATUS_MASK == 0 {
                stats.experience = stats
                    .experience
                    .saturating_add(u32::from(each))
                    .min(EXPERIENCE_CAP);
                paid.push(*id);
            }
        }
        paid
    }

    /// The out-of-party half: pays every seated character who is not in
    /// `party` and whose `gain_exp_flag` is set.
    ///
    /// `reunion` is whether `EventFlag_Reunion` is set. When it is, this pays
    /// nobody — the cartridge branches past the whole pass.
    ///
    /// Returns the ids paid. Note this pass does **not** check status: the
    /// cartridge's second loop tests only `gain_exp_flag` before adding.
    pub fn award_absent(&mut self, party: &[CharId], each: u16, reunion: bool) -> Vec<CharId> {
        if reunion {
            return Vec::new();
        }
        let mut paid = Vec::new();
        for index in 0..self.seats.len() {
            let id = CharId(index as u8);
            if party.contains(&id) {
                continue;
            }
            let Some(stats) = self.get_mut(id) else {
                continue;
            };
            if !stats.gain_exp_flag {
                continue;
            }
            stats.experience = stats
                .experience
                .saturating_add(u32::from(each))
                .min(EXPERIENCE_CAP);
            paid.push(id);
        }
        paid
    }

    /// The raw seats, for a snapshot.
    #[must_use]
    pub fn seats(&self) -> &[Option<Stats>] {
        &self.seats
    }

    /// Rebuilds from raw seats, for a save load. Extra seats are dropped and
    /// missing ones read as unseated.
    #[must_use]
    pub fn from_seats(seats: &[Option<Stats>]) -> CharacterRoster {
        let mut roster = CharacterRoster::new();
        for (index, seat) in seats.iter().take(CHARACTER_COUNT).enumerate() {
            roster.seats[index] = seat.clone();
        }
        roster
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::{StatPair, StatTriple};

    fn stats(level: u16, experience: u32) -> Stats {
        Stats {
            name_bytes: [0; 6],
            profession: 0,
            level,
            experience,
            curr_hp: 20,
            max_hp: 25,
            curr_tp: 10,
            max_tp: 10,
            status: 0,
            strength: StatTriple::uniform(8),
            mental: StatTriple::uniform(6),
            agility: StatTriple::uniform(7),
            dexterity: StatTriple::uniform(5),
            attack: StatPair::default(),
            defence: StatPair::default(),
            mental_defence: StatPair::default(),
            element_props: [0; 14],
            element_shadow: [0; 14],
            equipment: [0; 4],
            techniques: [0; 16],
            skills: [0; 8],
            curr_skill_uses: [0; 8],
            max_skill_uses: [0; 8],
            enemy_id: 0,
            gain_exp_flag: false,
            weapon_elements: Default::default(),
            physical_prop_save: 0,
        }
    }

    fn full() -> CharacterRoster {
        let mut roster = CharacterRoster::new();
        for id in 0..CHARACTER_COUNT as u8 {
            roster.seat(CharId(id), stats(1, 0)).unwrap();
        }
        roster
    }

    #[test]
    fn the_roster_holds_eleven_not_five() {
        // The party is five; the roster is everyone, because benched
        // characters keep levelling.
        let roster = full();
        assert_eq!(CHARACTER_COUNT, 11, "$F500..$FA80 at stride $80");
        assert_eq!(roster.len(), 11);
        assert_eq!(roster.seated().count(), 11);
        assert!(roster.get(CharId(10)).is_some(), "Seth");
        assert!(roster.get(CharId(11)).is_none(), "there is no twelfth");
    }

    #[test]
    fn a_new_roster_is_unseated() {
        let roster = CharacterRoster::new();
        assert!(roster.is_empty());
        assert!(roster.get(CharId(0)).is_none());
    }

    #[test]
    fn seating_past_the_last_character_is_rejected() {
        let mut roster = CharacterRoster::new();
        assert!(matches!(
            roster.seat(CharId(11), stats(1, 0)),
            Err(MapError::UnknownCharacter { id: 11 })
        ));
    }

    #[test]
    fn absorbing_a_battle_writes_records_back_whole() {
        // The round trip. `into_party` returns the same records that went in,
        // so this replaces rather than merges — which is what makes a field
        // added to Stats later survive without anyone updating this.
        let mut roster = full();
        let mut fought = stats(2, 26);
        fought.curr_hp = 3;
        fought.gain_exp_flag = true;
        fought.status = 0;

        let written = roster.absorb(&[PartyMember {
            character: 0,
            name: "Chaz".into(),
            stats: fought.clone(),
        }]);

        assert_eq!(written, 1);
        assert_eq!(roster.get(CharId(0)), Some(&fought), "byte for byte");
        assert_eq!(roster.get(CharId(1)).unwrap().level, 1, "nobody else moved");
    }

    #[test]
    fn hp_spent_in_a_battle_is_carried_out_of_it() {
        // Not restored. The field carries the damage; healing is an explicit
        // act.
        let mut roster = full();
        let mut fought = stats(1, 0);
        fought.curr_hp = 1;
        roster.absorb(&[PartyMember {
            character: 0,
            name: "Chaz".into(),
            stats: fought,
        }]);
        assert_eq!(roster.get(CharId(0)).unwrap().curr_hp, 1);
        assert_eq!(roster.get(CharId(0)).unwrap().max_hp, 25);
    }

    #[test]
    fn a_battle_cannot_recruit_an_unseated_character() {
        let mut roster = CharacterRoster::new();
        roster.seat(CharId(0), stats(1, 0)).unwrap();
        let written = roster.absorb(&[
            PartyMember {
                character: 0,
                name: "Chaz".into(),
                stats: stats(2, 26),
            },
            PartyMember {
                character: 4,
                name: "Gryz".into(),
                stats: stats(9, 900),
            },
        ]);
        assert_eq!(written, 1);
        assert!(roster.get(CharId(4)).is_none(), "still unseated");
    }

    #[test]
    fn a_dead_party_member_earns_the_flag_but_not_the_experience() {
        // `st gain_exp_flag(a0)` runs before `andi.b #$44, d0`.
        let mut roster = full();
        roster.get_mut(CharId(1)).unwrap().status = DEAD_STATUS_MASK & 0x04;

        let paid = roster.award_party(&[CharId(0), CharId(1)], 9);

        assert_eq!(paid, vec![CharId(0)], "only the living one was paid");
        assert_eq!(roster.get(CharId(0)).unwrap().experience, 9);
        assert_eq!(roster.get(CharId(1)).unwrap().experience, 0);
        assert!(
            roster.get(CharId(1)).unwrap().gain_exp_flag,
            "the dead one still earned the flag"
        );
    }

    #[test]
    fn absent_characters_are_paid_only_once_they_have_the_flag() {
        let mut roster = full();
        roster.get_mut(CharId(5)).unwrap().gain_exp_flag = true;

        let paid = roster.award_absent(&[CharId(0), CharId(1)], 9, false);

        assert_eq!(paid, vec![CharId(5)]);
        assert_eq!(roster.get(CharId(5)).unwrap().experience, 9);
        assert_eq!(
            roster.get(CharId(6)).unwrap().experience,
            0,
            "no flag, no pay"
        );
        assert_eq!(
            roster.get(CharId(0)).unwrap().experience,
            0,
            "party members are the first pass's business, not this one"
        );
    }

    #[test]
    fn reunion_is_a_hard_stop_on_absent_pay() {
        // Not a taper — the cartridge branches past the whole second pass.
        let mut roster = full();
        roster.get_mut(CharId(5)).unwrap().gain_exp_flag = true;

        assert!(roster.award_absent(&[CharId(0)], 9, true).is_empty());
        assert_eq!(roster.get(CharId(5)).unwrap().experience, 0);

        // And it really was the flag that would otherwise have paid.
        assert_eq!(roster.award_absent(&[CharId(0)], 9, false), vec![CharId(5)]);
    }

    #[test]
    fn the_absent_pass_ignores_status_where_the_party_pass_does_not() {
        // The cartridge's second loop tests `gain_exp_flag` and nothing else —
        // there is no `andi.b #$44` in it. A dead benched character is paid.
        let mut roster = full();
        let seth = roster.get_mut(CharId(10)).unwrap();
        seth.gain_exp_flag = true;
        seth.status = 0x04;

        assert_eq!(
            roster.award_absent(&[CharId(0)], 9, false),
            vec![CharId(10)]
        );
        assert_eq!(roster.get(CharId(10)).unwrap().experience, 9);
    }

    #[test]
    fn experience_clamps_at_the_cartridges_ceiling() {
        let mut roster = full();
        roster.get_mut(CharId(0)).unwrap().experience = EXPERIENCE_CAP - 1;
        roster.get_mut(CharId(5)).unwrap().experience = EXPERIENCE_CAP - 1;
        roster.get_mut(CharId(5)).unwrap().gain_exp_flag = true;

        roster.award_party(&[CharId(0)], 500);
        roster.award_absent(&[CharId(0)], 500, false);

        assert_eq!(EXPERIENCE_CAP, 9_999_999, "cmpi.l #9999999");
        assert_eq!(roster.get(CharId(0)).unwrap().experience, EXPERIENCE_CAP);
        assert_eq!(roster.get(CharId(5)).unwrap().experience, EXPERIENCE_CAP);
    }

    #[test]
    fn the_level_up_tape_round_trips_through_the_roster() {
        // Ground truth: oracle tape 10, `oracle/logs/verify_levelup.csv`.
        // Chaz's second level-up, frame by frame:
        //
        //   f51600  level 1, exp 17, hp 20/25, tp 10/10, str 8 men 6 agi 7 dex 5
        //   f51785  exp 17 -> 26        (pool 27 split three ways = 9 each)
        //   f51786  level 1 -> 2
        //   f51817  str 8 -> 9, men 6 -> 7
        //   f51833  agi 7 -> 8, dex 5 -> 6
        //   f51849  maxhp 25 -> 31, maxtp 10 -> 13
        //   f51899  level 2, exp 26, hp 20/31, tp 10/13
        //
        // The stat rises are staged over ~64 frames for presentation; only the
        // end state persists. Computing the rise is `battle::apply_level_ups`
        // and is tested there. What this pins is the half this module owns:
        // the award arithmetic, and that the record comes out of a battle
        // holding exactly what went into it.
        let mut roster = CharacterRoster::new();
        let mut chaz = stats(1, 17);
        chaz.curr_hp = 20;
        chaz.max_hp = 25;
        chaz.curr_tp = 10;
        chaz.max_tp = 10;
        roster.seat(CharId(0), chaz).unwrap();

        // The award, three recipients sharing a pool of 27.
        let paid = roster.award_party(&[CharId(0)], 9);
        assert_eq!(paid, vec![CharId(0)]);
        assert_eq!(roster.get(CharId(0)).unwrap().experience, 26, "f51785");

        // What the battle hands back, as the tape measured it at f51899.
        let mut levelled = roster.get(CharId(0)).unwrap().clone();
        levelled.level = 2;
        levelled.strength = StatTriple::uniform(9);
        levelled.mental = StatTriple::uniform(7);
        levelled.agility = StatTriple::uniform(8);
        levelled.dexterity = StatTriple::uniform(6);
        levelled.max_hp = 31;
        levelled.max_tp = 13;

        roster.absorb(&[PartyMember {
            character: 0,
            name: "Chaz".into(),
            stats: levelled,
        }]);

        let persisted = roster.get(CharId(0)).unwrap();
        assert_eq!(persisted.level, 2);
        assert_eq!(persisted.experience, 26);
        assert_eq!(persisted.strength.base, 9);
        assert_eq!(persisted.mental.base, 7);
        assert_eq!(persisted.agility.base, 8);
        assert_eq!(persisted.dexterity.base, 6);
        assert_eq!(persisted.max_hp, 31);
        assert_eq!(persisted.max_tp, 13);
        assert!(persisted.gain_exp_flag, "won a battle");

        // The load-bearing half of the contract: a level-up raises the maxima
        // and does **not** refill. Chaz walks out on 20 of 31.
        assert_eq!(persisted.curr_hp, 20, "hp did not rise with max_hp");
        assert_eq!(persisted.curr_tp, 10, "tp did not rise with max_tp");
    }

    #[test]
    fn a_stale_battle_tier_survives_a_save_because_retail_saves_it() {
        // The save routine copies $F100..$FB00 whole, character records
        // included, so the `_battle` bytes are in the file. They are stale
        // between battles and never read while stale — `FillBattleStats`
        // overwrites all seven from `_mod` at the start of the next one.
        //
        // Carrying them is therefore correct, and filtering them out on the
        // save path would be the deviation.
        let mut roster = full();
        let chaz = roster.get_mut(CharId(0)).unwrap();
        chaz.strength = StatTriple {
            base: 8,
            modified: 8,
            battle: 99,
        };

        let rebuilt = CharacterRoster::from_seats(roster.seats());
        assert_eq!(
            rebuilt.get(CharId(0)).unwrap().strength.battle,
            99,
            "the stale value came back, unfiltered"
        );
        assert_eq!(rebuilt, roster);
    }

    #[test]
    fn seats_round_trip() {
        let roster = full();
        let rebuilt = CharacterRoster::from_seats(roster.seats());
        assert_eq!(rebuilt, roster);

        // A short save leaves the tail unseated rather than panicking.
        let short = CharacterRoster::from_seats(&roster.seats()[..3]);
        assert_eq!(short.len(), 3);
        assert!(short.get(CharId(10)).is_none());
    }
}
