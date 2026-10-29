//! The Settings dialog: a centred panel with its sections listed on the left (Appearance, Workspace,
//! About). Opened from the sidebar's cog, the Settings… menu item and Cmd/Ctrl+,; closed with Esc or a
//! click outside.

use std::path::PathBuf;

use gpui::{
    AppContext as _, ClickEvent, Context, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::button::Button;
use gpui_component::{ActiveTheme as _, Icon, Selectable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_store::Appearance;

use crate::{settings, theme};

/// A part of the Settings dialog.
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
    const ALL: [Self; 3] = [Self::Appearance, Self::Workspace, Self::About];

    fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Workspace => "Workspace",
            Self::About => "About",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Appearance => IconName::Sun,
            Self::Workspace => IconName::FolderOpen,
            Self::About => IconName::Info,
        }
    }
}

/// What the Workspace section shows about the open workspace.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceInfo {
    /// The workspace folder, or `None` for one that lives only in memory (tests).
    pub folder: Option<PathBuf>,
}

/// The dialog's content. Kept as an entity so the chosen section survives re-renders.
pub struct SettingsPanel {
    section: Section,
    workspace: WorkspaceInfo,
    /// Focus inside the dialog, so Esc reaches it and closes it.
    focus: FocusHandle,
}

/// Opens the Settings dialog on `section`.
pub fn open(section: Section, workspace: WorkspaceInfo, window: &mut Window, cx: &mut gpui::App) {
    let panel = cx.new(|cx| SettingsPanel {
        section,
        workspace,
        focus: cx.focus_handle(),
    });
    let focus = panel.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title("Settings").w(px(640.0)).child(panel.clone())
    });
    window.focus(&focus, cx);
}

impl SettingsPanel {
    /// The section list: quiet rows like the sidebar's, the chosen one highlighted.
    fn nav(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (active, hover) = (cx.theme().sidebar_accent, cx.theme().accent);
        v_flex()
            .w(px(160.0))
            .flex_none()
            .gap_0p5()
            .children(Section::ALL.map(|section| {
                let chosen = self.section == section;
                h_flex()
                    .id(SharedString::from(format!("settings-{}", section.label())))
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    .rounded(cx.theme().radius)
                    .text_sm()
                    .when(chosen, |row| row.bg(active).font_weight(FontWeight::MEDIUM))
                    .when(!chosen, |row| row.hover(|row| row.bg(hover)))
                    .child(Icon::new(section.icon()).size_4())
                    .child(section.label())
                    .on_click(cx.listener(move |panel, _: &ClickEvent, _, cx| {
                        panel.section = section;
                        cx.notify();
                    }))
            }))
    }

    fn appearance(cx: &mut Context<Self>) -> impl IntoElement {
        let current = settings::get(cx).appearance();
        let choice = |appearance: Appearance, label: &'static str, icon: IconName| {
            Button::new(SharedString::from(format!("appearance-{label}")))
                .label(label)
                .icon(Icon::new(icon))
                .outline()
                .selected(current == appearance)
                .flex_1()
                .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                    theme::set_appearance(appearance, window, cx);
                    cx.notify();
                }))
        };
        v_flex()
            .gap_3()
            .child(heading("Theme"))
            .child(muted(
                "System follows your computer's light or dark setting and switches when it does.",
                cx,
            ))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(choice(Appearance::System, "System", IconName::Monitor))
                    .child(choice(Appearance::Light, "Light", IconName::Sun))
                    .child(choice(Appearance::Dark, "Dark", IconName::Moon)),
            )
    }

    fn workspace(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(folder) = self.workspace.folder.clone() else {
            return v_flex()
                .gap_3()
                .child(heading("Workspace"))
                .child(muted("This workspace isn't saved to disk.", cx));
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
        v_flex()
            .gap_3()
            .child(heading(name))
            .child(muted(
                "Your pages are stored in this folder. Back it up like any other.",
                cx,
            ))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted)
                    .text_sm()
                    .child(folder.display().to_string()),
            )
            .child(
                Button::new("reveal-workspace")
                    .label(reveal)
                    .icon(Icon::new(IconName::FolderOpen))
                    .outline()
                    .on_click(move |_, _, cx| cx.reveal_path(&folder)),
            )
    }

    fn about(cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(heading(format!("Leaf {}", env!("CARGO_PKG_VERSION"))))
            .child(muted(
                "A calm, local-first notebook. Your pages stay on this computer.",
                cx,
            ))
            .child(muted("Typeface: Inter, under the SIL Open Font License 1.1.", cx))
    }
}

fn heading(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_lg()
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}

fn muted(text: &'static str, cx: &Context<SettingsPanel>) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

impl Render for SettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.section {
            Section::Appearance => Self::appearance(cx).into_any_element(),
            Section::Workspace => self.workspace(cx).into_any_element(),
            Section::About => Self::about(cx).into_any_element(),
        };
        h_flex()
            .track_focus(&self.focus)
            .items_start()
            .gap_6()
            .min_h(px(240.0))
            .child(self.nav(cx))
            // `min_w_0` lets the content shrink to the dialog, so long lines wrap instead of spilling out.
            .child(div().flex_1().min_w_0().child(content))
    }
}
