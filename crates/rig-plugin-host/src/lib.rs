//! `rig-plugin-host` — subprocess IPC for external plugins.
//!
//! Spawns external adapter / unit-type / command binaries, speaks a stable
//! versioned JSON protocol over stdin/stdout. Lets 3rd parties extend Rig
//! without touching core. See `docs/PLUGIN-PROTOCOL.md`.
