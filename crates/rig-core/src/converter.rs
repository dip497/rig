//! Per-unit-type translation between canonical [`Unit`] content and
//! agent-native bytes.
//!
//! Adapters implement one `Converter<T>` for every unit type they
//! support. This keeps the translation code separated from the
//! filesystem code in `Adapter::install`.
//!
//! [`Unit`]: crate::unit::Unit

use crate::adapter::AdapterResult;

/// Agent-native on-disk payload: a set of files the adapter will
/// write. Keyed path is relative to the unit's install directory;
/// interpretation is adapter-specific.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLayout {
    pub files: Vec<NativeFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

/// Translate canonical unit type `T` ↔ adapter-native bytes.
pub trait Converter<T> {
    /// Translate canonical → agent-native bytes.
    ///
    /// # Errors
    /// Returns [`AdapterError::Lossy`] when the canonical form has
    /// fields the target agent cannot represent.
    ///
    /// [`AdapterError::Lossy`]: crate::adapter::AdapterError::Lossy
    fn to_native(&self, canonical: &T) -> AdapterResult<NativeLayout>;

    /// Parse agent-native bytes back into the canonical form.
    ///
    /// # Errors
    /// Returns [`AdapterError::Other`] when the native payload is
    /// malformed.
    ///
    /// [`AdapterError::Other`]: crate::adapter::AdapterError::Other
    fn parse_native(&self, native: &NativeLayout) -> AdapterResult<T>;
}
