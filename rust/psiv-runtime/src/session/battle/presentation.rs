//! The battle mode's view: what the frame shows, and the names it says.
//!
//! Split out of `mod.rs` with the S3 node so the loop's own file stays about
//! the loop. Everything here reads state the mode already decided.

use psiv_core::battle::Side;

use crate::Runtime;

use super::view::{
    BattleBeat, BattleView, BeatView, DamageView, EnemyStatus, MenuView, PartyStatus,
};
use super::{BattleMode, Window};

impl BattleMode {
    /// Seats the narration names, the party strip and the enemy list from the
    /// battle the runtime started.
    pub(super) fn seat(&mut self, runtime: &Runtime) {
        for (index, member) in runtime.battle_party().into_iter().enumerate() {
            let fighter = index as u8 + 1;
            self.names.insert(fighter, member.name.clone());
            self.character_names
                .insert(member.character, member.name.clone());
            self.party.push(PartyStatus {
                fighter,
                name: member.name.clone(),
                hp: member.stats.curr_hp,
                max_hp: member.stats.max_hp,
                tp: member.stats.curr_tp,
                status: member.stats.status,
            });
        }
        let Some(roster) = runtime.battle_roster() else {
            return;
        };
        for fighter in roster.side(Side::Enemy) {
            let name = self
                .enemy_name(runtime, fighter.stats.enemy_id)
                .unwrap_or_else(|| fighter.name.clone());
            self.names.insert(fighter.id.get(), name.clone());
            // Enemy fighter ids are not character ids: `LearnedAbility` and
            // `LevelUp` name a character, and a name inserted here would
            // shadow that character's own name for those pages.
            self.enemies.push(EnemyStatus {
                fighter: fighter.id.get(),
                name,
                visible: true,
            });
        }
    }

    fn enemy_name(&self, runtime: &Runtime, enemy_id: u16) -> Option<String> {
        runtime
            .battle_enemy_name(enemy_id)
            .map(std::string::ToString::to_string)
    }

    /// What this frame draws.
    pub(super) fn view(&self, runtime: &Runtime, close_ready: bool) -> BattleView {
        let menu = if self.command_open {
            Some(match self.window.as_ref() {
                Some(Window::Commands(commands)) => MenuView::Commands(commands.view()),
                Some(Window::VehicleSkills(skills)) => skills.view(runtime),
                None => MenuView::Top {
                    cursor: self.cursor,
                },
            })
        } else {
            None
        };
        let current = self.current.map(|active| BeatView {
            beat: active.beat,
            remaining: active.remaining,
            total: active.total,
            waits_for_confirm: active.waits_for_confirm,
        });
        let poses = match current.as_ref().map(|beat| beat.beat) {
            Some(BattleBeat::Attack(actor)) => vec![actor],
            _ => Vec::new(),
        };
        let damage = match current.as_ref().map(|beat| beat.beat) {
            Some(BattleBeat::Damage {
                target,
                amount: Some(amount),
                ..
            }) => Some(DamageView { target, amount }),
            _ => None,
        };
        BattleView {
            menu,
            ready: self.command_open,
            finishing: self.finish_outcome.is_some() || self.finish_request.is_some(),
            cursor: self.cursor,
            message: self.message.clone(),
            message_kind: self.message_kind,
            reward_each: self.reward_each,
            reward_meseta: self.reward_meseta,
            damage,
            party: self.party.clone(),
            enemies: self.enemies.clone(),
            poses,
            current,
            transient: self.transient,
            sounds: self.sounds.clone(),
            animations: self.animations.clone(),
            close_ready,
            fault: self.fault.clone(),
        }
    }
}
