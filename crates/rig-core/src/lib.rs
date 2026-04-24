//! `rig-core` — canonical unit types, traits, manifest schema.
//!
//! Pure data model shared by every adapter, source, frontend, and
//! plugin in Rig. **Zero I/O.** No `std::fs`, no network, no process
//! spawning. All side effects happen in sibling crates.
//!
//! Modules:
//! - [`agent`] — opaque agent id
//! - [`source`] — source strings + content-addressed [`Sha256`]
//! - [`scope`] — global vs project
//! - [`unit`] — canonical structs for the seven M1 unit types
//! - [`manifest`] — `rig.toml` parser
//! - [`lockfile`] — `rig.lock` schema
//! - [`drift`] — SHA tracking + state machine
//! - [`adapter`] — [`Adapter`] trait contract
//! - [`converter`] — per-type [`Converter`] trait
//!
//! Resolver + bundle expansion arrive when a caller needs them;
//! per [`docs/DECISIONS.md`](../../docs/DECISIONS.md) ADR-015 we
//! direct-execute and backfill specs as the API solidifies.
//!
//! [`Sha256`]: source::Sha256
//! [`Adapter`]: adapter::Adapter
//! [`Converter`]: converter::Converter

#![forbid(unsafe_code)]

pub mod adapter;
pub mod agent;
pub mod converter;
pub mod drift;
pub mod lockfile;
pub mod manifest;
pub mod scope;
pub mod source;
pub mod unit;

pub use agent::AgentId;
pub use scope::Scope;
pub use source::{Sha256, Source};
pub use unit::{Unit, UnitType};
