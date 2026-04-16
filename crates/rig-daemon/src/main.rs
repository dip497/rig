//! `rig-daemon` — optional always-on process that serves `rig-api`.
//!
//! Launched on demand when multiple frontends need to share state (e.g.,
//! GUI + CLI simultaneously). Single-frontend usage skips the daemon and
//! links `rig-core` directly.

fn main() -> anyhow::Result<()> {
    // TODO: initialize tracing, bind unix socket, serve rig-api JSON-RPC.
    Ok(())
}
