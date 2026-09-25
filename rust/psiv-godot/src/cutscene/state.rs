//! Renderer state the runtime deliberately does not hold.
//!
//! Panel planes, temporary objects, hidden characters, loaded palettes and art
//! and the saved-music word are bookkeeping between the ordered `SceneOp`s the
//! runtime emits. Keeping it here is what lets `psiv-runtime` stop at field
//! semantics: nothing in this module advances a scene or sets a scene flag.

use std::collections::BTreeMap;

use psiv_data::DialogueSet;

/// Renderer state that has no place in `psiv-runtime`'s field semantics.
#[derive(Debug, Default)]
pub(crate) struct PresentationState {
    /// Literal Render_Sprites_In_Cutscenes byte: nonzero suppresses field
    /// sprites in a bit-15 cutscene and selects the panel portrait layout.
    suppress_cutscene_sprites: bool,
    /// `InitVramAndCram` wiped the stage: no map, no actors, until a map
    /// redraw reloads the art. Distinct from `render_sprites`, which mirrors
    /// retail's explicit cutscene sprite toggle and survives map loads.
    vram_blanked: bool,
    saved_music: Option<u8>,
    pub(super) temporary_objects: BTreeMap<usize, TemporaryObject>,
    pub(super) hidden_characters: std::collections::BTreeSet<u8>,
    pub(super) loaded_palettes: BTreeMap<u32, u16>,
    loaded_art: BTreeMap<(u32, u16), u64>,
    pub(super) current_dialogue_tree: Option<u32>,
    dialogue_trees: BTreeMap<u32, u8>,
    pub(super) ending_waiting_for_start: bool,
    pub(super) op_count: u64,
}

/// The literal fields of a retail temporary object.  A matching map sprite is
/// used when available; no synthetic art is invented when the pack has not
/// decoded the object's Nemesis blob yet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TemporaryObject {
    pub(crate) object_id: u16,
    pub(crate) art_tile: u16,
    pub(crate) frames_left: u16,
    pub(crate) elapsed: u64,
    pub(crate) destination: Option<(i32, i32)>,
    persistent: bool,
    motion: Option<ObjectMotion>,
}

#[derive(Debug, Clone, Copy)]
struct ObjectMotion {
    position: (i64, i64),
    step: (i32, i32),
    remaining: u16,
}

impl PresentationState {
    pub(crate) fn reset_scene(&mut self) {
        self.suppress_cutscene_sprites = false;
        self.vram_blanked = false;
        self.temporary_objects.clear();
        self.hidden_characters.clear();
        self.loaded_art.clear();
        self.current_dialogue_tree = None;
        self.ending_waiting_for_start = false;
    }

    pub(crate) fn character_visible(&self, who: u8) -> bool {
        !self.hidden_characters.contains(&who)
    }

    pub(crate) fn panel_dialogue_mode(&self, scene_event: Option<psiv_core::EventIndex>) -> bool {
        self.suppress_cutscene_sprites && scene_event.is_some_and(|event| event.0 & 0x8000 != 0)
    }

    pub(crate) fn sprites_visible(&self, scene_event: Option<psiv_core::EventIndex>) -> bool {
        scene_event.is_none() || (!self.panel_dialogue_mode(scene_event) && !self.vram_blanked)
    }

    pub(crate) fn set_vram_blanked(&mut self, blanked: bool) {
        self.vram_blanked = blanked;
    }

    pub(crate) fn set_render_sprites(&mut self, enabled: bool) {
        self.suppress_cutscene_sprites = enabled;
    }

    pub(crate) fn set_saved_music(&mut self, id: u8) {
        self.saved_music = (id != 0).then_some(id);
    }

    pub(crate) fn load_art(&mut self, rom_addr: u32, tile: u16) {
        self.loaded_art.insert((rom_addr, tile), self.op_count);
    }

    pub(crate) fn art_loaded(&self, rom_addr: u32, tile: u16) -> bool {
        self.loaded_art.contains_key(&(rom_addr, tile))
    }

    pub(crate) fn configure_dialogue_trees(&mut self, set: &DialogueSet) {
        self.dialogue_trees = set
            .trees
            .trees
            .iter()
            .filter_map(|tree| tree.rom_address().map(|address| (address, tree.tree)))
            .collect();
    }

    pub(crate) fn scene_dialogue_tree(&self, fallback: u8) -> Option<u8> {
        match self.current_dialogue_tree {
            Some(address) => self.dialogue_trees.get(&address).copied(),
            None => Some(fallback),
        }
    }

    pub(crate) fn take_saved_music(&mut self) -> Option<u8> {
        self.saved_music.take()
    }

    pub(crate) fn temporary_draws(&self) -> Vec<(usize, TemporaryObject)> {
        self.temporary_objects
            .iter()
            .map(|(&slot, &object)| (slot, object))
            .collect()
    }

    pub(super) fn object_animation(
        &mut self,
        slot: usize,
        object_id: u16,
        art_tile: u16,
        frames: u16,
    ) {
        self.temporary_objects.insert(
            slot,
            TemporaryObject {
                object_id,
                art_tile,
                frames_left: frames,
                elapsed: 0,
                destination: None,
                persistent: false,
                motion: None,
            },
        );
    }

    pub(super) fn object_destination(&mut self, slot: usize, x: i32, y: i32) {
        if let Some(object) = self.temporary_objects.get_mut(&slot) {
            object.destination = Some((x, y));
            object.motion = None;
        }
    }

    pub(super) fn create_field_object(&mut self, slot: usize, id: u16, tile: u16, x: i32, y: i32) {
        self.object_animation(slot, id, tile, u16::MAX);
        let object = self.temporary_objects.get_mut(&slot).unwrap();
        object.persistent = true;
        object.destination = Some((x, y));
    }

    pub(super) fn step_field_object(&mut self, slot: usize, step_x: i32, step_y: i32, frames: u16) {
        if let Some(object) = self.temporary_objects.get_mut(&slot)
            && let Some((x, y)) = object.destination
        {
            object.motion = (frames > 0).then_some(ObjectMotion {
                position: object
                    .motion
                    .map_or((i64::from(x) << 16, i64::from(y) << 16), |motion| {
                        motion.position
                    }),
                step: (step_x, step_y),
                remaining: frames,
            });
        }
    }

    pub(crate) fn despawn_objects(&mut self, first: usize, count: usize) {
        self.temporary_objects
            .retain(|slot, _| !(first..first + count).contains(slot));
    }

    pub(crate) fn reload_field_objects(&mut self) {
        self.temporary_objects.clear();
    }

    pub(super) fn advance_objects(&mut self) {
        for object in self.temporary_objects.values_mut() {
            if !object.persistent {
                object.frames_left = object.frames_left.saturating_sub(1);
            }
            if let Some(motion) = object.motion.as_mut()
                && motion.remaining > 0
            {
                motion.position.0 += i64::from(motion.step.0);
                motion.position.1 += i64::from(motion.step.1);
                object.destination = Some((
                    (motion.position.0 >> 16) as i32,
                    (motion.position.1 >> 16) as i32,
                ));
                motion.remaining -= 1;
            }
            object.elapsed += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PresentationState;

    #[test]
    fn field_object_keeps_its_identity_and_fractional_position_until_despawn() {
        let mut state = PresentationState::default();
        state.create_field_object(7, 0x188, 0x3A5, 480, 160);
        for _ in 0..500 {
            state.advance_objects();
        }
        assert_eq!(state.temporary_objects[&7].destination, Some((480, 160)));
        assert!(state.temporary_objects[&7].frames_left > 0);
        state.step_field_object(7, 0, 0x8000, 64);
        for _ in 0..63 {
            state.advance_objects();
        }
        assert_eq!(state.temporary_objects[&7].destination, Some((480, 191)));
        state.advance_objects();
        assert_eq!(state.temporary_objects[&7].destination, Some((480, 192)));
        for _ in 0..100 {
            state.advance_objects();
        }
        assert_eq!(state.temporary_objects[&7].destination, Some((480, 192)));
        assert_eq!(state.temporary_objects[&7].object_id, 0x188);
        state.step_field_object(7, -0x8000, 0, 1);
        state.advance_objects();
        assert_eq!(state.temporary_objects[&7].destination, Some((479, 192)));
        state.step_field_object(7, 0x8000, 0, 1);
        state.advance_objects();
        assert_eq!(
            state.temporary_objects[&7].destination,
            Some((480, 192)),
            "successive calls preserve the fraction"
        );
        state.despawn_objects(0, 7);
        assert_eq!(state.temporary_objects.len(), 1);
        state.despawn_objects(7, 1);
        assert!(state.temporary_objects.is_empty());
    }

    #[test]
    fn every_scene_dialogue_tree_resolves_from_the_original_pack() {
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        if !pack.join("dialogue/trees.json").is_file() {
            eprintln!("runtime pack absent; skipping scene dialogue census");
            return;
        }
        let set = psiv_data::DialogueSet::load(&pack).expect("original dialogue pack");
        let mut state = PresentationState::default();
        state.configure_dialogue_trees(&set);
        assert_eq!(state.dialogue_trees.len(), 43);
        assert_eq!(state.scene_dialogue_tree(4), Some(4));
        for scene in psiv_core::SCENES {
            for op in scene.ops {
                if let psiv_core::SceneOp::SetDialogueTree { rom_addr } = op {
                    state.current_dialogue_tree = Some(*rom_addr);
                    assert!(
                        state.scene_dialogue_tree(4).is_some(),
                        "{}: {rom_addr:#x}",
                        scene.name
                    );
                }
            }
        }
        // Alshline explicitly switches away from Zema's map tree. Its first
        // conversation pauses and resumes; the map-tree entries are empty.
        state.current_dialogue_tree = Some(0x1E0BA0);
        assert_eq!(state.scene_dialogue_tree(4), Some(3));
        for entry in [0x67, 0x68, 0x69] {
            assert!(!set.entry(3, entry).unwrap().text.is_empty());
            assert!(set.entry(4, entry).unwrap().text.is_empty());
        }
        state.current_dialogue_tree = Some(0x123456);
        assert_eq!(
            state.scene_dialogue_tree(4),
            None,
            "unknown addresses must not silently use town text"
        );
        state.reset_scene();
        assert_eq!(state.scene_dialogue_tree(4), Some(4));
        assert_eq!(
            state.dialogue_trees.len(),
            43,
            "scene reset retains pack metadata"
        );
    }

    #[test]
    fn saved_music_is_a_one_shot_restore_word_and_zero_clears_it() {
        let mut state = PresentationState::default();
        assert_eq!(state.take_saved_music(), None);

        state.set_saved_music(0x91);
        assert_eq!(state.take_saved_music(), Some(0x91));
        assert_eq!(state.take_saved_music(), None);

        state.set_saved_music(0x91);
        state.set_saved_music(0);
        assert_eq!(state.take_saved_music(), None);
    }

    #[test]
    fn original_panel_flag_suppresses_only_bit_15_cutscenes() {
        let mut state = PresentationState::default();
        let cutscene = Some(psiv_core::EventIndex(0x8005));
        let field_event = Some(psiv_core::EventIndex(0x17));
        assert!(state.sprites_visible(cutscene));
        state.set_render_sprites(true);
        assert!(!state.sprites_visible(cutscene));
        assert!(state.sprites_visible(field_event));
        assert!(state.sprites_visible(None));
        state.set_render_sprites(false);
        assert!(state.sprites_visible(cutscene));
        state.set_vram_blanked(true);
        assert!(!state.sprites_visible(cutscene));
        assert!(!state.sprites_visible(field_event));
        state.set_vram_blanked(false);
        assert!(state.sprites_visible(cutscene));
    }
}
