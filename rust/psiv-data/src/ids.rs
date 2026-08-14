//! Identifier newtypes: map ids and the ROM hash the pack was built from.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A field-map id: an index into `FieldMapPtrs`, which the ROM addresses with
/// three hex digits (417 entries, so `0x000`..=`0x1A0`).
///
/// Displayed the way the disassembly and the extractor both write it -- `0x010`
/// -- so an error message can be pasted straight into a symbol search.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MapId(pub u16);

impl fmt::Display for MapId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:03X}", self.0)
    }
}

impl fmt::Debug for MapId {
    /// Same text as [`Display`](fmt::Display); a derived `MapId(16)` in a
    /// `{:?}` error dump is useless next to the hex the ROM uses.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MapId({self})")
    }
}

impl From<u16> for MapId {
    fn from(value: u16) -> Self {
        MapId(value)
    }
}

/// A SHA-256 digest as 64 lowercase hex characters.
///
/// The pack records the ROM it was extracted from; the same fail-closed hash
/// discipline as the rest of the project (docs/RUNTIME_DESIGN.md, "Data path").
/// Parsing rejects anything that is not exactly 64 hex digits, so a truncated
/// or placeholder hash cannot reach a comparison and quietly not match.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RomHash(String);

impl RomHash {
    /// Parse a hex digest, accepting either case and normalising to lowercase.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() != 64 {
            return Err(format!(
                "expected a 64-character sha256 hex digest, got {} characters",
                text.len()
            ));
        }
        if let Some(bad) = text.chars().find(|c| !c.is_ascii_hexdigit()) {
            return Err(format!(
                "sha256 digest contains the non-hex character {bad:?}"
            ));
        }
        Ok(RomHash(text.to_ascii_lowercase()))
    }

    /// The digest as 64 lowercase hex characters.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RomHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for RomHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RomHash({})", self.0)
    }
}

impl Serialize for RomHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RomHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        RomHash::parse(&text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn map_id_displays_as_three_hex_digits() {
        assert_eq!(MapId(0x010).to_string(), "0x010");
        assert_eq!(MapId(0).to_string(), "0x000");
        assert_eq!(MapId(0x1A0).to_string(), "0x1A0");
        assert_eq!(format!("{:?}", MapId(0x013)), "MapId(0x013)");
    }

    #[test]
    fn rom_hash_normalises_case() {
        let upper = RomHash::parse(&DIGEST.to_ascii_uppercase()).unwrap();
        assert_eq!(upper.as_str(), DIGEST);
    }

    #[test]
    fn rom_hash_rejects_wrong_length_and_non_hex() {
        assert!(RomHash::parse("abc").is_err());
        let mut bad = DIGEST.to_string();
        bad.replace_range(0..1, "z");
        assert!(RomHash::parse(&bad).is_err());
    }

    #[test]
    fn rom_hash_deserialises_from_a_json_string() {
        let parsed: RomHash = serde_json::from_str(&format!("\"{DIGEST}\"")).unwrap();
        assert_eq!(parsed.as_str(), DIGEST);
        assert!(serde_json::from_str::<RomHash>("\"nope\"").is_err());
    }
}
