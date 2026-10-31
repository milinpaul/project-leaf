//! Where the app writes problems it can't show on screen yet: standard error, one line each, starting `leaf:`.

use std::fmt::Display;

/// Writes `message` to the app's log (standard error) as `leaf: message`.
///
/// The one place the app prints (E08-S15), so a log file or an on-screen notice (E06) later changes only this.
#[expect(
    clippy::print_stderr,
    reason = "standard error is the app's log until E06 gives problems a better place"
)]
pub fn log(message: impl Display) {
    eprintln!("leaf: {message}");
}
