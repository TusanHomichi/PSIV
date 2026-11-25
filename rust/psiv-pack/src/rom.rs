//! The one ROM the pack is built from, and the check that refuses any other.
//!
//! The identity is the image's SHA-256 (`psiv_tools/core.py`,
//! `EXPECTED_SHA256`). The refusal's wording is the Python builder's, so the
//! setup screen keeps the established refusal wording. CLI tests exercise
//! wrong and unreadable ROMs without changing the output.

use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::PackError;
use crate::png::hex;

/// SHA-256 of the US retail image, as `psiv_tools/core.py` states it.
pub const EXPECTED_SHA256: &str =
    "511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a";

/// A pinned region of the US retail ROM: `(length, sha256)`.
/// Held by digest rather than raw bytes so source carries no cartridge content (#461).
pub type Pin = (usize, &'static str);

/// SHA-256 of `bytes`, as lowercase hex.
pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Whether `rom[at..at + len]` exists and matches `digest`.
pub fn matches_pin(rom: &[u8], at: usize, (len, digest): (usize, &str)) -> bool {
    rom.get(at..at + len)
        .is_some_and(|bytes| sha256(bytes) == digest)
}

/// Return `rom[at..at + len]` if it matches `digest`, otherwise refuse.
pub fn pinned<'a>(
    rom: &'a [u8],
    at: usize,
    (len, digest): (usize, &str),
    what: &str,
) -> Result<&'a [u8], PackError> {
    let bytes = rom.get(at..at + len).ok_or_else(|| {
        PackError(format!(
            "{what} at 0x{at:06X} (+{len}) runs past the end of the ROM"
        ))
    })?;
    if sha256(bytes) != digest {
        return Err(PackError(format!(
            "{what} at 0x{at:06X}: retail digest mismatch (got {})",
            hex(bytes)
        )));
    }
    Ok(bytes)
}

/// The verified ROM image.
pub struct Rom {
    bytes: Vec<u8>,
}

impl Rom {
    /// Read `path` and refuse anything but the US retail image.
    pub fn read(path: &Path) -> Result<Self, PackError> {
        let bytes = std::fs::read(path)
            .map_err(|e| PackError(format!("cannot read {}: {e}", path.display())))?;
        Self::verify(path, bytes)
    }

    /// Refuse `bytes`, read from `path`, unless they are the US retail image.
    pub fn verify(path: &Path, bytes: Vec<u8>) -> Result<Self, PackError> {
        let sha = hex(&Sha256::digest(&bytes));
        if sha != EXPECTED_SHA256 {
            return Err(PackError(wrong_rom(path, &sha)));
        }
        Ok(Rom { bytes })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Unchecked bytes, for tests of the machinery around the sections.
    #[cfg(test)]
    pub(crate) fn for_tests(bytes: Vec<u8>) -> Self {
        Rom { bytes }
    }
}

/// The refusal, worded as `psiv_tools.core.read_rom` words it. It names the
/// file, not its path, which would put the player's folders on the setup
/// screen and in every screenshot of it.
pub fn wrong_rom(path: &Path, sha: &str) -> String {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    format!(
        "{name} is not the US retail ROM PSIV builds from: its SHA-256 is {sha}, \
         expected {EXPECTED_SHA256}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_another_image_in_the_python_wording() {
        let error = Rom::verify(Path::new("other-image"), vec![0; 0x400])
            .err()
            .expect("zeros are not the retail ROM");
        let sha = hex(&Sha256::digest(vec![0u8; 0x400]));
        assert_eq!(
            error.0,
            format!(
                "other-image is not the US retail ROM PSIV builds from: its SHA-256 is {sha}, \
                 expected {EXPECTED_SHA256}"
            )
        );
    }

    #[test]
    fn the_refusal_names_the_file_not_the_folders_it_is_in() {
        let path = Path::new("home")
            .join("player")
            .join("Downloads")
            .join("game.bin");
        let error = Rom::verify(&path, vec![0; 0x400])
            .err()
            .expect("zeros are refused");
        assert!(
            error.0.starts_with("game.bin is not the US retail ROM"),
            "{}",
            error.0
        );
        assert!(!error.0.contains("player"), "{}", error.0);
    }

    #[test]
    fn pinned_regions_accept_matching_digests_and_refuse_mutations_or_truncation() {
        let rom = [1u8, 2, 3, 4];
        let pin: Pin = (
            2,
            "a12871fee210fb8619291eaea194581cbd2531e4b23759d225f6806923f63222",
        );
        assert!(matches_pin(&rom, 0, pin));
        assert!(!matches_pin(&rom, 1, pin));
        assert!(!matches_pin(&rom, 3, pin));
        assert_eq!(pinned(&rom, 0, pin, "region").unwrap(), &[1, 2]);
        let err = pinned(&rom, 1, pin, "region").unwrap_err();
        assert!(
            err.0.contains("retail digest mismatch (got 0203)"),
            "{}",
            err.0
        );
        assert!(pinned(&rom, 3, pin, "region").is_err());
    }
}
