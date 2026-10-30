use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext, Window, WindowBounds, point, px,
    size,
};
use gpui_component::theme::Theme;
use gpui_component::{Root, WindowExt as _};
use leaf_store::{Appearance, WindowFrame};

use super::frame_of;
use crate::menus::{About, OpenSettings, ToggleSidebar};
use crate::shell::Shell;
use crate::{settings, theme};

/// A window built like the app's: the shell inside gpui-component's `Root`, which hosts dialogs.
fn open_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let made: Rc<RefCell<Option<Entity<Shell>>>> = Rc::default();
    let keep = made.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let shell = cx.new(|cx| Shell::new(window, cx));
        *keep.borrow_mut() = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let shell = made.borrow().clone().unwrap();
    (shell, cx)
}

fn hidden(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| settings::get(cx).sidebar_hidden())
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn the_collapse_and_menu_buttons_hide_and_show_the_sidebar(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    cx.simulate_input("Notes");
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert!(
        cx.debug_bounds("show-sidebar").is_none(),
        "no ☰ while the sidebar shows"
    );

    let collapse = cx.debug_bounds("collapse-sidebar").unwrap().center();
    cx.simulate_click(collapse, Modifiers::none());
    redraw(cx);
    assert!(hidden(cx));
    assert!(
        cx.debug_bounds("collapse-sidebar").is_none(),
        "the sidebar is gone"
    );
    assert!(
        cx.debug_bounds("sidebar-search").is_none(),
        "with its search field"
    );
    let page_focused =
        |cx: &mut VisualTestContext| cx.update(|window, cx| shell.read(cx).page_focus.is_focused(window));
    assert!(page_focused(cx), "the page keeps the keyboard");

    let open = cx.debug_bounds("show-sidebar").unwrap().center();
    cx.simulate_click(open, Modifiers::none());
    redraw(cx);
    assert!(!hidden(cx));
    assert!(cx.debug_bounds("show-sidebar").is_none());
    assert!(cx.debug_bounds("collapse-sidebar").is_some());
    assert!(page_focused(cx));
}

#[gpui::test]
fn the_sidebar_toggles_and_is_remembered(cx: &mut TestAppContext) {
    let (_shell, cx) = open_window(cx);
    assert!(!cx.update(|_, cx| settings::get(cx).sidebar_hidden()));
    cx.dispatch_action(ToggleSidebar);
    assert!(cx.update(|_, cx| settings::get(cx).sidebar_hidden()));
    cx.dispatch_action(ToggleSidebar);
    assert!(!cx.update(|_, cx| settings::get(cx).sidebar_hidden()));
}

#[gpui::test]
fn settings_open_from_the_menu_and_about(cx: &mut TestAppContext) {
    let (_shell, cx) = open_window(cx);
    assert!(!cx.update(Window::has_active_dialog));
    cx.dispatch_action(OpenSettings);
    assert!(cx.update(Window::has_active_dialog));
    cx.update(Window::close_dialog);
    cx.dispatch_action(About);
    assert!(cx.update(Window::has_active_dialog));
}

#[gpui::test]
fn choosing_an_appearance_applies_and_remembers_it(cx: &mut TestAppContext) {
    let (_shell, cx) = open_window(cx);
    cx.update(|window, cx| theme::set_appearance(Appearance::Dark, window, cx));
    cx.update(|_, cx| {
        assert!(Theme::global(cx).is_dark());
        assert_eq!(settings::get(cx).appearance(), Appearance::Dark);
    });
    cx.update(|window, cx| theme::set_appearance(Appearance::Light, window, cx));
    cx.update(|_, cx| assert!(!Theme::global(cx).is_dark()));
}

#[test]
fn a_maximised_window_remembers_its_restore_size() {
    let rect = gpui::Bounds {
        origin: point(px(10.0), px(20.0)),
        size: size(px(800.0), px(600.0)),
    };
    let expected = WindowFrame {
        x: 10.0,
        y: 20.0,
        width: 800.0,
        height: 600.0,
        maximized: false,
    };
    assert_eq!(frame_of(WindowBounds::Windowed(rect)), expected);
    assert_eq!(
        frame_of(WindowBounds::Maximized(rect)),
        WindowFrame {
            maximized: true,
            ..expected
        }
    );
    assert_eq!(
        frame_of(WindowBounds::Fullscreen(rect)),
        WindowFrame {
            maximized: true,
            ..expected
        }
    );
}
