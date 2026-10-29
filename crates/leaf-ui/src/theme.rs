//! Leaf's look in one place: colours, fonts, sizes and shapes for light and dark (ADR 0027).
//!
//! The direction came from the owner: warm and friendly, on warm "paper" neutrals, with the app icon's
//! green used sparingly (selection, caret, links, focus, the main button) and a soft charcoal dark mode.
//! Friendliness comes from shape and softness (rounded corners, gentle tints, roomy spacing), not from
//! more colour; that restraint is what makes an app look calm and premium.
//!
//! Everything else in the UI reads these values through gpui-component's theme (`cx.theme()`) or the
//! constants below, so changing the look means changing this file only.

use std::borrow::Cow;
use std::rc::Rc;

use gpui::{App, Subscription, Window};
use gpui_component::theme::{Theme, ThemeConfig, ThemeConfigColors, ThemeMode};

/// The one typeface for everything (interface, text, titles and headings), bundled so Leaf looks the
/// same on every OS. One family keeps the page calm; hierarchy comes from size and weight.
pub const UI_FONT: &str = "Inter";

/// Body text size in pixels. Comfortable for long writing, Phase 0's goal.
pub const BODY_SIZE: f32 = 16.0;

/// Body line height as a multiple of the text size.
pub const LINE_HEIGHT: f32 = 1.6;

/// Page title, then heading 1 to 3, in pixels.
pub const TITLE_SIZE: f32 = 32.0;
/// Heading sizes, largest first: heading 1, 2 and 3.
pub const HEADING_SIZES: [f32; 3] = [26.0, 21.0, 18.0];

/// Corner radius of buttons, menus, selected rows and code blocks: soft but crisp.
pub const RADIUS: usize = 6;
/// Corner radius of larger surfaces such as dialogs and popovers.
pub const RADIUS_LG: usize = 8;

/// Widest the text column gets, in pixels; longer lines are tiring to read.
pub const READING_WIDTH: f32 = 720.0;

/// One appearance's colours, as `#RRGGBB`. Named by role, not by hue, so a screen asks for "muted
/// text" and both modes answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// The page the owner writes on.
    pub page: &'static str,
    /// The sidebar, a shade away from the page so the two read as separate surfaces without a hard line.
    pub sidebar: &'static str,
    /// Background under the pointer on rows and ghost buttons.
    pub hover: &'static str,
    /// Background of the selected sidebar row and pressed buttons.
    pub active: &'static str,
    /// Dividers and outlines; kept faint.
    pub border: &'static str,
    /// Main text: warm near-black (light) or warm off-white (dark), never pure.
    pub text: &'static str,
    /// Secondary text: labels, hints, done to-dos.
    pub muted: &'static str,
    /// Placeholder text, the faintest readable tone.
    pub faint: &'static str,
    /// Leaf green for links, focus and the main button.
    pub accent: &'static str,
    /// Accent under the pointer.
    pub accent_hover: &'static str,
    /// The text caret.
    pub caret: &'static str,
    /// Background of selected text.
    pub selection: &'static str,
    /// Text on an accent background (the main button).
    pub on_accent: &'static str,
    /// Background of code blocks and inline code.
    pub code: &'static str,
    /// A faint green wash, for gentle emphasis such as a focused row.
    pub accent_soft: &'static str,
}

/// Warm paper. The accent is the icon's #16A970, deepened so text in it meets WCAG AA on the page.
pub const LIGHT: Palette = Palette {
    page: "#FCFBF8",
    sidebar: "#F6F4EF",
    hover: "#EEEBE4",
    active: "#E8E4DB",
    border: "#E6E2D9",
    text: "#2A2926",
    muted: "#6E6A62",
    faint: "#9A958B",
    accent: "#0B7A50",
    accent_hover: "#09693F",
    caret: "#0F8A5B",
    selection: "#CDEEDD",
    on_accent: "#FFFFFF",
    code: "#F3F1EC",
    accent_soft: "#E3F4EB",
};

/// Soft charcoal, warm like the light mode, with the green brightened to stay readable.
pub const DARK: Palette = Palette {
    page: "#1C1B19",
    sidebar: "#222120",
    hover: "#2B2A27",
    active: "#33312D",
    border: "#363430",
    text: "#ECE9E3",
    muted: "#A8A399",
    faint: "#77736B",
    accent: "#45D199",
    accent_hover: "#5EDBA8",
    caret: "#45D199",
    selection: "#1F4A38",
    on_accent: "#0F1D16",
    code: "#252422",
    accent_soft: "#1E3A2E",
};

/// The bundled font files, embedded so Leaf needs nothing installed. Inter is under the SIL Open Font
/// License; the licence sits next to the files.
const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Italic.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Inter-Bold.ttf"),
    include_bytes!("../assets/fonts/Inter-BoldItalic.ttf"),
];

/// Loads the fonts and installs Leaf's light and dark themes, following the system appearance (or
/// `LEAF_APPEARANCE=light|dark`, used to take screenshots of either). Call once at startup, after
/// gpui-component is set up.
pub fn init(cx: &mut App) {
    let fonts = FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect();
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        // The system font stands in; the app still works, only the look is off.
        eprintln!("leaf: could not load the bundled fonts: {err:#}");
    }
    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new(config(ThemeMode::Light, &LIGHT));
    theme.dark_theme = Rc::new(config(ThemeMode::Dark, &DARK));
    apply(None, cx);
}

/// Keeps `window` in step with the system: when the OS switches between light and dark, so does Leaf.
/// Keep the subscription for as long as the window lives.
pub fn follow_system(window: &Window) -> Subscription {
    window.observe_window_appearance(|window, cx| apply(Some(window), cx))
}

/// Switches to the forced mode, or the window's (or system's) appearance.
fn apply(window: Option<&mut Window>, cx: &mut App) {
    match forced_mode() {
        Some(mode) => Theme::change(mode, window, cx),
        None => Theme::sync_system_appearance(window, cx),
    }
}

/// The mode `LEAF_APPEARANCE` asks for, if any.
fn forced_mode() -> Option<ThemeMode> {
    mode_named(&std::env::var("LEAF_APPEARANCE").ok()?)
}

fn mode_named(name: &str) -> Option<ThemeMode> {
    match name.trim().to_ascii_lowercase().as_str() {
        "light" => Some(ThemeMode::Light),
        "dark" => Some(ThemeMode::Dark),
        _ => None,
    }
}

/// gpui-component's theme for one appearance, built from Leaf's tokens. Roles gpui-component has but
/// Leaf doesn't use yet (charts, tables, tabs, ...) keep its defaults.
fn config(mode: ThemeMode, p: &Palette) -> ThemeConfig {
    let c = |hex: &'static str| Some(hex.into());
    // Some of the struct's fields are private, so it can't be written as one literal.
    let mut colors = ThemeConfigColors::default();
    colors.background = c(p.page);
    colors.foreground = c(p.text);
    colors.border = c(p.border);
    colors.input = c(p.border);
    colors.ring = c(p.accent);
    colors.caret = c(p.caret);
    colors.selection = c(p.selection);
    colors.muted = c(p.code);
    colors.muted_foreground = c(p.muted);
    colors.accent = c(p.hover);
    colors.accent_foreground = c(p.text);
    colors.primary = c(p.accent);
    colors.primary_hover = c(p.accent_hover);
    colors.primary_active = c(p.accent_hover);
    colors.primary_foreground = c(p.on_accent);
    colors.secondary = c(p.hover);
    colors.secondary_hover = c(p.active);
    colors.secondary_active = c(p.active);
    colors.secondary_foreground = c(p.text);
    colors.link = c(p.accent);
    colors.link_hover = c(p.accent_hover);
    colors.link_active = c(p.accent_hover);
    colors.popover = c(p.page);
    colors.popover_foreground = c(p.text);
    colors.list = c(p.page);
    colors.list_hover = c(p.hover);
    colors.list_active = c(p.active);
    colors.list_active_border = c(p.active);
    colors.sidebar = c(p.sidebar);
    colors.sidebar_foreground = c(p.text);
    colors.sidebar_border = c(p.border);
    colors.sidebar_accent = c(p.active);
    colors.sidebar_accent_foreground = c(p.text);
    colors.sidebar_primary = c(p.accent);
    colors.sidebar_primary_foreground = c(p.on_accent);
    colors.title_bar = c(p.sidebar);
    colors.title_bar_border = c(p.border);
    colors.window_border = c(p.border);
    ThemeConfig {
        name: if mode == ThemeMode::Dark {
            "Leaf Dark"
        } else {
            "Leaf Light"
        }
        .into(),
        mode,
        font_size: Some(BODY_SIZE),
        font_family: Some(UI_FONT.into()),
        radius: Some(RADIUS),
        radius_lg: Some(RADIUS_LG),
        shadow: Some(false),
        colors,
        ..ThemeConfig::default()
    }
}

#[cfg(test)]
mod tests;
