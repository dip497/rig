# Governance

Rig is under active design. This document describes how decisions are
made today, and how that will evolve.

## Current (pre-1.0)

- **BDFL-led.** Dipendra Sharma is the benevolent dictator for life —
  through 1.0. Architectural decisions, roadmap, releases, and merge
  authority all rest there.
- **Issue-driven RFCs.** Larger proposals (a new unit type, a new seam,
  a breaking change) require an issue labelled `rfc:` with:
  - the problem being solved,
  - the proposed mechanism,
  - alternatives considered,
  - a migration story if applicable.
  Comment period: one week minimum before merge of implementation PRs.
- **PRs welcome, issue-first for non-trivial changes.** Small bug
  fixes and documentation improvements go straight to PR. Anything
  that changes public APIs or adds new concepts needs an issue first.
- **Releases.** Semantic versioning. Every tagged release has a
  CHANGELOG entry and passes CI. No yanks unless there is a severe
  security issue.

## Post-1.0 (indicative, subject to revision)

- **Invite 2–3 co-maintainers** when the project reaches ~100 stars or
  has clear evidence of adoption. Chosen based on sustained
  contribution quality, not volume.
- **Maintainer council.** Core decisions require a majority of
  co-maintainers; the BDFL retains veto on direction-setting items
  only.
- **RFC repo.** If the project outgrows issue-based RFCs, move to a
  dedicated `rig-rfcs` repo with a formal template.

## Long-term (moonshot)

- **Foundation.** When five or more unrelated organisations depend on
  Rig, move spec documents (plugin protocol, manifest schema) to a
  neutral foundation (Apache, Linux Foundation AAIF, or independent
  501(c)(6)). The commercial entity (`rig-cloud`, if it exists)
  steps back from spec authority; the project becomes the standard
  and Rig becomes one of multiple implementations.

## Commercial entity

Rig is OSS first. There is no commercial entity today. If / when a
paid SaaS (`rig-cloud`) is introduced:

- Core crates, CLI, GUI, official adapters remain **OSS forever**,
  dual MIT / Apache-2.0.
- Paid offerings are net-new features, separate repository, separate
  licence. No rug-pulls of existing OSS surface.
- Contributor Licence Agreement (CLA) is introduced only if a
  commercial entity requires it for legal reasons, and is announced
  publicly before taking effect.

## Code of conduct

See [`CODE_OF_CONDUCT.md`](../CODE_OF_CONDUCT.md). Enforcement is the
BDFL's call today; a maintainer council takes it over post-1.0.
