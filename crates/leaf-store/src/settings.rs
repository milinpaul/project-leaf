//! App-wide settings: the small JSON file in the OS's settings folder that remembers how Leaf was
//! left (window size and position, sidebar width), across all workspaces.
//!
//! It is not part of any workspace, so it never travels with one. Settings are a convenience: a
//! missing, unreadable or damaged file never stops Leaf from starting; it starts with defaults
//! instead. Fields this version doesn't know (written by a newer Leaf) are kept on save. It also
//! remembers the workspaces opened recently, for reopening the last one and for the welcome screen.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{StoreError, json_file};

mod recent;

pub use recent::{MAX_RECENT_WORKSPACES, RecentWorkspace};

/// The settings file's name inside Leaf's settings folder.
pub const SETTINGS_FILE: &str = "settings.json";

/// The sidebar's width when nothing is saved, in pixels.
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;

/// The narrowest and widest the sidebar may be, so it stays readable and leaves room for the page.
pub const SIDEBAR_WIDTH_RANGE: (f32, f32) = (180.0, 480.0);

/// The smallest window Leaf restores; anything smaller was probably a glitch, not a choice.
pub const MIN_WINDOW_SIZE: (f32, f32) = (480.0, 320.0);

/// A window's place on screen, in logical pixels, as the OS reports it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowFrame {
    /// Left edge, from the left of the main display.
    pub x: f32,
    /// Top edge, from the top of the main display.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Whether the window was maximised; the frame is then the size it returns to.
    #[serde(default)]
    pub maximized: bool,
}

/// A display's area, in the same coordinates as [`WindowFrame`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayArea {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Which look Leaf uses: following the OS, or always light or dark.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// Follow the OS setting, switching when it does.
    #[default]
    System,
    /// Always the light theme.
    Light,
    /// Always the dark theme.
    Dark,
}

/// Everything Leaf remembers between launches, outside any workspace.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    /// The main window's last frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    window: Option<WindowFrame>,
    /// The sidebar's width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sidebar_width: Option<f32>,
    /// Whether the owner hid the sidebar.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    sidebar_hidden: bool,
    /// Light, dark or following the OS. An unknown value (from a newer Leaf) reads as following the OS.
    #[serde(
        default,
        skip_serializing_if = "is_system",
        deserialize_with = "appearance_or_system"
    )]
    appearance: Appearance,
    /// Workspaces opened recently, most recent first (see `recent`). Entries this version can't read
    /// are skipped rather than failing the whole file.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "recent::readable_entries"
    )]
    recent_workspaces: Vec<recent::Saved>,
    /// Whether Leaf may go online to fetch web bookmark previews (ADR-080): `None` until the owner is
    /// asked, then their answer. Off unless they said yes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    web_previews: Option<bool>,
    /// Fields from a newer Leaf, kept as they were.
    #[serde(flatten)]
    other: Map<String, Value>,
}

impl AppSettings {
    /// Reads the settings at `path`. Never fails: a missing file gives defaults, and an unreadable or
    /// damaged one is logged, moved aside to `settings.json.bad` (so it can be inspected and is not
    /// overwritten) and replaced by defaults.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        json_file::load_or_default(path, "settings")
    }

    /// Writes the settings to `path`, creating its folder if needed. Writes a temporary file and then
    /// renames it over the old one, so a crash mid-save leaves either the old or the new settings, never
    /// half a file.
    pub fn save(&self, path: &Path) -> Result<(), StoreError> {
        json_file::save(self, path)
    }

    /// The window frame to restore: the saved one if it is a sensible size and still overlaps one of
    /// `displays` (a monitor may have been unplugged), else `None` to let the window open centred.
    #[must_use]
    pub fn window_frame(&self, displays: &[DisplayArea]) -> Option<WindowFrame> {
        let frame = self.window?;
        let finite = [frame.x, frame.y, frame.width, frame.height]
            .iter()
            .all(|v| v.is_finite());
        let big_enough = frame.width >= MIN_WINDOW_SIZE.0 && frame.height >= MIN_WINDOW_SIZE.1;
        (finite && big_enough && visible_on(&frame, displays)).then_some(frame)
    }

    /// Records the window's frame.
    pub fn set_window_frame(&mut self, frame: WindowFrame) {
        self.window = Some(frame);
    }

    /// The sidebar width to use, kept within [`SIDEBAR_WIDTH_RANGE`].
    #[must_use]
    pub fn sidebar_width(&self) -> f32 {
        clamp_sidebar_width(self.sidebar_width.unwrap_or(DEFAULT_SIDEBAR_WIDTH))
    }

    /// Records the sidebar's width, kept within [`SIDEBAR_WIDTH_RANGE`].
    pub fn set_sidebar_width(&mut self, width: f32) {
        self.sidebar_width = Some(clamp_sidebar_width(width));
    }

    /// Whether the sidebar is hidden.
    #[must_use]
    pub fn sidebar_hidden(&self) -> bool {
        self.sidebar_hidden
    }

    /// Hides or shows the sidebar.
    pub fn set_sidebar_hidden(&mut self, hidden: bool) {
        self.sidebar_hidden = hidden;
    }

    /// Light, dark or following the OS.
    #[must_use]
    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    /// Chooses light, dark or following the OS.
    pub fn set_appearance(&mut self, appearance: Appearance) {
        self.appearance = appearance;
    }

    /// Whether Leaf may fetch web bookmark previews: only when the owner said yes (ADR-080).
    #[must_use]
    pub fn web_previews_allowed(&self) -> bool {
        self.web_previews == Some(true)
    }

    /// Whether the owner has answered the question about web bookmark previews, yes or no.
    #[must_use]
    pub fn web_previews_answered(&self) -> bool {
        self.web_previews.is_some()
    }

    /// Records the owner's answer: whether Leaf may fetch web bookmark previews.
    pub fn set_web_previews(&mut self, allowed: bool) {
        self.web_previews = Some(allowed);
    }
}

// serde's `skip_serializing_if` passes a reference, whatever the type's size.
#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's `skip_serializing_if` passes a reference"
)]
fn is_system(appearance: &Appearance) -> bool {
    *appearance == Appearance::System
}

/// Reads an appearance, treating any value this version doesn't know as [`Appearance::System`] rather
/// than failing the whole file.
fn appearance_or_system<'de, D: serde::Deserializer<'de>>(reader: D) -> Result<Appearance, D::Error> {
    let value = Value::deserialize(reader)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

fn clamp_sidebar_width(width: f32) -> f32 {
    let (min, max) = SIDEBAR_WIDTH_RANGE;
    if width.is_finite() {
        width.clamp(min, max)
    } else {
        DEFAULT_SIDEBAR_WIDTH
    }
}

/// Whether at least a 100 × 50 px corner of `frame`'s top edge is on one of `displays`, enough to grab
/// the window and drag it. No displays known means "trust the frame".
#[must_use]
pub fn visible_on(frame: &WindowFrame, displays: &[DisplayArea]) -> bool {
    const GRAB: (f32, f32) = (100.0, 50.0);
    displays.is_empty()
        || displays.iter().any(|d| {
            let overlap_x = (frame.x + frame.width).min(d.x + d.width) - frame.x.max(d.x);
            let overlap_y = (frame.y + GRAB.1).min(d.y + d.height) - frame.y.max(d.y);
            overlap_x >= GRAB.0 && overlap_y >= GRAB.1
        })
}

/// Where the settings file lives on `os` (as in `std::env::consts::OS`), using `env` to read
/// environment variables. `None` if the variables it needs are missing.
///
/// - macOS: `~/Library/Application Support/Leaf/settings.json`
/// - Windows: `%APPDATA%\Leaf\settings.json`
/// - Linux and others: `$XDG_CONFIG_HOME/leaf/settings.json`, else `~/.config/leaf/settings.json`
pub fn settings_path(os: &str, env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let var = |name: &str| env(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let folder = match os {
        "macos" => var("HOME")?
            .join("Library")
            .join("Application Support")
            .join("Leaf"),
        "windows" => var("APPDATA")?.join("Leaf"),
        _ => var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|home| home.join(".config")))?
            .join("leaf"),
    };
    Some(folder.join(SETTINGS_FILE))
}

/// [`settings_path`] for this machine.
#[must_use]
pub fn default_settings_path() -> Option<PathBuf> {
    settings_path(std::env::consts::OS, |name| std::env::var(name).ok())
}
