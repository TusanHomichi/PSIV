//! The status strip's per-pane icon: the command or status icon beside each
//! name, which the runtime decides every frame
//! (`session/battle/panes.rs`, `Battle_DrawCommandIcons`, `ps4.asm:11056`).

use psiv_runtime::{BattleView, PartyStatus};

use super::chrome::Quad;
use super::command_window::ICON_WORDS;
use super::layout::tile_dest;
use super::tiles::CommandTiles;

/// The icon and ink line a pane draws: the runtime's pane decision, or for a
/// view that carries none (a static fixture) the idle default
/// (`Battle_DrawCommandIcons`: no command yet, the window line).
fn pane_icon(view: &BattleView, member: Option<&PartyStatus>, fighter: u8) -> (u8, usize) {
    if let Some(pane) = view.panes.iter().find(|pane| pane.fighter == fighter) {
        return (pane.icon, usize::from(pane.ink_line));
    }
    (if member.is_some() { 0 } else { 9 }, 3)
}

/// Draws a pane's icon two cells wide and two high at the pane's fifth cell
/// (`loc_75D8`, `ps4.asm:11182`: `$8, $A, $88, $8A` from the name cell) and
/// returns the CRAM line the pane's text takes.
pub(super) fn append_pane_icon(
    tiles: &mut CommandTiles,
    view: &BattleView,
    member: Option<&PartyStatus>,
    fighter: u8,
    start: i32,
    quads: &mut Vec<Quad>,
) -> usize {
    let (icon, line) = pane_icon(view, member, fighter);
    for (index, word) in ICON_WORDS[usize::from(icon).min(9)].iter().enumerate() {
        let at = tile_dest(start + 5 + (index as i32 % 2), 22 + (index as i32 / 2));
        if let Some(quad) = tiles.word(*word, at) {
            quads.push(quad);
        }
    }
    line
}
