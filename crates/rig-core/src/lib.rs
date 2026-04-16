//! `rig-core` — canonical unit types, traits, manifest schema, and resolver.
//!
//! This crate defines the portable data model shared by every adapter, source,
//! frontend, and plugin in Rig. It performs **no** filesystem or network access.
//! All I/O happens in sibling crates (`rig-fs`, `rig-source`, adapters).
//!
//! Subsystems (to be populated — see `docs/ARCHITECTURE.md`):
//! - `unit` — per-unit-type canonical structs (skill, mcp, rule, hook, command, subagent, plugin)
//! - `bundle` — composition model
//! - `manifest` — `rig.toml` parser
//! - `lockfile` — `rig.lock` format
//! - `adapter` — `Adapter` trait contract
//! - `converter` — `Converter<A: Agent>` trait
//! - `resolver` — bundle/dependency resolution
//! - `drift` — SHA tracking + state machine
