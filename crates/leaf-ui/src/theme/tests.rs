use gpui::TestAppContext;
use gpui_component::theme::{Theme, ThemeMode};

use super::{CODE_FONT, DARK, FONTS, LIGHT, Palette, UI_FONT, config, mode_named};

/// Reads `#RRGGBB` as 0-1 channels.
fn rgb(hex: &str) -> [f64; 3] {
    let hex = hex.strip_prefix('#').unwrap_or_else(|| panic!("{hex} lacks #"));
    assert_eq!(hex.len(), 6, "{hex} is not #RRGGBB");
    let channel = |i: usize| f64::from(u8::from_str_radix(&hex[i..i + 2], 16).unwrap()) / 255.0;
    [channel(0), channel(2), channel(4)]
}

/// WCAG 2 relative luminance.
fn luminance(hex: &str) -> f64 {
    let linear = |c: f64| {
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = rgb(hex);
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// WCAG 2 contrast ratio, 1 to 21.
fn contrast(a: &str, b: &str) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The pairs that must stay readable, with WCAG's minimum: 4.5 for text, 3 for UI parts such as the
/// caret. Placeholders only hint, so they need less, but still must be visible.
fn readable_pairs(p: &Palette) -> Vec<(&'static str, &'static str, &'static str, f64)> {
    vec![
        ("text on page", p.text, p.page, 7.0),
        ("text on sidebar", p.text, p.sidebar, 7.0),
        ("text on selected row", p.text, p.active, 4.5),
        ("text on hovered row", p.text, p.hover, 4.5),
        ("text on selection", p.text, p.selection, 4.5),
        ("text in code", p.text, p.code, 4.5),
        ("muted on page", p.muted, p.page, 4.5),
        ("muted on sidebar", p.muted, p.sidebar, 4.5),
        ("link on page", p.accent, p.page, 4.5),
        ("link on sidebar", p.accent, p.sidebar, 4.5),
        ("accent on soft accent", p.accent, p.accent_soft, 4.5),
        ("button text", p.on_accent, p.accent, 4.5),
        ("button text, hovered", p.on_accent, p.accent_hover, 4.5),
        ("remove actions on page", p.danger, p.page, 4.5),
        ("remove actions on hovered menu row", p.danger, p.hover, 4.5),
        ("caret", p.caret, p.page, 3.0),
        ("placeholder", p.faint, p.page, 2.5),
    ]
}

#[test]
fn both_palettes_meet_wcag_contrast() {
    for (mode, palette) in [("light", &LIGHT), ("dark", &DARK)] {
        for (what, fg, bg, minimum) in readable_pairs(palette) {
            let ratio = contrast(fg, bg);
            assert!(
                ratio >= minimum,
                "{mode}: {what} is {ratio:.2}:1, needs {minimum}:1"
            );
        }
    }
}

#[test]
fn surfaces_are_distinct_but_quiet() {
    for (mode, p) in [("light", &LIGHT), ("dark", &DARK)] {
        for (what, colour) in [("sidebar", p.sidebar), ("hover", p.hover), ("border", p.border)] {
            let ratio = contrast(colour, p.page);
            assert!(
                (1.04..1.6).contains(&ratio),
                "{mode}: {what} vs page is {ratio:.2}:1"
            );
        }
    }
}

#[test]
fn the_accent_is_the_icons_green() {
    // Hue of the icon's middle green #16A970, give or take a little for the deepened shades.
    let hue = |hex: &str| {
        let channels = rgb(hex);
        let [red, green, blue] = channels;
        let strongest = (0..3)
            .max_by(|&x, &y| channels[x].total_cmp(&channels[y]))
            .unwrap_or(0);
        let spread = channels[strongest] - red.min(green).min(blue);
        let sector = match strongest {
            0 => (green - blue) / spread % 6.0,
            1 => (blue - red) / spread + 2.0,
            _ => (red - green) / spread + 4.0,
        };
        (sector * 60.0 + 360.0) % 360.0
    };
    let icon = hue("#16A970");
    for colour in [LIGHT.accent, LIGHT.caret, DARK.accent, DARK.caret] {
        assert!(
            (hue(colour) - icon).abs() < 8.0,
            "{colour} drifts from the icon's green"
        );
    }
}

#[test]
fn appearance_names_parse() {
    assert_eq!(mode_named("dark"), Some(ThemeMode::Dark));
    assert_eq!(mode_named(" Light "), Some(ThemeMode::Light));
    assert_eq!(mode_named("sepia"), None);
}

#[test]
fn configs_carry_the_tokens() {
    let light = config(ThemeMode::Light, &LIGHT);
    let dark = config(ThemeMode::Dark, &DARK);
    assert_eq!((light.mode, dark.mode), (ThemeMode::Light, ThemeMode::Dark));
    assert_eq!(light.colors.background.as_deref(), Some(LIGHT.page));
    assert_eq!(dark.colors.background.as_deref(), Some(DARK.page));
    assert_eq!(light.colors.selection.as_deref(), Some(LIGHT.selection));
    assert_eq!(dark.colors.danger.as_deref(), Some(DARK.danger));
    assert_eq!(light.font_family.as_deref(), Some(UI_FONT));
    assert_eq!(dark.mono_font_family.as_deref(), Some(CODE_FONT));
}

/// A font file's family names, from its `name` table: the legacy family (ID 1) and the typographic
/// family (ID 16), Windows platform, UTF-16. Enough to check a file holds the face we ask for.
fn family_names(font: &[u8]) -> Vec<String> {
    let u16_at = |at: usize| u16::from_be_bytes([font[at], font[at + 1]]) as usize;
    let u32_at =
        |at: usize| u32::from_be_bytes([font[at], font[at + 1], font[at + 2], font[at + 3]]) as usize;
    let tables = u16_at(4);
    let name = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&record| &font[record..record + 4] == b"name")
        .map_or_else(|| panic!("no name table"), |record| u32_at(record + 8));
    let (count, strings) = (u16_at(name + 2), name + u16_at(name + 4));
    (0..count)
        .map(|i| name + 6 + 12 * i)
        .filter(|&r| u16_at(r) == 3 && matches!(u16_at(r + 6), 1 | 16))
        .map(|r| {
            let (len, offset) = (u16_at(r + 8), u16_at(r + 10));
            let units: Vec<u16> = font[strings + offset..strings + offset + len]
                .chunks(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        })
        .collect()
}

#[test]
fn bundled_fonts_provide_the_families_the_theme_names() {
    let families: Vec<Vec<String>> = FONTS.iter().map(|font| family_names(font)).collect();
    let has = |family: &str| {
        families
            .iter()
            .filter(|names| names.iter().any(|n| n == family))
            .count()
    };
    assert_eq!(has(UI_FONT), 5, "regular, italic, semibold, bold and bold italic");
    assert_eq!(has(CODE_FONT), 2, "regular and bold");
}

#[gpui::test]
fn the_theme_follows_the_mode(cx: &mut TestAppContext) {
    cx.update(|cx| {
        super::install(cx);
        gpui_component::init(cx);
        super::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        assert!(Theme::global(cx).is_dark());
        assert_eq!(Theme::global(cx).font_family.as_ref(), UI_FONT);
        assert_eq!(Theme::global(cx).mono_font_family.as_ref(), CODE_FONT);
        let dark_bg = Theme::global(cx).background;
        Theme::change(ThemeMode::Light, None, cx);
        assert!(!Theme::global(cx).is_dark());
        assert_ne!(Theme::global(cx).background, dark_bg, "light and dark differ");
        assert_eq!(Theme::global(cx).radius, gpui::px(6.0));
    });
}

/// Every button shows the hand (E07-S12): buttons are made with `theme::button`, never `Button::new`
/// directly, which would show the arrow.
#[test]
fn every_button_is_made_by_theme_button() {
    fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let needle = ["Button", "::new("].concat();
    let offenders: Vec<_> = files
        .iter()
        .filter(|path| !path.ends_with("src/theme.rs"))
        .filter(|path| std::fs::read_to_string(path).unwrap().contains(&needle))
        .collect();
    assert!(
        offenders.is_empty(),
        "use theme::button instead of Button::new in {offenders:?}"
    );
}

#[test]
fn callout_text_and_icons_stay_readable_in_every_tone() {
    use leaf_model::CalloutTone;

    let mut tones = CalloutTone::ALL.to_vec();
    tones.push(CalloutTone::Other("celebrate".into()));
    for (mode, palette, dark) in [("light", &LIGHT, false), ("dark", &DARK, true)] {
        for tone in &tones {
            let (background, icon) = super::callout_hex(tone, dark);
            let text = contrast(palette.text, background);
            assert!(text >= 7.0, "{mode} {tone:?}: text on callout is {text:.1}:1");
            let icon = contrast(icon, background);
            assert!(icon >= 4.5, "{mode} {tone:?}: icon on callout is {icon:.1}:1");
            let edge = contrast(background, palette.page);
            assert!(
                edge < 1.6,
                "{mode} {tone:?}: the box stays soft against the page ({edge:.2}:1)"
            );
        }
    }
}

/// E08-S08: gpui-component's setup lists every installed font (about 220 ms on a Mac) only to replace
/// its own default faces. Installed first, Leaf's faces are what its setup finds and keeps, so the
/// listing never runs; the bundled faces are loaded by then.
#[gpui::test]
fn the_library_setup_finds_leafs_fonts_already_in_place(cx: &mut TestAppContext) {
    cx.update(|cx| {
        super::install(cx);
        gpui_component::init(cx);
        let theme = Theme::global(cx);
        assert_eq!(theme.font_family.as_ref(), UI_FONT);
        assert_eq!(theme.mono_font_family.as_ref(), CODE_FONT);
        assert_eq!(theme.radius, gpui::px(6.0), "Leaf's config, not the library's");
        let installed = cx.text_system().all_font_names();
        for family in [UI_FONT, CODE_FONT] {
            assert!(installed.iter().any(|f| f == family), "{family} is loaded");
        }
    });
}
