//! Retail-timed presentation transitions.
//!
//! The cartridge does not tween a transparent black layer.  Its palette
//! helpers rewrite CRAM in seven increments between the full palette and
//! black, with the visible step held for two frames.  This small state machine
//! keeps that cadence in integer frames; the Godot bridge only turns the
//! current visual into one or more `ColorRect`s.

const PALETTE_LEVELS: u8 = 7;

/// A transition requested by the field presentation seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransitionKind {
    /// Doorway/map-change fade-through-black: 13 out, 54 black, 13 in.
    Doorway,
    /// Encounter flash: field to white, white hold, then battle reveal.
    BattleEntry,
    /// Cutscene initialization: fade to black and hold until the scene's
    /// first presentation window is ready.
    SceneStart,
    /// Cutscene return: start black and reveal the field in the retail ramp.
    SceneEnd,
    /// The title hand-off's centered screen wipe.  The Godot field has no
    /// title plane, so this is used as a matching black-to-field reveal.
    GameStart,
    /// A scene's direct `Pal_FadeIn` ramp. The scene interpreter owns the
    /// blocking/tick semantics; this is the renderer's 13-step black ramp.
    SceneFadeIn,
    /// A scene's direct `PalFadeOut_ClrSpriteTbl` ramp.
    SceneFadeOut,
}

/// The solid colour used by the current palette ramp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransitionColor {
    Black,
    White,
}

/// What the renderer needs for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransitionVisual {
    pub(crate) color: TransitionColor,
    /// Quantized CRAM level, 0 = transparent and 7 = opaque.
    pub(crate) level: u8,
    /// For the game-start wipe, the fraction of the centered window that is
    /// open. `None` means one full-screen rectangle.
    pub(crate) opening: Option<f32>,
}

/// Frame counter and retail timing for one active transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Transition {
    kind: TransitionKind,
    age: u16,
}

impl Transition {
    pub(crate) fn new(kind: TransitionKind) -> Self {
        Self { kind, age: 0 }
    }

    pub(crate) fn kind(self) -> TransitionKind {
        self.kind
    }

    /// Advances one renderer frame. Returns true once the measured sequence
    /// has completed.
    pub(crate) fn tick(&mut self) -> bool {
        if !self.finished() {
            self.age = self.age.saturating_add(1);
        }
        self.finished()
    }

    pub(crate) fn finished(self) -> bool {
        self.age >= self.duration()
    }

    /// Returns the stepped visual for the current age. A `None` visual means
    /// the transition is active for timing purposes but has no cover to draw
    /// (the title plane is the one such missing presentation surface).
    pub(crate) fn visual(self) -> Option<TransitionVisual> {
        match self.kind {
            TransitionKind::Doorway => self.doorway_visual(),
            TransitionKind::BattleEntry => self.battle_visual(),
            TransitionKind::SceneStart => self.scene_start_visual(),
            TransitionKind::SceneEnd => self.scene_end_visual(),
            TransitionKind::GameStart => self.game_start_visual(),
            TransitionKind::SceneFadeIn => self.scene_fade_in_visual(),
            TransitionKind::SceneFadeOut => self.scene_fade_out_visual(),
        }
    }

    /// Scene fades sit below the dialogue window but above the cutscene plane;
    /// battle and boot covers must sit above the battle/field nodes too.
    pub(crate) fn front_layer(self) -> bool {
        matches!(
            self.kind,
            TransitionKind::BattleEntry
                | TransitionKind::GameStart
                | TransitionKind::SceneFadeIn
                | TransitionKind::SceneFadeOut
        )
    }

    fn duration(self) -> u16 {
        match self.kind {
            TransitionKind::Doorway => 80,
            TransitionKind::BattleEntry => 56,
            TransitionKind::SceneStart => 37,
            TransitionKind::SceneEnd => 55,
            TransitionKind::GameStart => 18,
            TransitionKind::SceneFadeIn | TransitionKind::SceneFadeOut => 14,
        }
    }

    fn doorway_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        let level = match self.age {
            0 => return None,
            1..=13 => step_up(self.age),
            14..=67 => PALETTE_LEVELS,
            68..=79 => step_down(self.age - 67),
            _ => return None,
        };
        Some(black(level))
    }

    fn battle_visual(self) -> Option<TransitionVisual> {
        if self.finished() || self.age == 0 {
            return None;
        }
        let visual = match self.age {
            1..=14 => white(step_up(self.age)),
            15..=41 => white(PALETTE_LEVELS),
            42..=55 => white(step_down(self.age - 41)),
            _ => return None,
        };
        Some(visual)
    }

    fn scene_start_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        let level = match self.age {
            0 => PALETTE_LEVELS,
            1..=13 => step_up(self.age),
            14..=36 => PALETTE_LEVELS,
            _ => return None,
        };
        Some(black(level))
    }

    fn scene_end_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        let level = match self.age {
            0..=41 => PALETTE_LEVELS,
            42..=54 => step_down(self.age - 41),
            _ => return None,
        };
        Some(black(level))
    }

    fn game_start_visual(self) -> Option<TransitionVisual> {
        if self.finished() || self.age >= 8 {
            return None;
        }
        Some(TransitionVisual {
            color: TransitionColor::Black,
            level: PALETTE_LEVELS,
            opening: Some(f32::from(self.age) / 8.0),
        })
    }

    fn scene_fade_in_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        Some(black(PALETTE_LEVELS.saturating_sub(step_up(self.age))))
    }

    fn scene_fade_out_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        Some(black(step_up(self.age)))
    }
}

fn black(level: u8) -> TransitionVisual {
    TransitionVisual {
        color: TransitionColor::Black,
        level,
        opening: None,
    }
}

fn white(level: u8) -> TransitionVisual {
    TransitionVisual {
        color: TransitionColor::White,
        level,
        opening: None,
    }
}

/// Retail holds each palette level for two frames. The first changed frame is
/// level one; the 13th frame reaches level seven (black/full white).
fn step_up(frame: u16) -> u8 {
    (frame.div_ceil(2) as u8).min(PALETTE_LEVELS)
}

fn step_down(frame: u16) -> u8 {
    PALETTE_LEVELS.saturating_sub(step_up(frame))
}

#[cfg(test)]
mod tests {
    use super::{Transition, TransitionColor, TransitionKind};

    fn age_to(transition: &mut Transition, age: u16) {
        for _ in 0..age {
            transition.tick();
        }
    }

    #[test]
    fn doorway_matches_thirteen_fade_hold_and_thirteen_fade_in() {
        let mut transition = Transition::new(TransitionKind::Doorway);
        assert!(transition.visual().is_none());

        age_to(&mut transition, 1);
        assert_eq!(transition.visual().unwrap().level, 1);
        age_to(&mut transition, 12);
        assert_eq!(transition.visual().unwrap().level, 7);
        age_to(&mut transition, 54);
        assert_eq!(transition.visual().unwrap().level, 7);
        age_to(&mut transition, 1);
        assert_eq!(transition.visual().unwrap().level, 6);
        age_to(&mut transition, 11);
        assert_eq!(transition.visual().unwrap().level, 1);
        age_to(&mut transition, 1);
        assert!(transition.finished());
        assert!(transition.visual().is_none());
    }

    #[test]
    fn battle_is_white_flash_hold_then_white_reveal() {
        let mut transition = Transition::new(TransitionKind::BattleEntry);
        age_to(&mut transition, 1);
        let first = transition.visual().unwrap();
        assert_eq!(first.color, TransitionColor::White);
        assert_eq!(first.level, 1);
        age_to(&mut transition, 13);
        assert_eq!(transition.visual().unwrap().level, 7);
        age_to(&mut transition, 27);
        assert_eq!(transition.visual().unwrap().level, 7);
        age_to(&mut transition, 1);
        assert_eq!(transition.visual().unwrap().level, 6);
        age_to(&mut transition, 13);
        assert_eq!(transition.visual().unwrap().level, 0);
        age_to(&mut transition, 1);
        assert!(transition.finished());
    }

    #[test]
    fn scene_edges_match_the_observed_black_ramp() {
        let mut start = Transition::new(TransitionKind::SceneStart);
        assert_eq!(start.visual().unwrap().level, 7);
        age_to(&mut start, 1);
        assert_eq!(start.visual().unwrap().level, 1);
        age_to(&mut start, 12);
        assert_eq!(start.visual().unwrap().level, 7);
        age_to(&mut start, 24);
        assert!(start.finished());

        let mut end = Transition::new(TransitionKind::SceneEnd);
        assert_eq!(end.visual().unwrap().level, 7);
        age_to(&mut end, 41);
        assert_eq!(end.visual().unwrap().level, 7);
        age_to(&mut end, 1);
        assert_eq!(end.visual().unwrap().level, 6);
        age_to(&mut end, 12);
        assert_eq!(end.visual().unwrap().level, 0);
        age_to(&mut end, 1);
        assert!(end.finished());
    }

    #[test]
    fn game_start_opens_the_center_window_for_eight_frames() {
        let mut transition = Transition::new(TransitionKind::GameStart);
        assert_eq!(transition.visual().unwrap().opening, Some(0.0));
        age_to(&mut transition, 4);
        assert_eq!(transition.visual().unwrap().opening, Some(0.5));
        age_to(&mut transition, 4);
        assert!(transition.visual().is_none());
        assert!(!transition.finished());
        age_to(&mut transition, 10);
        assert!(transition.finished());
    }
}
