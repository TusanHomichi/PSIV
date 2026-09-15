//! FieldRoutine_PlaceName and the field HINAS/RYUKA menu handoff.

use psiv_core::{Cell, CharId, Direction, Flag, MapId, WarpTrigger, battle::status};
use psiv_data::TownDestination;

use crate::{BridgeError, CampAbilityKind, Runtime, RuntimeEvent};

/// Retail technique IDs, distinct from SHIFT/SANER at 30/31.
pub const RYUKA: u8 = 39;
/// Retail HINAS technique ID.
pub const HINAS: u8 = 40;
/// Retail field item that selects a visited town.
pub const TELEPIPE: u8 = 132;
/// Retail field item that leaves the current dungeon.
pub const ESCAPIPE: u8 = 133;

#[derive(Clone, Copy)]
enum TravelPayment {
    Technique(CharId, u16),
    Item(usize),
}

/// The next field-teleport window. Browsing never spends TP or an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CampTravelMenu {
    /// Town destinations in original table order, filtered by visited bits.
    Towns(Vec<TownDestination>),
    /// Payment is complete. Any ordinary acknowledgement starts the teleport.
    Ready {
        /// Technique or item display name.
        name: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PendingTravel {
    map: u16,
    previous_map: u16,
    cell: Cell,
    facing: Direction,
}

impl Runtime {
    /// The live dungeon exit index. Negative map bytes preserve this value.
    #[must_use]
    pub fn dungeon_exit_index(&self) -> u8 {
        self.dungeon_exit_index
    }

    /// World_Index is a BYTE at $F400, the high byte of the saved word.
    #[must_use]
    pub fn world_index(&self) -> u8 {
        (self.saved_world_index >> 8) as u8
    }

    /// Current Field_Map_Index_2, updated by warps and explicit scene loads.
    #[must_use]
    pub fn previous_map_id(&self) -> u16 {
        self.saved_map_index_2
    }

    pub(super) fn apply_travel_entry(&mut self) {
        let Some(record) = self.data.map(psiv_data::MapId(self.map.id().0)) else {
            return;
        };
        if let Some(index) = record.flags.dungeon_teleport_index {
            self.dungeon_exit_index = index;
        }
        let Some(entry) = self.data.travel().and_then(|travel| {
            travel.entries.iter().find(|entry| {
                entry.map == self.map.id().0 && entry.previous_map == self.saved_map_index_2
            })
        }) else {
            return;
        };
        self.dungeon_exit_index = 0;
        match entry.selector {
            0..=0x7F => {
                let _ = self.game.set(Flag::town(u16::from(entry.selector)));
            }
            0xFE => {}
            selector => {
                self.dungeon_exit_index = selector & 0x7F;
                if self.dungeon_exit_index == 7 && self.state().cell().x == 0x520 / 16 {
                    self.dungeon_exit_index = 8;
                }
            }
        }
    }

    /// Visited RYUKA destinations. Eligibility requires both retail map bytes
    /// to allow town travel and a planetary world (0..2).
    #[must_use]
    pub fn town_destinations(&self) -> Vec<TownDestination> {
        let Some(record) = self.data.map(psiv_data::MapId(self.map.id().0)) else {
            return Vec::new();
        };
        if self.dungeon_exit_index != 0
            || !record.flags.allows_town_teleport()
            || self.world_index() >= 3
        {
            return Vec::new();
        }
        self.data.travel().map_or_else(Vec::new, |travel| {
            travel
                .towns
                .iter()
                .filter(|town| {
                    town.world == self.world_index() && self.game.is_set(Flag::town(town.town_flag))
                })
                .cloned()
                .collect()
        })
    }

    fn travel_available(&self) -> Result<(), String> {
        if self.pending_travel.is_some()
            || self.game_over
            || self.scene_active()
            || self.battle.is_some()
            || self.loot.is_some()
        {
            return Err("TRAVEL UNAVAILABLE".into());
        }
        Ok(())
    }

    fn travel_caster(&self, slot: usize, id: u8) -> Result<(CharId, u16, String), String> {
        self.travel_available()?;
        let ability = self
            .camp_abilities(slot, CampAbilityKind::Technique)
            .into_iter()
            .find(|ability| ability.id == id && matches!(id, RYUKA | HINAS))
            .ok_or("ABILITY NOT LEARNED")?;
        let who = self.game.party_slot(slot).ok_or("NO PARTY MEMBER")?;
        let stats = self.game.roster().get(who).ok_or("NO PARTY MEMBER")?;
        if stats.status & status::DEAD != 0 {
            return Err("CASTER IS DOWN".into());
        }
        if stats.status & status::PARALYZED != 0 {
            return Err("CASTER IS PARALYZED".into());
        }
        if stats.curr_tp < u16::from(ability.cost) {
            return Err("NOT ENOUGH TP".into());
        }
        Ok((who, u16::from(ability.cost), ability.name))
    }

    fn prepare_travel(
        &mut self,
        payment: TravelPayment,
        name: String,
        destination: PendingTravel,
    ) -> Result<CampTravelMenu, String> {
        let record = self
            .data
            .map(psiv_data::MapId(destination.map))
            .ok_or("DESTINATION NOT PACKED")?;
        if u32::from(destination.cell.x) >= record.collision.grid.width()
            || u32::from(destination.cell.y) >= record.collision.grid.height()
        {
            return Err("DESTINATION OUTSIDE MAP".into());
        }
        match payment {
            TravelPayment::Technique(who, cost) => {
                self.game.roster_mut().get_mut(who).unwrap().curr_tp -= cost;
            }
            TravelPayment::Item(slot) => {
                self.game
                    .inventory_mut()
                    .remove_in_field(slot)
                    .ok_or("ITEM SLOT EMPTY")?;
            }
        }
        self.pending_travel = Some(destination);
        Ok(CampTravelMenu::Ready { name })
    }

    /// Select a learned travel technique. HINAS pays when its message opens;
    /// RYUKA first opens its free-to-cancel destination list.
    pub fn begin_camp_travel(&mut self, slot: usize, id: u8) -> Result<CampTravelMenu, String> {
        let (who, cost, name) = self.travel_caster(slot, id)?;
        if id == RYUKA {
            let towns = self.town_destinations();
            return if towns.is_empty() {
                Err("CANNOT TELEPORT HERE".into())
            } else {
                Ok(CampTravelMenu::Towns(towns))
            };
        }
        let destination = self.dungeon_destination()?;
        self.prepare_travel(TravelPayment::Technique(who, cost), name, destination)
    }

    fn dungeon_destination(&self) -> Result<PendingTravel, String> {
        let exit = self
            .data
            .travel()
            .and_then(|travel| {
                travel
                    .dungeons
                    .iter()
                    .find(|exit| exit.index != 0 && exit.index == self.dungeon_exit_index)
            })
            .ok_or("CANNOT TELEPORT HERE")?;
        let facing = match exit.facing {
            0 => Direction::Down,
            4 => Direction::Up,
            8 => Direction::Right,
            12 => Direction::Left,
            _ => unreachable!("validated travel facing"),
        };
        Ok(PendingTravel {
            map: exit.map,
            previous_map: self.map.id().0,
            cell: Cell::new(exit.x / 2, exit.y / 2 + 1),
            facing,
        })
    }

    /// Confirm one RYUKA table index, rechecking membership and caster state
    /// before spending TP. Passing an unvisited town cannot teleport there.
    pub fn select_camp_town(&mut self, slot: usize, index: u8) -> Result<CampTravelMenu, String> {
        let (who, cost, name) = self.travel_caster(slot, RYUKA)?;
        let destination = self.town_destination(index)?;
        self.prepare_travel(TravelPayment::Technique(who, cost), name, destination)
    }

    fn town_destination(&self, index: u8) -> Result<PendingTravel, String> {
        let town = self
            .town_destinations()
            .into_iter()
            .find(|town| town.index == index)
            .ok_or("TOWN NOT AVAILABLE")?;
        Ok(PendingTravel {
            map: u16::from(town.world),
            previous_map: town.previous_map,
            cell: Cell::new(town.x / 2, town.y / 2 + 1),
            facing: Direction::Down,
        })
    }

    fn travel_item(&self, slot: usize) -> Result<(u8, String), String> {
        self.travel_available()?;
        let id = self.game.inventory().get(slot).ok_or("ITEM SLOT EMPTY")?;
        if !matches!(id, TELEPIPE | ESCAPIPE) {
            return Err("NOT A TRAVEL ITEM".into());
        }
        let name = self
            .camp_state()
            .inventory
            .into_iter()
            .find(|item| item.slot == slot)
            .map(|item| item.name)
            .ok_or("ITEM DATA UNAVAILABLE")?;
        Ok((id, name))
    }

    /// Field ITEM USE bypasses character selection for both pipes. ESCAPIPE
    /// removes the selected slot when the valid exit message opens. TELEPIPE
    /// only browses here; cancelling the town window keeps the entire inventory.
    pub fn begin_camp_item_travel(&mut self, slot: usize) -> Result<CampTravelMenu, String> {
        let (id, name) = self.travel_item(slot)?;
        if id == TELEPIPE {
            let towns = self.town_destinations();
            return if towns.is_empty() {
                Err("CANNOT TELEPORT HERE".into())
            } else {
                Ok(CampTravelMenu::Towns(towns))
            };
        }
        let destination = self.dungeon_destination()?;
        self.prepare_travel(TravelPayment::Item(slot), name, destination)
    }

    /// Confirm a TELEPIPE destination. Recheck the exact selected inventory
    /// slot and town before removing that item and shifting the first hole.
    /// This is GetInventoryOffset, not the chest swap's first matching ID.
    pub fn select_camp_item_town(
        &mut self,
        slot: usize,
        index: u8,
    ) -> Result<CampTravelMenu, String> {
        let (id, name) = self.travel_item(slot)?;
        if id != TELEPIPE {
            return Err("NOT A TELEPIPE".into());
        }
        let destination = self.town_destination(index)?;
        self.prepare_travel(TravelPayment::Item(slot), name, destination)
    }

    /// Acknowledge the paid teleport message. It is consumed once; the shell
    /// closes camp and processes the ordinary map-change event for art/music.
    pub fn complete_camp_travel(&mut self) -> Result<Vec<RuntimeEvent>, BridgeError> {
        let Some(destination) = self.pending_travel.take() else {
            return Ok(Vec::new());
        };
        self.change_map_from(
            MapId(destination.map),
            destination.cell,
            destination.facing,
            destination.previous_map,
        )?;
        self.set_field_suspended(false);
        Ok(vec![RuntimeEvent::MapChanged {
            map: MapId(destination.map),
            trigger: WarpTrigger::MapChange,
        }])
    }
}
