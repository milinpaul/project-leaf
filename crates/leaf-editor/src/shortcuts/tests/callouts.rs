//! E21-S03: GitHub's alert syntax typed at the start of a block makes a callout.

use leaf_model::{BlockKind, CalloutTone};

use super::{at, outline, page, para, type_into_empty};

fn callout(tone: CalloutTone) -> BlockKind {
    BlockKind::Callout { tone }
}

#[test]
fn typing_an_alert_makes_a_callout_of_its_tone() {
    for (typed, tone) in [
        ("> [!NOTE] ", CalloutTone::Info),
        ("> [!TIP] ", CalloutTone::Success),
        ("> [!IMPORTANT] ", CalloutTone::Note),
        ("> [!WARNING] ", CalloutTone::Warning),
        ("> [!CAUTION] ", CalloutTone::Error),
        ("> [!warning] ", CalloutTone::Warning),
    ] {
        let (editor, _, caret) = type_into_empty(typed);
        assert_eq!(
            outline(&editor),
            vec![(callout(tone), String::new())],
            "{typed:?}"
        );
        assert_eq!(caret.at, 0, "{typed:?}: the typed syntax is gone");
    }
}

#[test]
fn a_plain_quote_and_other_brackets_stay_as_they_were() {
    let (editor, _, _) = type_into_empty("> hello");
    assert_eq!(outline(&editor), vec![(BlockKind::Quote, "hello".into())]);
    let (editor, _, _) = type_into_empty("> [!OTHER] ");
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Quote, "[!OTHER] ".into())],
        "an alert name GitHub doesn't have is just text"
    );
    let (editor, _, _) = type_into_empty("[!NOTE] ");
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Paragraph, "[!NOTE] ".into())],
        "without the quote mark it's text"
    );
}

#[test]
fn the_whole_alert_typed_at_once_also_works_and_undo_gives_the_text_back() {
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.insert_text("> [!WARNING] ").unwrap();
    assert_eq!(
        outline(&editor),
        vec![(callout(CalloutTone::Warning), String::new())]
    );
    editor.undo().unwrap();
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Paragraph, "> [!WARNING] ".into())],
        "undo keeps what was typed, as plain text"
    );
}
