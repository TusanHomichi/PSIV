//! `Event_HuntersGuild`'s indexed reads ([`GuildOp`]): the row in `d1`, the
//! flags and the purse. The table and its rules are `scenes/hunters_guild.rs`.

use super::{Blocked, SceneRunner};
use crate::scene::{DialogueId, GuildOp, SceneEffect};
use crate::scenes::hunters_guild::{GUILD_JOBS, TREE_27_FROM, choice_target, dispatch_target};
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
    ) {
        let job = GUILD_JOBS[self.guild_row];
        match op {
            GuildOp::Dispatch => {
                (self.pc, self.guild_row) = dispatch_target(|id| state.is_set(flag(id)));
            }
            GuildOp::BranchChoice => {
                (self.pc, self.guild_row) =
                    choice_target(self.job_pick, |id| state.is_set(flag(id)));
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
            GuildOp::SetTaken | GuildOp::SetPaid => {
                let id = if op == GuildOp::SetTaken {
                    job.taken
                } else {
                    job.paid
                };
                // Row flags are all event flags in range.
                let _ = state.set(flag(id));
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
    }
}
