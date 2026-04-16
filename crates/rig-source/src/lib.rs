//! `rig-source` — pluggable source fetchers.
//!
//! Resolves `Source` refs (github:owner/repo@sha, git-url, npm, local path,
//! claude-marketplace:plugin@ver) into cached on-disk content that adapters
//! can then install.
