//! The battle engine — Tiers 0 and 1 of `docs/battle/BATTLE_SCOUT.md` §15.
//!
//! Integer-only and I/O-free like the rest of the crate. Tier 0 is the math
//! kernel: damage, healing, the chance roll, the two selection tables. Tier 1
//! is a battle you can win — formation to rewards, with Attack, Defend and Run.
//!
//! # Rolls
//!
//! Every formula takes a [`Rolls`] and masks the raw word where the cartridge
//! masks it, because the mask is part of the formula. Two generators are
//! provided, both over the cartridge's single 32-bit seed:
//!
//! - [`Lcg41`] — `UpdateRNGSeed` (`$04236C`), the portable multiply-by-41.
//!   Encounters and field draws use it, and it is bit-exact against hardware.
//! - [`Rng2`] — `UpdateRNGSeed2` (`$04239E`), the battle mixer. Its entropy on
//!   hardware is the VDP's H/V counter, which no headless core can reproduce;
//!   `docs/RUNTIME_DESIGN.md` "RNG design" ratifies substituting
//!   [`HV_SURROGATE`] for that one term and keeping everything else. Read
//!   [`Rng2`]'s note on the frame count before wiring it up — a fixed frame
//!   count can pin the damage roll into a regime with almost no variance.
//!
//! [`Rng2`] borrows the [`Lcg41`] rather than owning a seed, so battle and
//! field share one word exactly as the cartridge does.
//!
//! # What is here, and where it came from
//!
//! | item | retail | scout |
//! |---|---|---|
//! | [`Lcg41`] | `$04236C` | §2 |
//! | [`Rng2`] | `$04239E` | §2 |
//! | [`calculate_damage`] | `$00B5CA` | §4.5 |
//! | [`clamp_damage`] | `$00266C` | §4.5 |
//! | [`calc_healing`] | `$00B5FE` | §4.5 |
//! | [`calculate_chances`] | `$00B5A6` | §5 |
//! | [`STAT_OFFSETS`] | `$00275A` | §4.1 |
//! | [`ELEMENT_OFFSETS`] | `$00276A` | §4.1 |
//! | [`Stats::update_mod_stats`] | `$0005F754` | §12 |
//! | [`Stats::update_char_elems`] | `$0005FD2A` | — |
//! | [`Stats::from_enemy`] | `ps4.asm:11939` | §9 |
//! | [`roll_priority`] | `$00B62A` | §8 |
//! | [`build_queue`] | `ps4.asm:7723` | §3 |
//! | [`choose_target`] | `ps4.asm:7978` | §3 |
//! | [`choose_ability`] | `ps4.asm:19146` | §7 |
//! | [`roll_hits`] | `$00B6A2` | §5 |
//! | [`split_rewards`] | `ps4.asm:4705` | §10 |
//! | [`level_up`] | `ps4.asm:5993` | §10 |
//!
//! # What this tier does not do
//!
//! Unimplemented technique effects, ordinary skills, items, combos, macros, most status effects,
//! drops and boss formations are Tier 2 and later. The proven retail vehicle
//! skill records and their damage/death dispatcher are included below; unknown
//! future effect ids still stop at an explicit boundary. Enemy AI
//! rolls its ability but implements only the plain attack; anything else raises
//! [`BattleEvent::UnsupportedAbility`] and falls back rather than inventing a
//! number. Vehicle selection and saved-use consumption are exposed on the
//! battle surface, and an unproven vehicle effect raises
//! [`BattleEvent::VehicleSkillEffectUnavailable`] rather than faking a physical
//! attack. The broader 44-entry ability-effect dispatch is absent, as is the
//! weapon-element fallback for physical skills; [`ability_element_factor`]
//! exists for that future dispatcher.
//!
//! Player damage, healing and stat-support techniques now execute through
//! [`Technique`], including TP payment and target validation. The Godot shell
//! selects individual commands; unsupported technique effects are rejected.

mod action;
mod ai;
mod chances;
mod damage;
mod enemy_damage;
mod enemy_skill;
mod engine;
mod equipment;
mod event;
mod fighters;
mod item;
mod order;
mod records;
mod rewards;
mod rng;
mod skill;
mod stats;
mod tables;
mod technique;
mod vehicle_attack;
mod vehicle_skill;

pub use enemy_skill::EnemySkill;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod party_fixtures;
#[cfg(test)]
#[path = "party_tests.rs"]
mod party_tests;

pub use action::{
    HitPass, Reach, ability_element_factor, candidate_targets, character_element_factor,
    critical_bonus, enemy_element_factor, resolve_attack, roll_hits, weapon_reach,
};
pub use ai::{ABILITY_ROLL_MASK, TARGET_RATES, choose_ability, choose_target, targetable_party};
pub use chances::{CHANCE_ROLL_MASK, ESCAPE, PHYSICAL, START_PRIORITY, Verdict, calculate_chances};
pub use damage::{
    DAMAGE_DRAWS, DAMAGE_ROLL_MASK, MAX_DAMAGE, MIN_DAMAGE, calc_healing, calculate_damage,
    clamp_damage,
};
pub use engine::{Battle, Command, PartyMember, RoundOrders};
pub use equipment::{
    EquipmentCandidate, EquipmentError, equip_item, equip_item_in_hand, equipment_candidates,
    unequip_item,
};
pub use event::{BattleEvent, FirstZioAction, Outcome, Skipped};
pub use fighters::{
    ENEMY_SLOTS, FIGHTER_SLOTS, Fighter, FighterId, LAST_PARTY_ID, PARTY_SLOTS, Roster, Side,
};
pub use item::{BattleItem, ItemRejection, ItemSource, item_targets};
pub use order::{Priority, QueueEntry, build_queue, roll_priority};
pub use records::{
    AI_CONDITIONS, BattleData, BattleDataError, Bonuses, CharacterRecord, ELEMENT_SLOTS,
    EQUIPMENT_SLOTS, ElementRole, EnemyRecord, EquipSlot, FormationEnemy, FormationRecord,
    ItemKind, ItemRecord, LevelRecord, LevelTable, REGULAR_ABILITIES, SKILL_SLOTS, TECHNIQUE_SLOTS,
    UNRUNNABLE,
};
pub use rewards::{MAX_LEVEL, POOL_CAP, Pools, Split, level_up, level_up_absent, split_rewards};
pub use rng::{HV_SURROGATE, Lcg41, RESEED, Rng2, Rolls, SliceRolls};
pub use skill::{Skill, SkillRejection, skill_targets};
pub use stats::{
    DEFENDING_PHYSICAL_PROP, GRANTED_RESISTANCE, PROFESSION_ANDROID, StatPair, StatTriple, Stats,
    status,
};
pub use tables::{
    ELEMENT_NAMES, ELEMENT_OFFSETS, STAT_INDEX_MASK, STAT_OFFSETS, StatSlot, StatWidth,
    WEAPON_ELEMENT_SENTINEL, WORD_STAT_THRESHOLD, element_name, element_offset, stat_slot,
};
pub use technique::{Technique, TechniqueRejection, TechniqueStat, technique_targets};
pub use vehicle_attack::{
    VEHICLE_ATTACK_ELEMENT, VEHICLE_ATTACK_OBJECTS, VEHICLE_FIGHTER_IDS, VehicleAttackObject,
    attack_object, hit_passes, is_vehicle_fighter, resolve_vehicle_attack, vehicle_index,
};
pub use vehicle_skill::{
    VehicleSkillData, VehicleSkillEffectKind, VehicleSkillResistance, resolve_vehicle_skill,
    vehicle_skill_data,
};
