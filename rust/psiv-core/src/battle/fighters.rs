//! The nine fighter slots, and who occupies them.
//!
//! `Obj_Fighters` (`$FFFF4400`) is twelve `$40`-byte slots but only nine hold
//! combatants: five characters then four enemies. Everything in battle
//! identifies a combatant by its **one-based** index into that array —
//! `Current_Actor_Index`, `Current_Target_Index`, `Battle_Turn_Order`'s entry
//! word — and the ubiquitous `cmpi.w #5, d0 / bgt` is how the cartridge asks
//! "is this an enemy".
//!
//! The two indexing conventions sit one apart and are a classic source of
//! off-by-one bugs: `Fighters_Hit_Flags` and `Battle_Heal_Damage_List` are
//! indexed by **slot** (zero-based), while the queue and the actor/target
//! variables hold **ids** (one-based). [`FighterId::slot`] is the only place
//! that conversion happens.

use super::records::EnemyRecord;
use super::stats::Stats;

/// How many party slots there are.
pub const PARTY_SLOTS: usize = 5;

/// How many enemy slots there are.
pub const ENEMY_SLOTS: usize = 4;

/// How many fighter slots battle uses.
pub const FIGHTER_SLOTS: usize = PARTY_SLOTS + ENEMY_SLOTS;

/// The highest id that is still a character (`cmpi.w #5, d0 / bgt`).
pub const LAST_PARTY_ID: u8 = 5;

/// Which half of the battlefield a fighter is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// Ids 1..=5.
    Party,
    /// Ids 6..=9.
    Enemy,
}

impl Side {
    /// The other side.
    #[must_use]
    pub const fn opposing(&self) -> Side {
        match self {
            Side::Party => Side::Enemy,
            Side::Enemy => Side::Party,
        }
    }
}

/// A one-based fighter index, 1..=9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FighterId(u8);

impl FighterId {
    /// Builds from a one-based index, rejecting anything outside 1..=9.
    #[must_use]
    pub const fn new(index: u8) -> Option<FighterId> {
        if index >= 1 && index as usize <= FIGHTER_SLOTS {
            Some(FighterId(index))
        } else {
            None
        }
    }

    /// The one-based index, as the cartridge stores it.
    #[must_use]
    pub const fn get(&self) -> u8 {
        self.0
    }

    /// The zero-based slot, for the hit-flag and damage arrays.
    #[must_use]
    pub const fn slot(&self) -> usize {
        self.0 as usize - 1
    }

    /// Which side this id is on.
    #[must_use]
    pub const fn side(&self) -> Side {
        if self.0 > LAST_PARTY_ID {
            Side::Enemy
        } else {
            Side::Party
        }
    }
}

/// `reaction_flags` (`$2A` of the fighter object) — mainly for enemies.
///
/// The AI's memory of what was done to it since its last action, read by the
/// `EnemyAIInstructionsOffs` arms (see `super::enemy_ai`) and written by the
/// party's damage routines. The four arms that fire on a bit clear the **whole
/// byte**, so a bit survives exactly until the arm that reads it runs.
pub mod reaction {
    /// Bit 0 — hit by a plain physical attack: `bset #0, $2A(a4)` in
    /// `Character_DamageEnemy`, on both arms of its `if bugfixes` pair
    /// (`ps4.asm:3916`, the fork's; `3946`, retail's).
    pub const PHYSICAL: u8 = 1 << 0;
    /// Bit 1 — hit by a character's ability: `bset #1, $2A(a4)` at
    /// `ps4.asm:3989`, on the arms of `Character_DamageEnemy` that run when the
    /// actor's `ability(a3)` is nonzero — a technique, skill, item or combo.
    pub const MAGIC: u8 = 1 << 1;
    /// Bit 2 — hit by a technique or a combo: `bset #2, $2A(a4)` at
    /// `ps4.asm:4025` (`loc_281E`, the technique arm) and `ps4.asm:8817` (the
    /// combo path). A plain *skill* sets bit 1 only.
    pub const TECHNIQUE: u8 = 1 << 2;
    /// Bit 3 — this battle opened with an ambush: `bset #3, $2A(a0)` for every
    /// enemy slot in `loc_B62A`'s tail (`ps4.asm:17456-17463`), which runs once,
    /// off the opening priority roll. A failed escape sets `Battle_Priority` to
    /// `$FF` without touching this bit (`Battle_RunFailMsg`).
    pub const AMBUSH: u8 = 1 << 3;
    /// Bit 4 — hit by a multi-target ability: `bset #4, $2A(a4)` at
    /// `ps4.asm:3992`, beside bit 1, when the command's `Current_Target_Index`
    /// is negative (the whole side).
    pub const MULTI_TARGET: u8 = 1 << 4;
}

/// One occupied fighter slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fighter {
    /// Its one-based index.
    pub id: FighterId,
    /// Display name, for the event timeline.
    pub name: String,
    /// For a party member, its index into `Character_Stats`. `None` for
    /// enemies.
    pub character: Option<u8>,
    /// Live stats.
    pub stats: Stats,
    /// The ability id the enemy AI picked this round, `0` for a plain attack.
    /// Mirrors `ability` (`$24`) of the fighter object.
    pub ability: u8,
    /// What has been done to this fighter since its last action. Mirrors
    /// `reaction_flags` (`$2A`) of the fighter object; see [`reaction`] for the
    /// bits and where retail sets them.
    pub reaction_flags: u8,
    /// Whether the fighter object is occupied. Dormant formation neighbors
    /// keep their cached identity/stats but cannot act or receive attacks.
    pub active: bool,
}

impl Fighter {
    /// Character_Dead ($84DE): retain sealing, clear other ailments, and
    /// distinguish android shutdown from human death. Enemy object removal
    /// is represented by the port's existing DEAD bit.
    pub(super) fn mark_defeated(&mut self) {
        use super::stats::status;
        self.stats.curr_hp = 0;
        if self.id.side() == Side::Party {
            self.stats.status = (self.stats.status & status::TECH_SEALED)
                | if self.stats.is_android() {
                    status::ANDROID_DEAD
                } else {
                    status::DEAD
                };
        } else {
            self.stats.status |= status::DEAD;
        }
    }

    /// Whether this fighter is still standing.
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        self.active && !self.stats.is_out()
    }
}

/// The nine slots, occupied or not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Roster {
    slots: Vec<Option<Fighter>>,
}

impl Roster {
    /// An empty battlefield.
    #[must_use]
    pub fn new() -> Roster {
        Roster {
            slots: vec![None; FIGHTER_SLOTS],
        }
    }

    /// Places a party member in the next free party slot.
    ///
    /// Returns the id it took, or `None` when all five are full.
    pub fn add_party_member(
        &mut self,
        character: u8,
        name: String,
        stats: Stats,
    ) -> Option<FighterId> {
        let index = (0..PARTY_SLOTS).find(|i| self.slots[*i].is_none())?;
        let id = FighterId::new(index as u8 + 1)?;
        self.slots[index] = Some(Fighter {
            id,
            name,
            character: Some(character),
            stats,
            ability: 0,
            reaction_flags: 0,
            active: true,
        });
        Some(id)
    }

    /// Places an enemy in a specific enemy slot, 1..=4 as the formation
    /// numbers them.
    ///
    /// Returns the id it took, or `None` for a slot outside 1..=4 or already
    /// occupied.
    pub fn add_enemy(&mut self, slot: u8, record: &EnemyRecord) -> Option<FighterId> {
        let index = PARTY_SLOTS + usize::from(slot.checked_sub(1)?);
        if index >= FIGHTER_SLOTS || self.slots[index].is_some() {
            return None;
        }
        let id = FighterId::new(index as u8 + 1)?;
        self.slots[index] = Some(Fighter {
            id,
            name: record.name.clone(),
            character: None,
            stats: Stats::from_enemy(record),
            ability: 0,
            reaction_flags: 0,
            active: true,
        });
        Some(id)
    }

    /// The fighter in a slot, if any.
    #[must_use]
    pub fn get(&self, id: FighterId) -> Option<&Fighter> {
        self.slots[id.slot()].as_ref()
    }

    /// The fighter in a slot, mutably.
    pub fn get_mut(&mut self, id: FighterId) -> Option<&mut Fighter> {
        self.slots[id.slot()].as_mut()
    }

    /// Consumes the roster, yielding every occupied slot in id order.
    ///
    /// The way a finished battle hands its fighters back rather than leaving
    /// the caller to remember to copy them.
    pub fn into_iter_fighters(self) -> impl Iterator<Item = Fighter> {
        self.slots.into_iter().flatten()
    }

    /// Every occupied slot, in id order.
    pub fn iter(&self) -> impl Iterator<Item = &Fighter> {
        self.slots.iter().flatten()
    }

    /// Every occupied slot, mutably, in id order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Fighter> {
        self.slots.iter_mut().flatten()
    }

    /// Every occupied slot on one side, in id order.
    pub fn side(&self, side: Side) -> impl Iterator<Item = &Fighter> {
        self.iter().filter(move |f| f.id.side() == side)
    }

    /// Every living fighter on one side, in id order.
    pub fn living(&self, side: Side) -> impl Iterator<Item = &Fighter> {
        self.side(side).filter(|f| f.is_alive())
    }

    /// Whether any fighter on `side` is still standing.
    #[must_use]
    pub fn any_alive(&self, side: Side) -> bool {
        self.living(side).next().is_some()
    }

    /// The first living fighter on `side`, which is where the cartridge's
    /// target cursor starts.
    #[must_use]
    pub fn first_living(&self, side: Side) -> Option<FighterId> {
        self.living(side).next().map(|f| f.id)
    }

    /// `Battle_GetCharHighestAgility` — `ps4.asm:17469`.
    ///
    /// The highest `agility_battle` among party members who are not dead or
    /// android-dead. Note the mask: only `$44`, so a *sleeping* or paralysed
    /// character still contributes, which is why a fully incapacitated party
    /// can still roll a good escape chance.
    #[must_use]
    pub fn highest_party_agility(&self) -> u8 {
        self.side(Side::Party)
            .filter(|f| !f.stats.is_out())
            .map(|f| f.stats.agility.battle)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::fixtures::zoran_bult;
    use crate::battle::stats::status;

    #[test]
    fn ids_split_at_five_the_way_the_cartridge_compares_them() {
        for index in 1..=5u8 {
            let id = FighterId::new(index).expect("a party id");
            assert_eq!(id.side(), Side::Party, "id {index}");
            assert_eq!(id.slot(), usize::from(index) - 1);
        }
        for index in 6..=9u8 {
            let id = FighterId::new(index).expect("an enemy id");
            assert_eq!(id.side(), Side::Enemy, "id {index}");
        }
        assert_eq!(FighterId::new(0), None, "ids are one-based");
        assert_eq!(FighterId::new(10), None, "nine slots, no more");
    }

    #[test]
    fn the_first_enemy_slot_is_id_six() {
        let mut roster = Roster::new();
        let id = roster.add_enemy(1, &zoran_bult()).expect("slot 1");
        assert_eq!(id.get(), 6, "Fighter_Enemy_1 is $FFFF4540, the sixth slot");
        assert_eq!(id.slot(), 5, "and the sixth slot is index 5");
        assert_eq!(roster.add_enemy(1, &zoran_bult()), None, "already occupied");
        assert_eq!(roster.add_enemy(5, &zoran_bult()), None, "four enemy slots");
        assert_eq!(
            roster.add_enemy(0, &zoran_bult()),
            None,
            "slots are one-based"
        );
    }

    #[test]
    fn highest_party_agility_ignores_the_dead_but_not_the_asleep() {
        let mut roster = Roster::new();
        let mut fast = Stats::from_enemy(&zoran_bult());
        fast.agility.battle = 40;
        let mut slow = Stats::from_enemy(&zoran_bult());
        slow.agility.battle = 10;

        roster.add_party_member(0, "fast".into(), fast);
        roster.add_party_member(1, "slow".into(), slow);
        assert_eq!(roster.highest_party_agility(), 40);

        // Asleep still counts — the mask is $44, not $6E.
        let sleeper = FighterId::new(1).expect("id 1");
        roster.get_mut(sleeper).expect("present").stats.status = status::ASLEEP;
        assert_eq!(roster.highest_party_agility(), 40);

        // Dead does not.
        roster.get_mut(sleeper).expect("present").stats.status = status::DEAD;
        assert_eq!(roster.highest_party_agility(), 10);

        // An empty party yields zero rather than panicking.
        assert_eq!(Roster::new().highest_party_agility(), 0);
    }

    #[test]
    fn the_target_cursor_starts_on_the_first_living_enemy() {
        let mut roster = Roster::new();
        roster.add_enemy(1, &zoran_bult());
        roster.add_enemy(2, &zoran_bult());
        assert_eq!(roster.first_living(Side::Enemy).map(|i| i.get()), Some(6));

        let first = FighterId::new(6).expect("id 6");
        roster.get_mut(first).expect("present").stats.status = status::DEAD;
        assert_eq!(
            roster.first_living(Side::Enemy).map(|i| i.get()),
            Some(7),
            "the cursor skips a corpse"
        );
        assert!(roster.any_alive(Side::Enemy));
        assert!(!roster.any_alive(Side::Party), "nobody was added");
    }
}
