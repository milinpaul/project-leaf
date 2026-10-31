# ADR-073. CI and supply chain: read-only CI, pinned actions, deny on new duplicates and git sources

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E08-S10

## Context

The repository is public and every pull request runs CI. A security review (2026-10-31) found three gaps:

- `ci.yml` ran with GitHub's default token scope, more than any job needs.
- Third-party actions were referenced by moving tags (`@v1`, `@v2`). Whoever controls that tag decides what code
  runs in our CI; a compromised release would run without any change on our side.
- `deny.toml` only *warned* on duplicate crate versions and unknown git sources, so neither ever failed a build.

## Decision

- **Read-only CI.** `ci.yml` sets `permissions: contents: read` at the top. A job that needs more asks for it itself.
- **Pinned actions.** Every third-party action is pinned to a full commit SHA, with its version in a comment
  (`uses: actions/checkout@<sha> # v4.4.0`). Dependabot (`.github/dependabot.yml`) proposes updates weekly,
  grouped, for actions and for Cargo crates, so the pins don't go stale.
- **No git dependencies by accident.** `unknown-git = "deny"`. Everything comes from crates.io today; a git
  dependency must be added to `allow-git` with a reason.
- **No new duplicate crates.** `multiple-versions = "deny"`. GPUI's tree alone holds about 75 duplicates we cannot
  fix, so GPUI's crates are `skip-tree` entries. The two duplicates inside Loro are listed in `skip` by name.
  Each entry has a reason. Any other duplicate fails `just deny` and CI.
- **Secret scanning** and push protection are turned on in the GitHub repository settings (owner action).

## Alternatives considered

- **Keep tags, trust the publishers:** simplest, but a tag can be moved to any commit at any time.
- **Pin by exact version tag (`@v4.4.0`):** better, but tags can still be rewritten; only a SHA is immutable.
- **One `skip` entry per duplicate crate:** about 76 entries, nearly all of them GPUI's business; noisy, and it
  would need editing every time GPUI is upgraded.

## Consequences

- Updating an action means taking a Dependabot PR (or editing the SHA and its comment together), not nothing.
- A Cargo change that brings a second version of a crate fails CI. Fix it by aligning versions, or add a `skip` entry
  with a reason when the duplicate is upstream's.
- `skip-tree` on GPUI also hides a duplicate between GPUI's tree and ours. That is accepted: GPUI is upgraded rarely
  and on purpose (ADR-002).
- In plain English: CI can only read the code, it runs exactly the tool versions we chose, and adding a second copy
  of a library, or a library from outside the official registry, is caught automatically.

## Update 2026-10-31 (E08-S16): one run per main merge

`main` runs shared one concurrency group, so a burst of merges cancelled the older waiting run, and the README badge
(the newest finished `main` run) read "failing" with nothing broken. Each `main` push now gets its own group, keyed
by its commit, so every merge is tested on all three OSes and none is cancelled. Pull request runs still cancel the
run they supersede. The cost is CI time on busy days (free for a public repository), and results can queue longer.

## Update 2026-10-31 (E00-S12): still read-only

A workflow that pushed decision-record numbers to `main` was tried
([ADR-076](ADR-076-E00-S12-adr-numbers-at-merge.md)); `main`'s ruleset refuses direct pushes, so it was removed and
every job stays read-only.
