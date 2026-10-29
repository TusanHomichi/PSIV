//! The one ROM the pack is built from, and the check that refuses any other.
//!
//! The identity is the image's SHA-256 (`psiv_tools/core.py`,
//! `EXPECTED_SHA256`). The refusal's wording is the Python builder's, so the
//! setup screen shows one sentence whichever builder ran; the parity step
//! (`tools/pack_parity.py`) runs both on a wrong ROM and compares them.

use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::PackError;
use crate::png::hex;

/// SHA-256 of the US retail image, as `psiv_tools/core.py` states it.
pub const EXPECTED_SHA256: &str =
    "511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a";

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
}
