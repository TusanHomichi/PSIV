//! The one error type this crate returns.
//!
//! Hand-rolled rather than derived with `thiserror`. The crate would otherwise
//! have no dependency outside the two the workspace already declares, and a
//! new dependency also rewrites the shared `rust/Cargo.lock` -- which this lane
//! does not own while other lanes are building. Eight variants of `Display` is
//! a cheaper price than either.

use crate::ids::MapId;
use std::fmt;
use std::path::{Path, PathBuf};

/// Everything that can go wrong loading a runtime pack.
///
/// Every variant names the file or the map and field it came from: a pack
/// defect should be actionable from the message alone, without re-running under
/// a debugger.
#[derive(Debug)]
pub enum DataError {
    /// A file could not be read. Carries the path, which `io::Error` does not.
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// What the filesystem said.
        source: std::io::Error,
    },
    /// A file was read but is not valid JSON, or does not match the schema.
    Json {
        /// The file that could not be parsed.
        path: PathBuf,
        /// What serde said, including the line and column.
        source: serde_json::Error,
    },
    /// The pack was written by a different version of the packer.
    FormatVersion {
        /// The version the manifest declares.
        found: u32,
        /// The version this build reads, [`PACK_FORMAT_VERSION`](crate::PACK_FORMAT_VERSION).
        expected: u32,
    },
    /// The manifest lists the same map id twice, or lists one as both packed
    /// and skipped.
    DuplicateMapId {
        /// The id that appears more than once.
        id: MapId,
        /// Where the collision is.
        detail: String,
    },
    /// A map file's own id or symbol disagrees with the manifest entry that
    /// pointed at it -- the manifest and the file have drifted apart.
    ManifestMismatch {
        /// The map file that disagrees.
        path: PathBuf,
        /// Which field disagrees: `"id"` or `"symbol"`.
        field: &'static str,
        /// What the manifest entry says.
        manifest: String,
        /// What the map record says.
        record: String,
    },
    /// A structural rule was broken: dimensions, bounds, cross-references.
    Validation {
        /// The map the failure is about.
        map: MapId,
        /// The field, down to the array index, such as `warps[3].rect`.
        field: String,
        /// What is wrong with it.
        message: String,
    },
    /// A sprite sheet or a sprite reference is defective.
    Sprite {
        /// The sheet id, or the `map/npc` path of the offending reference.
        who: String,
        /// What is wrong with it.
        message: String,
    },
    /// The dialogue pack is defective: a tree entry, glyph, portrait, or
    /// window record failed validation.
    Dialogue {
        /// The file the defect lives in.
        path: PathBuf,
        /// What is wrong, naming the tree/entry/glyph.
        message: String,
    },
}

impl DataError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        DataError::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn json(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        DataError::Json {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn validation(
        map: MapId,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        DataError::Validation {
            map,
            field: field.into(),
            message: message.into(),
        }
    }

    /// The map a failure is about, when it is about one. Lets a caller group
    /// or filter failures without matching every variant.
    pub fn map_id(&self) -> Option<MapId> {
        match self {
            DataError::DuplicateMapId { id, .. } | DataError::Validation { map: id, .. } => {
                Some(*id)
            }
            _ => None,
        }
    }

    /// The file a failure is about, when it is about one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            DataError::Io { path, .. }
            | DataError::Json { path, .. }
            | DataError::ManifestMismatch { path, .. } => Some(path),
            _ => None,
        }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
            DataError::Json { path, source } => {
                write!(f, "could not parse {}: {source}", path.display())
            }
            DataError::FormatVersion { found, expected } => write!(
                f,
                "runtime pack format version {found} is not supported; this build reads \
                 version {expected}. Re-run `python -m psiv_tools pack` to rebuild the pack."
            ),
            DataError::DuplicateMapId { id, detail } => {
                write!(f, "manifest lists map {id} more than once: {detail}")
            }
            DataError::ManifestMismatch {
                path,
                field,
                manifest,
                record,
            } => write!(
                f,
                "{}: manifest says {field} is {manifest} but the map record says {record}",
                path.display()
            ),
            DataError::Validation {
                map,
                field,
                message,
            } => write!(f, "map {map}: {field}: {message}"),
            DataError::Sprite { who, message } => write!(f, "sprite {who}: {message}"),
            DataError::Dialogue { path, message } => {
                write!(f, "dialogue pack {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for DataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DataError::Io { source, .. } => Some(source),
            DataError::Json { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_messages_name_the_map_and_field() {
        let err = DataError::validation(MapId(0x010), "npcs[2].x", "cell 99 is outside 0..62");
        assert_eq!(
            err.to_string(),
            "map 0x010: npcs[2].x: cell 99 is outside 0..62"
        );
        assert_eq!(err.map_id(), Some(MapId(0x010)));
        assert!(err.path().is_none());
    }

    #[test]
    fn io_errors_carry_the_path() {
        let err = DataError::io(
            "runtime-pack/maps/010_Piata.json",
            std::io::Error::new(std::io::ErrorKind::NotFound, "no such file"),
        );
        assert!(err.to_string().contains("010_Piata.json"));
        assert!(err.path().is_some());
    }
}
