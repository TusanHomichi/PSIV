//! Overworld planet map screen (`FieldRoutine_WorldMap`, `ps4.asm:136977-137245`, #122).
//!
//! Opened when `ItemAction_Map` (`$8C`) is used on Motavia (`$000`) or Dezolis (`$001`).
//! The D-pad scrolls `2` pixels per frame (both axes on Motavia, horizontal-only on
//! Dezolis starting at `y = -48` / `$FFD0`), and Cancel, Speak/Accept, Camp, or Start
//! dismisses back to the field (`ps4.asm:137138-137157`).

use psiv_core::Input;
use psiv_data::WorldMapAsset;

use crate::Runtime;
use crate::pad::{Button, Pad};

use super::{Frame, FrameMode, Session};

/// Scroll speed in pixels per frame (`loc_668CA`: `-2` / `+2`).
const SCROLL_STEP: i16 = 2;
/// Frames per party-marker blink half-period.
const BLINK_HALF_PERIOD: u16 = 16;

/// Snapshot rendered by the Godot world-map overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldMapView {
    /// World byte: `0` Motavia, `1` Dezolis.
    pub world: u8,
    /// Planet name (`MOTAVIA` / `DEZOLIS`).
    pub name: String,
    /// Relative PNG path inside the runtime pack (`travel/world_map_*.png`).
    pub png: String,
    /// Planet image width in pixels (`256`).
    pub width: u16,
    /// Planet image height in pixels (`256` Motavia, `128` Dezolis).
    pub height: u16,
    /// Whether vertical scrolling is active.
    pub scroll_y_enabled: bool,
    /// Horizontal camera offset in pixels (`0..width`).
    pub scroll_x: i16,
    /// Vertical camera offset in pixels (`0..height` on Motavia, `-48` on Dezolis).
    pub scroll_y: i16,
    /// Party marker X on the planet image (`chunk_x * 2`).
    pub marker_x: i16,
    /// Party marker Y on the planet image (`chunk_y * 2`).
    pub marker_y: i16,
    /// Whether the blinking party marker is lit on this frame.
    pub blink_on: bool,
}

/// Active world-map modal state owned by [`Session`].
#[derive(Debug, Clone)]
pub(crate) struct WorldMapMode {
    asset: WorldMapAsset,
    scroll_x: i16,
    scroll_y: i16,
    marker_x: i16,
    marker_y: i16,
    frames: u16,
}

impl WorldMapMode {
    /// Opens the world map for the current overworld map (`0` Motavia, `1` Dezolis).
    pub(crate) fn open(runtime: &Runtime) -> Option<Self> {
        let map_id = runtime.map().id().0;
        let world = match map_id {
            0 => 0u8,
            1 => 1u8,
            _ => return None,
        };
        let asset = runtime
            .data()
            .travel()
            .and_then(|travel| travel.world_map(world))
            .cloned()
            .unwrap_or_else(|| fallback_asset(world));
        let cell = runtime.state().cell();
        let max_x = i16::try_from(asset.width).unwrap_or(256).saturating_sub(2);
        let max_y = i16::try_from(asset.height).unwrap_or(256).saturating_sub(2);
        let marker_x = i16::try_from((cell.x / 2) * 2).unwrap_or(0).clamp(0, max_x);
        let marker_y = i16::try_from((cell.y / 2) * 2).unwrap_or(0).clamp(0, max_y);
        Some(Self {
            scroll_x: 0,
            scroll_y: asset.initial_y,
            marker_x,
            marker_y,
            frames: 0,
            asset,
        })
    }

    pub(crate) fn view(&self) -> WorldMapView {
        WorldMapView {
            world: self.asset.world,
            name: self.asset.name.clone(),
            png: self.asset.png.clone(),
            width: self.asset.width,
            height: self.asset.height,
            scroll_y_enabled: self.asset.scroll_y,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
            marker_x: self.marker_x,
            marker_y: self.marker_y,
            blink_on: (self.frames / BLINK_HALF_PERIOD).is_multiple_of(2),
        }
    }

    /// Steps one frame of `FieldRoutine_WorldMap` (`loc_66888`, `ps4.asm:137138-137157`).
    /// Returns `true` when dismissed.
    pub(crate) fn step(&mut self, held: Pad, pressed: Pad) -> bool {
        if pressed.held(Button::Cancel)
            || pressed.held(Button::Speak)
            || pressed.held(Button::Camp)
            || pressed.held(Button::Start)
        {
            return true;
        }
        let width = i16::try_from(self.asset.width).unwrap_or(256).max(1);
        let height = i16::try_from(self.asset.height).unwrap_or(256).max(1);
        let dx = match (held.held(Button::Left), held.held(Button::Right)) {
            (true, false) => -SCROLL_STEP,
            (false, true) => SCROLL_STEP,
            _ => 0,
        };
        self.scroll_x = (self.scroll_x + dx).rem_euclid(width);
        if self.asset.scroll_y {
            let dy = match (held.held(Button::Up), held.held(Button::Down)) {
                (true, false) => -SCROLL_STEP,
                (false, true) => SCROLL_STEP,
                _ => 0,
            };
            self.scroll_y = (self.scroll_y + dy).rem_euclid(height);
        }
        self.frames = self.frames.wrapping_add(1);
        false
    }
}

fn fallback_asset(world: u8) -> WorldMapAsset {
    if world == 0 {
        WorldMapAsset {
            world: 0,
            name: "MOTAVIA".to_owned(),
            png: "travel/world_map_motavia.png".to_owned(),
            width: 256,
            height: 256,
            scroll_y: true,
            initial_y: 0,
        }
    } else {
        WorldMapAsset {
            world: 1,
            name: "DEZOLIS".to_owned(),
            png: "travel/world_map_dezolis.png".to_owned(),
            width: 256,
            height: 128,
            scroll_y: false,
            initial_y: -48,
        }
    }
}

impl Session {
    /// Active world-map screen view, if open.
    #[must_use]
    pub fn world_map_view(&self) -> Option<WorldMapView> {
        self.world_map.as_ref().map(WorldMapMode::view)
    }

    /// Opens the world-map screen directly on Motavia or Dezolis.
    pub fn open_world_map(&mut self) -> bool {
        let Some(mode) = WorldMapMode::open(&self.runtime) else {
            return false;
        };
        self.world_map = Some(mode);
        self.runtime.set_field_suspended(true);
        true
    }

    pub(super) fn world_map_frame(&mut self, pad: Pad, pressed: Pad) -> Frame {
        let closed = self
            .world_map
            .as_mut()
            .is_some_and(|mode| mode.step(pad, pressed));
        if closed {
            self.world_map = None;
            self.runtime.set_field_suspended(false);
            self.accept_blocked = true;
            return Frame {
                mode: FrameMode::WorldMap,
                ..Frame::default()
            };
        }
        self.runtime.set_field_suspended(true);
        let events = self.runtime.tick(Input::Neutral);
        let (events, routed) = self.route(events);
        Frame {
            mode: FrameMode::WorldMap,
            events,
            routed,
            ..Frame::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::Pad;

    fn test_mode(world: u8) -> WorldMapMode {
        let asset = fallback_asset(world);
        WorldMapMode {
            scroll_x: 0,
            scroll_y: asset.initial_y,
            marker_x: 100,
            marker_y: 80,
            frames: 0,
            asset,
        }
    }

    #[test]
    fn motavia_scrolls_two_pixels_per_frame_both_axes_and_wraps() {
        let mut mode = test_mode(0);
        assert!(!mode.step(Pad::new(Button::Left), Pad::NEUTRAL));
        assert_eq!((mode.scroll_x, mode.scroll_y), (254, 0));
        assert!(!mode.step(Pad::new(Button::Up), Pad::NEUTRAL));
        assert_eq!((mode.scroll_x, mode.scroll_y), (254, 254));
        assert!(!mode.step(Pad::new(Button::Right), Pad::NEUTRAL));
        assert!(!mode.step(Pad::new(Button::Down), Pad::NEUTRAL));
        assert_eq!((mode.scroll_x, mode.scroll_y), (0, 0));
    }

    #[test]
    fn dezolis_starts_at_minus_48_and_locks_vertical_scroll() {
        let mut mode = test_mode(1);
        assert_eq!(mode.view().scroll_y, -48);
        assert!(!mode.step(Pad::new(Button::Up), Pad::NEUTRAL));
        assert!(!mode.step(Pad::new(Button::Down), Pad::NEUTRAL));
        assert_eq!(mode.view().scroll_y, -48);
        assert!(!mode.step(Pad::new(Button::Right), Pad::NEUTRAL));
        assert_eq!(mode.view().scroll_x, 2);
    }

    #[test]
    fn dismisses_on_cancel_speak_camp_or_start() {
        for button in [Button::Cancel, Button::Speak, Button::Camp, Button::Start] {
            let mut mode = test_mode(0);
            assert!(
                mode.step(Pad::new(button), Pad::new(button)),
                "button {button:?} must dismiss"
            );
        }
    }
}
