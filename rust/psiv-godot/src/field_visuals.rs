//! Per-frame field drawing, split from the Godot bridge so the bridge stays
//! below the repository's one-thousand-line maintenance limit.

use godot::classes::Sprite2D;
use godot::prelude::*;

use psiv_core::{Cell, Direction};

use super::view::{SheetView, sequence_name};
use super::{CELL_PIXELS, Field};

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
                    let x = entry.base.0 + (i32::from(w.cell.0) - entry.spawn.0) * 16 + w.offset.0;
                    let y = entry.base.1 + (i32::from(w.cell.1) - entry.spawn.1) * 16 + w.offset.1;
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

        if let Some(camera) = self.camera.as_mut() {
            let center = Vector2::new(
                f32::from(cell.x) * CELL_PIXELS + offset.0 as f32 + CELL_PIXELS / 2.0,
                f32::from(cell.y) * CELL_PIXELS + offset.1 as f32 + CELL_PIXELS / 2.0,
            );
            camera.set_position(center);
        }
        self.place_letterbox();
    }
}
