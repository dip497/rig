# Security Policy

## Supported versions

Rig is pre-1.0. The `main` branch is the only supported line today.
Security fixes will ship as patch releases against the latest tagged
version.

Once 1.0 lands, the latest minor release plus the previous minor
release will both receive security backports.

## Reporting a vulnerability

If you believe you have found a security issue in Rig or any
first-party crate, please **do not** open a public GitHub issue.

Instead, email the maintainer at `utsav.itsm@gmail.com` with:

- a description of the vulnerability,
- steps to reproduce (ideally a minimal test case),
- the affected version / commit hash,
- any relevant logs or context,
- your preferred disclosure timeline and credit preference.

We will:

- acknowledge receipt within **48 hours**,
- confirm or contest the vulnerability within **7 days**,
- patch high-severity issues within **14 days**, medium-severity
  within **30 days**, low-severity at the next release,
- publish a GitHub Security Advisory crediting you (unless you
  prefer otherwise) and linking to the patch.

## Threat model

See [docs/security.md](./docs/security.md) for the current threat
model — what is in scope today, what is explicitly deferred to M2 /
M3, and your responsibilities as a user (reviewing lock diffs,
trusting plugin binaries, inspecting third-party bundles).

## Coordinated disclosure

If a vulnerability requires coordinated disclosure with other
projects (for example, a shared issue with an upstream adapter target
or plugin author), we will coordinate privately before publication.
