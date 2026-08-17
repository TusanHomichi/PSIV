//! Per-frame field drawing, split from the Godot bridge so the bridge stays
//! below the repository's one-thousand-line maintenance limit.

use godot::classes::{ColorRect, Sprite2D};
use godot::prelude::*;

use psiv_core::{Cell, Direction, SCREEN_HEIGHT, SCREEN_WIDTH};

use super::Field;
use super::transitions::{Transition, TransitionKind};
use super::view::{SheetView, sequence_name};

impl Field {
    /// Places and animates the party sprite, animates NPCs, moves the camera.
    pub(super) fn sync_visuals(&mut self, walking: bool) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let state = runtime.state();
        let cell = state.cell();
        let offset = state.render_offset_16ths();
        let scene_actors = runtime.scene_actors();
        let scene_sprites_visible = self.presentation.sprites_visible(runtime.scene_active());
        let active_npcs: Vec<bool> = runtime.map().npcs().iter().map(|npc| npc.active).collect();
        let camera_position = runtime.camera().position();
        let kind = if walking { "walk" } else { "idle" };
        let sequence = sequence_name(kind, state.facing());
        if sequence != self.party_sequence {
            self.party_sequence = sequence;
            self.party_seq_start = self.anim_tick;
        }

        if let (Some(party), Some(view)) = (self.party.as_mut(), self.party_view.as_ref()) {
            let frame = view.frame_at(&self.party_sequence, self.anim_tick - self.party_seq_start);
            view.apply(party, frame);
            party.set_position(view.draw_pos(cell, offset));
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
                fdraws.push(sheet_id.map(|sheet_id| FollowerDraw {
                    sheet_id,
                    // The caterpillar is lockstep: followers walk exactly when
                    // the leader walks, and the leader's `walking` flag also
                    // bridges the one-tick gap between chained steps that made
                    // followers glide like the dead.
                    kind: if walking { "walk" } else { "idle" },
                    facing: member.facing,
                    cell: member.cell,
                    offset: member.render_offset_16ths,
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
                    && let Some(view) = SheetView::build(&self.pack_dir, sheet)
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
        }
        for (idx, draw) in fdraws.iter().enumerate() {
            let Some(draw) = draw else { continue };
            let Some(view) = self.sheet_views.get(&draw.sheet_id) else {
                continue;
            };
            let (node, seq, start) = &mut self.follower_nodes[idx];
            let name = sequence_name(draw.kind, draw.facing);
            if *seq != name {
                *seq = name;
                *start = self.anim_tick;
            }
            let frame = view.frame_at(seq, self.anim_tick - *start);
            view.apply(node, frame);
            node.set_position(view.draw_pos(draw.cell, draw.offset));
        }

        // Static NPCs replay their idle sequence in place; wanderers are
        // placed and animated from the engine — cell, facing and step offset
        // all live there, so the picture can't desync from the collision.
        struct WanderView {
            index: usize,
            facing: Direction,
            stepping: bool,
            offset: (i32, i32),
            npc_offset: (i32, i32),
            cell: (u16, u16),
        }
        let wander_states: Vec<WanderView> = self
            .runtime
            .as_ref()
            .map(|rt| {
                rt.wanderers()
                    .iter()
                    .filter_map(|w| {
                        let npc = rt.map().npcs().get(w.npc_index())?;
                        Some(WanderView {
                            index: w.npc_index(),
                            facing: npc.facing,
                            stepping: w.is_stepping(),
                            offset: w.render_offset_16ths(),
                            npc_offset: (i32::from(npc.offset.x), i32::from(npc.offset.y)),
                            cell: (npc.cell.x, npc.cell.y),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        for entry in &mut self.npc_nodes {
            let Some(view) = self.sheet_views.get(&entry.sheet) else {
                continue;
            };
            let wander = wander_states.iter().find(|w| w.index == entry.index);
            match wander {
                Some(w) => {
                    let kind = if w.stepping { "walk" } else { "idle" };
                    let name = sequence_name(kind, w.facing);
                    // Not every sheet animates every way; frame 0 is the
                    // sheet's own fallback, same as the cartridge's art.
                    let frame = view.frame_at(&name, self.anim_tick);
                    view.apply(&mut entry.node, frame);
                    let x = entry.base.0
                        + (i32::from(w.cell.0) - entry.spawn.0) * 16
                        + w.npc_offset.0
                        + w.offset.0;
                    let y = entry.base.1
                        + (i32::from(w.cell.1) - entry.spawn.1) * 16
                        + w.npc_offset.1
                        + w.offset.1;
                    entry.node.set_position(Vector2::new(
                        (x - view.origin_x) as f32,
                        (y - view.origin_y + view.frame_height) as f32,
                    ));
                }
                None => {
                    let frame = view.frame_at(&entry.idle, self.anim_tick);
                    view.apply(&mut entry.node, frame);
                }
            }
        }

        // Scene actors: scripted positions override the static NPC layout.
        if !scene_actors.is_empty() {
            let mut moves: Vec<(usize, Cell, Direction)> = Vec::new();
            for (actor, acell, afacing) in &scene_actors {
                if let psiv_core::ActorRef::Npc(i) = actor {
                    moves.push((*i, *acell, *afacing));
                }
            }
            for (index, acell, afacing) in moves {
                for entry in &mut self.npc_nodes {
                    if entry.index == index
                        && let Some(view) = self.sheet_views.get(&entry.sheet)
                    {
                        let name = sequence_name("idle", afacing);
                        entry.idle = name;
                        let frame = view.frame_at(&entry.idle, self.anim_tick);
                        view.apply(&mut entry.node, frame);
                        entry.node.set_position(view.draw_pos(acell, (0, 0)));
                    }
                }
            }
        }

        // Temporary `$C340`/`$C4C0` objects retain their literal scene slot
        // and animation fields. When the slot corresponds to an existing
        // map sprite, reuse that decoded retail sheet rather than fabricating
        // art; a future asset decode can add a standalone node without
        // changing the scene event contract.
        for (slot, object) in self.presentation.temporary_draws() {
            let Some(entry) = self.npc_nodes.iter_mut().find(|entry| entry.index == slot) else {
                continue;
            };
            let Some(view) = self.sheet_views.get(&entry.sheet) else {
                continue;
            };
            let _retail_fields = (object.object_id, object.art_tile);
            let frame = view.frame_at("idle_down", self.anim_tick);
            view.apply(&mut entry.node, frame);
            if let Some((x, y)) = object.destination {
                entry.node.set_position(Vector2::new(
                    (x - view.origin_x) as f32,
                    (y - view.origin_y + view.frame_height) as f32,
                ));
            }
        }

        if let Some(camera) = self.camera.as_mut() {
            let (x, y) = camera_position;
            let center = Vector2::new(
                (x + SCREEN_WIDTH / 2) as f32,
                (y + SCREEN_HEIGHT / 2) as f32,
            );
            camera.set_position(center);
        }
        if let Some(party) = self.party.as_mut() {
            party.set_visible(scene_sprites_visible);
        }
        for follower in &mut self.follower_nodes {
            follower.0.set_visible(scene_sprites_visible);
        }
        for npc in &mut self.npc_nodes {
            let active = active_npcs.get(npc.index).copied().unwrap_or(false);
            npc.node.set_visible(scene_sprites_visible && active);
        }
        self.place_letterbox();
        self.sync_transition();
    }

    /// Starts one retail-timed transition. The caller owns the semantic
    /// trigger; this layer only owns the cover and its frame cadence.
    pub(super) fn start_transition(&mut self, kind: TransitionKind) {
        self.transition = Some(Transition::new(kind));
        self.ensure_transition_nodes();
        self.sync_transition();
    }

    /// Advances transitions even while the battle presentation owns the
    /// normal runtime tick.
    pub(super) fn tick_transition(&mut self) {
        let finished_kind = self
            .transition
            .as_mut()
            .and_then(|transition| transition.tick().then_some(transition.kind()));
        if let Some(kind) = finished_kind {
            self.transition = None;
            if kind == TransitionKind::SceneEnd {
                self.set_letterbox(false);
            }
        }
        self.sync_transition();
    }

    fn ensure_transition_nodes(&mut self) {
        if !self.transition_nodes.is_empty() {
            return;
        }
        for _ in 0..4 {
            let mut node = ColorRect::new_alloc();
            node.set_visible(false);
            node.set_z_index(600);
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
            super::transitions::TransitionColor::White => Color::from_rgba(1.0, 1.0, 1.0, alpha),
        };
        let z = if transition.front_layer() { 600 } else { 25 };
        for node in &mut self.transition_nodes {
            node.set_color(color);
            node.set_z_index(z);
            node.set_visible(false);
        }
        if alpha <= 0.0 {
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
