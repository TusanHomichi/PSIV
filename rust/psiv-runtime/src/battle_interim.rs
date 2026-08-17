//! Small runtime seams that keep battle presentation out of `GameState`.

use std::collections::BTreeMap;

use psiv_core::battle::{BattleEvent, FighterId, RoundOrders, Side, Stats, Verdict};

use crate::{BattleSoundEvent, BattleTimeline, BridgeError, Runtime};

const SFX_ATTACK_MISS: u8 = 0xB8;
const SFX_ENEMY_KILLED: u8 = 0xB9;
const SFX_ENEMY_ATTACK_1: u8 = 0xBA;
const SFX_ROD: u8 = 0xB5;
const SFX_SHOT: u8 = 0xB6;
const SFX_CLAW: u8 = 0xE8;
const SFX_SWORD: u8 = 0xF5;

/// `Battle_WeaponIndex` (`ps4.asm:13082`) through the retail weapon sound
/// table (`ps4.asm:71157`). The index is the raw `InventoryData` id; the
/// value is the animation weapon class, not a guessed item category.
const BATTLE_WEAPON_INDEX: [u8; 0x90] = [
    0, 1, 1, 0x13, 0, 0, 0, 0, 5, 0x13, 0, 0, 0, 0, 0, 0, 9, 0, 5, 2, 0x14, 0x0C, 0x0C, 0, 0, 0, 0,
    0, 0, 0, 5, 0, 0x0F, 2, 0, 0x14, 0x0F, 0x0C, 0, 0, 6, 0x10, 0, 0x16, 0, 0, 0x16, 0x0D, 3, 0, 0,
    0, 0, 0, 0, 9, 0, 0x0A, 0, 0x17, 0, 0, 0x18, 0x16, 6, 0x10, 3, 0, 0x0A, 0, 0, 0, 0, 0, 0, 0,
    0x19, 0, 6, 0x10, 4, 0x0A, 0, 0x0B, 0x16, 0, 0, 0, 0, 0, 7, 4, 0x0B, 0x15, 0x0B, 0x1A, 0, 0,
    0x0B, 0x11, 0, 0x15, 0, 0, 0, 0, 0, 0, 0, 7, 0x1B, 0, 0, 4, 0x12, 0, 0, 0, 0, 8, 0x0E, 0x1C,
    0x0E, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 5, 0, 0, 0, 0, 0,
];

impl Runtime {
    /// Starts a battle and returns its presentation timeline with the SFX
    /// sidecar. The legacy `start_battle` API remains for non-presentation
    /// callers and tests.
    pub fn start_battle_timeline(
        &mut self,
        formation: u16,
        party: Vec<psiv_core::battle::PartyMember>,
    ) -> Result<BattleTimeline, BridgeError> {
        let events = self.start_battle(formation, party)?;
        Ok(BattleTimeline {
            events,
            sounds: Vec::new(),
        })
    }

    /// Resolves one round and attaches sounds in the same order as the core
    /// events. The actor sound context is captured before mutation because a
    /// lethal resolution changes the roster before `Died` is emitted.
    pub fn battle_round_timeline(
        &mut self,
        orders: &RoundOrders,
    ) -> Result<BattleTimeline, BridgeError> {
        let actor_sounds = self.battle_audio_context();
        let events = self.battle_round(orders)?;
        let sounds = battle_sound_events(&events, &actor_sounds);
        Ok(BattleTimeline { events, sounds })
    }

    fn battle_audio_context(&self) -> BTreeMap<FighterId, u8> {
        let Some(battle) = self.battle.as_ref() else {
            return BTreeMap::new();
        };
        battle
            .roster()
            .iter()
            .filter_map(|fighter| {
                let id = match fighter.id.side() {
                    Side::Party => player_attack_sound(&fighter.stats),
                    // Tier 1 has no enemy animation/object event surface yet.
                    // `$BA` is the documented generic physical fallback; the
                    // per-enemy object census remains deferred below.
                    Side::Enemy => Some(SFX_ENEMY_ATTACK_1),
                }?;
                Some((fighter.id, id))
            })
            .collect()
    }
}

fn battle_sound_events(
    events: &[BattleEvent],
    actor_sounds: &BTreeMap<FighterId, u8>,
) -> Vec<BattleSoundEvent> {
    let mut sounds = Vec::new();
    for (event_index, event) in events.iter().enumerate() {
        match event {
            BattleEvent::Attacked { actor, .. } => {
                if let Some(&id) = actor_sounds.get(actor) {
                    sounds.push(BattleSoundEvent { event_index, id });
                }
            }
            BattleEvent::Resolved { verdict, .. } if *verdict == Verdict::Miss => {
                sounds.push(BattleSoundEvent {
                    event_index,
                    id: SFX_ATTACK_MISS,
                });
            }
            BattleEvent::Died { .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: SFX_ENEMY_KILLED,
            }),
            // Run/flee, menu accept, victory confirmation, and level-up
            // confirmation have their own direct `Sound_Index` writes in the
            // retail menu routines. They are handled by the existing UI
            // input hook; these core events do not invent another write.
            _ => {}
        }
    }
    sounds
}

fn player_attack_sound(stats: &Stats) -> Option<u8> {
    stats.equipment.into_iter().find_map(weapon_sound)
}

fn weapon_sound(raw_id: u8) -> Option<u8> {
    let class = BATTLE_WEAPON_INDEX.get(usize::from(raw_id)).copied()?;
    Some(match class {
        1..=8 | 0x13..=0x15 => SFX_SWORD,
        9..=0x0E => SFX_ROD,
        0x0F..=0x12 => SFX_CLAW,
        0x16..=0x1C => SFX_SHOT,
        _ => return None,
    })
}

impl Runtime {
    /// Ends the battle and re-arms the encounter grace period, as the
    /// cartridge resets `$FFFFECE4` to 10 after every fight.
    pub fn finish_battle(&mut self) {
        self.battle = None;
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
    }

    /// Interim defeat policy pending the save system: put every current party
    /// member back on their feet at 1 HP so the field can continue. This is a
    /// runtime seam, not renderer access to `GameState`, and deliberately
    /// clears only the two dead bits; poison, paralysis and other statuses are
    /// still state that a later defeat/save design must adjudicate.
    pub fn revive_interim(&mut self) {
        let dead = psiv_core::DEAD_STATUS_MASK;
        for id in self.game.party_members() {
            if let Some(stats) = self.game.roster_mut().get_mut(id) {
                stats.curr_hp = 1;
                stats.status &= !dead;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> FighterId {
        FighterId::new(value).expect("valid fighter id")
    }

    #[test]
    fn weapon_table_selects_the_retail_attack_family() {
        assert_eq!(weapon_sound(2), Some(SFX_SWORD));
        assert_eq!(weapon_sound(0x10), Some(SFX_ROD));
        assert_eq!(weapon_sound(0x20), Some(SFX_CLAW));
        assert_eq!(weapon_sound(0x2B), Some(SFX_SHOT));
        assert_eq!(weapon_sound(4), None);
    }

    #[test]
    fn battle_sound_order_follows_attack_miss_and_death_events() {
        let actor = id(1);
        let target = id(6);
        let events = vec![
            BattleEvent::Attacked {
                actor,
                targets: vec![target],
            },
            BattleEvent::Resolved {
                actor,
                target,
                verdict: Verdict::Miss,
                damage: None,
                remaining_hp: 25,
            },
            BattleEvent::Died { fighter: target },
        ];
        let actor_sounds = BTreeMap::from([(actor, SFX_SWORD), (target, SFX_ENEMY_ATTACK_1)]);
        assert_eq!(
            battle_sound_events(&events, &actor_sounds),
            vec![
                BattleSoundEvent {
                    event_index: 0,
                    id: SFX_SWORD,
                },
                BattleSoundEvent {
                    event_index: 1,
                    id: SFX_ATTACK_MISS,
                },
                BattleSoundEvent {
                    event_index: 2,
                    id: SFX_ENEMY_KILLED,
                },
            ]
        );
    }
}
