//! The app settings while Leaf runs: loaded once at startup, changed in memory as the owner resizes
//! things, and written back on blur and quit (not on every pixel of a drag).
//!
//! The file format and its rules live in `leaf_store::settings`, where they are tested.

use std::path::PathBuf;

use gpui::{App, Global};
use leaf_store::AppSettings;

/// The loaded settings, the file they came from, and whether they changed since the last save.
#[derive(Default)]
struct Loaded {
    path: Option<PathBuf>,
    settings: AppSettings,
    dirty: bool,
}

impl Global for Loaded {}

/// Loads the settings from `path` (or starts with defaults if `None`, as tests do) and saves them when
/// the app quits. Call once at startup, before the theme and the first window.
pub fn init(path: Option<PathBuf>, cx: &mut App) {
    let settings = path.as_deref().map(AppSettings::load).unwrap_or_default();
    cx.set_global(Loaded {
        path,
        settings,
        dirty: false,
    });
    cx.on_app_quit(|cx| {
        save(cx);
        async {}
    })
    .detach();
}

/// The current settings; defaults if [`init`] wasn't called.
#[must_use]
pub fn get(cx: &App) -> AppSettings {
    cx.try_global::<Loaded>()
        .map(|loaded| loaded.settings.clone())
        .unwrap_or_default()
}

/// Changes the settings in memory; they are written on the next [`save`].
pub fn update(cx: &mut App, change: impl FnOnce(&mut AppSettings)) {
    if !cx.has_global::<Loaded>() {
        cx.set_global(Loaded::default());
    }
    let loaded = cx.global_mut::<Loaded>();
    let before = loaded.settings.clone();
    change(&mut loaded.settings);
    loaded.dirty |= loaded.settings != before;
}

/// Calls `changed` on `entity` whenever the settings change (E15-S06: previews turned on or off).
pub fn observe<T: 'static>(
    cx: &mut gpui::Context<T>,
    changed: impl Fn(&mut T, &mut gpui::Context<T>) + 'static,
) -> gpui::Subscription {
    if !cx.has_global::<Loaded>() {
        cx.set_global(Loaded::default());
    }
    cx.observe_global::<Loaded>(changed)
}

/// Writes the settings if they changed. A failure is logged: losing a window size is not worth
/// interrupting the owner for, and the next save tries again.
pub fn save(cx: &mut App) {
    let Some(loaded) = cx.try_global::<Loaded>() else {
        return;
    };
    let (Some(path), true) = (loaded.path.clone(), loaded.dirty) else {
        return;
    };
    match loaded.settings.save(&path) {
        Ok(()) => cx.global_mut::<Loaded>().dirty = false,
        Err(err) => crate::log::log(format_args!(
            "could not save the settings to {}: {err}",
            path.display()
        )),
    }
}
