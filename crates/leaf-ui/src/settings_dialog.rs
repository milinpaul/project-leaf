//! The Settings window: a large panel floating over the dimmed page, with a sidebar of sections on the
//! left (styled like the app's own sidebar, groups separated by hairline dividers) and a roomy content
//! area on the right. Opened from the sidebar's cog, the Settings… menu item and Cmd/Ctrl+,; closed with
//! Esc or a click outside.

use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Div, FocusHandle, FontWeight, Hsla, Image, ImageFormat,
    InteractiveElement as _, IntoElement, ObjectFit, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, StyledImage as _, Window, div, img, prelude::FluentBuilder as _,
    px, rgb,
};
use gpui_component::button::Button;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_store::Appearance;

use crate::settings;
use crate::theme::{self, DARK, LIGHT, Palette};

/// Leaf's app icon, shown in About and on the welcome screen.
pub(crate) static APP_ICON: LazyLock<Arc<Image>> = LazyLock::new(|| {
    let png = include_bytes!("../../leaf-app/resources/png/leaf-256.png");
    Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
});

/// A part of the Settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// Light, dark or following the OS.
    Appearance,
    /// The open workspace: name and folder.
    Workspace,
    /// Version and licences.
    About,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Workspace => "Workspace",
            Self::About => "About Leaf",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Appearance => IconName::Palette,
            Self::Workspace => IconName::FolderOpen,
            Self::About => IconName::Info,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Appearance => "Choose how Leaf looks. System follows your computer and switches with it.",
            Self::Workspace => "Where this workspace keeps your pages.",
            Self::About => "A calm, local-first notebook.",
        }
    }
}

/// What the Workspace section shows about the open workspace.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceInfo {
    /// The workspace folder, or `None` for one that lives only in memory (tests).
    pub folder: Option<PathBuf>,
}

/// The window's content. Kept as an entity so the chosen section survives re-renders.
pub struct SettingsPanel {
    section: Section,
    workspace: WorkspaceInfo,
    /// Focus inside the window, so Esc reaches it and closes it.
    focus: FocusHandle,
}

/// Opens the Settings window on `section`, sized to the app window: large, with a margin of dimmed page
/// around it.
pub fn open(section: Section, workspace: WorkspaceInfo, window: &mut Window, cx: &mut gpui::App) {
    let panel = cx.new(|cx| SettingsPanel {
        section,
        workspace,
        focus: cx.focus_handle(),
    });
    let focus = panel.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, window, _| {
        let view = window.viewport_size();
        let width = (f32::from(view.width) - 160.0).clamp(760.0, 1080.0);
        let height = (f32::from(view.height) - 140.0).clamp(480.0, 720.0);
        dialog
            .p_0()
            .centered()
            .w(px(width))
            .h(px(height))
            .child(panel.clone())
    });
    window.focus(&focus, cx);
}

impl SettingsPanel {
    /// The left column: the app's sidebar look, sections in groups split by hairline dividers, and the
    /// version at the foot.
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (line, label) = (cx.theme().sidebar_border, cx.theme().muted_foreground);
        let divider = || div().mx_2().my_3().h(px(1.0)).bg(line);
        let group = |title: &'static str| {
            div()
                .px_2()
                .pb_1()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(label)
                .child(title)
        };
        v_flex()
            .w(px(232.0))
            .flex_none()
            .h_full()
            .px_3()
            .pt_5()
            .pb_4()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(
                div()
                    .px_2()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(divider())
            .child(group("General"))
            .child(self.row(Section::Appearance, cx))
            .child(self.row(Section::Workspace, cx))
            .child(divider())
            .child(group("Leaf"))
            .child(self.row(Section::About, cx))
            .child(div().flex_1())
            .child(divider())
            .child(
                div()
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Leaf {}", env!("CARGO_PKG_VERSION"))),
            )
    }

    /// One section row, like a page row in the app's sidebar.
    fn row(&self, section: Section, cx: &mut Context<Self>) -> impl IntoElement {
        let chosen = self.section == section;
        let (active, hover) = (cx.theme().sidebar_accent, cx.theme().accent);
        h_flex()
            .id(SharedString::from(format!("settings-{}", section.label())))
            .gap_2()
            .px_2()
            .py_1p5()
            .rounded(cx.theme().radius)
            .text_sm()
            .when(chosen, |row| row.bg(active).font_weight(FontWeight::MEDIUM))
            .when(!chosen, |row| row.hover(|row| row.bg(hover)))
            .child(Icon::new(section.icon()).small())
            .child(section.label())
            .on_click(cx.listener(move |panel, _: &ClickEvent, _, cx| {
                panel.section = section;
                cx.notify();
            }))
    }

    fn appearance(cx: &mut Context<Self>) -> AnyElement {
        let current = settings::get(cx).appearance();
        let accent = cx.theme().primary;
        let border = cx.theme().border;
        let card = |appearance: Appearance, label: &'static str, preview: Div| {
            let chosen = current == appearance;
            v_flex()
                .id(SharedString::from(format!("appearance-{label}")))
                .flex_1()
                .gap_2()
                .child(
                    div()
                        .rounded(px(10.0))
                        .p(px(3.0))
                        .border_2()
                        .border_color(if chosen { accent } else { Hsla::transparent_black() })
                        .child(preview.border_1().border_color(border)),
                )
                .child(
                    h_flex()
                        .gap_1p5()
                        .justify_center()
                        .text_sm()
                        .when(chosen, |row| {
                            row.font_weight(FontWeight::MEDIUM)
                                .child(Icon::new(IconName::Check).small().text_color(accent))
                        })
                        .child(label),
                )
                .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                    theme::set_appearance(appearance, window, cx);
                    cx.notify();
                }))
        };
        h_flex()
            .w_full()
            .gap_4()
            .child(card(Appearance::System, "System", system_preview()))
            .child(card(Appearance::Light, "Light", preview(&LIGHT)))
            .child(card(Appearance::Dark, "Dark", preview(&DARK)))
            .into_any_element()
    }

    fn workspace(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(folder) = self.workspace.folder.clone() else {
            return muted("This workspace isn't saved to disk.", cx).into_any_element();
        };
        let name = folder.file_stem().map_or_else(
            || folder.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let reveal = if cfg!(target_os = "macos") {
            "Show in Finder"
        } else if cfg!(target_os = "windows") {
            "Show in Explorer"
        } else {
            "Show in Files"
        };
        card(cx)
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        div()
                            .size(px(40.0))
                            .flex_none()
                            .rounded(px(10.0))
                            .bg(cx.theme().accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::FolderOpen).text_color(cx.theme().primary)),
                    )
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                            .child(muted("Back this folder up like any other.", cx)),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .child(folder.display().to_string()),
            )
            .child(
                h_flex().child(
                    Button::new("reveal-workspace")
                        .label(reveal)
                        .icon(Icon::new(IconName::FolderOpen))
                        .outline()
                        .on_click(move |_, _, cx| cx.reveal_path(&folder)),
                ),
            )
            .into_any_element()
    }

    fn about(cx: &mut Context<Self>) -> AnyElement {
        card(cx)
            .items_center()
            .gap_3()
            .py_8()
            .child(
                img(APP_ICON.clone())
                    .size(px(88.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("Leaf"))
            .child(muted(concat!("Version ", env!("CARGO_PKG_VERSION")), cx))
            .child(
                div()
                    .text_sm()
                    .child("Your pages stay on this computer. No account, no cloud."),
            )
            .child(muted("Typeface: Inter, under the SIL Open Font License 1.1.", cx))
            .into_any_element()
    }
}

/// A rounded surface for a group of settings.
fn card(cx: &Context<SettingsPanel>) -> Div {
    v_flex()
        .w_full()
        .p_5()
        .rounded(px(10.0))
        .border_1()
        .border_color(cx.theme().border)
}

fn muted(text: &'static str, cx: &Context<SettingsPanel>) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

/// `#RRGGBB` as a colour; the palette's values are checked by the theme tests.
fn hex(colour: &str) -> gpui::Rgba {
    rgb(colour
        .strip_prefix('#')
        .and_then(|h| u32::from_str_radix(h, 16).ok())
        .unwrap_or(0))
}

/// A miniature Leaf window in `p`'s colours: sidebar, a title, a few lines and a touch of green.
fn preview(p: &Palette) -> Div {
    h_flex()
        .h(px(104.0))
        .w_full()
        .rounded(px(7.0))
        .overflow_hidden()
        .child(sidebar_strip(p))
        .child(page_strip(p))
}

/// Half light, half dark: the look follows the computer.
fn system_preview() -> Div {
    h_flex()
        .h(px(104.0))
        .w_full()
        .rounded(px(7.0))
        .overflow_hidden()
        .child(sidebar_strip(&LIGHT))
        .child(page_strip(&LIGHT))
        .child(page_strip(&DARK))
}

fn sidebar_strip(p: &Palette) -> Div {
    let line = |width: f32| {
        div()
            .h(px(4.0))
            .w(px(width))
            .rounded_full()
            .bg(hex(p.muted))
            .opacity(0.5)
    };
    v_flex()
        .w(px(30.0))
        .h_full()
        .flex_none()
        .gap_1p5()
        .p_2()
        .bg(hex(p.sidebar))
        .child(line(14.0))
        .child(line(12.0))
        .child(line(14.0))
}

fn page_strip(p: &Palette) -> Div {
    let line = |width: f32| {
        div()
            .h(px(4.0))
            .w(gpui::relative(width))
            .rounded_full()
            .bg(hex(p.muted))
            .opacity(0.45)
    };
    v_flex()
        .flex_1()
        .h_full()
        .gap_1p5()
        .p_3()
        .bg(hex(p.page))
        .child(
            div()
                .h(px(7.0))
                .w(gpui::relative(0.55))
                .rounded_full()
                .bg(hex(p.text))
                .opacity(0.85),
        )
        .child(line(0.9))
        .child(line(0.75))
        .child(
            h_flex()
                .gap_1()
                .child(div().size(px(6.0)).rounded_full().bg(hex(p.accent)))
                .child(line(0.5)),
        )
}

impl Render for SettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let section = self.section;
        let body = match section {
            Section::Appearance => Self::appearance(cx),
            Section::Workspace => self.workspace(cx),
            Section::About => Self::about(cx),
        };
        h_flex()
            .track_focus(&self.focus)
            .size_full()
            .items_start()
            .rounded(cx.theme().radius_lg)
            .overflow_hidden()
            .child(self.sidebar(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .px_10()
                    .pt_10()
                    .gap_6()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(section.label()),
                            )
                            .child(muted(section.description(), cx)),
                    )
                    .child(div().h(px(1.0)).w_full().bg(cx.theme().border))
                    .child(body),
            )
    }
}
