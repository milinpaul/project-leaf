# ADR-XXX. Faster tests and builds: test depth per OS, a split round trip, and slimmer debug info

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E08-S17

## Context

The owner asked to cut testing time. Measured on `main` after #199:

| | Time | Where it went |
|---|---|---|
| Windows tests | 49 min | three property tests that save real files (crash safety, image containment, search input) took 37–43 min each at `PROPTEST_CASES=10000`; on Linux each takes 1–2 min |
| Linux and macOS tests | about 6 min | the undo/redo round trip (about 5 min) |
| Local tests | about 15 s once built | compiling and linking: one core-crate edit cost 42 s before the tests could run, and `target/debug` grew to 18 GB |

Creating and syncing files is far slower on Windows runners (and virus scanning adds to it). With proptest 1.12,
`PROPTEST_CASES` overrides every test's own case count, so the long run makes all property tests do 10,000 cases.

## Decision

- **Test depth per OS.** CI's test job sets `PROPTEST_CASES` per OS: 10,000 on Linux and macOS, 500 on Windows. The
  deep search for logic bugs (which don't depend on the OS) happens on two OSes; Windows still runs every property
  test, enough to catch Windows file and path behaviour.
- **The undo/redo round trip runs as four tests side by side**, each drawing its own random sequences for a quarter of
  the cases (`PROPTEST_CASES / 4`, or 128 / 4 by default). The same number of cases runs in about a quarter of the
  wall time. It uses proptest's `TestRunner` directly, since the macro would let `PROPTEST_CASES` override the share.
- **Development and test builds keep only line tables** (`[profile.dev] debug = "line-tables-only"`, dependencies
  none): panics and backtraces still give file and line. Rebuilding every test after a `leaf-model` edit went from
  42 s to 25 s here, and `target/debug` from 18 GB to 3.1 GB. A debugger session can turn full debug info back on
  with `CARGO_PROFILE_DEV_DEBUG=full`.
- **Linker:** no change. Linux already links with `lld` by default (Rust 1.90 and later), and macOS uses Apple's fast
  linker.

## Alternatives considered

- **Fewer cases everywhere:** shortens Linux and macOS too, but weakens the deep search that has found real bugs
  (E17-S11 came from the convergence test).
- **Skipping the file-heavy tests on Windows:** saves the most, but Windows' own file behaviour is exactly what they
  check there.
- **A build cache such as sccache:** helps repeat cold builds, but CI already caches with `rust-cache` and local
  builds are mostly incremental; more moving parts for little gain now.

## Consequences

- Windows tests should take about 10 minutes instead of 49, and the Linux and macOS runs lose about 3–4 minutes.
- A debugger shows lines but not local variables unless full debug info is turned back on.
- Plain English: the long test run on Windows now tries fewer random cases (Linux and macOS still try 10,000), the
  slowest test is split into four that run at once, and builds skip debug detail nobody uses, so they link faster
  and take a sixth of the disk space.
