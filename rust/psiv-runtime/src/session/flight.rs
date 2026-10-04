//! The flight's drawable records; all clocks stay in the scene interpreter.

use psiv_core::SceneOp;

use super::Session;
use crate::RuntimeEvent;

/// Complete text records for RunText2 (d4=1) and FieldRoutine_PlaceName.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlightView {
    /// The caption, including its ROM-authored leading spaces.
    pub caption: Option<String>,
    /// The landing's place name from the travel pack.
    pub arrival: Option<String>,
}

impl Session {
    pub(super) fn sync_flight_view(&mut self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::ScenePresentation {
                op: SceneOp::FlightPlanet,
            } => {
                self.flight = Some(FlightView {
                    caption: None,
                    arrival: None,
                });
            }
            RuntimeEvent::ScenePresentation {
                op: SceneOp::SetTextColour { colour: 0 },
            } if self.flight.is_some() => {
                // RunText2's final FF queues the completed line's DMA, after
                // the setup interval, immediately before Pal_FadeIn.
                if let Some(view) = self.flight.as_mut() {
                    view.caption = self
                        .runtime
                        .data()
                        .ship_menu()
                        .and_then(|menu| menu.flight_caption(self.runtime.world_index()))
                        .map(str::to_owned);
                }
            }
            RuntimeEvent::ScenePresentation {
                op: SceneOp::FlightArrivalName,
            } => {
                let arrival = self
                    .runtime
                    .data()
                    .travel()
                    .and_then(|travel| {
                        travel.entries.iter().find(|entry| {
                            entry.map == self.runtime.map_id().0
                                && entry.previous_map == self.runtime.previous_map_id()
                        })
                    })
                    .map(|entry| entry.name.clone());
                self.flight = Some(FlightView {
                    caption: None,
                    arrival,
                });
            }
            RuntimeEvent::MapChanged { .. } | RuntimeEvent::SceneEnded => self.flight = None,
            _ => {}
        }
    }

    /// Text the shell draws during a flight; no presentation clock lives there.
    #[must_use]
    pub fn flight_view(&self) -> Option<&FlightView> {
        self.flight.as_ref()
    }
}
