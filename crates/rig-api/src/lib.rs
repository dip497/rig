//! `rig-api` — stable IPC contract between `rig-daemon` and frontends.
//!
//! Defines the JSON-RPC protocol (method names, request/response shapes,
//! error codes) that every Rig frontend speaks: official `rig-cli`,
//! `rig-gui`, and any community frontend (VSCode extension, mobile app,
//! web dashboard). Versioned for forward/backward compat.
