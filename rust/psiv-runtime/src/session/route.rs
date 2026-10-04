//! The events that open a window, applied where the cartridge applied them:
//! between the field node and the window node of the same frame.
//!
//! A talk is the cartridge's `FieldRoutine_Interaction`: the object the party
//! faces, the shop table checked when the hit came across a counter
//! (`Shop_FindCounter`, `0x065D12`; the scan matches the shopkeeper's position
//! and never reads the collision grid), the dialogue tree otherwise. The scene's
//! own requests — a line, a resumed line, a yes/no branch — and an empty hand's
//! "nothing here" open the box the same way.

use psiv_core::InteractReach;
use psiv_data::ShopCounter;

use crate::{RuntimeEvent, SceneDialogueOpen};

use super::shop::ShopView;
use super::{Routed, Session};

impl Session {
    /// Applies the events that open windows and hands back the rest, in order,
    /// with a record of what opened.
    pub(super) fn route(&mut self, events: Vec<RuntimeEvent>) -> (Vec<RuntimeEvent>, Vec<Routed>) {
        let mut kept = Vec::with_capacity(events.len());
        let mut routed = Vec::new();
        for event in events {
            self.sync_flight_view(&event);
            match event {
                RuntimeEvent::Interact {
                    npc_index,
                    cell,
                    reach,
                } => {
                    if matches!(reach, InteractReach::AcrossCounter) {
                        if let Some(counter) = self.counter_at(npc_index, cell) {
                            self.open_counter(&counter);
                            let facing = self.runtime.state().facing().opposite();
                            self.runtime.face_npc(npc_index, facing);
                            routed.push(Routed::ShopOpened {
                                counter: counter.id,
                            });
                            continue;
                        }
                        routed.push(Routed::CounterWithoutShop { cell });
                    }
                    // The runtime resolves the map's tree and the object's
                    // dialogue id, opens the window and turns the object to
                    // face the party (the cartridge's default for a talk).
                    let outcome = self.runtime.open_npc_dialogue(npc_index);
                    routed.push(Routed::Talk {
                        npc_index,
                        cell,
                        outcome,
                    });
                }
                RuntimeEvent::InteractNothing { .. } => {
                    self.runtime.open_nothing_here(0);
                    routed.push(Routed::NothingHere);
                }
                RuntimeEvent::SceneDialogue { entry } => {
                    if self.scene_dialogue_autoclose {
                        self.runtime.close_dialogue();
                        self.runtime.dialogue_closed();
                        routed.push(Routed::SceneDialogueSkipped { entry });
                        continue;
                    }
                    // The runtime resolves the entry against the tree the
                    // scene selected (`SetDialogueTree`, or the map's own
                    // binding); a tree the pack does not have is the one case
                    // that must leave the scene's dialogue barrier pending.
                    let outcome = self.runtime.open_scene_dialogue(entry, self.panel_layout());
                    if outcome == SceneDialogueOpen::Empty {
                        self.runtime.dialogue_closed();
                    }
                    routed.push(Routed::SceneDialogue { entry, outcome });
                }
                RuntimeEvent::SceneDialogueResume => {
                    if self.scene_dialogue_autoclose {
                        self.runtime.close_dialogue();
                        self.runtime.dialogue_closed();
                        routed.push(Routed::SceneDialogueResumeSkipped);
                        continue;
                    }
                    let opened = self.runtime.resume_scene_dialogue(self.panel_layout());
                    if !opened {
                        self.runtime.dialogue_closed();
                    }
                    routed.push(Routed::SceneDialogueResume { opened });
                }
                // The ship's destination menu: the scene blocked on its op and
                // the session opens the window (`destination.rs`).
                RuntimeEvent::ScenePresentation {
                    op: psiv_core::SceneOp::DestinationMenu { mask, .. },
                } => {
                    self.open_destination(mask);
                    routed.push(Routed::DestinationMenu);
                }
                RuntimeEvent::SceneChoiceRequested => {
                    self.runtime.open_scene_choice();
                    routed.push(Routed::SceneChoice);
                }
                other => kept.push(other),
            }
        }
        (kept, routed)
    }

    /// The counter whose shopkeeper the party reached across a counter: the
    /// object's own cell first, then the probe cell.
    fn counter_at(&self, npc_index: usize, cell: psiv_core::Cell) -> Option<ShopCounter> {
        let shops = self.runtime.data().shops()?;
        let map = self.runtime.map_id().0;
        let object_cell = self.runtime.map().npcs().get(npc_index).map(|npc| npc.cell);
        object_cell
            .into_iter()
            .chain(std::iter::once(cell))
            .find_map(|at| shops.counter_at(map, at.x, at.y))
            .cloned()
    }

    fn open_counter(&mut self, counter: &ShopCounter) {
        if let Some(shops) = self.runtime.data().shops() {
            self.shop = Some(ShopView::open(counter, shops, &self.runtime));
        }
    }

    /// Whether a scene line takes the panel cutscene's portrait layout: the
    /// runtime's panel byte is up and the running scene is a high-bit cutscene.
    fn panel_layout(&self) -> bool {
        self.runtime.panel_dialogue_mode()
    }
}
