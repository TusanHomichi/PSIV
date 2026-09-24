//! Small runtime seams that keep battle presentation out of `GameState`.

use std::collections::BTreeMap;

use psiv_core::battle::{BattleEvent, FighterId, RoundOrders, Side, Stats, Verdict};

use crate::{BattleAnimationEvent, BattleSoundEvent, BattleTimeline, BridgeError, Runtime};

const SFX_ATTACK_MISS: u8 = 0xB8;
const SFX_ENEMY_KILLED: u8 = 0xB9;
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
    /// Item activation definitions for the native command menu.
    pub fn battle_items(&self) -> impl Iterator<Item = &psiv_core::battle::BattleItem> {
        self.battles.iter().flat_map(|set| set.data.battle_items())
    }

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
            animations: Vec::new(),
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
        let actor_animations = self.battle_animation_context();
        let events = self.battle_round(orders)?;
        let sounds = battle_sound_events(&events, &actor_sounds);
        let animations = battle_animation_events(&events, &actor_animations);
        Ok(BattleTimeline {
            events,
            sounds,
            animations,
        })
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
                    Side::Enemy => self
                        .battles
                        .as_ref()
                        .and_then(|set| set.enemy_animations.get(&fighter.stats.enemy_id))
                        .map(|animation| animation.sfx_id),
                }?;
                Some((fighter.id, id))
            })
            .collect()
    }

    fn battle_animation_context(&self) -> BTreeMap<FighterId, psiv_data::EnemyAnimation> {
        let Some(battle) = self.battle.as_ref() else {
            return BTreeMap::new();
        };
        let Some(set) = self.battles.as_ref() else {
            return BTreeMap::new();
        };
        battle
            .roster()
            .side(Side::Enemy)
            .filter_map(|fighter| {
                Some((
                    fighter.id,
                    set.enemy_animations.get(&fighter.stats.enemy_id)?.clone(),
                ))
            })
            .collect()
    }
}

/// The enemy skill that opened the action a [`BattleEvent::Resolved`] belongs
/// to, when that action was an enemy skill.
///
/// The last action-opening event before the resolution owns it, and the damage
/// skills' objects write their second sound cue as part of that same action, so
/// the cue is keyed on the skill rather than on the acting enemy alone.
fn resolving_enemy_skill(
    events: &[BattleEvent],
    event_index: usize,
    actor: FighterId,
) -> Option<u8> {
    match events[..event_index].iter().rev().find(|e| {
        matches!(
            e,
            BattleEvent::Attacked { .. }
                | BattleEvent::EnemySkillUsed { .. }
                | BattleEvent::TechniqueUsed { .. }
                | BattleEvent::SkillUsed { .. }
                | BattleEvent::ItemUsed { .. }
        )
    }) {
        Some(BattleEvent::EnemySkillUsed {
            actor: caster,
            skill,
            ..
        }) if *caster == actor => Some(*skill),
        _ => None,
    }
}

fn battle_sound_events(
    events: &[BattleEvent],
    actor_sounds: &BTreeMap<FighterId, u8>,
) -> Vec<BattleSoundEvent> {
    let mut sounds = Vec::new();
    for (event_index, event) in events.iter().enumerate() {
        match event {
            // BattleObj_Thread's wind-up uses EnemyAttack5; no attack damage follows.
            BattleEvent::EnemySkillUsed { skill: 16, .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xDA,
            }),
            // BattleObj_Poison writes EnemyAttack4 in the same wind-up slot,
            // and like THREAD it never requests a damage reaction.
            BattleEvent::EnemySkillUsed { skill: 17, .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xD8,
            }),
            // BattleObj_Brose and BattleObj_Rimit both start with SFX_Brose.
            // This binds the original sound, not its still-missing animation.
            BattleEvent::TechniqueUsed {
                actor,
                technique: technique @ (17 | 23),
                ..
            } => {
                // CharTech_Cast ($9208): a late seal returns before creating
                // the spell object. The core records payment as TechniqueUsed
                // immediately followed by its rejection; payment is not a cast.
                if !matches!(events.get(event_index + 1),
                    Some(BattleEvent::TechniqueRejected { actor: rejected_actor, technique: rejected_technique, .. })
                        if rejected_actor == actor && rejected_technique == technique)
                {
                    sounds.push(BattleSoundEvent {
                        event_index,
                        id: 0xCB,
                    });
                }
            }
            // AcidBreathChild starts the wind-up with MoleAttack; the main
            // object writes EnemyAttack4 before its damage reaction. These
            // event cues do not claim the retail 10/36-frame object timing.
            BattleEvent::EnemySkillUsed { skill: 51, .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xD5,
            }),
            // BattleObj_HelexFlameBolt writes EnemyAttack3 as it loads and its
            // child BattleObj_HelexFlameBolt2 writes FireBreath, again in the
            // wind-up slot ahead of the same single damage request.
            BattleEvent::EnemySkillUsed { skill: 2, .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xD7,
            }),
            // SAND STORM `$37` and MAELSTROM `$39`: BattleObj_SandStorm and
            // BattleObj_Maelstrom write MoleAttack on the first frame of their
            // wind-up phase (`ps4.asm:48228`, `ps4.asm:47800`) and make their
            // one request in the phase after it, with no second cue.
            BattleEvent::EnemySkillUsed { skill: 55 | 57, .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xD5,
            }),
            // FLODBREATH `$3F`: BattleObj_FlodBreath (`ps4.asm:46343`) and
            // loc_22A90 (`ps4.asm:46220`) write MoleAttack as their wind-up
            // starts, then EnemyAttack4 (`ps4.asm:46366`, `ps4.asm:46234`)
            // before the damage phase — the same pair of cues for all three
            // carriers.
            BattleEvent::EnemySkillUsed { skill: 63, .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xD5,
            }),
            // WAT `$40`: all five carriers' chains write MoleAttack as the
            // wind-up starts (`ps4.asm:46002` BattleObj_EnemyWat,
            // `ps4.asm:45056` loc_218D6, `ps4.asm:56445` loc_2B006).
            //
            // BattleObj_EnemyWat's second cue, TechCast at `ps4.asm:46016`, is
            // left unmapped: loc_218D6 and loc_2B006 write nothing there, a
            // skill-wide cue would invent it for those three carriers, and the
            // sound context carries no per-carrier identity to key it on.
            BattleEvent::EnemySkillUsed { skill: 64, .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xD5,
            }),
            // FOI `$44`: both chains write MoleAttack at the wind-up
            // (`ps4.asm:45238` loc_21BF0, `ps4.asm:56485` loc_2B08E) and
            // TechCast when the request phase starts (`ps4.asm:45261`,
            // `ps4.asm:56509`).
            BattleEvent::EnemySkillUsed { skill: 68, .. } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xD5,
            }),
            // ROUND EYES `$6D` and LOVEL EYES `$6E`: the phase that flinches
            // the target writes MoleAttack as its wind-up starts
            // (`ps4.asm:67787`) and EnemyAttack1 as it hands the request on
            // (`ps4.asm:67795`). Both objects run that same shared phase
            // (`loc_347BE`, `ps4.asm:67776`).
            BattleEvent::EnemySkillUsed {
                skill: 109 | 110, ..
            } => sounds.push(BattleSoundEvent {
                event_index,
                id: 0xD5,
            }),
            // The damage-skill animation objects write their second cue just
            // before the one damage request: EnemyAttack4 at
            // BattleObj_AcidBreath's reaction, FireBreath at the Helex child's.
            BattleEvent::Resolved { actor, .. }
                if resolving_enemy_skill(events, event_index, *actor) == Some(51) =>
            {
                sounds.push(BattleSoundEvent {
                    event_index,
                    id: 0xD8,
                });
            }
            BattleEvent::Resolved { actor, .. }
                if resolving_enemy_skill(events, event_index, *actor) == Some(2) =>
            {
                sounds.push(BattleSoundEvent {
                    event_index,
                    id: 0xC2,
                });
            }
            // FLODBREATH's EnemyAttack4, written by both objects as the damage
            // phase begins — the same cue for all three carriers, so unlike
            // WAT's it is safe to key on the skill alone.
            BattleEvent::Resolved { actor, .. }
                if resolving_enemy_skill(events, event_index, *actor) == Some(63) =>
            {
                sounds.push(BattleSoundEvent {
                    event_index,
                    id: 0xD8,
                });
            }
            // FOI's TechCast, written as the request phase starts in both
            // chains.
            BattleEvent::Resolved { actor, .. }
                if resolving_enemy_skill(events, event_index, *actor) == Some(68) =>
            {
                sounds.push(BattleSoundEvent {
                    event_index,
                    id: 0xBB,
                });
            }
            // The eyes' EnemyAttack1, written at the flinch that hands the one
            // request on.
            BattleEvent::Resolved { actor, .. }
                if matches!(
                    resolving_enemy_skill(events, event_index, *actor),
                    Some(109 | 110)
                ) =>
            {
                sounds.push(BattleSoundEvent {
                    event_index,
                    id: 0xBA,
                });
            }
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

fn battle_animation_events(
    events: &[BattleEvent],
    actor_animations: &BTreeMap<FighterId, psiv_data::EnemyAnimation>,
) -> Vec<BattleAnimationEvent> {
    events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let BattleEvent::Attacked { actor, .. } = event else {
                return None;
            };
            if actor.side() != Side::Enemy {
                return None;
            }
            let animation = actor_animations.get(actor)?;
            let sequence = animation.frame_sequence.as_ref();
            Some(BattleAnimationEvent {
                event_index,
                actor: *actor,
                enemy_id: animation.enemy_id,
                sfx_id: animation.sfx_id,
                frame_duration: sequence.map(|sequence| sequence.frame_duration),
                frame_count: sequence.map(|sequence| sequence.frame_count),
                total_frames: sequence.map(|sequence| sequence.total_frames),
                frame_durations: sequence.map(|sequence| {
                    if sequence.frame_durations.is_empty() {
                        vec![sequence.frame_duration; usize::from(sequence.frame_count)]
                    } else {
                        sequence.frame_durations.clone()
                    }
                }),
                movement_proven: animation.movement_proven,
                sprite_sheet_proven: animation.sprite_sheet_proven,
                flash_timing_proven: animation.flash_timing_proven,
            })
        })
        .collect()
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
        self.battle_field_refresh_pending |= self.battle.take().is_some();
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
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
    fn late_seal_cancels_brose_and_rimit_sound_but_a_miss_does_not() {
        for technique in [17, 23] {
            let used = BattleEvent::TechniqueUsed {
                actor: id(1),
                technique,
                name: String::new(),
                remaining_tp: 10,
            };
            let rejected = BattleEvent::TechniqueRejected {
                actor: id(1),
                technique,
                reason: psiv_core::battle::TechniqueRejection::Sealed,
            };
            assert!(
                battle_sound_events(&[used.clone(), rejected], &BTreeMap::new()).is_empty(),
                "a paid but sealed technique {technique} must not start its spell sound"
            );
            let missed = BattleEvent::Resolved {
                actor: id(1),
                target: id(6),
                verdict: Verdict::Miss,
                damage: None,
                remaining_hp: 20,
            };
            let sounds = battle_sound_events(&[used, missed], &BTreeMap::new());
            assert_eq!(
                sounds.first(),
                Some(&BattleSoundEvent {
                    event_index: 0,
                    id: 0xcb
                }),
                "an executed spell still starts its sound when its target resists"
            );
        }
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
    fn damage_skill_cues_follow_the_object_writes_not_the_last_skill() {
        // 51 ACIDBREATH: BattleObj_AcidBreathChild writes MoleAttack $D5,
        // BattleObj_AcidBreath writes EnemyAttack4 $D8. 2 FLAME BOLT:
        // BattleObj_HelexFlameBolt writes EnemyAttack3 $D7 and its child
        // BattleObj_HelexFlameBolt2 writes FireBreath $C2. Each pair brackets the
        // ability event and the one damage reaction.
        for (skill, wind_up, reaction) in [(51u8, 0xD5u8, 0xD8u8), (2, 0xD7, 0xC2)] {
            let events = vec![
                BattleEvent::EnemySkillUsed {
                    actor: id(6),
                    skill,
                    name: String::new(),
                },
                BattleEvent::Resolved {
                    actor: id(6),
                    target: id(1),
                    verdict: Verdict::Normal,
                    damage: Some(7),
                    remaining_hp: 30,
                },
            ];
            assert_eq!(
                battle_sound_events(&events, &BTreeMap::new()),
                vec![
                    BattleSoundEvent {
                        event_index: 0,
                        id: wind_up,
                    },
                    BattleSoundEvent {
                        event_index: 1,
                        id: reaction,
                    },
                ],
                "ability {skill}"
            );
        }

        // The guard is the skill that opened this action, not the last enemy
        // skill on the field: a party attack's miss after a FLAME BOLT keeps
        // only the miss cue.
        let events = vec![
            BattleEvent::EnemySkillUsed {
                actor: id(6),
                skill: 2,
                name: String::new(),
            },
            BattleEvent::Attacked {
                actor: id(1),
                targets: vec![id(6)],
            },
            BattleEvent::Resolved {
                actor: id(1),
                target: id(6),
                verdict: Verdict::Miss,
                damage: None,
                remaining_hp: 30,
            },
        ];
        assert_eq!(
            battle_sound_events(&events, &BTreeMap::new()),
            vec![
                BattleSoundEvent {
                    event_index: 0,
                    id: 0xD7,
                },
                BattleSoundEvent {
                    event_index: 2,
                    id: SFX_ATTACK_MISS,
                },
            ]
        );
    }

    #[test]
    fn battle_sound_order_follows_attack_miss_and_death_events() {
        let actor = id(1);
        let target = id(6);
        let second_enemy = id(7);
        let events = vec![
            BattleEvent::Attacked {
                actor,
                targets: vec![target],
            },
            BattleEvent::Attacked {
                actor: target,
                targets: vec![actor],
            },
            BattleEvent::Attacked {
                actor: second_enemy,
                targets: vec![actor],
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
        let actor_sounds =
            BTreeMap::from([(actor, SFX_SWORD), (target, 0xD7), (second_enemy, 0xD6)]);
        assert_eq!(
            battle_sound_events(&events, &actor_sounds),
            vec![
                BattleSoundEvent {
                    event_index: 0,
                    id: SFX_SWORD,
                },
                BattleSoundEvent {
                    event_index: 1,
                    id: 0xD7,
                },
                BattleSoundEvent {
                    event_index: 2,
                    id: 0xD6,
                },
                BattleSoundEvent {
                    event_index: 3,
                    id: SFX_ATTACK_MISS,
                },
                BattleSoundEvent {
                    event_index: 4,
                    id: SFX_ENEMY_KILLED,
                },
            ]
        );
    }
}
