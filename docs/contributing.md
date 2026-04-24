# Contributing

Rig is in early design. Most useful contributions right now are **ideas**
and **review of the public docs**, not code.

## Ways to help today

1. Open an issue if the docs in `/docs` are unclear, wrong, or missing
   something. Short, specific, and linked to the paragraph are best.
2. Point out prior art we should interop with. See
   [comparison.md](./comparison.md) for what's already on our radar.
3. Try the architecture against a real agent setup you use and tell us
   which concepts break down.
4. Open a `rfc:`-labeled issue proposing a concrete mechanic (e.g., how
   Rig should handle a particular adapter's hook semantics). Small and
   specific wins.

## Code contributions

Rig is in pre-scaffold transition. Until M1 ships, large PRs to the new
workspace are unlikely to merge cleanly — the APIs are moving. Helpful
code contributions before M1 are:

- CI improvements (Linux + macOS matrix).
- Test fixtures for adapters (real `~/.claude/` / `~/.codex/` layouts
  we can snapshot).
- Review of `rig-core` trait signatures once they land as issues.

After M1 the bar drops. New adapters, unit types, source backends, and
bundles will all be welcome as PRs.

## Setup

```sh
git clone https://github.com/dipendra-sharma/rig
cd rig
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The workspace excludes `crates/rig-legacy/` from the default build. If
you need it for reference: `cargo build -p rig-legacy`.

## Pull request etiquette

- One logical change per PR.
- Title in imperative mood: "Add Codex MCP converter", not "Added…"
- Link the issue the PR closes (if any).
- Update `CHANGELOG.md` for user-visible changes.
- Run `cargo fmt`, `cargo clippy --workspace -- -D warnings`, and
  `cargo test --workspace` locally. CI will also enforce.
- Expect review. Most PRs go through at least one round of feedback.

## Governance

Rig is BDFL-led (Dipendra Sharma) through pre-1.0. Larger proposals go
through an `rfc:`-labeled issue before implementation. When the project
has 100+ stars or a critical mass of adopters, co-maintainers will be
invited. See [governance.md](./governance.md).

## Code of conduct

Be kind, be honest, be specific. Full [Code of Conduct](../CODE_OF_CONDUCT.md).
