//! Read-only views of the runtime for the shell and the renderer: pack data,
//! the current map's record and render paths, and the live party and world
//! objects.

use psiv_core::{BespokeActor, FieldMap, FieldState, GameState, MapId, MemberView, Wanderer};
use psiv_data::GameData;

use crate::{EffectOutcome, Runtime};

impl Runtime {
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

    /// An object's live dialogue id: the map-effect override when one is
    /// active, the record's own binding otherwise. The renderer's talk path
    /// must use this, not the record directly — `object_dialogue` patches
    /// are how clinics and story rooms change what a person says.
    #[must_use]
    pub fn npc_dialogue_id(&self, index: usize) -> Option<u16> {
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
