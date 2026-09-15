//! STATE/ORDER commits a complete permutation, as loc_5E5A0 does.
use crate::{BridgeError, Runtime, RuntimeEvent};
use psiv_core::{CharId, PARTY_SLOTS};

impl Runtime {
    /// Commit the ORDER chooser's complete list. Partial choices are UI state.
    /// Character records and the walking slots' positions remain untouched:
    /// loc_5E6DE replaces each slot's art, not its field coordinates.
    pub fn order_camp_party(&mut self, order: &[u8]) -> Result<Vec<RuntimeEvent>, BridgeError> {
        if self.battle.is_some() || self.scene_active() || self.party.leader().is_stepping() {
            return Err(BridgeError::Rejected(
                "party order requires idle field".into(),
            ));
        }
        let current: Vec<_> = self.game.party_members().iter().map(|id| id.0).collect();
        let mut expected = current.clone();
        let mut proposed = order.to_vec();
        expected.sort_unstable();
        proposed.sort_unstable();
        if order.is_empty() || order.len() > PARTY_SLOTS || proposed != expected {
            return Err(BridgeError::Rejected(
                "party order must contain each current member once".into(),
            ));
        }
        if current == order {
            return Ok(Vec::new());
        }
        self.game.set_party(std::array::from_fn(|slot| {
            order.get(slot).map(|id| CharId(*id))
        }));
        Ok(vec![RuntimeEvent::PartyChanged])
    }
}
