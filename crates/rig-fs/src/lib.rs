//! `rig-fs` — filesystem primitives.
//!
//! Atomic writes, symlink management, home-dir resolution, path normalization,
//! content hashing. Isolates every direct filesystem touch so higher layers
//! stay pure and testable.
