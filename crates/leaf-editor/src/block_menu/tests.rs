//! Tests for the block menu (E07-S04, E17-S03): its entries, keyboard, clicks and hovering.

use leaf_model::BlockKind;

use crate::block_actions::tests::{at, outline, p, page, row};
use crate::{HoverChange, MenuEntry, MenuKey, PageEditor};

fn labels(editor: &mut PageEditor) -> Vec<(&'static str, bool)> {
    editor
        .block_menu()
        .unwrap()
        .unwrap()
        .items
        .iter()
        .map(|i| (i.entry.label(), i.enabled))
        .collect()
}

#[test]
fn the_menu_lists_the_actions_and_greys_out_moves_that_cannot_happen() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    assert_eq!(editor.block_menu().unwrap(), None, "closed until opened");
    editor.open_block_menu(ids[0]).unwrap();
    assert_eq!(
        labels(&mut editor),
        vec![
            ("Turn into", true),
            ("Duplicate", true),
            ("Move up", false),
            ("Move down", true),
            ("Delete", true),
        ]
    );
    editor.open_block_menu(ids[1]).unwrap();
    let moves: Vec<bool> = labels(&mut editor)[2..4].iter().map(|l| l.1).collect();
    assert_eq!(moves, vec![true, false]);
}

#[test]
fn keys_move_the_highlight_skipping_greyed_out_entries() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.open_block_menu(ids[0]).unwrap();
    let selected = |editor: &mut PageEditor, key| editor.block_menu_key(key).unwrap().unwrap().selected;
    assert_eq!(selected(&mut editor, MenuKey::Down), 1);
    assert_eq!(selected(&mut editor, MenuKey::Down), 3, "Move up is skipped");
    assert_eq!(selected(&mut editor, MenuKey::Down), 4);
    assert_eq!(selected(&mut editor, MenuKey::Down), 0, "wraps around");
    assert_eq!(selected(&mut editor, MenuKey::Up), 4);
    assert_eq!(editor.block_menu_key(MenuKey::Escape).unwrap(), None);
    assert_eq!(editor.block_menu().unwrap(), None, "Escape closes it");
}

#[test]
fn enter_runs_the_highlighted_action_and_closes_the_menu() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.open_block_menu(ids[0]).unwrap();
    editor.block_menu_key(MenuKey::Down).unwrap();
    editor.block_menu_key(MenuKey::Down).unwrap();
    assert_eq!(editor.block_menu_key(MenuKey::Enter).unwrap(), None);
    let texts: Vec<String> = outline(&editor).into_iter().map(|r| r.2).collect();
    assert_eq!(texts, vec!["b", "a"], "Move down");
    assert!(editor.undo().unwrap());
    let texts: Vec<String> = outline(&editor).into_iter().map(|r| r.2).collect();
    assert_eq!(texts, vec!["a", "b"], "one undo step");
}

#[test]
fn turn_into_opens_a_list_of_kinds_with_the_current_one_marked() {
    let (mut editor, ids) = page(&[(0, BlockKind::Heading { level: 2 }, "title")]);
    editor.open_block_menu(ids[0]).unwrap();
    let menu = editor.block_menu_key(MenuKey::Right).unwrap().unwrap();
    let (kinds, selected) = menu.kinds.unwrap();
    assert_eq!(selected, 0);
    assert_eq!(kinds.len(), 10, "the text kinds, action items included (E19-S02)");
    let current: Vec<&str> = kinds
        .iter()
        .filter(|k| k.current)
        .map(|k| k.entry.label())
        .collect();
    assert_eq!(current, vec!["Heading 2"]);

    let menu = editor.block_menu_key(MenuKey::Left).unwrap().unwrap();
    assert_eq!(menu.kinds, None, "Left goes back");
    editor.block_menu_key(MenuKey::Enter).unwrap();
    editor.block_menu_key(MenuKey::Up).unwrap();
    let menu = editor.block_menu_key(MenuKey::Up).unwrap().unwrap();
    assert_eq!(menu.kinds.unwrap().1, 8, "Up wraps to Quote");
    let menu = editor.block_menu_key(MenuKey::Escape).unwrap().unwrap();
    assert_eq!(menu.kinds, None, "Escape leaves the list first");
    editor.block_menu_key(MenuKey::Right).unwrap();
    editor.block_menu_key(MenuKey::Up).unwrap();
    editor.block_menu_key(MenuKey::Up).unwrap();
    assert_eq!(editor.block_menu_key(MenuKey::Enter).unwrap(), None);
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Quote, "title")]);
}

#[test]
fn clicks_choose_entries_directly() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.open_block_menu(ids[0]).unwrap();
    let still_open = editor.choose(MenuEntry::MoveUp).unwrap();
    assert!(still_open.is_some(), "a greyed-out entry does nothing");
    assert!(
        editor
            .choose(MenuEntry::Kind(BlockKind::Quote))
            .unwrap()
            .is_some(),
        "kinds need the list open"
    );
    assert!(
        editor
            .choose(MenuEntry::TurnInto)
            .unwrap()
            .unwrap()
            .kinds
            .is_some()
    );
    assert!(
        editor
            .choose(MenuEntry::TurnInto)
            .unwrap()
            .unwrap()
            .kinds
            .is_some(),
        "a second click keeps it open (hovering may have opened it already)"
    );
    assert_eq!(editor.choose(MenuEntry::Duplicate).unwrap(), None);
    assert_eq!(outline(&editor).len(), 3);
    editor.open_block_menu(ids[1]).unwrap();
    editor.choose(MenuEntry::Delete).unwrap();
    let texts: Vec<String> = outline(&editor).into_iter().map(|r| r.2).collect();
    assert_eq!(texts, vec!["a", "a"]);
}

#[test]
fn the_caret_opens_the_menu_and_a_vanished_block_closes_it() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(0, "c")]);
    assert_eq!(
        editor.open_block_menu_at_caret().unwrap(),
        None,
        "no caret, no menu"
    );
    editor.set_caret(at(ids[1], 0)).unwrap();
    let menu = editor.open_block_menu_at_caret().unwrap().unwrap();
    assert_eq!(menu.block, ids[1]);
    editor.close_block_menu();
    assert_eq!(editor.block_menu().unwrap(), None);

    editor.open_block_menu(ids[1]).unwrap();
    editor.choose(MenuEntry::Delete).unwrap();
    assert_eq!(
        editor.open_block_menu(ids[1]).unwrap(),
        None,
        "a deleted block has no menu"
    );
    editor.open_block_menu(ids[0]).unwrap();
    editor.block_action(ids[0], crate::BlockAction::Delete).unwrap();
    assert_eq!(editor.block_menu().unwrap(), None);
}

#[test]
fn opening_the_block_menu_closes_the_slash_menu() {
    let (mut editor, ids) = page(&[p(0, "")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.insert_text("/").unwrap();
    assert!(editor.slash_menu().unwrap().is_some());
    editor.open_block_menu(ids[0]).unwrap();
    assert!(editor.slash_menu().unwrap().is_none());
}

#[test]
fn the_mouse_highlights_entries_but_not_greyed_out_ones() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.open_block_menu(ids[0]).unwrap();
    editor.highlight(&MenuEntry::Delete).unwrap();
    assert_eq!(editor.block_menu().unwrap().unwrap().selected, 4);
    editor.highlight(&MenuEntry::MoveUp).unwrap();
    assert_eq!(
        editor.block_menu().unwrap().unwrap().selected,
        4,
        "Move up is greyed out"
    );

    editor.choose(MenuEntry::TurnInto).unwrap();
    editor.highlight(&MenuEntry::Kind(BlockKind::Quote)).unwrap();
    assert_eq!(editor.block_menu().unwrap().unwrap().kinds.unwrap().1, 8);
    editor.highlight(&MenuEntry::TurnInto).unwrap();
    assert!(
        editor.block_menu().unwrap().unwrap().kinds.is_some(),
        "still open over Turn into"
    );
    editor.highlight(&MenuEntry::Duplicate).unwrap();
    editor.settle_hover().unwrap();
    let menu = editor.block_menu().unwrap().unwrap();
    assert_eq!(
        (menu.selected, menu.kinds),
        (1, None),
        "resting on another entry closes the list"
    );
    editor.close_block_menu();
    editor.highlight(&MenuEntry::Delete).unwrap();
    assert_eq!(editor.block_menu().unwrap(), None);
}

#[test]
fn resting_on_turn_into_opens_the_list() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.open_block_menu(ids[1]).unwrap();
    editor.highlight(&MenuEntry::Duplicate).unwrap();
    assert_eq!(
        editor.highlight(&MenuEntry::TurnInto).unwrap(),
        Some(HoverChange::OpenKinds)
    );
    let menu = editor.block_menu().unwrap().unwrap();
    assert_eq!(
        (menu.selected, menu.kinds.is_some()),
        (0, false),
        "highlighted at once, opened later"
    );
    let menu = editor.settle_hover().unwrap().unwrap();
    assert_eq!(menu.kinds.map(|(_, at)| at), Some(0));
    assert_eq!(
        editor.highlight(&MenuEntry::TurnInto).unwrap(),
        None,
        "already open"
    );
}

#[test]
fn passing_over_turn_into_does_not_open_the_list() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.open_block_menu(ids[1]).unwrap();
    editor.highlight(&MenuEntry::TurnInto).unwrap();
    assert_eq!(editor.highlight(&MenuEntry::Duplicate).unwrap(), None);
    let menu = editor.settle_hover().unwrap().unwrap();
    assert_eq!((menu.selected, menu.kinds), (1, None));
}

#[test]
fn crossing_other_entries_into_the_list_keeps_it_open() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.open_block_menu(ids[1]).unwrap();
    editor.choose(MenuEntry::TurnInto).unwrap();
    // Diagonally from "Turn into" down to "Heading 2", over Duplicate and a greyed-out Move down.
    assert_eq!(
        editor.highlight(&MenuEntry::Duplicate).unwrap(),
        Some(HoverChange::CloseKinds)
    );
    assert_eq!(
        editor.highlight(&MenuEntry::MoveDown).unwrap(),
        Some(HoverChange::CloseKinds)
    );
    assert_eq!(
        editor.highlight(&MenuEntry::MoveDown).unwrap(),
        None,
        "moving within the same row doesn't restart the pause"
    );
    let menu = editor.block_menu().unwrap().unwrap();
    assert_eq!(
        (menu.selected, menu.kinds.is_some()),
        (0, true),
        "nothing changes until the pointer rests"
    );
    let heading = MenuEntry::Kind(BlockKind::Heading { level: 2 });
    assert_eq!(editor.highlight(&heading).unwrap(), None);
    let menu = editor.settle_hover().unwrap().unwrap();
    assert_eq!(
        menu.kinds.map(|(_, at)| at),
        Some(2),
        "still open, Heading 2 highlighted"
    );
    assert_eq!(editor.choose(heading).unwrap(), None);
    assert_eq!(
        editor.doc().kind(ids[1]).unwrap(),
        BlockKind::Heading { level: 2 }
    );
}

#[test]
fn resting_on_a_greyed_out_entry_closes_the_list_without_highlighting_it() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.open_block_menu(ids[1]).unwrap();
    editor.choose(MenuEntry::TurnInto).unwrap();
    editor.highlight(&MenuEntry::MoveDown).unwrap();
    let menu = editor.settle_hover().unwrap().unwrap();
    assert_eq!((menu.selected, menu.kinds), (0, None));
    editor.close_block_menu();
    assert_eq!(editor.settle_hover().unwrap(), None, "a closed menu stays closed");
    assert_eq!(editor.highlight(&MenuEntry::TurnInto).unwrap(), None);
}
