//! Building the battle a fixture describes.
//!
//! Every number comes from the fixture, and the fixture's numbers come from the
//! oracle's RAM log: the party's live stats, the enemies the formation seated,
//! and - for a vehicle battle - the save record behind the one fighter
//! `loc_78EE` (`ps4.asm:11408`) seats. Where the log and the port's own
//! derivations both have an opinion (a character's battle stats from
//! `Stats::from_character`, an enemy's from `Stats::from_enemy`), this is where
//! the two are held against each other: a drift in either shows up as a failed
//! assertion rather than as a quietly different battle.

use super::*;

use crate::battle::*;

use crate::battle::fixtures;
use crate::state::VehicleRecord;
use crate::vehicle;

/// A one-based fighter id, as the fixture and the log number them.
pub(crate) fn id(n: u8) -> FighterId {
    FighterId::new(n).expect("a valid id")
}

/// The port's member for a character record: the engine's own fixture helper's
/// derivation, which the fixture's live RAM is checked against.
pub(crate) fn member(record: &CharacterRecord, data: &BattleData) -> PartyMember {
    let item = |id: u8| data.item(id).ok().cloned();
    PartyMember {
        character: record.id,
        name: record.name.clone(),
        stats: Stats::from_character(record, item),
    }
}

/// The fixture's records, checked against the hand-built fixtures' derivation.
///
/// `fixtures::alys()` and friends are the pack's numbers; the fixture's are
/// the oracle's live RAM. They have to agree, and this is where a drift in
/// either shows up. The one exception is a **durable** capture: when the
/// fixture's provenance says the capture patched the party-side HP
/// (`oracle/force/durable.py`), the entry whose two HP cells both read that
/// value is the patch rather than the record, and every other number is still
/// held to the record's.
pub(crate) fn party_member(
    entry: &PartyEntry,
    data: &BattleData,
    hp_patch: Option<&HpPatch>,
) -> PartyMember {
    let record = match entry.id {
        1 => fixtures::alys(),
        2 => fixtures::chaz(),
        3 => fixtures::hahn(),
        other => panic!("no fixture record for character {other}"),
    };
    let patched = hp_patch
        .filter(|patch| patch.hp == entry.hp && patch.hp == entry.max_hp)
        .is_some();
    assert_eq!(record.name, entry.name, "character {0}", entry.id);
    if !patched {
        assert_eq!(record.hp, entry.max_hp, "character {} max_hp", entry.id);
    }
    assert_eq!(record.tp, entry.max_tp, "character {} max_tp", entry.id);
    let member = member(&record, data);
    let stats = &member.stats;
    let oracle = [
        ("level", stats.level, entry.level),
        ("strength", u16::from(stats.strength.battle), entry.strength),
        ("mental", u16::from(stats.mental.battle), entry.mental),
        ("agility", u16::from(stats.agility.battle), entry.agility),
        (
            "dexterity",
            u16::from(stats.dexterity.battle),
            entry.dexterity,
        ),
        ("attack", stats.attack.battle, entry.attack),
        ("defence", stats.defence.battle, entry.defence),
    ];
    for (what, derived, logged) in oracle {
        assert_eq!(
            derived, logged,
            "character {}: {what} derived from the record is not the oracle's",
            entry.id
        );
    }
    // The log's live HP, TP and status are what the battle starts with.
    let mut member = member;
    if !patched {
        assert_eq!(
            member.stats.curr_hp, entry.hp,
            "character {}: HP the log shows at the battle's start",
            entry.id
        );
    }
    assert_eq!(member.stats.curr_tp, entry.tp, "character {} TP", entry.id);
    assert_eq!(
        member.stats.status, entry.status,
        "character {} status",
        entry.id
    );
    member.stats.curr_hp = entry.hp;
    // The log's maximum is the record's for every capture but a durable one,
    // which is exactly what the provenance above declares.
    member.stats.max_hp = entry.max_hp;
    member.stats.curr_tp = entry.tp;
    member
}

/// A vehicle battle's party side, from the fixture's `vehicle` section.
///
/// `crate::vehicle::battle_member` is the port's own `loc_78EE`: the profile it
/// selects by `Vehicle_Index` carries the attack, defence and element
/// properties the log does not, and the record built here carries the HP and
/// skill cells the log does. The two halves are held together by the replay
/// itself - the vehicle's HP is what the log's `vehicle_fighter_hp` column
/// shows, action by action.
pub(crate) fn vehicle_member(section: &VehicleSection, party_slot: u8) -> PartyMember {
    assert_eq!(
        section.fighter_id, party_slot,
        "the fixture's vehicle fights from the party-side slot the log shows"
    );
    let saved = &section.saved_record;
    let mut current_skill_uses = [0u8; 8];
    let mut max_skill_uses = [0u8; 8];
    // The log carries the first skill slot's cells (`Saved_Vehicle_Stats`'s
    // `$06`/`$07`); the other seven have no column in `oracle/ram_map.json`.
    if let (Some(current), Some(maximum)) = (saved.skill1_current, saved.skill1_max) {
        current_skill_uses[0] = current;
        max_skill_uses[0] = maximum;
    }
    let hp = u16::try_from(section.hp).expect("the log's vehicle HP is not negative");
    // The saved maximum is the battle copy's only when the saved record's own
    // current HP was the fighter's at the battle's first frame. The extractor
    // is what measured that; this is the same claim read back.
    if let Some(maximum) = section.max_hp {
        assert_eq!(
            saved.hp,
            Some(section.hp),
            "the fixture's vehicle max rests on the saved record's HP"
        );
        assert!(
            hp <= maximum,
            "the fighter's HP {hp} is inside the saved maximum {maximum}"
        );
    }
    let record = VehicleRecord {
        current_hp: hp,
        // The saved record's maximum is the battle copy's only when the
        // extractor measured the two HP cells equal at the battle's first
        // frame; without that, the fighter's own HP is what the log has.
        max_hp: section.max_hp.unwrap_or(hp),
        skill_mask: saved.skill_mask.unwrap_or(0),
        current_skill_uses,
        max_skill_uses,
        ..VehicleRecord::default()
    };
    vehicle::battle_member(section.index, record)
        .unwrap_or_else(|| panic!("Vehicle_Index {} has no profile", section.index))
}

pub(crate) fn formation(fixture: &Fixture) -> FormationRecord {
    FormationRecord {
        // The log carries no formation index (the encounter's `loc_7E4C` masks
        // it out of the draw before the battle exists), and nothing in the
        // battle reads one back.
        id: 0,
        ambush_chance: fixture.formation.ambush_chance,
        run_chance: fixture.formation.run_chance,
        drop_rate: fixture.formation.drop_rate,
        // `dropped_item` is the *result* byte the log shows, not the formation
        // header's item; the cartridge dropped nothing here.
        drop_item: None,
        enemies: fixture
            .formation
            .enemies
            .iter()
            .map(|entry| FormationEnemy {
                slot: entry.formation_slot,
                enemy_id: entry.enemy_id,
                // `position` is the on-screen placement byte.
                position: 0,
            })
            .collect(),
    }
}

/// Starts the battle from the fixture's state on the given stream.
pub(crate) fn start(fixture: &Fixture, data: &BattleData, rolls: &mut impl Rolls) -> Battle {
    let formation = formation(fixture);
    let (battle, events) = match &fixture.vehicle {
        Some(section) => {
            let vehicle = vehicle_member(section, section.fighter_id);
            Battle::start_vehicle(&formation, vec![vehicle], data, rolls)
                .expect("the fixture's formation resolves")
        }
        None => {
            let hp_patch = fixture.provenance.hp_patch.as_ref();
            let party = fixture
                .party
                .iter()
                .map(|entry| party_member(entry, data, hp_patch))
                .collect();
            Battle::start(&formation, party, data, false, rolls)
                .expect("the fixture's formation resolves")
        }
    };

    let started = events
        .iter()
        .find_map(|event| match event {
            BattleEvent::Started { priority, enemies } => Some((*priority, enemies.clone())),
            _ => None,
        })
        .expect("Battle::start reports what it rolled");
    // A battle that drew no opening roll - the vehicle capture, whose trace
    // holds only the formation draw before round 1 - leaves the port's own
    // `loc_B62A` value nothing to check: the fixture's own `priority` reading
    // is the log's, and the port's extra draw is a divergence the manifest
    // records, not one this builder hides.
    if !priority_roll(fixture).is_empty() {
        assert_eq!(
            started.0,
            match fixture.formation.priority {
                0 => Priority::Normal,
                1 => Priority::Preemptive,
                _ => Priority::Ambush,
            },
            "the priority draw is the cartridge's own roll"
        );
    }
    let expected: Vec<FighterId> = fixture.formation.enemies.iter().map(|e| id(e.id)).collect();
    assert_eq!(
        started.1, expected,
        "the formation seats the cartridge's enemies"
    );

    for entry in &fixture.formation.enemies {
        let fighter = battle.roster().get(id(entry.id)).expect("seated");
        let stats = &fighter.stats;
        assert_eq!(stats.curr_hp, entry.hp, "enemy {} HP", entry.id);
        assert_eq!(u32::from(stats.status), u32::from(entry.status));
        assert_eq!(u32::from(stats.attack.battle), u32::from(entry.attack));
        assert_eq!(u32::from(stats.defence.battle), u32::from(entry.defence));
        assert_eq!(u32::from(stats.agility.battle), u32::from(entry.agility));
        assert_eq!(u32::from(stats.strength.battle), u32::from(entry.strength));
        assert_eq!(u32::from(stats.mental.battle), u32::from(entry.mental));
        assert_eq!(
            u32::from(stats.dexterity.battle),
            u32::from(entry.dexterity)
        );
    }
    if let Some(section) = &fixture.vehicle {
        // The party side the log's `vehicle_fighter_hp` column holds: the
        // fixture's HP at the battle's first frame, as the fight starts.
        let fighter = battle.roster().get(id(section.fighter_id)).expect("seated");
        assert_eq!(
            fighter.stats.curr_hp,
            u16::try_from(section.hp).expect("not negative"),
            "the log's vehicle HP at the battle's start"
        );
        assert!(battle.is_vehicle(), "the port fights this one mounted");
    }
    battle
}

/// The round's orders, from the fixture's own command cells.
///
/// The fixture records one `attack` per party-side fighter the round's queue
/// held, with the target the cartridge's `Character_Command_Data` cell named
/// (`assembly.command_entry`). A command with no target cell - a capture taken
/// before the `bcmd` group existed - is [`Command::Attack`], this port's own
/// default cursor, and `-1` is the same thing by another route: it is the
/// whole-side command `Battle_AttackCommand` writes (`ps4.asm:8464`), whose
/// negative index widens the window instead of naming a slot
/// (`ps4.asm:17501`). A target is [`Command::AttackTarget`], which is what lets
/// `candidate_targets` tell a swing that kept its aim from one the cartridge's
/// retarget scan moved (`docs/oracle/BATTLE_ORACLE_SWEEP.md` §4.4 W1).
pub(crate) fn orders(fixture: &Fixture, round: &Round) -> RoundOrders {
    let party_side = fixture.party.len() + usize::from(fixture.vehicle.is_some());
    let mut commands = vec![Command::Attack; party_side];
    assert_eq!(
        round.commands.len(),
        round
            .order
            .iter()
            .filter(|fighter| **fighter <= LAST_PARTY_ID)
            .count(),
        "round {}: one command per party-side fighter the queue held",
        round.round
    );
    for entry in &round.commands {
        let Some(slot) = usize::from(entry.id)
            .checked_sub(1)
            .filter(|slot| *slot < usize::from(LAST_PARTY_ID))
        else {
            panic!(
                "round {}: a command names a party-side fighter, not {}",
                round.round, entry.id
            );
        };
        if slot >= commands.len() {
            commands.resize(slot + 1, Command::Attack);
        }
        commands[slot] = match entry.target {
            Some(target) if target > 0 => {
                Command::AttackTarget(id(u8::try_from(target).expect("the cell is one byte")))
            }
            // `-1`, and a capture with no cell at all: no single target.
            _ => Command::Attack,
        };
    }
    RoundOrders::Commands(commands)
}

/// The battle the fixture describes, started on its own priority roll.
pub(crate) fn started(fixture: &Fixture, data: &BattleData) -> Battle {
    let priority = priority_roll(fixture);
    let mut rolls = SliceRolls::new(&priority);
    let battle = start(fixture, data, &mut rolls);
    if priority.is_empty() {
        // The cartridge drew no opening roll here (`Battle_Priority` stays
        // `$00`), so the fixture's stream carries none and the port's own draw
        // comes from an empty slice, outside the captured stream.
    } else {
        assert_eq!(priority.len(), 1, "the battle opens on one draw");
        assert_eq!(rolls.drawn(), 1, "Battle::start draws loc_B62A's roll");
    }
    battle
}

pub(crate) fn priority_roll(fixture: &Fixture) -> Vec<u16> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.role == "priority")
        .map(|roll| roll.roll)
        .collect()
}
