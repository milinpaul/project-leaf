//! Enforces architecture rule 1: core crates never depend on GPUI or `leaf-ui`.
//!
//! A text check on each manifest is crude but needs no extra dependencies, and it catches the
//! mistake at the moment someone adds the dependency rather than in review.

use std::fs;
use std::path::Path;

const CORE_CRATES: [&str; 8] = [
    "leaf-core",
    "leaf-model",
    "leaf-editor",
    "leaf-store",
    "leaf-export",
    "leaf-search",
    "leaf-fixtures",
    "leaf-bench",
];
const FORBIDDEN: [&str; 2] = ["gpui", "leaf-ui"];

#[test]
fn core_crates_do_not_depend_on_ui() {
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for name in CORE_CRATES {
        let manifest_path = crates_dir.join(name).join("Cargo.toml");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest_path.display()));
        for forbidden in FORBIDDEN {
            assert!(
                !manifest.contains(forbidden),
                "{name} must not depend on {forbidden} (architecture rule 1)"
            );
        }
    }
}
