//! `Event_HuntersGuild`'s indexed reads ([`GuildOp`]): the row in `d1`, the
//! flags and the purse. The table and its rules are `scenes/hunters_guild.rs`.

use super::{Blocked, SceneRunner};
use crate::scene::{DialogueId, GuildOp, SceneEffect, SceneFault};
use crate::scenes::hunters_guild::{TREE_27_FROM, choice_target, dispatch_target};
use crate::state::{Flag, GameState};

fn flag(id: u8) -> Flag {
    Flag::event(u16::from(id))
}

impl SceneRunner {
    pub(super) fn guild_op(
        &mut self,
        op: GuildOp,
        state: &mut GameState,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        let Some(jobs) = self.guild_jobs else {
            return Some(SceneFault::NoGuildTable);
        };
        let job = jobs[self.guild_row];
        match op {
            GuildOp::Dispatch => {
                (self.pc, self.guild_row) = dispatch_target(&jobs, |id| state.is_set(flag(id)));
            }
            GuildOp::BranchChoice => {
                (self.pc, self.guild_row) =
                    choice_target(&jobs, self.job_pick, |id| state.is_set(flag(id)));
            }
            GuildOp::SkipTreeLoad { line, skip } => {
                self.pc += 1;
                if line.entry(job) < TREE_27_FROM {
                    self.pc += usize::from(skip);
                }
            }
            GuildOp::RunLine { line } => {
                effects.push(SceneEffect::DialogueOpen(DialogueId(u16::from(
                    line.entry(job),
                ))));
                self.dialogue_window = crate::scene::DialogueWindow::Standard;
                self.blocked = Blocked::Dialogue;
                self.pc += 1;
            }
            GuildOp::RetireFirst => {
                for id in [jobs[0].taken, jobs[0].earned] {
                    if state.write(flag(id), false).is_err() {
                        return Some(SceneFault::BadWrite);
                    }
                    effects.push(SceneEffect::FlagChanged {
                        flag: flag(id),
                        value: false,
                    });
                }
                self.pc += 1;
            }
            GuildOp::SetTaken | GuildOp::SetPaid => {
                let id = if op == GuildOp::SetTaken {
                    job.taken
                } else {
                    job.paid
                };
                if state.write(flag(id), true).is_err() {
                    return Some(SceneFault::BadWrite);
                }
                effects.push(SceneEffect::FlagChanged {
                    flag: flag(id),
                    value: true,
                });
                self.pc += 1;
            }
            GuildOp::AddReward => {
                state.add_money(u32::from(job.money) * 1000);
                effects.push(SceneEffect::MoneyChanged {
                    total: state.money(),
                });
                self.pc += 1;
            }
        }
        None
    }
}
