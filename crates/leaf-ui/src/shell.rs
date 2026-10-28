//! The app shell: sidebar on the left, page area on the right.
//!
//! Spike version (E00-S04): proves gpui-component's sidebar and text input render and accept
//! input. The real page tree (E06) and editor (E07) replace the placeholder content.

use gpui::{App, AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::input::{Input, InputState};
use gpui_component::sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};

/// Placeholder page titles shown in the spike sidebar.
const SAMPLE_PAGES: [&str; 3] = ["Getting started", "Ideas", "Reading list"];

/// Top-level view of a Leaf window.
///
/// Owns the state of the widgets it shows; later stories feed it view models from `leaf-editor`
/// instead of holding data itself.
pub struct Shell {
    input: Entity<InputState>,
}

impl Shell {
    /// Builds the shell for a new window. Needs the window because text inputs own focus state.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type something…"));
        Self { input }
    }

    /// Puts the keyboard focus in the input, so the user can type as soon as the window opens.
    pub fn focus_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }

    /// Current text of the spike's input, so tests can check typing reaches it.
    #[must_use]
    pub fn input_text(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pages = SidebarMenu::new().children(SAMPLE_PAGES.map(SidebarMenuItem::new));
        let sidebar = Sidebar::new("sidebar").child(SidebarGroup::new("Pages").child(pages));
        let page = v_flex()
            .flex_1()
            .p_6()
            .gap_3()
            .child(div().text_xl().child("Leaf"))
            .child(Input::new(&self.input));
        h_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(sidebar)
            .child(page)
    }
}

#[cfg(test)]
mod tests {
    use super::Shell;
    use gpui::{TestAppContext, VisualTestContext};

    fn open_shell(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext) {
        cx.update(crate::init);
        let (shell, cx) = cx.add_window_view(Shell::new);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (shell, cx)
    }

    #[gpui::test]
    fn shell_renders_with_an_empty_input(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        assert_eq!(shell.read_with(cx, Shell::input_text), "");
    }

    #[gpui::test]
    fn typing_reaches_the_focused_input(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.focus_input(window, cx)));
        cx.simulate_input("Hello, Leaf");
        assert_eq!(shell.read_with(cx, Shell::input_text), "Hello, Leaf");
    }
}
