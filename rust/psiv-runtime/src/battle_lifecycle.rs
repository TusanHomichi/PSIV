//! Arming and running the pack's battles: the battle set, the seated roster,
//! and the start, round and absorb seams the shell drives.

use psiv_core::CharId;
use psiv_core::battle::{Battle, BattleEvent, Rng2, RoundOrders};

use crate::boss_battles;
use crate::camp;
use crate::encounters;
use crate::loot;
use crate::{BattleSet, BridgeError, EncounterClock, EncounterTable, Runtime, battle_data};

impl Runtime {
    /// Converts the pack's battle files and arms random encounters.
    ///
    /// # Errors
    /// [`BridgeError::Rejected`] when a record does not fit the engine.
    pub fn enable_battles(&mut self, files: &psiv_data::BattleFiles) -> Result<(), BridgeError> {
        let data = battle_data(files)?;
        // Seat all eleven characters, exactly as InitializeCharStats does
        // whether or not they are in the party. The roster refuses a second
        // constructor path by design, so this goes through the same
        // PartyMember::seat every battle uses.
        for character in &files.characters.characters {
            let id = CharId(character.character_id);
            if self.game.roster().get(id).is_some() {
                continue;
            }
            let record = encounters::character_record(character, &files.enemies.properties)?;
            let member = psiv_core::battle::PartyMember::seat(&record, &data)
                .map_err(|e| BridgeError::Rejected(e.to_string()))?;
            self.game
                .roster_mut()
                .seat(id, member.stats)
                .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        }
        self.battles = Some(BattleSet {
            data,
            table: EncounterTable::from_files(files)?,
            clock: EncounterClock::new(),
            names: files
                .characters
                .characters
                .iter()
                .map(|c| {
                    (
                        c.character_id,
                        c.display_name.clone().unwrap_or_else(|| c.symbol.clone()),
                    )
                })
                .collect(),
            camp: camp::catalog(files),
            loot: loot::catalog(files),
            boss_formations: boss_battles::boss_formation_records(files)?,
            enemy_animations: files
                .enemy_animations
                .animations
                .iter()
                .map(|animation| (animation.enemy_id, animation.clone()))
                .collect(),
        });
        Ok(())
    }

    /// The current party as battle members, drawn from the roster — the
    /// records battles read and write in place, per the cartridge's own
    /// model. Empty if battles are not enabled or the roster is unseated.
    #[must_use]
    pub fn battle_party(&self) -> Vec<psiv_core::battle::PartyMember> {
        let Some(set) = self.battles.as_ref() else {
            return Vec::new();
        };
        if let Some(index) = self.vehicle_index()
            && let Some(record) = self.game.vehicles().get(index.saturating_sub(1) as usize)
            && let Some(member) = psiv_core::battle_member(index, *record)
        {
            return vec![member];
        }
        self.game
            .party_members()
            .into_iter()
            .filter_map(|id| {
                let stats = self.game.roster().get(id)?.clone();
                Some(psiv_core::battle::PartyMember {
                    character: id.0,
                    name: set.names.get(&id.0).cloned().unwrap_or_default(),
                    stats,
                })
            })
            .collect()
    }

    /// Live combatants for command selection; includes current HP, TP and status.
    #[must_use]
    pub fn battle_roster(&self) -> Option<&psiv_core::battle::Roster> {
        self.battle.as_ref().map(Battle::roster)
    }

    /// Cartridge technique definitions for presentation of names and costs.
    pub fn battle_techniques(&self) -> impl Iterator<Item = &psiv_core::battle::Technique> {
        self.battles.iter().flat_map(|set| set.data.techniques())
    }

    /// Character skill names, targeting rules and availability for the menu.
    pub fn battle_skills(&self) -> impl Iterator<Item = &psiv_core::battle::Skill> {
        self.battles.iter().flat_map(|set| set.data.skills())
    }

    /// Fighters with an actual weapon in either hand. Shields do not qualify.
    #[must_use]
    pub fn battle_armed_fighters(&self) -> Vec<psiv_core::battle::FighterId> {
        let Some(set) = self.battles.as_ref() else {
            return Vec::new();
        };
        self.battle_roster()
            .into_iter()
            .flat_map(|roster| roster.side(psiv_core::battle::Side::Party))
            .filter(|fighter| {
                psiv_core::battle::weapon_reach(&fighter.stats, &set.data)
                    .ok()
                    .flatten()
                    .is_some()
            })
            .map(|fighter| fighter.id)
            .collect()
    }

    /// Ends a battle by absorbing the party records back into the roster and
    /// running both award passes — the full cartridge epilogue. The caller
    /// passes the per-member award (the split the battle computed).
    pub fn finish_battle_absorbing(&mut self, each: u16) -> Vec<BattleEvent> {
        let mut timeline = Vec::new();
        if let Some(battle) = self.battle.take() {
            self.battle_field_refresh_pending = true;
            // Battle_VictoryMessage ($30E6): the displayed pool must reach
            // Current_Money once, including vehicle battles. Escaping after
            // killing an enemy does not pay its accumulated pool.
            if battle.outcome() == Some(psiv_core::battle::Outcome::Victory) {
                self.game.add_money(u32::from(battle.pools().meseta));
                self.game.set_money(self.game.money().min(9_999_999));
            }
            if battle.is_vehicle() {
                let index = self.vehicle_index().unwrap_or(0);
                if let Some(member) = battle.into_party().into_iter().next()
                    && let Some(record) = self
                        .game
                        .vehicles_mut()
                        .get_mut(index.saturating_sub(1) as usize)
                {
                    record.current_hp = member.stats.curr_hp;
                    record.current_skill_uses = member.stats.curr_skill_uses;
                }
            } else {
                // The cartridge's results order, load-bearing: absorb the
                // records whole, pay both award passes, then level everyone the
                // pay reached — levelling first levels nobody, and levelling
                // battle's copies levels stale numbers.
                let party = battle.into_party();
                self.game.roster_mut().absorb(&party);
                let (paid_party, paid_absent) = self.game.award_experience(each);
                if let Some(set) = self.battles.as_ref() {
                    for id in &paid_party {
                        let Some(stats) = self.game.roster_mut().get_mut(*id) else {
                            continue;
                        };
                        let techniques = stats.techniques;
                        let skills = stats.skills;
                        if let Ok(Some(event)) = psiv_core::battle::level_up(id.0, stats, &set.data)
                        {
                            timeline.push(event);
                            for (before, &now) in techniques.iter().zip(&stats.techniques) {
                                if *before != now
                                    && now != 0
                                    && let Some(ability) = set.data.technique(now)
                                {
                                    timeline.push(BattleEvent::LearnedAbility {
                                        character: id.0,
                                        name: ability.name.clone(),
                                    });
                                }
                            }
                            for (before, &now) in skills.iter().zip(&stats.skills) {
                                if *before != now
                                    && now != 0
                                    && let Some(ability) = set.data.skill(now)
                                {
                                    timeline.push(BattleEvent::LearnedAbility {
                                        character: id.0,
                                        name: ability.name.clone(),
                                    });
                                }
                            }
                        }
                    }
                    // The benched roster levels silently and replenishes its
                    // skill uses, unlike the visible party's results sequence.
                    for id in &paid_absent {
                        if let Some(stats) = self.game.roster_mut().get_mut(*id) {
                            let _ = psiv_core::battle::level_up_absent(id.0, stats, &set.data);
                        }
                    }
                }
            }
        }
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
        timeline
    }

    /// Whether a battle currently owns the frame.
    #[must_use]
    pub fn battle_active(&self) -> bool {
        self.battle.is_some()
    }

    /// Starts a battle against `formation`, seating `party`.
    ///
    /// The party's stats are the caller's until the pack carries
    /// `battle/characters.json` + `battle/equipment.json`; then the runtime
    /// seats its own party from game state and this takes only the formation.
    ///
    /// # Errors
    /// [`BridgeError::Rejected`] when battles are not enabled, the formation
    /// id is unknown, or the engine refuses the setup.
    pub fn start_battle(
        &mut self,
        formation: u16,
        party: Vec<psiv_core::battle::PartyMember>,
    ) -> Result<Vec<BattleEvent>, BridgeError> {
        let set = self
            .battles
            .as_ref()
            .ok_or_else(|| BridgeError::Rejected("battles not enabled".into()))?;
        let record = set
            .table
            .formation(formation)
            .ok_or_else(|| BridgeError::Rejected(format!("unknown formation {formation}")))?;
        let mut rng2 = Rng2::with_surrogate(&mut self.rng, self.frames);
        let (battle, events) = if self.vehicle.is_some() {
            Battle::start_vehicle(record, party, &set.data, &mut rng2)
        } else {
            Battle::start(record, party, &set.data, false, &mut rng2)
        }
        .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.battle = Some(battle);
        self.scene_battle = None;
        Ok(events)
    }

    /// Resolves one battle round with the party's orders.
    ///
    /// After an `Ended` event appears in the timeline the shell calls
    /// [`Runtime::finish_battle_absorbing`] to return the records to the
    /// field roster (and to run the victory award pass).
    ///
    /// # Errors
    /// [`BridgeError::Rejected`] when no battle is active or a data lookup
    /// fails mid-round.
    pub fn battle_round(&mut self, orders: &RoundOrders) -> Result<Vec<BattleEvent>, BridgeError> {
        let set = self
            .battles
            .as_ref()
            .ok_or_else(|| BridgeError::Rejected("battles not enabled".into()))?;
        let battle = self
            .battle
            .as_mut()
            .ok_or_else(|| BridgeError::Rejected("no battle in progress".into()))?;
        let mut rng2 = Rng2::with_surrogate(&mut self.rng, self.frames);
        battle
            .round_with_inventory(orders, &set.data, self.game.inventory_mut(), &mut rng2)
            .map_err(|e| BridgeError::Rejected(e.to_string()))
    }
}
