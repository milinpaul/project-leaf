//! The Leaf desktop application: windows, menus and wiring between storage, editor and UI.
//!
//! This is the only binary. It assembles the other crates and handles platform glue, and holds
//! no logic of its own that could live in a testable crate instead.
//!
//! **Boundaries:** UI crate, excluded from the coverage gate. Nothing depends on it.

fn main() {
    // The first window arrives with the GPUI spike (E00-S04).
}
