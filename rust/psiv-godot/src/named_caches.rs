//! The places the shell deliberately keeps Godot objects that it fills while
//! the game runs, for the #92 memory check (`tools/native_rss.py`).
//!
//! Everything the shell loads at start-up (chrome, fonts, battle art tables,
//! scene panels, camp portraits) is in memory before the check's first probe
//! and stays the same size. Two kinds of holder grow during a run, and that
//! growth is not a leak: a texture cache filled on first use, bounded by the
//! pack's distinct assets of its kind, and a node pool made on first use and
//! reused, bounded by its fixed size or by content. Each reports its entries,
//! the Godot objects it keeps alive and their approximate bytes, beside that
//! ceiling. The memory bracket subtracts them and asserts the ceilings.

use godot::classes::ImageTexture;
use godot::prelude::*;

/// One named cache's size, as the debug accessor reports it.
///
/// Godot objects are reported by kind, because a texture is not always one
/// object: a headless engine's dummy renderer keeps each texture's image as a
/// second object. The memory probe measures that ratio itself
/// (`tools/native/native_tape.gd`).
pub(crate) struct CacheReport {
    pub(crate) name: &'static str,
    pub(crate) entries: usize,
    /// Textures kept alive.
    pub(crate) textures: usize,
    /// Nodes kept alive.
    pub(crate) nodes: usize,
    /// Other Godot objects kept alive (a material, a shader).
    pub(crate) other_objects: usize,
    /// The textures' pixel bytes (width x height x 4).
    pub(crate) bytes: u64,
    /// The most entries the pack's content (or the pool's fixed size) allows.
    pub(crate) ceiling: usize,
}

impl CacheReport {
    /// A texture cache's report: one texture per entry.
    pub(crate) fn textures<'a>(
        name: &'static str,
        textures: impl Iterator<Item = &'a Gd<ImageTexture>>,
        ceiling: usize,
    ) -> CacheReport {
        let mut entries = 0;
        let mut bytes = 0u64;
        for texture in textures {
            entries += 1;
            bytes += u64::try_from(texture.get_width()).unwrap_or(0)
                * u64::try_from(texture.get_height()).unwrap_or(0)
                * 4;
        }
        CacheReport {
            name,
            entries,
            textures: entries,
            nodes: 0,
            other_objects: 0,
            bytes,
            ceiling,
        }
    }

    /// A node pool's report: one node per entry, and `other_objects` the
    /// pool also keeps (a material and its shader).
    pub(crate) fn pool(
        name: &'static str,
        entries: usize,
        other_objects: usize,
        ceiling: usize,
    ) -> CacheReport {
        CacheReport {
            name,
            entries,
            textures: 0,
            nodes: entries,
            other_objects,
            bytes: 0,
            ceiling,
        }
    }

    pub(crate) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "entries": self.entries, "textures": self.textures, "nodes": self.nodes,
            "other_objects": self.other_objects, "bytes": self.bytes, "ceiling": self.ceiling,
        })
    }
}

/// The most scene temporary-object slots any transcribed scene uses: a
/// `SceneOp::ObjectAnimation` writes one field-object slot, and the shell keeps
/// one sprite per slot it has seen.
pub(crate) fn temporary_slot_ceiling() -> usize {
    psiv_core::SCENES
        .iter()
        .flat_map(|scene| scene.ops.iter())
        .filter_map(|op| match op {
            psiv_core::SceneOp::ObjectAnimation { slot, .. } => Some(*slot + 1),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}
