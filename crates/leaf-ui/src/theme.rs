//! Leaf's look in one place: colours, fonts, sizes and shapes for light and dark (ADR-027).
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

use gpui::{App, ElementId, Styled as _, Subscription, Window};
use gpui_component::button::Button;
use gpui_component::theme::{Theme, ThemeConfig, ThemeConfigColors, ThemeMode};
use leaf_store::Appearance;

use crate::settings;

/// The one typeface for everything (interface, text, titles and headings), bundled so Leaf looks the
/// same on every OS. One family keeps the page calm; hierarchy comes from size and weight.
pub const UI_FONT: &str = "Inter";

/// The monospace face for code blocks and inline code, bundled too (E08-S08, ADR-077). Naming a
/// bundled face also keeps gpui-component from listing every installed font at startup to check the
/// platform's default, which cost about 220 ms of a 500 ms cold-start budget. The "NL" cut has no
/// ligatures, so `->` stays two characters to move the caret through.
pub const CODE_FONT: &str = "JetBrains Mono NL";

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
    /// Red for actions that remove things (Move to Trash), readable on the page and on a hovered row.
    pub danger: &'static str,
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
    danger: "#C2362E",
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
    danger: "#FF7B72",
};

/// The bundled font files, embedded so Leaf needs nothing installed: Inter and the code face. Both
/// are under the SIL Open Font License; the licences sit next to the files.
const FONTS: [&[u8]; 7] = [
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Italic.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Inter-Bold.ttf"),
    include_bytes!("../assets/fonts/Inter-BoldItalic.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNL-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNL-Bold.ttf"),
];

/// A button with Leaf's pointer: the hand. gpui-component's buttons show the arrow, as desktop apps
/// often do, but the owner wants the hand on everything clickable (E07-S12). Every button in Leaf is
/// made here; a test fails if one is made with `Button::new` elsewhere.
pub fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id).cursor_pointer()
}

/// Loads the fonts and puts Leaf's light and dark themes in place before gpui-component sets itself
/// up. Its setup applies the theme it finds, and only probes the installed fonts (a slow listing)
/// when the theme still names the library's default faces; with Leaf's own named here first, it
/// never does (E08-S08). Call once at startup, before `gpui_component::init`.
pub fn install(cx: &mut App) {
    let fonts = FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect();
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        // The system font stands in; the app still works, only the look is off.
        crate::log::log(format_args!("could not load the bundled fonts: {err:#}"));
    }
    cx.set_global(Theme {
        light_theme: Rc::new(config(ThemeMode::Light, &LIGHT)),
        dark_theme: Rc::new(config(ThemeMode::Dark, &DARK)),
        font_family: UI_FONT.into(),
        mono_font_family: CODE_FONT.into(),
        ..Theme::default()
    });
}

/// Switches to the appearance the owner chose in Settings (following the system by default;
/// `LEAF_APPEARANCE=light|dark` overrides both, for screenshots). Call once at startup, after
/// gpui-component and the settings are set up.
pub fn init(cx: &mut App) {
    apply(None, cx);
}

/// Keeps `window` in step with the system: when the OS switches between light and dark, so does Leaf
/// (unless the owner chose light or dark in Settings).
/// Keep the subscription for as long as the window lives.
pub fn follow_system(window: &Window) -> Subscription {
    window.observe_window_appearance(|window, cx| apply(Some(window), cx))
}

/// Chooses light, dark or following the OS (the Settings dialog), remembers it and applies it now.
pub fn set_appearance(appearance: Appearance, window: &mut Window, cx: &mut App) {
    settings::update(cx, |settings| settings.set_appearance(appearance));
    apply(Some(window), cx);
}

/// Switches to `LEAF_APPEARANCE` if set, else the owner's choice in the settings, else the window's (or
/// system's) appearance.
fn apply(window: Option<&mut Window>, cx: &mut App) {
    let chosen = match settings::get(cx).appearance() {
        Appearance::Light => Some(ThemeMode::Light),
        Appearance::Dark => Some(ThemeMode::Dark),
        Appearance::System => None,
    };
    match forced_mode().or(chosen) {
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

/// An action item's priority flag colour (E19-S03): calm, clearly different hues, readable on the
/// page in both appearances. High is the danger red; Medium a warm amber; Low a quiet blue.
#[must_use]
pub fn priority_colour(priority: leaf_model::Priority, dark: bool) -> gpui::Hsla {
    let hex = match (priority, dark) {
        (leaf_model::Priority::High, false) => LIGHT.danger,
        (leaf_model::Priority::High, true) => DARK.danger,
        (leaf_model::Priority::Medium, false) => "#B26B00",
        (leaf_model::Priority::Medium, true) => "#E3B341",
        (leaf_model::Priority::Low, false) => "#3A6EA5",
        (leaf_model::Priority::Low, true) => "#79B0E8",
    };
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    gpui::rgb(value).into()
}

/// A callout's colours (E21-S02) as `#RRGGBB`: its soft background, and the colour of its icon. The
/// text is the page's usual text colour. A tone from a newer version is drawn as a note.
#[must_use]
pub fn callout_hex(tone: &leaf_model::CalloutTone, dark: bool) -> (&'static str, &'static str) {
    use leaf_model::CalloutTone;
    match (tone, dark) {
        (CalloutTone::Info, false) => ("#EAF2FB", "#2F6DC0"),
        (CalloutTone::Info, true) => ("#1C2A3A", "#6EA8F0"),
        (CalloutTone::Success, false) => ("#EAF6EE", "#23714A"),
        (CalloutTone::Success, true) => ("#1B2E24", "#5CC08A"),
        (CalloutTone::Warning, false) => ("#FDF4E3", "#9A6512"),
        (CalloutTone::Warning, true) => ("#33291A", "#E3A94B"),
        (CalloutTone::Error, false) => ("#FCECEB", "#B83A33"),
        (CalloutTone::Error, true) => ("#3A1F1F", "#F07A72"),
        (CalloutTone::Note | CalloutTone::Other(_), false) => ("#F1F0EC", "#5F5E59"),
        (CalloutTone::Note | CalloutTone::Other(_), true) => ("#2A2A28", "#A8A8A2"),
    }
}

/// [`callout_hex`] as colours to draw with.
#[must_use]
pub fn callout_colours(tone: &leaf_model::CalloutTone, dark: bool) -> (gpui::Hsla, gpui::Hsla) {
    let (background, icon) = callout_hex(tone, dark);
    let colour = |hex: &str| -> gpui::Hsla {
        gpui::rgb(u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0)).into()
    };
    (colour(background), colour(icon))
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
    colors.danger = c(p.danger);
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
        mono_font_family: Some(CODE_FONT.into()),
        radius: Some(RADIUS),
        radius_lg: Some(RADIUS_LG),
        shadow: Some(false),
        colors,
        ..ThemeConfig::default()
    }
}

#[cfg(test)]
mod tests;

/// Background and text colours of a select option's chip, by colour name (`leaf_editor::OPTION_COLORS`), soft in
/// both modes. Names this version doesn't know (from a newer Leaf) draw as gray.
#[expect(clippy::unreadable_literal, reason = "colours read best as plain 0xRRGGBB")]
pub fn option_colors(name: &str, cx: &gpui::App) -> (gpui::Hsla, gpui::Hsla) {
    use gpui_component::ActiveTheme as _;
    // (light background, light text, dark background, dark text) as 0xRRGGBB.
    let (lb, lf, db, df) = match name {
        "brown" => (0xF1E6DD, 0x6B4A32, 0x4A3A2E, 0xE8D3C2),
        "orange" => (0xFBE6D3, 0x8A4B12, 0x55391F, 0xF4C9A0),
        "yellow" => (0xF7EDC8, 0x77600D, 0x4F4620, 0xEEDB98),
        "green" => (0xDCEFE2, 0x235E3B, 0x24452F, 0xB7E1C3),
        "blue" => (0xDDE9F6, 0x24507E, 0x233A52, 0xBBD3EE),
        "purple" => (0xE9E1F4, 0x553482, 0x3A2F4F, 0xD6C6EE),
        "pink" => (0xF6E0EA, 0x84305A, 0x4E2C3C, 0xF0C2D7),
        "red" => (0xF8DEDC, 0x8E2B25, 0x55302D, 0xF2BFBA),
        _ => (0xECEAE6, 0x4D4A45, 0x3A3835, 0xDAD6CF),
    };
    let (background, text) = if cx.theme().mode.is_dark() {
        (db, df)
    } else {
        (lb, lf)
    };
    (gpui::rgb(background).into(), gpui::rgb(text).into())
}
