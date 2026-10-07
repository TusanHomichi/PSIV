//! The `inspect` subcommand: what a save file holds, read-only.
//!
//! A route author asks the same questions of a chapter save every time (who is
//! in the party, what do they wear, what is in the pack, which story flags are
//! set); this answers them from the save through the runtime's own views, so no
//! ad-hoc decoder drifts from the game.

use std::fmt::Write as _;
use std::path::Path;

use psiv_core::Flag;

use crate::start::{SetupError, StartPoint, open_session};

/// A text report of the save at `save`: position, purse, party with
/// equipment, inventory and the event flags that are set.
///
/// # Errors
///
/// [`SetupError`] when the pack or the save cannot be opened.
pub fn inspect(pack: &Path, save: &Path) -> Result<String, SetupError> {
    let (session, _) = open_session(pack, &StartPoint::Save(save.to_path_buf()))?;
    let runtime = session.runtime();
    let camp = runtime.camp_state();
    let cell = crate::driver::standing_cell(runtime);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "map {:#x} cell ({},{}) money {}",
        runtime.map_id().0,
        cell.x,
        cell.y,
        camp.money
    );
    for member in &camp.party {
        let _ = writeln!(
            out,
            "party {} {} L{} xp {} hp {}/{} tp {}/{} status {:#04x} str {} men {} agi {} dex {} atk {} def {} wears {:?}",
            member.party_slot,
            member.name,
            member.level,
            member.experience,
            member.current_hp,
            member.max_hp,
            member.current_tp,
            member.max_tp,
            member.status,
            member.strength,
            member.mental,
            member.agility,
            member.dexterity,
            member.attack_power,
            member.defense_power,
            member.equipment
        );
        if let Some(stats) = runtime.game().roster().get(psiv_core::CharId(member.id)) {
            let skills: Vec<String> = stats
                .skills
                .iter()
                .zip(stats.curr_skill_uses.iter().zip(&stats.max_skill_uses))
                .filter(|(id, _)| **id != 0)
                .map(|(id, (uses, max))| {
                    let name = runtime
                        .battle_skills()
                        .find(|skill| skill.id == *id)
                        .map_or("?", |skill| skill.name.as_str());
                    format!("{name} {uses}/{max}")
                })
                .collect();
            let techniques: Vec<&str> = stats
                .techniques
                .iter()
                .filter(|id| **id != 0)
                .map(|id| {
                    runtime
                        .battle_techniques()
                        .find(|tech| tech.id == *id)
                        .map_or("?", |tech| tech.name.as_str())
                })
                .collect();
            let _ = writeln!(
                out,
                "  skills [{}] techniques [{}]",
                skills.join(", "),
                techniques.join(", ")
            );
        }
    }
    let items: Vec<String> = camp
        .inventory
        .iter()
        .map(|item| format!("{}:{}", item.id, item.name))
        .collect();
    let _ = writeln!(out, "inventory {}", items.join(", "));
    let set: Vec<String> = (0..512u16)
        .filter(|id| runtime.game().is_set(Flag::event(*id)))
        .map(|id| format!("{id:#x}"))
        .collect();
    let _ = writeln!(out, "event flags {}", set.join(" "));
    Ok(out)
}
