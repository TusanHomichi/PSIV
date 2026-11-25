//! Version compatibility policy for `mod.json` and runtime pack formats (#790).
//!
//! Distinguishes additive pack-format bumps (which add optional fields or new
//! tables without breaking existing patches) from breaking changes (which
//! rename/remove fields or change keyed-table layouts):
//!
//! - On an **additive** pack-format bump, [`PACK_FORMAT_VERSION`] increments
//!   while [`MIN_SUPPORTED_PACK_FORMAT_VERSION`] stays pinned at the earliest
//!   compatible version. Existing mods declaring an older version in
//!   `MIN_SUPPORTED_PACK_FORMAT_VERSION..=MAX_SUPPORTED_PACK_FORMAT_VERSION`
//!   continue to load cleanly while [`super::PackOverlay::load`] still validates every patched
//!   table fail-closed.
//! - On a **breaking** pack-format bump, both [`PACK_FORMAT_VERSION`] and
//!   [`MIN_SUPPORTED_PACK_FORMAT_VERSION`] advance to the new version so mods
//!   targeting the pre-break shape fail closed with an actionable upgrade
//!   diagnostic.
//! - The same rule governs `mod.json`'s own `format_version` via
//!   [`MOD_FORMAT_VERSION`] and [`MIN_SUPPORTED_MOD_FORMAT_VERSION`].

use crate::manifest::PACK_FORMAT_VERSION;

/// Current `mod.json` schema version emitted by `ModManifest::new` and
/// `psiv-pack mod init`.
pub const MOD_FORMAT_VERSION: u32 = 1;

/// Oldest `mod.json` `format_version` accepted by this build.
pub const MIN_SUPPORTED_MOD_FORMAT_VERSION: u32 = 1;

/// Newest `mod.json` `format_version` accepted by this build.
pub const MAX_SUPPORTED_MOD_FORMAT_VERSION: u32 = MOD_FORMAT_VERSION;

/// Oldest `pack_format_version` compatible with this build's pack tables.
///
/// Remains pinned across additive `PACK_FORMAT_VERSION` bumps and advances
/// only when a pack-format change breaks existing mod patches.
pub const MIN_SUPPORTED_PACK_FORMAT_VERSION: u32 = 2;

/// Newest `pack_format_version` accepted by this build.
pub const MAX_SUPPORTED_PACK_FORMAT_VERSION: u32 = PACK_FORMAT_VERSION;

const _: () = {
    assert!(MIN_SUPPORTED_MOD_FORMAT_VERSION >= 1);
    assert!(MIN_SUPPORTED_MOD_FORMAT_VERSION <= MOD_FORMAT_VERSION);
    assert!(MOD_FORMAT_VERSION <= MAX_SUPPORTED_MOD_FORMAT_VERSION);
    assert!(MIN_SUPPORTED_PACK_FORMAT_VERSION >= 1);
    assert!(MIN_SUPPORTED_PACK_FORMAT_VERSION <= PACK_FORMAT_VERSION);
    assert!(PACK_FORMAT_VERSION <= MAX_SUPPORTED_PACK_FORMAT_VERSION);
};

/// Inclusive supported version window and current default version for a format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionCompatibilityPolicy {
    /// Oldest compatible version accepted without breaking changes.
    pub min: u32,
    /// Current version emitted by newly generated manifests/packs.
    pub current: u32,
    /// Newest version this build understands.
    pub max: u32,
}

impl VersionCompatibilityPolicy {
    /// Returns `true` when `declared` falls inside `self.min..=self.max`.
    #[must_use]
    pub const fn accepts(self, declared: u32) -> bool {
        declared >= self.min && declared <= self.max
    }

    /// Checks whether `declared` is compatible or reports whether it is too old
    /// or too new.
    pub const fn classify(self, declared: u32) -> Result<(), VersionMismatchKind> {
        if declared < self.min {
            Err(VersionMismatchKind::TooOld)
        } else if declared > self.max {
            Err(VersionMismatchKind::TooNew)
        } else {
            Ok(())
        }
    }

    /// Formats the inclusive supported range (`"min..=max"`).
    #[must_use]
    pub fn format_range(self) -> String {
        format!("{}..={}", self.min, self.max)
    }
}

/// Active compatibility window for `mod.json`'s `format_version`.
pub const MOD_FORMAT_COMPATIBILITY: VersionCompatibilityPolicy = VersionCompatibilityPolicy {
    min: MIN_SUPPORTED_MOD_FORMAT_VERSION,
    current: MOD_FORMAT_VERSION,
    max: MAX_SUPPORTED_MOD_FORMAT_VERSION,
};

/// Active compatibility window for `mod.json`'s `pack_format_version`.
pub const PACK_FORMAT_COMPATIBILITY: VersionCompatibilityPolicy = VersionCompatibilityPolicy {
    min: MIN_SUPPORTED_PACK_FORMAT_VERSION,
    current: PACK_FORMAT_VERSION,
    max: MAX_SUPPORTED_PACK_FORMAT_VERSION,
};

/// Which version field of `mod.json` is being checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionKind {
    /// `mod.json`'s own `format_version`.
    ModFormat,
    /// `mod.json`'s target `pack_format_version`.
    PackFormat,
}

/// Direction of an incompatible version declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionMismatchKind {
    /// Declared version predates the minimum compatible version.
    TooOld,
    /// Declared version exceeds the maximum version this build understands.
    TooNew,
}

/// Validates `declared` against `policy` and returns an actionable diagnostic
/// naming the declared version, supported range, current version, and upgrade
/// path when incompatible.
pub fn check_version_compatibility(
    kind: VersionKind,
    declared: u32,
    policy: VersionCompatibilityPolicy,
) -> Result<(), String> {
    let Err(mismatch) = policy.classify(declared) else {
        return Ok(());
    };
    let range = policy.format_range();
    let current = policy.current;
    let message = match (kind, mismatch) {
        (VersionKind::ModFormat, VersionMismatchKind::TooOld) => format!(
            "unsupported mod format_version {declared} (supported range {range}, current {current}): \
             update mod.json to format_version {current} and re-validate with `psiv-pack mod check`"
        ),
        (VersionKind::ModFormat, VersionMismatchKind::TooNew) => format!(
            "unsupported mod format_version {declared} (supported range {range}, current {current}): \
             update PSIV to a newer build, or target format_version {current} in mod.json"
        ),
        (VersionKind::PackFormat, VersionMismatchKind::TooOld) => format!(
            "targets pack format_version {declared}, which predates the compatible range {range} \
             (current {current}): breaking pack changes require updating this mod's patches for \
             pack format {current} and running `psiv-pack mod check`"
        ),
        (VersionKind::PackFormat, VersionMismatchKind::TooNew) => format!(
            "targets pack format_version {declared}, which exceeds the compatible range {range} \
             (current {current}): update PSIV and rebuild your runtime pack, or lower \
             `pack_format_version` to {current} if this mod uses no newer pack fields"
        ),
    };
    Err(message)
}
