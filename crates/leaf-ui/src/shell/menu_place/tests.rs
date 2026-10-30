//! The block menu and its "Turn into" list stay where they open and inside the window (E17-S07).

use gpui::{Bounds, point, px, size};

use super::{list_bounds, menu_origin};

const WINDOW: (f32, f32) = (1280.0, 800.0);
const MENU: (f32, f32) = (220.0, 250.0);
const LIST: (f32, f32) = (220.0, 400.0);

fn window() -> gpui::Size<gpui::Pixels> {
    size(px(WINDOW.0), px(WINDOW.1))
}

#[test]
fn the_menu_opens_below_its_handle_when_it_fits() {
    let at = menu_origin(
        px(300.0),
        px(100.0),
        px(120.0),
        size(px(MENU.0), px(MENU.1)),
        window(),
    );
    assert_eq!(at, point(px(300.0), px(124.0)));
}

#[test]
fn near_the_bottom_the_menu_opens_above_its_handle() {
    let at = menu_origin(
        px(300.0),
        px(700.0),
        px(720.0),
        size(px(MENU.0), px(MENU.1)),
        window(),
    );
    // Its bottom edge sits just over the handle's top.
    assert_eq!(at.y + px(MENU.1), px(696.0));
    assert_eq!(at.x, px(300.0));
}

#[test]
fn the_menu_stays_inside_the_window_sideways() {
    let at = menu_origin(
        px(1250.0),
        px(100.0),
        px(120.0),
        size(px(MENU.0), px(MENU.1)),
        window(),
    );
    assert_eq!(at.x, px(WINDOW.0 - 8.0 - MENU.0));
}

#[test]
fn the_list_opens_level_with_turn_into_when_it_fits() {
    let menu = Bounds::new(point(px(300.0), px(100.0)), size(px(MENU.0), px(MENU.1)));
    let list = list_bounds(menu, px(140.0), size(px(LIST.0), px(LIST.1)), window());
    assert_eq!(list.origin, point(px(524.0), px(140.0)));
    assert_eq!(list.size.height, px(LIST.1));
}

#[test]
fn near_the_bottom_the_list_moves_up_only_as_far_as_it_must() {
    let menu = Bounds::new(point(px(300.0), px(450.0)), size(px(MENU.0), px(MENU.1)));
    let list = list_bounds(menu, px(560.0), size(px(LIST.0), px(LIST.1)), window());
    // Its bottom rests on the window's margin; the menu itself is untouched.
    assert_eq!(list.origin.y + list.size.height, px(WINDOW.1 - 8.0));
    assert_eq!(list.size.height, px(LIST.1));
}

#[test]
fn a_list_taller_than_the_window_scrolls() {
    let short = size(px(WINDOW.0), px(300.0));
    let menu = Bounds::new(point(px(300.0), px(20.0)), size(px(MENU.0), px(MENU.1)));
    let list = list_bounds(menu, px(60.0), size(px(LIST.0), px(LIST.1)), short);
    assert_eq!(list.origin.y, px(8.0));
    assert_eq!(list.size.height, px(300.0 - 16.0));
}

#[test]
fn at_the_right_edge_the_list_opens_on_the_left() {
    let menu = Bounds::new(point(px(1000.0), px(100.0)), size(px(MENU.0), px(MENU.1)));
    let list = list_bounds(menu, px(140.0), size(px(LIST.0), px(LIST.1)), window());
    assert_eq!(list.origin.x, px(1000.0 - 4.0 - LIST.0));
}

#[test]
fn with_no_room_on_either_side_the_list_stays_on_the_right() {
    let narrow = size(px(500.0), px(WINDOW.1));
    let menu = Bounds::new(point(px(150.0), px(100.0)), size(px(MENU.0), px(MENU.1)));
    let list = list_bounds(menu, px(140.0), size(px(LIST.0), px(LIST.1)), narrow);
    assert_eq!(list.origin.x, px(374.0));
}
