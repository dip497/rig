//! `rig-sync` — drift engine + reconciliation.
//!
//! Compares three SHAs per unit (install-time, current-disk, upstream-source)
//! to classify state (Clean / LocalDrift / UpstreamDrift / BothDrift / Orphan /
//! Missing) and resolve via user-selected mode (keep / overwrite / diff-per-file
//! / snapshot-then-overwrite / cancel). See `docs/DRIFT.md`.
