//! Per-frame field drawing, split from the Godot bridge so the bridge stays
//! below the repository's one-thousand-line maintenance limit.

use std::collections::HashSet;

use godot::classes::{ColorRect, Sprite2D};
use godot::prelude::*;

use psiv_core::{Cell, Direction, SCREEN_HEIGHT, SCREEN_WIDTH};

use super::battle::{BATTLE_FRAME_HEIGHT, BATTLE_FRAME_WIDTH};
use super::transitions::{Transition, TransitionKind};
use super::view::{SheetView, camp_receipt_frame, sequence_name};
use super::window_palette::{WINDOW_LINE_ROM, WindowFamily, WindowLine, WindowNode, unlit_line};
use super::{Field, LETTERBOX_BARS, Session};

/// The one material per CRAM line that the native windows draw their art
/// through (`window_palette`), and `debug_window_line`'s words for each
/// line, if any.
#[derive(Default)]
pub(crate) struct WindowLineState {
    materials: [Option<Gd<godot::classes::ShaderMaterial>>; 4],
    debug_live: [Option<[u16; 16]>; 4],
}

#[godot_api(secondary)]
impl Field {
    /// Evaluation only (`PSIV_DEBUG_ROUTE=1`): draws the window art on CRAM
    /// `line` in `words` (sixteen CRAM words) instead of the live CRAM's, or
    /// in the live CRAM's again when `words` is empty. Nothing else reads
    /// it: the runtime's CRAM, the map, the sprites and the windows on other
    /// lines keep their words. False when refused.
    #[func]
    fn debug_window_line(&mut self, line: i32, words: PackedInt32Array) -> bool {
        self.window_line_probe(line, words.as_slice())
    }
}

/// The transition overlay: four rects, made on the first transition.
pub(crate) const TRANSITION_RECTS: usize = 4;

impl Field {
    /// Places and animates the party sprite, animates NPCs, moves the camera.
    pub(super) fn sync_visuals(&mut self, walking: bool) {
        self.sync_map_palette();
        self.sync_map_art_frame();
        self.sync_belt_tiles();
        if let Some(layer) = self.cutscene_layer.as_mut() {
            let credits = self
                .session
                .as_ref()
                .and_then(|session| session.runtime().ending_credits());
            layer.bind_mut().sync_credits(credits);
        }
        let Some(runtime) = self.session.as_ref().map(Session::runtime) else {
            return;
        };
        let state = runtime.state();
        let cell = state.cell();
        let offset = state.render_offset_16ths();
        let scene_actors: Vec<psiv_core::ScriptedActor> = runtime.scene_actor_draws();
        let party_visible: Vec<bool> = rt_party_visibility(runtime, &self.presentation);
        let scene_sprites_visible = self
            .presentation
            .sprites_visible(runtime.scene_event(), runtime.panel_dialogue_mode());
        let active_npcs: Vec<bool> = runtime.map().npcs().iter().map(|npc| npc.active).collect();
        let beams: Vec<_> = runtime
            .bespoke_actors()
            .iter()
            .filter_map(|actor| actor.beam_draw().map(|draw| (actor.npc_index(), draw)))
            .collect();
        let torch_objects: Vec<_> = runtime.eclipse_torch_objects().copied().collect();
        let camera_position = runtime.camera().position();
        let step_frames = runtime.step_frames();
        // A scene that drives the leader displaces the party draw: position,
        // facing, and walk state all come from the scripted actor, so the
        // walk to the door is a walk and not a freeze-then-teleport.
        let leader_scripted = runtime.scene_party_actor(0).copied();
        let (kind, party_facing, party_cell, party_offset) = match &leader_scripted {
            Some(actor) => (
                if actor.is_walking() { "walk" } else { "idle" },
                actor.facing,
                actor.cell,
                actor.render_offset_16ths(step_frames),
            ),
            None => (
                if walking { "walk" } else { "idle" },
                state.facing(),
                cell,
                offset,
            ),
        };
        let sequence = sequence_name(kind, party_facing);
        let camp_field_anchor = self
            .camp_menu
            .as_ref()
            .is_some_and(|menu| menu.bind().is_open() && !menu.bind().is_loot());
        // A paralysed or dead member, or one a conveyor belt carries, holds
        // its sequence's first frame (`FieldObj_UpdateStatus`); it starts
        // the sequence over when it thaws.
        let frozen: Vec<bool> = (0..psiv_core::PARTY_SLOTS)
            .map(|slot| runtime.member_walk_frozen(slot))
            .collect();
        if sequence != self.party_sequence || frozen[0] {
            self.party_sequence = sequence;
            self.party_seq_start = self.anim_tick;
        }

        if let (Some(party), Some(view)) = (self.party.as_mut(), self.party_view.as_ref()) {
            let frame = view.frame_at(&self.party_sequence, self.anim_tick - self.party_seq_start);
            view.apply(party, frame);
            let position = if camp_field_anchor {
                view.draw_pos_camp(party_cell, party_offset)
            } else {
                view.draw_pos(party_cell, party_offset)
            };
            party.set_position(position);
        }

        // The boarding event replaces the party body before it writes the
        // persistent selector; the same draw slot serves that brief pan.
        let vehicle_state = runtime.vehicle_draw_state();
        if let Some(vehicle_state) = vehicle_state {
            let sheet_id = runtime
                .data()
                .vehicle_sheet_for_map(runtime.map_id().0, vehicle_state.index())
                .map(|sheet| sheet.id.clone());
            if let Some(sheet_id) = sheet_id {
                if !self.sheet_views.contains_key(&sheet_id)
                    && let Some(sheet) = runtime.data().sheet(&sheet_id)
                    && let Some(view) = SheetView::build(&self.pack_dir, sheet, Some(&self.cram))
                {
                    self.sheet_views.insert(sheet_id.clone(), view);
                }
                if let Some(view) = self.sheet_views.get(&sheet_id)
                    && let Some(node) = self.vehicle.as_mut()
                {
                    let kind = if vehicle_state.is_moving() {
                        "walk"
                    } else {
                        "idle"
                    };
                    let name = sequence_name(kind, vehicle_state.facing());
                    if name != self.vehicle_sequence {
                        self.vehicle_sequence = name.clone();
                        self.vehicle_seq_start = self.anim_tick;
                    }
                    let frame = view.frame_at(&name, self.anim_tick - self.vehicle_seq_start);
                    view.apply(node, frame);
                    node.set_position(
                        view.draw_pos(vehicle_state.cell(), vehicle_state.render_offset_16ths()),
                    );
                }
            }
        }

        // Followers: members()[1..] walk the leader's vacated cells. Which
        // character occupies which slot comes from game-start state; until it
        // lands, slot index selects the party sheet directly (no followers
        // exist yet, so this is forward wiring, not a guess shipped).
        struct FollowerDraw {
            sheet_id: String,
            kind: &'static str,
            facing: Direction,
            cell: Cell,
            offset: (i32, i32),
        }
        let mut fdraws: Vec<Option<FollowerDraw>> = Vec::new();
        {
            let members = runtime.members();
            for (slot, member) in members.iter().enumerate().skip(1) {
                // Sheet by the CHARACTER in the slot, not the slot number.
                let char_id = runtime
                    .game()
                    .party_slot(slot)
                    .map(|c| c.0 as usize)
                    .unwrap_or(slot);
                let sheet_id = runtime.data().party_sheet(char_id).map(|s| s.id.clone());
                // A scene that drives this member displaces the caterpillar
                // draw, same as the leader above.
                let scripted = runtime.scene_party_actor(slot).copied();
                fdraws.push(sheet_id.map(|sheet_id| match scripted {
                    Some(actor) => FollowerDraw {
                        sheet_id,
                        kind: if actor.is_walking() { "walk" } else { "idle" },
                        facing: actor.facing,
                        cell: actor.cell,
                        offset: actor.render_offset_16ths(step_frames),
                    },
                    // The caterpillar is lockstep: followers walk exactly when
                    // the leader walks, and the leader's `walking` flag also
                    // bridges the one-tick gap between chained steps that made
                    // followers glide like the dead.
                    None => FollowerDraw {
                        sheet_id,
                        kind: if walking { "walk" } else { "idle" },
                        facing: member.facing,
                        cell: member.cell,
                        offset: member.render_offset_16ths,
                    },
                }));
            }
            let missing: Vec<String> = fdraws
                .iter()
                .flatten()
                .filter(|d| !self.sheet_views.contains_key(&d.sheet_id))
                .map(|d| d.sheet_id.clone())
                .collect();
            for sheet_id in missing {
                if let Some(sheet) = runtime.data().sheet(&sheet_id)
                    && let Some(view) = SheetView::build(&self.pack_dir, sheet, Some(&self.cram))
                {
                    self.sheet_views.insert(sheet_id, view);
                }
            }
        }
        while self.follower_nodes.len() < fdraws.len() {
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(5);
            self.base_mut().add_child(&node);
            self.follower_nodes.push((node, String::new(), 0));
            // Equal feet positions still need a stable sprite priority.
            // Followers start overlapped on save/load; the party leader must
            // be in front instead of being covered by the last added member.
            if let Some(party) = self.party.clone() {
                self.base_mut().move_child(&party, -1);
            }
        }
        for (idx, draw) in fdraws.iter().enumerate() {
            let Some(draw) = draw else { continue };
            let Some(view) = self.sheet_views.get(&draw.sheet_id) else {
                continue;
            };
            let (node, seq, start) = &mut self.follower_nodes[idx];
            let name = sequence_name(draw.kind, draw.facing);
            if *seq != name || frozen.get(idx + 1).copied().unwrap_or(false) {
                *seq = name;
                *start = self.anim_tick;
            }
            let frame = view.frame_at(seq, self.anim_tick - *start);
            view.apply(node, frame);
            node.set_position(view.draw_pos(draw.cell, draw.offset));
        }

        // Static NPCs replay their idle sequence in place; objects a field
        // routine moves (wanderers, bespoke routines, a scene's
        // RunObjectRoutine) are placed and animated from the engine — cell,
        // facing and step offset all live there, so the picture can't desync
        // from the collision.
        let motions: Vec<psiv_runtime::ObjectMotion> = self
            .runtime()
            .map(psiv_runtime::Runtime::object_motions)
            .unwrap_or_default();
        let npc_offsets: Vec<(i32, i32)> = self
            .runtime()
            .map(|rt| {
                rt.map()
                    .npcs()
                    .iter()
                    .map(|npc| (i32::from(npc.offset.x), i32::from(npc.offset.y)))
                    .collect()
            })
            .unwrap_or_default();
        for entry in &mut self.npc_nodes {
            let Some(view) = self.sheet_views.get(&entry.sheet) else {
                continue;
            };
            if let Some((_, (_, age))) = beams.iter().find(|(index, _)| *index == entry.index) {
                // Init writes duration 4; Animate2 decrements before testing,
                // so call 5 advances to mapping 1 (`ps4.asm:88830-88857`).
                let frame = view.frame_at("walk_down", *age);
                view.apply(&mut entry.node, frame);
                continue;
            }
            let motion = motions.iter().find(|m| m.npc_index == entry.index);
            match motion {
                Some(w) => {
                    let kind = if w.stepping { "walk" } else { "idle" };
                    let name = sequence_name(kind, w.facing);
                    let npc_offset = npc_offsets.get(w.npc_index).copied().unwrap_or((0, 0));
                    // Not every sheet animates every way; frame 0 is the
                    // sheet's own fallback, same as the cartridge's art.
                    let frame = camp_receipt_frame(entry.index, &entry.sheet)
                        .unwrap_or_else(|| view.frame_at(&name, self.anim_tick));
                    view.apply(&mut entry.node, frame);
                    let (x, y) =
                        super::view::motion_pixel_position(w.cell, npc_offset, w.offset_16ths);
                    entry.node.set_position(Vector2::new(
                        (x - view.origin_x) as f32,
                        (y - view.origin_y + view.frame_height) as f32,
                    ));
                }
                None => {
                    let npc = self.session.as_ref().and_then(|session| {
                        let rt = session.runtime();
                        rt.map().chest_at_slot(entry.index)?;
                        rt.map().npcs().get(entry.index)
                    });
                    if let Some(npc) = npc {
                        entry.idle = sequence_name("idle", npc.facing);
                    }
                    let frame = camp_receipt_frame(entry.index, &entry.sheet)
                        .unwrap_or_else(|| view.frame_at(&entry.idle, self.anim_tick));
                    view.apply(&mut entry.node, frame);
                }
            }
        }

        // Scene actors: scripted positions override the static NPC layout,
        // with live step interpolation so scripted walks read as walks.
        for actor in &scene_actors {
            let psiv_core::ActorRef::Npc(index) = actor.actor else {
                continue;
            };
            // A scene cast includes the map's stationary NPCs too. Their
            // generic idle pose must not overwrite the beam routine's mapping.
            if beams.iter().any(|(beam, _)| *beam == index) {
                continue;
            }
            for entry in &mut self.npc_nodes {
                if entry.index == index
                    && let Some(view) = self.sheet_views.get(&entry.sheet)
                {
                    let kind = if actor.is_walking() { "walk" } else { "idle" };
                    entry.idle = sequence_name(kind, actor.facing);
                    let frame = match actor.animation {
                        Some(animation) => view
                            .sequences
                            .get(&format!("walk_facing_0x{:02X}", animation.word))
                            .and_then(|(frames, _)| frames.get(usize::from(animation.index)))
                            .map_or(0, |(frame, _)| *frame),
                        None => view.frame_at(&entry.idle, self.anim_tick),
                    };
                    view.apply(&mut entry.node, frame);
                    entry.node.set_position(
                        view.draw_pos(actor.cell, actor.render_offset_16ths(step_frames)),
                    );
                }
            }
        }

        // Temporary objects are keyed by the scene's literal slot, not by a
        // map sprite.  The pack carries their decoded Nemesis/field-art sheet;
        // map sprites remain a fallback only for an unrecognised asset.
        let mut custom_slots = HashSet::new();
        for (slot, object) in self.presentation.temporary_draws() {
            let asset = self.cutscene_layer.as_ref().and_then(|layer| {
                layer
                    .bind()
                    .temporary_asset(object.object_id, object.art_tile)
            });
            let Some(asset) = asset else {
                let Some(entry) = self.npc_nodes.iter_mut().find(|entry| entry.index == slot)
                else {
                    continue;
                };
                let Some(view) = self.sheet_views.get(&entry.sheet) else {
                    continue;
                };
                let frame = view.frame_at("idle_down", self.anim_tick);
                view.apply(&mut entry.node, frame);
                if let Some((x, y)) = object.destination {
                    entry.node.set_position(Vector2::new(
                        (x - view.origin_x) as f32,
                        (y - view.origin_y + view.frame_height) as f32,
                    ));
                }
                continue;
            };
            if !asset.art_loaded(&self.presentation) || object.frames_left == 0 {
                continue;
            }
            let Some(position) = temporary_position(self, slot, object.destination, &scene_actors)
            else {
                continue;
            };
            if !self.temporary_nodes.contains_key(&slot) {
                let mut node = Sprite2D::new_alloc();
                node.set_centered(false);
                node.set_z_index(5);
                self.base_mut().add_child(&node);
                self.temporary_nodes.insert(slot, node);
            }
            if let Some(node) = self.temporary_nodes.get_mut(&slot) {
                // Moving platforms (`$124`, `#99`) ride under the party and
                // followers (z-index 5) and above the map/belts (0..=1).
                node.set_z_index(if object.object_id == 0x0124 { 4 } else { 5 });
                let facing_seq = object
                    .facing_word
                    .map(|w| format!("walk_facing_0x{w:02X}"))
                    .filter(|name| asset.has_sequence(name));
                let walk_seq = object
                    .walk_dir
                    .map(|dir| {
                        let kind = if object.is_walking() { "walk" } else { "idle" };
                        sequence_name(kind, dir)
                    })
                    .filter(|name| asset.has_sequence(name));
                let sequence = facing_seq
                    .as_deref()
                    .or(walk_seq.as_deref())
                    .or(asset.playback_sequence.as_deref())
                    .unwrap_or("idle_down");
                let age = if asset.playback_sequence.is_some()
                    && facing_seq.is_none()
                    && walk_seq.is_none()
                {
                    object.elapsed
                } else {
                    self.anim_tick
                };
                let frame = asset.frame_at(sequence, age);
                asset.apply(node, frame, &self.cram.borrow());
                node.set_position(Vector2::new(
                    (position.0 - asset.origin_x) as f32,
                    (position.1 - asset.origin_y + asset.frame_height) as f32,
                ));
                node.set_visible(scene_sprites_visible);
                custom_slots.insert(slot);
            }
        }

        self.sync_rewritten_npcs(scene_sprites_visible, &mut custom_slots);
        custom_slots.extend(self.sync_eclipse_torch(&torch_objects));

        if let Some(camera) = self.camera.as_mut() {
            let (x, y) = camera_position;
            let center = Vector2::new(
                (x + SCREEN_WIDTH / 2) as f32,
                (y + SCREEN_HEIGHT / 2) as f32,
            );
            camera.set_position(center);
        }
        if let Some(party) = self.party.as_mut() {
            party.set_visible(
                scene_sprites_visible
                    && vehicle_state.is_none()
                    && party_visible.first().copied().unwrap_or(true),
            );
        }
        for (index, follower) in self.follower_nodes.iter_mut().enumerate() {
            follower.0.set_visible(
                scene_sprites_visible
                    && vehicle_state.is_none()
                    && fdraws.get(index).is_some_and(Option::is_some)
                    && party_visible.get(index + 1).copied().unwrap_or(true),
            );
        }
        if let Some(vehicle) = self.vehicle.as_mut() {
            vehicle.set_visible(scene_sprites_visible && vehicle_state.is_some());
        }
        for npc in &mut self.npc_nodes {
            let active = active_npcs.get(npc.index).copied().unwrap_or(false);
            npc.node.set_visible(
                scene_sprites_visible
                    && active
                    && !custom_slots.contains(&npc.index)
                    && beams
                        .iter()
                        .find(|(index, _)| *index == npc.index)
                        .is_none_or(|(_, (visible, _))| *visible),
            );
        }
        for (slot, node) in &mut self.temporary_nodes {
            node.set_visible(scene_sprites_visible && custom_slots.contains(slot));
        }
        self.place_letterbox();
        self.sync_transition();
    }

    /// Copies the runtime's CRAM model into the field's live palette
    /// (`cram_art`), which sprite sheets and scene panels draw from, and
    /// uploads lines 0 and 1 to the indexed map shader. A legacy record
    /// without the 64-word shadow is not live ([`psiv_runtime::CramView`]),
    /// and every surface keeps its baked colours.
    fn sync_map_palette(&mut self) {
        let Some(runtime) = self.session.as_ref().map(Session::runtime) else {
            return;
        };
        let view = runtime.cram();
        let (live, words, revision) = (view.live, view.words, view.revision);
        let changed = {
            let mut cram = self.cram.borrow_mut();
            let changed = cram.live != live || cram.revision != revision;
            cram.live = live;
            cram.words = words;
            cram.revision = revision;
            changed
        };
        if changed {
            if let Some(layer) = self.cutscene_layer.as_mut() {
                layer.bind_mut().palette_changed();
            }
            // The VDP's backdrop is CRAM word 0 (register 7 `$8700`: palette
            // 0, colour 0, `ps4.asm:86053`): it shows wherever no plane or
            // sprite pixel does, so a tone or a fade reaches it too. With no
            // live palette it stays the project's black.
            let [r, g, b] = if live {
                psiv_data::color::cram_rgb(words[0])
            } else {
                [0, 0, 0]
            };
            godot::classes::RenderingServer::singleton()
                .set_default_clear_color(Color::from_rgba8(r, g, b, 255));
            self.apply_window_line();
        }
        if !live {
            return;
        }
        let palette = super::field_map::map_palette_colors(&words, revision);
        for sprite in [&mut self.map_sprite, &mut self.overlay_sprite]
            .into_iter()
            .flatten()
        {
            super::field_map::upload_map_palette(sprite, &palette);
        }
        if let Some(material) = self.belt_tile_material.as_mut() {
            super::field_map::upload_map_palette_material(material, &palette);
        }
    }

    /// Gives every native window node ([`WindowFamily::nodes`]) the
    /// material of the CRAM line its art is drawn on, one per line. Run once
    /// the window nodes exist.
    pub(super) fn attach_window_line(&mut self) {
        for family in WindowFamily::ALL {
            for &node in family.nodes() {
                let material = self.window_line.materials[node.line()]
                    .get_or_insert_with(crate::cram_art::window_line_material)
                    .clone();
                match node {
                    WindowNode::Dialogue => {
                        if let Some(dialogue) = self.dialogue.as_mut() {
                            dialogue.set_material(&material);
                        }
                    }
                    WindowNode::Camp => {
                        if let Some(camp) = self.camp_menu.as_mut() {
                            camp.set_material(&material);
                        }
                    }
                    WindowNode::Shop => {
                        if let Some(shop) = self.shop.as_mut() {
                            shop.set_material(&material);
                        }
                    }
                    WindowNode::JobBoard | WindowNode::JobDone | WindowNode::JobClosed => {
                        if let Some(board) = self.job_board.as_mut() {
                            board.bind_mut().layer(node).set_material(&material);
                        }
                    }
                    WindowNode::Destination => {
                        if let Some(screen) = self.destination.as_mut() {
                            screen.bind_mut().windows().set_material(&material);
                        }
                    }
                }
            }
        }
        self.apply_window_line();
    }

    /// Points each line's window material at that line of the shared CRAM:
    /// the art's `Pal_Init_Line_3` words and the live ones, or, while no
    /// live palette is known, the baked words on line 2 and the guild's
    /// colours on lines 1 and 3 ([`WindowLine::new`], [`unlit_line`]).
    pub(super) fn apply_window_line(&mut self) {
        let Some(runtime) = self.session.as_ref().map(Session::runtime) else {
            return;
        };
        let data = runtime.data();
        let baked = data.scene_palettes().words(WINDOW_LINE_ROM).unwrap_or(&[]);
        let cram = self.cram.borrow();
        for (line, material) in self.window_line.materials.iter_mut().enumerate() {
            let Some(material) = material.as_mut() else {
                continue;
            };
            let unlit = unlit_line(line, data.guild());
            let Some(drawn) = WindowLine::new(line, &cram.words, cram.live, baked, unlit) else {
                continue;
            };
            let drawn =
                self.window_line.debug_live[line].map_or(drawn, |live| drawn.with_live(live));
            crate::cram_art::set_window_line(material, &drawn);
        }
    }

    /// `debug_window_line`'s body: `words` empty clears `line`'s override.
    fn window_line_probe(&mut self, line: i32, words: &[i32]) -> bool {
        if std::env::var("PSIV_DEBUG_ROUTE").as_deref() != Ok("1") {
            return false;
        }
        let Some(line) = usize::try_from(line).ok().filter(|&line| line < 4) else {
            godot_error!("debug_window_line: {line} is not a CRAM line");
            return false;
        };
        if self.window_line.materials[line].is_none() {
            godot_error!("debug_window_line: no window draws on line {line}");
            return false;
        }
        self.window_line.debug_live[line] = if words.is_empty() {
            None
        } else {
            let Ok(live) = <[i32; 16]>::try_from(words) else {
                godot_error!("debug_window_line takes sixteen words, not {}", words.len());
                return false;
            };
            let mut drawn = [0u16; 16];
            for (word, &value) in drawn.iter_mut().zip(&live) {
                let Ok(value) = u16::try_from(value) else {
                    godot_error!("debug_window_line: {value} is not a CRAM word");
                    return false;
                };
                *word = value;
            }
            Some(drawn)
        };
        self.apply_window_line();
        true
    }
}

fn rt_party_visibility(
    runtime: &psiv_runtime::Runtime,
    presentation: &super::PresentationState,
) -> Vec<bool> {
    runtime
        .game()
        .party_members()
        .iter()
        .map(|id| presentation.character_visible(id.0))
        .collect()
}

fn temporary_position(
    field: &Field,
    slot: usize,
    destination: Option<(i32, i32)>,
    scene_actors: &[psiv_core::ScriptedActor],
) -> Option<(i32, i32)> {
    if let Some(position) = destination {
        return Some(position);
    }
    if let Some(actor) = scene_actors
        .iter()
        .find(|a| matches!(a.actor, psiv_core::ActorRef::Npc(index) if index == slot))
    {
        let cell = actor.cell;
        return Some((i32::from(cell.x) * 16, (i32::from(cell.y) - 1) * 16));
    }
    field
        .npc_nodes
        .iter()
        .find(|entry| entry.index == slot)
        .map(|entry| entry.base)
}

impl Field {
    /// Starts one retail-timed transition. The caller owns the semantic
    /// trigger; this layer only owns the cover and its frame cadence.
    pub(super) fn start_transition(&mut self, kind: TransitionKind) {
        self.transition = Some(Transition::new(kind));
        self.ensure_transition_nodes();
        self.sync_transition();
    }

    /// A transition whose length the runtime measured (a teleport's load).
    pub(super) fn start_transition_frames(&mut self, kind: TransitionKind, frames: u16) {
        self.transition = Some(Transition::with_frames(kind, frames));
        self.ensure_transition_nodes();
        self.sync_transition();
    }

    /// Whether the runtime's CRAM is the cartridge's, so its fades draw
    /// themselves ([`psiv_runtime::CramView::live`]).
    pub(super) fn cram_live(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.runtime().cram().live)
    }

    /// A cutscene (event `$8000` and up) starts. `FieldRoutine_Cutscene`
    /// fades nothing itself (`ps4.asm:120738-120758`): the scene's own
    /// `InitVRAMAndCRAM` does, on the CRAM when it is live (#637). Off a
    /// palette that is not live the shell's cover stands in for it.
    pub(super) fn start_cutscene_cover(&mut self) {
        if !self.cram_live() {
            self.start_transition(TransitionKind::SceneStart);
        }
    }

    /// A scene's `Pal_FadeIn` on a live CRAM draws its own reveal from black,
    /// so whatever cover is up ends now, as the shell's own fade-in used to
    /// replace it.
    pub(super) fn palette_fade_in_ends_cover(&mut self) {
        self.transition = None;
        self.sync_transition();
    }

    /// A runtime-owned palette fade-in starts: a cover it replaces ends now
    /// ([`super::transitions::after_palette_fade_in`]).
    pub(super) fn palette_fade_in_takes_over(&mut self) {
        self.transition = super::transitions::after_palette_fade_in(self.transition);
        self.sync_transition();
    }

    /// Hands the window this frame's runtime view. The runtime owns the
    /// window's state; the node owns its pixels.
    pub(super) fn sync_dialogue_view(&mut self) {
        // The field's visibility mask still applies when its window keeps a
        // page. Syncing that page must not reopen a node the battle hid.
        let view = if self.battle_field_visibility.is_some() {
            None
        } else {
            self.runtime().and_then(|rt| rt.dialogue_view())
        };
        if let Some(window) = self.dialogue.as_mut() {
            window.bind_mut().set_view(view);
        }
    }

    /// Draw the runtime's loaded stage, or its band and uniform hold before setup.
    /// A loaded stage replaces the cover and retains its own stepped palette.
    pub(super) fn sync_battle_entry_screen(&mut self) {
        if let Some(view) = self
            .runtime()
            .and_then(psiv_runtime::Runtime::battle_load_view)
        {
            self.present_battle_load(view);
            self.transition = None;
            self.sync_transition();
            return;
        }
        let screen = self
            .runtime()
            .and_then(psiv_runtime::Runtime::battle_entry_screen);
        if let Some(word) = screen {
            self.start_transition(TransitionKind::Screen(word));
        } else if let Some(band) = self
            .runtime()
            .and_then(psiv_runtime::Runtime::event_battle_entry_band)
        {
            self.start_transition(TransitionKind::Band(band));
        }
    }

    /// Advances transitions even while the battle presentation owns the
    /// normal runtime tick.
    pub(super) fn tick_transition(&mut self) {
        let finished = self
            .transition
            .as_mut()
            .is_some_and(|transition| transition.tick());
        if finished {
            self.transition = None;
        }
        self.sync_transition();
    }

    /// Cinema mode: letterbox bars over the world, under the dialogue box.
    pub(super) fn set_letterbox(&mut self, on: bool) {
        if on && self.letterbox.is_empty() {
            for _ in 0..LETTERBOX_BARS {
                let mut bar = godot::classes::ColorRect::new_alloc();
                bar.set_color(Color::from_rgb(0.0, 0.0, 0.0));
                bar.set_z_index(500);
                self.base_mut().add_child(&bar);
                self.letterbox.push(bar);
            }
        }
        for bar in &mut self.letterbox {
            bar.set_visible(on);
        }
    }

    /// Keeps the bars glued to the camera view and masks everything outside
    /// the authentic 320x224 frame. At the existing 3x camera zoom this is a
    /// lossless 960x672 presentation inside the 1280x800 wide viewport.
    pub(super) fn place_letterbox(&mut self) {
        if self.letterbox.is_empty() || !self.letterbox[0].is_visible() {
            return;
        }
        let Some(camera) = self.camera.as_ref() else {
            return;
        };
        let viewport = self.base().get_viewport_rect().size;
        let zoom = camera.get_zoom().x.max(0.01);
        let view = viewport / zoom;
        let center = camera.get_position();
        let top_left = center - view / 2.0;
        let frame_top_left =
            center - Vector2::new(BATTLE_FRAME_WIDTH / 2.0, BATTLE_FRAME_HEIGHT / 2.0);
        let frame_bottom_right =
            center + Vector2::new(BATTLE_FRAME_WIDTH / 2.0, BATTLE_FRAME_HEIGHT / 2.0);
        // The camera centre is fractional, so a bar that starts exactly at
        // the view edge can rasterize a sub-pixel short and leak a one-pixel
        // strip of world along the screen border. Overdraw every outward
        // edge by a couple of world pixels; the frame-facing edges stay
        // exact.
        const BLEED: f32 = 2.0;
        let sizes = [
            (
                top_left - Vector2::new(BLEED, BLEED),
                Vector2::new(
                    view.x + 2.0 * BLEED,
                    (frame_top_left.y - top_left.y).max(0.0) + BLEED,
                ),
            ),
            (
                Vector2::new(top_left.x - BLEED, frame_bottom_right.y),
                Vector2::new(
                    view.x + 2.0 * BLEED,
                    (top_left.y + view.y - frame_bottom_right.y).max(0.0) + BLEED,
                ),
            ),
            (
                Vector2::new(top_left.x - BLEED, frame_top_left.y),
                Vector2::new(
                    (frame_top_left.x - top_left.x).max(0.0) + BLEED,
                    BATTLE_FRAME_HEIGHT,
                ),
            ),
            (
                Vector2::new(frame_bottom_right.x, frame_top_left.y),
                Vector2::new(
                    (top_left.x + view.x - frame_bottom_right.x).max(0.0) + BLEED,
                    BATTLE_FRAME_HEIGHT,
                ),
            ),
        ];
        for (bar, (pos, size)) in self.letterbox.iter_mut().zip(sizes) {
            bar.set_position(pos);
            bar.set_size(size);
        }
    }

    /// `InitVramAndCram` blanks the tile planes on hardware; the shell
    /// mirrors that by hiding the map layers until a scene map redraw
    /// (`load_map_visuals`) restores them. The overlay's restore stays with
    /// `load_map_visuals`, which knows whether the map has one.
    pub(super) fn set_field_map_visible(&mut self, visible: bool) {
        if let Some(sprite) = self.map_sprite.as_mut() {
            sprite.set_visible(visible);
        }
        if !visible && let Some(sprite) = self.overlay_sprite.as_mut() {
            sprite.set_visible(false);
        }
    }

    fn ensure_transition_nodes(&mut self) {
        if !self.transition_nodes.is_empty() {
            return;
        }
        for _ in 0..TRANSITION_RECTS {
            let mut node = ColorRect::new_alloc();
            node.set_visible(false);
            self.base_mut().add_child(&node);
            self.transition_nodes.push(node);
        }
    }

    fn sync_transition(&mut self) {
        let Some(transition) = self.transition else {
            for node in &mut self.transition_nodes {
                node.set_visible(false);
            }
            return;
        };
        let Some(visual) = transition.visual() else {
            for node in &mut self.transition_nodes {
                node.set_visible(false);
            }
            return;
        };
        let Some((top_left, view)) = self.transition_view() else {
            return;
        };
        let alpha = f32::from(visual.level) / 7.0;
        let color = match visual.color {
            super::transitions::TransitionColor::Black => Color::from_rgba(0.0, 0.0, 0.0, alpha),
            super::transitions::TransitionColor::Cram(word) => {
                let [r, g, b] = psiv_data::color::cram_rgb(word);
                Color::from_rgba8(r, g, b, 255)
            }
        };
        let z = transition.z_index();
        for node in &mut self.transition_nodes {
            node.set_color(color);
            node.set_z_index(z);
            node.set_visible(false);
        }
        if alpha <= 0.0 {
            return;
        }

        if let Some(band) = transition.band() {
            let node = &mut self.transition_nodes[0];
            node.set_position(
                top_left
                    + (view - super::retail_surface::SURFACE) / 2.0
                    + Vector2::new(0.0, f32::from(band.top)),
            );
            node.set_size(Vector2::new(
                super::retail_surface::SURFACE.x,
                f32::from(band.bottom - band.top),
            ));
            node.set_visible(true);
            return;
        }

        match visual.opening {
            None => {
                let node = &mut self.transition_nodes[0];
                node.set_position(top_left);
                node.set_size(view);
                node.set_visible(true);
            }
            Some(opening) => self.place_wipe(top_left, view, opening),
        }
    }

    fn transition_view(&self) -> Option<(Vector2, Vector2)> {
        let camera = self.camera.as_ref()?;
        let viewport = self.base().get_viewport_rect().size;
        let zoom = camera.get_zoom().x.max(0.01);
        let view = viewport / zoom;
        Some((camera.get_position() - view / 2.0, view))
    }

    fn place_wipe(&mut self, top_left: Vector2, view: Vector2, opening: f32) {
        let opening = opening.clamp(0.0, 1.0);
        if opening >= 1.0 {
            return;
        }
        if opening <= 0.0 {
            let node = &mut self.transition_nodes[0];
            node.set_position(top_left);
            node.set_size(view);
            node.set_visible(true);
            return;
        }

        let window = view * opening;
        let window_pos = top_left + (view - window) / 2.0;
        let bottom = window_pos.y + window.y;
        let right = window_pos.x + window.x;
        let sizes = [
            (
                top_left,
                Vector2::new(view.x, (window_pos.y - top_left.y).max(0.0)),
            ),
            (
                Vector2::new(top_left.x, bottom),
                Vector2::new(view.x, (top_left.y + view.y - bottom).max(0.0)),
            ),
            (
                Vector2::new(top_left.x, window_pos.y),
                Vector2::new((window_pos.x - top_left.x).max(0.0), window.y),
            ),
            (
                Vector2::new(right, window_pos.y),
                Vector2::new((top_left.x + view.x - right).max(0.0), window.y),
            ),
        ];
        for (node, (position, size)) in self.transition_nodes.iter_mut().zip(sizes) {
            node.set_position(position);
            node.set_size(size);
            node.set_visible(true);
        }
    }
}
