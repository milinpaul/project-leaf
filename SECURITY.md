# Security policy

## Supported versions

Leaf is pre-alpha and has no releases yet. Security fixes go into `main`.

## Reporting a vulnerability

**Please don't open a public issue for security problems.** Report them privately through GitHub's
[private vulnerability reporting](https://github.com/milinpaul/project-leaf/security/advisories/new) instead.

Include what you found, the steps to reproduce it, and what an attacker could do with it. You'll get an answer
within a week. Once a fix is merged, we'll credit you in the advisory unless you'd rather stay anonymous.

## Scope

Leaf is a local, offline app. Its only network requests fetch web bookmark previews (a page's title, description and
pictures), and only after you allow it, within the limits in ADR-080: public addresses only, small and quick
downloads, no scripts. That fetcher (`crates/leaf-fetch`) is in scope too. The most relevant problems are files
that crash Leaf or make it lose data when opened (a crafted workspace, image or pasted content), and anything
that lets Leaf read or write outside the workspace folder you chose.

## How we protect the code

CI only has read access to the repository, and every GitHub Action it uses is pinned to an exact commit.
`cargo deny` checks every dependency for known vulnerabilities, licences, its source (crates.io only) and duplicate
versions, and Dependabot proposes updates weekly. See ADR-073.
