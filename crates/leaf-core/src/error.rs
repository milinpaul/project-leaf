//! Errors shared by every Leaf crate.

use thiserror::Error;

/// Errors from `leaf-core` itself. Other crates wrap this in their own error types rather than
/// growing it, so each crate's failures stay readable on their own.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// Text that should name a page, block or asset could not be read as one.
    ///
    /// Keeps the offending input so a corrupt file or a bad link can be reported precisely.
    #[error("invalid {kind} id {input:?}: {reason}")]
    InvalidId {
        /// Which kind of ID was expected, for example `"page"`.
        kind: &'static str,
        /// The text that failed to parse.
        input: String,
        /// Why it failed, from the underlying parser.
        reason: String,
    },
}
