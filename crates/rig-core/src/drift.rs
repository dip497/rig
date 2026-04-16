//! Drift classification + resolution modes.
//!
//! Three SHAs per installed unit: the SHA recorded at install time,
//! the SHA of the bytes currently on disk, and the SHA of the upstream
//! source at last check. Their equality matrix yields the six states
//! below; `rig sync` offers five resolution modes — none silent.

use serde::{Deserialize, Serialize};

use crate::source::Sha256;

/// Triple of SHAs describing the current state of one installed unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftShas {
    /// SHA recorded in the lockfile at install time.
    pub install_time: Sha256,
    /// SHA of the bytes currently on disk. `None` = missing.
    pub current_disk: Option<Sha256>,
    /// SHA of the upstream source at last refresh. `None` = not checked.
    pub upstream: Option<Sha256>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DriftState {
    /// install ≟ disk ✓, install ≟ upstream ✓
    Clean,
    /// disk diverged from install
    LocalDrift,
    /// upstream diverged from install
    UpstreamDrift,
    /// both diverged
    BothDrift,
    /// on disk, not in manifest
    Orphan,
    /// in manifest, not on disk
    Missing,
}

impl DriftShas {
    /// Classify drift. `Orphan` and `Missing` are set by the caller;
    /// this only handles the four "present on both sides" cases.
    #[must_use]
    pub fn classify(&self) -> DriftState {
        let local = self
            .current_disk
            .as_ref()
            .is_some_and(|d| d != &self.install_time);
        let upstream = self
            .upstream
            .as_ref()
            .is_some_and(|u| u != &self.install_time);

        match (local, upstream) {
            (false, false) => DriftState::Clean,
            (true, false) => DriftState::LocalDrift,
            (false, true) => DriftState::UpstreamDrift,
            (true, true) => DriftState::BothDrift,
        }
    }
}

/// How to resolve a non-clean drift state. Nothing touches bytes
/// silently; one of these is always chosen explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolutionMode {
    /// Leave disk untouched; bump lockfile to match current disk.
    Keep,
    /// Overwrite disk with upstream bytes.
    Overwrite,
    /// Prompt per-file when multiple files differ.
    DiffPerFile,
    /// Copy current disk to a snapshot dir, then overwrite.
    SnapshotThenOverwrite,
    /// Abort the sync without changes.
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(b: &[u8]) -> Sha256 {
        Sha256::of(b)
    }

    #[test]
    fn clean_when_all_equal() {
        let s = sha(b"x");
        let d = DriftShas {
            install_time: s.clone(),
            current_disk: Some(s.clone()),
            upstream: Some(s),
        };
        assert_eq!(d.classify(), DriftState::Clean);
    }

    #[test]
    fn local_drift() {
        let d = DriftShas {
            install_time: sha(b"a"),
            current_disk: Some(sha(b"b")),
            upstream: Some(sha(b"a")),
        };
        assert_eq!(d.classify(), DriftState::LocalDrift);
    }

    #[test]
    fn both_drift() {
        let d = DriftShas {
            install_time: sha(b"a"),
            current_disk: Some(sha(b"b")),
            upstream: Some(sha(b"c")),
        };
        assert_eq!(d.classify(), DriftState::BothDrift);
    }

    #[test]
    fn unknown_upstream_treated_as_no_drift() {
        let s = sha(b"a");
        let d = DriftShas {
            install_time: s.clone(),
            current_disk: Some(s),
            upstream: None,
        };
        assert_eq!(d.classify(), DriftState::Clean);
    }
}
