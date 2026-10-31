//! A file dropped or pasted onto a page: told apart by its first bytes, an SVG checked before it may be
//! drawn, and the file stored in a workspace's `assets/` folder and checked again from there.
#![no_main]

use std::sync::Mutex;

use leaf_fuzz::scratch;
use libfuzzer_sys::fuzz_target;

/// One workspace for the whole run, emptied every so often so stored files don't fill the disk.
static WORKSPACE: Mutex<Option<(leaf_store::Workspace, u32)>> = Mutex::new(None);
/// Runs between emptying the workspace.
const FRESH_EVERY: u32 = 2_000;

fuzz_target!(|data: &[u8]| {
    leaf_fuzz::allow_caught_panics();
    let _ = leaf_store::image_mime(data);
    let _ = leaf_store::check_svg(data);
    let Ok(mut guard) = WORKSPACE.lock() else {
        return;
    };
    if guard.as_ref().is_none_or(|(_, runs)| *runs >= FRESH_EVERY) {
        *guard = None; // drops the old workspace (and its lock) before its folder is emptied
        let Ok(fresh) = leaf_store::Workspace::create(&scratch("assets.leaf")) else {
            return;
        };
        *guard = Some((fresh, 0));
    }
    let Some((workspace, runs)) = guard.as_mut() else {
        return;
    };
    *runs += 1;
    if let Ok(asset) = workspace.import_asset(data, 0) {
        let _ = workspace.asset(asset.id);
        let _ = workspace.drawable_asset(asset.id);
    }
});
