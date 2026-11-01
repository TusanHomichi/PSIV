//! Read-only views of the runtime for the shell and the renderer: pack data,
//! the current map's record and render paths, and the live party and world
//! objects.

use psiv_core::{
    BespokeActor, Cell, Direction, FieldMap, FieldState, GameState, MapId, MemberView, Wanderer,
};
use psiv_data::GameData;

use crate::{EffectOutcome, Runtime};

/// A field object the engine moves, as the renderer places it: its committed
/// cell, facing and the step still to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectMotion {
    /// The object's map slot.
    pub npc_index: usize,
    /// The cell it stands on or is stepping to.
    pub cell: Cell,
    /// The way it faces.
    pub facing: Direction,
    /// Whether a step is in progress.
    pub stepping: bool,
    /// Sub-cell displacement in sixteenths, back from `cell`.
    pub offset_16ths: (i32, i32),
}

impl Runtime {
    /// Shared RNG longword, read-only, for source/draw-order receipts.
    #[must_use]
    pub fn rng_seed(&self) -> u32 {
        self.rng.seed()
    }
    /// Current palette/scroll outputs and explicitly unsupported map inputs.
    #[must_use]
    pub fn map_updates(&self) -> &crate::MapUpdateView {
        &self.effects.updates
    }
    /// The currently loaded map.
    #[must_use]
    pub fn map_id(&self) -> MapId {
        self.map.id()
    }

    /// The engine map, for the renderer's collision/NPC queries.
    #[must_use]
    pub fn map(&self) -> &FieldMap {
        &self.map
    }

    /// The loaded pack, for presentation-layer queries (sprite sheets, the
    /// current map's record). Read-only; the runtime owns all mutation.
    #[must_use]
    pub fn data(&self) -> &GameData {
        &self.data
    }

    /// The current map's record, for presentation-layer queries (NPC sprite
    /// bindings and the like).
    #[must_use]
    pub fn map_record(&self) -> Option<&psiv_data::MapRecord> {
        self.data.map(psiv_data::MapId(self.map.id().0))
    }

    /// The current map's composed-render path, exactly as the pack declares
    /// it (relative to the pack root). The renderer must never invent pack
    /// filenames; the pack names its own files. When a `layout_replace` is
    /// active this is the variant's render.
    #[must_use]
    pub fn map_png(&self) -> Option<&str> {
        let record = self.data.map(psiv_data::MapId(self.map.id().0))?;
        match self.effects.variant {
            Some(index) => record.layout_variants.get(index).map(|v| v.png.as_str()),
            None => Some(record.png.as_str()),
        }
    }

    /// The current map's priority-overlay path — tiles the VDP draws above
    /// sprites — or `None` when the map has no priority tiles. Variant-aware
    /// like [`Runtime::map_png`].
    #[must_use]
    pub fn map_png_over(&self) -> Option<&str> {
        let record = self.data.map(psiv_data::MapId(self.map.id().0))?;
        match self.effects.variant {
            Some(index) => record
                .layout_variants
                .get(index)
                .and_then(|v| v.png_over.as_deref()),
            None => record.png_over.as_deref(),
        }
    }

    /// An object's live dialogue id: the line its own routine last wrote,
    /// then the map-effect override when one is active, then the record's own
    /// binding. The renderer's talk path must use this, not the record
    /// directly — `object_dialogue` patches are how clinics and story rooms
    /// change what a person says.
    #[must_use]
    pub fn npc_dialogue_id(&self, index: usize) -> Option<u16> {
        if let Some(id) = self
            .bespoke
            .get(index)
            .and_then(psiv_core::BespokeActor::dialogue_id)
        {
            return Some(id);
        }
        if let Some(id) = self.effects.dialogue_overrides.get(&index) {
            return Some(*id);
        }
        self.data
            .map(psiv_data::MapId(self.map.id().0))
            .and_then(|record| record.npcs.get(index))
            .map(|npc| npc.dialogue_id)
    }

    /// The current map's evaluated effect outcome, for the renderer's gap
    /// logging (unresolved layout writes, undecoded entries).
    #[must_use]
    pub fn map_effects(&self) -> &EffectOutcome {
        &self.effects
    }

    /// The field state, for the renderer's position/interpolation queries.
    #[must_use]
    pub fn state(&self) -> &FieldState {
        self.party.leader()
    }

    /// Every party member in draw order (0 = leader), for the renderer.
    #[must_use]
    pub fn members(&self) -> Vec<MemberView> {
        self.party.members()
    }

    /// The map's wanderers, for the renderer's per-frame positions.
    #[must_use]
    pub fn wanderers(&self) -> &[Wanderer] {
        self.wander.wanderers()
    }

    /// Every active object a field routine moves (a wanderer, or a bespoke
    /// actor whose routine steps, including one a scene's `RunObjectRoutine`
    /// installed), in slot order, at its step's live interpolation and its
    /// routine's own speed. The renderer places these from the engine, never
    /// from the map layout or a scene's cast.
    #[must_use]
    pub fn object_motions(&self) -> Vec<ObjectMotion> {
        let npcs = self.map.npcs();
        let live = |index: usize| npcs.get(index).filter(|npc| npc.active);
        let mut motions: Vec<ObjectMotion> = self
            .wander
            .wanderers()
            .iter()
            .filter_map(|w| {
                let npc = live(w.npc_index())?;
                Some(ObjectMotion {
                    npc_index: w.npc_index(),
                    cell: npc.cell,
                    facing: npc.facing,
                    stepping: w.is_stepping(),
                    offset_16ths: w.render_offset_16ths(),
                })
            })
            .chain(
                self.bespoke
                    .actors()
                    .iter()
                    .filter(|actor| actor.kind().reports_state())
                    .filter_map(|actor| {
                        let npc = live(actor.npc_index())?;
                        Some(ObjectMotion {
                            npc_index: actor.npc_index(),
                            cell: npc.cell,
                            facing: npc.facing,
                            stepping: actor.is_stepping(),
                            offset_16ths: actor.render_offset_16ths(),
                        })
                    }),
            )
            .collect();
        motions.sort_by_key(|motion| motion.npc_index);
        motions
    }

    /// The map's bespoke field-object actors, for renderer and replay state.
    #[must_use]
    pub fn bespoke_actors(&self) -> &[BespokeActor] {
        self.bespoke.actors()
    }

    /// The persistent game state (flags, party, money).
    #[must_use]
    pub fn game(&self) -> &GameState {
        &self.game
    }
}
