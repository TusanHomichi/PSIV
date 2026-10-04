//! The battle command menu: who answers, what they may choose, and the
//! `RoundOrders` the round runs with.
//!
//! Ported from `psiv-godot/src/battle/commands.rs` and `ui_input.rs` with the
//! S3 node, because the cartridge decides this in `BattleRoutines2`
//! (`ps4.asm:1138`) and the shell was making game decisions it could not own:
//! eligible actors, the TP/weapon/item-reservation enable rules,
//! target lists, the Defend default and the per-slot order vector.
//!
//! # The cartridge's buttons
//!
//! `RunBattleRoutines2` (`ps4.asm:1123`) sets `d5 = Joypad_Pressed` once per
//! battle frame and every menu routine tests it:
//!
//! | window | accept | cancel | cursor |
//! | --- | --- | --- | --- |
//! | main options, `Battle_MainOptions` (`ps4.asm:1916`) | `ButtonSpeak\|ButtonCamp` (`ps4.asm:1926`) | none | Up/Down, wrapping (`Battle_UpdateRedCursor2`, `ps4.asm:1572`) |
//! | character command, `Battle_CharCommand` (`ps4.asm:2192`) | `ButtonSpeak\|ButtonCamp` (`ps4.asm:2210`) | `ButtonCancel` (`ps4.asm:2206`) | Left/Right, wrapping (`Battle_UpdateCursor`, `ps4.asm:70646`, `d1=4`) |
//! | technique/skill/item windows, `Battle_TechWindow` (`ps4.asm:2593`) and twins | `ButtonSpeak\|ButtonCamp` (`ps4.asm:2614`) | `ButtonCancel` (`ps4.asm:2610`) | Up/Down inside a four-row page (`Battle_UpdateRedCursor2`), Left/Right flip pages |
//! | mounted skills, `Battle_VehSkills` (`ps4.asm:7461`) | `ButtonSpeak\|ButtonCamp` (`ps4.asm:7465`) | `ButtonCancel` (`ps4.asm:7463`) | Left/Right (`Battle_UpdateRedCursor`), `d1=1` |
//!
//! The per-character strip and the three list windows follow the cartridge's
//! direction tests (decoded in `docs/battle/BATTLE_COMMAND_UI.md`). Only the
//! target pages keep the port's own one-list mapping, because the cartridge
//! picks targets with a sprite cursor over the fighters.
//!
//! # What an empty row does
//!
//! `Battle_TechCommand` (`ps4.asm:2318`), `Battle_SkillCommand`
//! (`ps4.asm:2336`) and `Battle_ItemCommand` (`ps4.asm:2355`) each return
//! without changing the routine when their list is empty, and
//! `Battle_AttackCommand` (`ps4.asm:2233`) falls out at `loc_1682` when
//! neither hand holds a weapon. This port keeps the row-level rule it has
//! always had — a disabled row ignores the accept — and leaves the unarmed
//! attack to the engine's own `TurnSkipped::Unarmed`, which the cartridge
//! reaches through the same command path.

use psiv_core::battle::{Command, RoundOrders, Side};

use crate::Runtime;
use crate::pad::{Button, Pad};

use super::view::{MenuView, SkillSlotView};

/// The `VehicleAttackNames` slots a mounted record can carry
/// (`ps4.asm:321193`). The shell owns the display names; the runtime only
/// counts the slots.
const VEHICLE_SKILL_SLOTS: usize = 8;

mod commands;

pub(crate) use commands::CommandsMenu;

/// What a main-options accept chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopChoice {
    /// Nothing this frame.
    Nothing,
    /// COMD: open the per-character command window.
    Commands,
    /// OPTIN: open the mounted surface's skill window.
    VehicleSkills,
    /// COMD on a mounted surface (its ATTAC row): everybody attacks.
    AttackAll,
    /// RUN.
    Run,
}

/// Reads one frame of the main options.
///
/// `Battle_MainOptions` (`ps4.asm:1916`): Up/Down move `Battle_Main_Option_Index`
/// with the wrap `Battle_UpdateRedCursor2` implements, `ButtonSpeak|ButtonCamp`
/// accept, and there is no cancel at all — a player must choose a row. The
/// mounted surface's window is the same one with different rows
/// (`Battle_VehMainOptions`, `ps4.asm:2003`): ATTAC, OPTIN, RUN.
pub(crate) fn top_input(cursor: &mut usize, vehicle: bool, previous: Pad, pad: Pad) -> TopChoice {
    const ROWS: usize = 3;
    if pad.is_pressed(previous, Button::Up) {
        *cursor = (*cursor + ROWS - 1) % ROWS;
    }
    if pad.is_pressed(previous, Button::Down) {
        *cursor = (*cursor + 1) % ROWS;
    }
    if !accept_pressed(previous, pad) {
        return TopChoice::Nothing;
    }
    match (*cursor, vehicle) {
        (0, false) => TopChoice::Commands,
        (0, true) => TopChoice::AttackAll,
        (1, true) => TopChoice::VehicleSkills,
        (2, _) => TopChoice::Run,
        // MACR is visible for ordinary-party parity; macro execution is
        // Tier 3 and the row does nothing.
        _ => TopChoice::Nothing,
    }
}

/// The accept rule every battle window shares:
/// `ButtonSpeak|ButtonCamp` (`ps4.asm:1926`, `2210`, `2614`, `7465`).
pub(crate) fn accept_pressed(previous: Pad, pad: Pad) -> bool {
    pad.is_pressed(previous, Button::Speak) || pad.is_pressed(previous, Button::Camp)
}

/// The confirm rule the post-battle pages share:
/// `ButtonCancel|ButtonSpeak|ButtonCamp` (`ps4.asm:4706`, `6351`).
///
/// Retail's own "any face button continues" rule for the victory, results and
/// last-message windows. The shell's beat playback used to test `ui_accept`
/// alone; the cartridge accepts B as well.
pub(crate) fn confirm_pressed(previous: Pad, pad: Pad) -> bool {
    accept_pressed(previous, pad) || pad.is_pressed(previous, Button::Cancel)
}

/// The cursor step this port's one-list windows take from a press edge.
///
/// Shared with the per-character window ([`commands`]) so both windows step the
/// same way.
pub(super) fn list_step(previous: Pad, pad: Pad) -> isize {
    let mut delta = 0;
    if pad.is_pressed(previous, Button::Up) || pad.is_pressed(previous, Button::Left) {
        delta -= 1;
    }
    if pad.is_pressed(previous, Button::Down) || pad.is_pressed(previous, Button::Right) {
        delta += 1;
    }
    delta
}

/// The mounted surface's skill window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VehicleWindow {
    /// The selected slot.
    pub(crate) cursor: usize,
}

impl VehicleWindow {
    /// Reads one frame of the mounted skill window.
    ///
    /// The cartridge's `Battle_VehSkills` (`ps4.asm:7461`) runs a
    /// one-dimensional cursor over two slots with Left/Right; this port's
    /// window lays every learned slot out in two columns, so it keeps the
    /// shell's own mapping: up/down step a row, left/right step a column.
    pub(crate) fn input(
        &mut self,
        runtime: &Runtime,
        previous: Pad,
        pad: Pad,
    ) -> Option<RoundOrders> {
        let slots = vehicle_slots(runtime);
        let last = slots.len().saturating_sub(1);
        if pad.is_pressed(previous, Button::Up) {
            self.cursor = self.cursor.saturating_sub(2);
        }
        if pad.is_pressed(previous, Button::Down) {
            self.cursor = (self.cursor + 2).min(last);
        }
        if pad.is_pressed(previous, Button::Left) {
            self.cursor = self.cursor.saturating_sub(1);
        }
        if pad.is_pressed(previous, Button::Right) {
            self.cursor = (self.cursor + 1).min(last);
        }
        if !accept_pressed(previous, pad) {
            return None;
        }
        let slot = slots.get(self.cursor).filter(|slot| slot.current > 0)?;
        Some(RoundOrders::Commands(vec![Command::VehicleSkill(slot.id)]))
    }

    /// What the window draws.
    pub(crate) fn view(self, runtime: &Runtime) -> MenuView {
        let slots = vehicle_slots(runtime);
        MenuView::VehicleSkills {
            cursor: self.cursor.min(slots.len().saturating_sub(1)),
            slots,
        }
    }
}

/// The mounted record's skill slots, read live from the battle roster.
///
/// This is the S3 fidelity fix the brief names: the shell kept its own copy of
/// the mounted record's skills and subtracted a use when the window closed
/// (`psiv-godot/src/battle/ui_input.rs`, `self.vehicle_skills[...].current -= 1`),
/// while the engine subtracts the use from the battle record the moment the
/// command resolves (`psiv-core/src/battle/engine.rs`, `Command::VehicleSkill`).
/// Reading the roster means the window shows the record, and the absorbed
/// record carries the count back into game state at battle end.
pub(crate) fn vehicle_slots(runtime: &Runtime) -> Vec<SkillSlotView> {
    let Some(fighter) = runtime
        .battle_roster()
        .and_then(|roster| roster.side(Side::Party).next())
    else {
        return Vec::new();
    };
    fighter
        .stats
        .skills
        .iter()
        .copied()
        .enumerate()
        .take(VEHICLE_SKILL_SLOTS)
        .filter(|(_, id)| *id != 0)
        .map(|(slot, id)| SkillSlotView {
            id,
            current: fighter.stats.curr_skill_uses[slot],
            max: fighter.stats.max_skill_uses[slot],
        })
        .collect()
}
