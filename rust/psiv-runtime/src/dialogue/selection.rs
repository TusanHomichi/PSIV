//! The single tree-selection owner for indexed dialogue opens.
//!
//! `ps4.asm:118299-118327` selects by live `World_Index` only for type-0
//! areas. NPCs retain the map-loaded tree (`:118603-118606`); scenes retain
//! it until `DialogueTreesToRAM` switches it (`:111677-111691`). Resumes and
//! mid-message jumps retain their already-selected tree/cursor.

use crate::Runtime;

pub(super) enum DialogueSource {
    Npc,
    Area { entry: u8 },
    Scene,
}

impl Runtime {
    pub(super) fn select_dialogue_tree(&self, source: DialogueSource) -> Option<u8> {
        match source {
            DialogueSource::Area { entry } => {
                let selection = self.dialogue_pack().trees.world_interaction.as_ref()?;
                let world = self.world_index();
                let tree = *selection.world_trees.get(usize::from(world))?;
                let high = &selection.first_world_override;
                Some(if world == 0 && entry >= high.entry_from {
                    high.tree
                } else {
                    tree
                })
            }
            DialogueSource::Scene if self.scene_tree_address.is_some() => {
                let address = self.scene_tree_address?;
                self.dialogue_pack()
                    .trees
                    .trees
                    .iter()
                    .find(|tree| tree.rom_address() == Some(address))
                    .map(|tree| tree.tree)
            }
            DialogueSource::Npc | DialogueSource::Scene => self
                .map_record()
                .map(|record| record.dialogue_tree)
                .filter(|tree| *tree != 0),
        }
    }
}
