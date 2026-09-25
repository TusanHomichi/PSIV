//! Small state seams the shell and replay drivers use directly, outside
//! gameplay rules: object position, facing and wander restoration, the window
//! suspension gate, the shared RNG seed and an immediate dialogue event-flag
//! write.

use psiv_core::battle::Lcg41;
use psiv_core::{Cell, Direction, Flag};

use crate::Runtime;

impl Runtime {
    /// Restores one object's position, facing and wander state — the
    /// object-side twin of [`Runtime::set_rng_seed`].
    ///
    /// A replay picking a tape up mid-run inherits objects that have been
    /// wandering since the opening scene: off their spawn cells, leashes no
    /// longer centred, several mid-step. Without this they start from the
    /// pack's spawn state and every object column diverges on frame one.
    ///
    /// A mid-step object's `cell` is its **destination**, because the engine
    /// commits that the moment a step starts.
    ///
    /// # Errors
    ///
    /// Whatever [`FieldMap`] or [`psiv_core::WanderSet`] rejects.
    pub fn restore_object(
        &mut self,
        npc_index: usize,
        cell: Cell,
        facing: Direction,
        state: psiv_core::WanderState,
    ) -> Result<(), psiv_core::MapError> {
        self.map.set_npc_cell(npc_index, cell)?;
        self.map.set_npc_facing(npc_index, facing)?;
        // Objects that do not wander (Alys on the academy floor) have position
        // and facing but no wander state; a missing wanderer is not an error.
        let _ = self.wander.restore(npc_index, state);
        Ok(())
    }

    /// Turns an object to face a direction — the cartridge's default when
    /// spoken to (`$F3` exists to suppress it). Out-of-range indices are the
    /// renderer's bug to log, not the engine's to crash on.
    pub fn face_npc(&mut self, index: usize, facing: Direction) {
        let _ = self.map.set_npc_facing(index, facing);
    }

    /// Places a field object at a cartridge pixel position.
    ///
    /// This is the position-side twin of [`Runtime::face_npc`]. It is used by
    /// receipt-backed replay/debug fixtures whose object has already wandered
    /// off its packed spawn point; normal gameplay reaches the same map seam
    /// through the field-object walker.
    ///
    /// # Errors
    ///
    /// [`psiv_core::MapError`] when the object index or pixel position is not
    /// valid for the loaded map.
    pub fn set_npc_pixel_position(
        &mut self,
        index: usize,
        x: i32,
        y: i32,
    ) -> Result<(), psiv_core::MapError> {
        self.map.set_npc_pixel_position(index, x, y)
    }

    /// Mirrors the cartridge's window-up suspension of field-object updates.
    /// The renderer sets this while a dialogue window is open.
    pub fn set_field_suspended(&mut self, suspended: bool) {
        self.field_suspended = suspended;
    }

    /// Seeds the shared RNG word — for replays that align to an oracle log.
    pub fn set_rng_seed(&mut self, seed: u32) {
        self.rng = Lcg41::new(seed);
    }

    /// Applies a retail dialogue `$F2` event-flag write immediately. The
    /// renderer uses this narrow mutator instead of reaching into save state.
    pub fn set_event_flag(&mut self, flag: u8) -> Result<(), psiv_core::MapError> {
        self.game.set(Flag::event(u16::from(flag)))
    }
}
