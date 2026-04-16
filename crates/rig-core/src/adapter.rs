//! `Adapter` trait — the contract every agent host implements.
//!
//! Adapters live in their own crates (`rig-adapter-claude`, …) and
//! perform the actual filesystem work. `rig-core` only declares the
//! shape; it never calls these methods itself.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::agent::AgentId;
use crate::drift::{DriftShas, DriftState};
use crate::scope::Scope;
use crate::source::Sha256;
use crate::unit::{Unit, UnitType};

/// Stable reference to an installed unit: `(type, name)` within a scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnitRef {
    pub unit_type: UnitType,
    pub name: String,
}

impl UnitRef {
    #[must_use]
    pub fn new(unit_type: UnitType, name: impl Into<String>) -> Self {
        Self {
            unit_type,
            name: name.into(),
        }
    }
}

/// What an adapter did on a successful install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub unit_ref: UnitRef,
    pub agent: AgentId,
    pub scope: Scope,
    /// Paths written (absolute). Used for uninstall and drift detection.
    pub paths: Vec<PathBuf>,
    /// SHA of the canonical unit bytes at install time.
    pub install_sha: Sha256,
}

/// One unit currently installed on disk as seen by the adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledUnit {
    pub unit_ref: UnitRef,
    pub scope: Scope,
    pub paths: Vec<PathBuf>,
}

/// Errors an adapter may raise. `rig-core` owns the error type so the
/// resolver can handle them uniformly; the I/O cause lives in `source`.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("unit type `{0:?}` is not supported by this adapter")]
    Unsupported(UnitType),
    #[error("unit `{0}` not found in scope `{1}`")]
    NotFound(String, Scope),
    #[error("lossy conversion for `{unit}`: {reason}")]
    Lossy { unit: String, reason: String },
    #[error("{message}")]
    Other {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },
}

pub type AdapterResult<T> = Result<T, AdapterError>;

/// Host-level contract. One impl per agent.
pub trait Adapter: Send + Sync {
    fn agent(&self) -> AgentId;
    fn capabilities(&self) -> BTreeSet<UnitType>;

    /// Write a unit into the agent's native layout.
    ///
    /// # Errors
    /// [`AdapterError::Unsupported`] if the unit type is outside
    /// [`capabilities`], [`AdapterError::Lossy`] on translation
    /// loss, [`AdapterError::Other`] on I/O failure.
    ///
    /// [`AdapterError::Unsupported`]: AdapterError::Unsupported
    /// [`AdapterError::Lossy`]: AdapterError::Lossy
    /// [`AdapterError::Other`]: AdapterError::Other
    /// [`capabilities`]: Self::capabilities
    fn install(&self, unit: &Unit, scope: Scope) -> AdapterResult<Receipt>;

    /// Remove a previously installed unit. Idempotent.
    ///
    /// # Errors
    /// [`AdapterError::Other`] on I/O failure. Never errors if the
    /// unit is already absent.
    ///
    /// [`AdapterError::Other`]: AdapterError::Other
    fn uninstall(&self, unit_ref: &UnitRef, scope: Scope) -> AdapterResult<()>;

    /// Enumerate what Rig currently has installed (Rig-managed only).
    ///
    /// # Errors
    /// [`AdapterError::Other`] on I/O failure.
    ///
    /// [`AdapterError::Other`]: AdapterError::Other
    fn list(&self, scope: Scope) -> AdapterResult<Vec<InstalledUnit>>;

    /// Read the unit back from disk as a canonical [`Unit`].
    ///
    /// # Errors
    /// [`AdapterError::NotFound`] when the unit is absent, or
    /// [`AdapterError::Other`] on I/O / parse failure.
    ///
    /// [`AdapterError::NotFound`]: AdapterError::NotFound
    /// [`AdapterError::Other`]: AdapterError::Other
    fn read_local(&self, unit_ref: &UnitRef, scope: Scope) -> AdapterResult<Unit>;

    /// Compute the current-disk SHA and return drift classification.
    /// `install_time` and `upstream` come from the lockfile / source
    /// check; adapter only supplies `current_disk`.
    ///
    /// # Errors
    /// [`AdapterError::Other`] on I/O failure reading the on-disk bytes.
    ///
    /// [`AdapterError::Other`]: AdapterError::Other
    fn detect_drift(
        &self,
        unit_ref: &UnitRef,
        scope: Scope,
        install_time: Sha256,
        upstream: Option<Sha256>,
    ) -> AdapterResult<(DriftState, DriftShas)>;
}
