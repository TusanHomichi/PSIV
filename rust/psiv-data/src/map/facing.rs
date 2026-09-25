//! Facings a map record stores, and the direction each one means.
//!
//! Warps carry an arrival facing and objects an own facing, both as the raw byte
//! `Map_Start_Facing_Dir` defines; a sprite binding carries the same byte as a
//! spelled-out `SpriteFacing` that tolerates the cartridge's one out-of-set value.

use serde::{Deserialize, Serialize};

/// A facing direction, as `Map_Start_Facing_Dir` encodes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facing {
    /// The raw byte: 0 down, 4 up, 8 right, `$C` left.
    pub id: u8,
    /// The decoded direction, or `None` for a byte outside those four.
    #[serde(default)]
    pub name: Option<Direction>,
}

impl Facing {
    /// The decoded direction, if the byte is one of the four the ROM defines.
    pub fn direction(&self) -> Option<Direction> {
        self.name
    }
}

/// One of the four facings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Facing down the screen; the raw byte is 0.
    Down,
    /// Raw byte 4.
    Up,
    /// Raw byte 8.
    Right,
    /// Raw byte `$C`.
    Left,
}

/// A sprite facing, tolerating the cartridge's one out-of-set byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpriteFacing {
    /// Facing down.
    Down,
    /// Facing up.
    Up,
    /// Facing left.
    Left,
    /// Facing right.
    Right,
    /// A facing byte outside {0, 4, 8, $C}; drawn as down.
    #[serde(other)]
    Unknown,
}

impl SpriteFacing {
    /// The typed direction, defaulting the unknown byte to down.
    #[must_use]
    pub fn direction_or_down(self) -> Direction {
        match self {
            SpriteFacing::Up => Direction::Up,
            SpriteFacing::Left => Direction::Left,
            SpriteFacing::Right => Direction::Right,
            SpriteFacing::Down | SpriteFacing::Unknown => Direction::Down,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_decodes_the_four_bytes_and_tolerates_the_rest() {
        let up: Facing = serde_json::from_str(r#"{"id": 4, "name": "up"}"#).unwrap();
        assert_eq!(up.direction(), Some(Direction::Up));
        let odd: Facing = serde_json::from_str(r#"{"id": 7, "name": null}"#).unwrap();
        assert_eq!(odd.direction(), None);
        assert_eq!(odd.id, 7);
    }
}
