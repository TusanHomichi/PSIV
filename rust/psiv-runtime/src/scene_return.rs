//! A scene's return, and what the cartridge does with it.
//!
//! `FieldRoutine_Event` (`ps4.asm:120542-120548`) sends a plain event through
//! `loc_5A27A` and a cutscene (`Event_Index` bit 15) through
//! `FieldRoutine_Cutscene`. The two read the scene's `d0` differently:
//!
//! ```text
//! loc_5A27A (events)           jsr (a0) ... no test of d0; no reload   (:120553-120568)
//! FieldRoutine_Cutscene        jsr (a0) ; bne .skipmapreload           (:120739-120758)
//!                              bset #2, Map_Load_Flags ; move.w #8, Game_Mode_Index
//! ```
//!
//! So a **cutscene that returns zero** hands the frame to
//! `GameMode_LoadFieldMap` with bit 2 set, and control comes back only after
//! that load and its `Pal_FadeIn` (and `FieldRoutine_PlaceName`, which opens a
//! window when the travel table names the map pair). The load's state effects
//! are `Runtime::reload_field_after_cutscene`; this file owns the frames.
//!
//! The scene stays active through them: the runtime emits one
//! [`PresentationOp::FieldReload`] where the reload begins, the
//! [`PresentationOp::PlaceNameWindow`] where the window opens, and
//! [`RuntimeEvent::SceneEnded`] on the frame control returns, which is the
//! contract the flights' tape-35 frame tests measure.
//!
//! The frames are `psiv_core::field_reload_rows(map, music)` (measured per map
//! from the oracle, with and without the load's music branch, see
//! `docs/scenes/FIELD_RELOAD.md`), less the fade when bit 7 skipped it, plus
//! the 124-frame window. A map the oracle fixture could not reach has no
//! measured setup and counts only its fade.

use psiv_core::{PresentationOp, SceneOp, field_reload_rows};

use crate::{Runtime, RuntimeEvent};

/// `Pal_FadeIn`'s frames, display enable and terminal pass included
/// (`docs/scenes/41_InsideSpaceship.md`, "Fade correction").
pub(crate) const FADE_IN_FRAMES: u16 = 16;
/// `FieldRoutine_PlaceName`'s window: the `$78` countdown plus draw and
/// teardown DMA (`ps4.asm:136552-136646`), the same 124 frames at both of
/// tape 35's landings.
pub(crate) const PLACE_NAME_FRAMES: u16 = 124;

/// The frames left between a cutscene's zero return and field control.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReturnTail {
    elapsed: u16,
    /// The frame the place-name window opens on, when there is one.
    window_at: Option<u16>,
    total: u16,
}

impl Runtime {
    /// Reads the finished scene's return. A cutscene's zero starts the field
    /// reload and returns `true`; anything else (an event, a non-zero
    /// cutscene return, a scene that faulted without returning) does not.
    pub(crate) fn begin_cutscene_return(&mut self, events: &mut Vec<RuntimeEvent>) -> bool {
        let returned = self.scene_returned.take();
        if !self.scene_event.is_cutscene() || returned != Some(0) {
            return false;
        }
        let reload = match self.reload_field_after_cutscene() {
            Ok(reload) => reload,
            Err(error) => {
                events.push(RuntimeEvent::MapRefreshFailed {
                    error: error.to_string(),
                });
                return false;
            }
        };
        // The scene's panel byte is spent with it: the renderer shows the
        // field from here.
        self.scene_panel_sprites = false;
        let map = self.map.id().0;
        let rows = field_reload_rows(map, reload.music_written).unwrap_or(FADE_IN_FRAMES);
        let fade = if reload.skip_fade { 0 } else { FADE_IN_FRAMES };
        let setup = rows - FADE_IN_FRAMES;
        let loaded = setup + fade;
        let window = self.place_name_applies();
        events.push(RuntimeEvent::ScenePresentation {
            op: SceneOp::Presentation {
                op: PresentationOp::FieldReload { setup, fade },
            },
        });
        self.scene_tail = Some(ReturnTail {
            elapsed: 0,
            window_at: window.then_some(loaded),
            total: loaded + if window { PLACE_NAME_FRAMES } else { 0 },
        });
        true
    }

    /// Whether `FieldRoutine_PlaceName` finds `(Field_Map_Index,
    /// Field_Map_Index_2)` in its table (`ps4.asm:136557-136571`).
    fn place_name_applies(&self) -> bool {
        let map = self.map.id().0;
        self.data.travel().is_some_and(|travel| {
            travel
                .entries
                .iter()
                .any(|entry| entry.map == map && entry.previous_map == self.saved_map_index_2)
        })
    }

    /// One frame of the reload the scene is waiting out. Control returns, with
    /// `SceneEnded`, on the frame the count runs out.
    pub(crate) fn tick_scene_tail(&mut self) -> Vec<RuntimeEvent> {
        let mut events = Vec::new();
        let Some(tail) = self.scene_tail.as_mut() else {
            return events;
        };
        tail.elapsed += 1;
        let ReturnTail {
            elapsed,
            window_at,
            total,
        } = *tail;
        if window_at == Some(elapsed) {
            events.push(RuntimeEvent::ScenePresentation {
                op: SceneOp::Presentation {
                    op: PresentationOp::PlaceNameWindow,
                },
            });
        }
        // The window's loop runs `RunMapUpdates` every frame it is up
        // (`ps4.asm:136631-136638`); the load before it does not.
        if window_at.is_some_and(|at| elapsed >= at) && elapsed < total && !self.field_suspended {
            self.run_map_updates();
        }
        if elapsed >= total {
            self.scene_tail = None;
            self.end_scene(&mut events);
        }
        events
    }
}
