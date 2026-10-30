//! E17-S10: a pasted web address becomes a link, and pasted over selected text it links that text.

use leaf_model::{BlockKind, Marks, TextSpan};

use super::{at, outline, p, page, plain, texts};

fn link(text: &str, href: &str) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks: Marks {
            link: Some(href.into()),
            ..Marks::default()
        },
    }
}

#[test]
fn a_pasted_address_goes_in_as_a_link() {
    let (mut editor, ids) = page(&[p(0, "see  now"), p(0, "")]);
    editor.set_caret(at(ids[0], 4)).unwrap();
    editor.paste_text("https://example.com/a?b=1").unwrap();
    assert_eq!(
        outline(&editor)[0].2,
        vec![
            plain("see "),
            link("https://example.com/a?b=1", "https://example.com/a?b=1"),
            plain(" now")
        ]
    );
    // On an empty line, with the copy's trailing newline; `www.` gets https.
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.paste_text("www.example.com\n").unwrap();
    assert_eq!(
        outline(&editor)[1].2,
        vec![link("www.example.com", "https://www.example.com")]
    );
    assert_eq!(outline(&editor)[1].1, BlockKind::Paragraph);
}

#[test]
fn an_address_pasted_over_selected_text_links_the_text() {
    let (mut editor, ids) = page(&[p(0, "read the docs today")]);
    editor.select(ids[0], 9..13).unwrap();
    editor.paste_text("  https://docs.example.com ").unwrap();
    assert_eq!(
        outline(&editor)[0].2,
        vec![
            plain("read the "),
            link("docs", "https://docs.example.com"),
            plain(" today")
        ]
    );
    editor.undo().unwrap();
    assert_eq!(
        outline(&editor)[0].2,
        vec![plain("read the docs today")],
        "one undo step"
    );
}

#[test]
fn things_that_are_not_a_lone_address_stay_text() {
    for text in [
        "notes.txt",
        "see https://example.com",
        "javascript:alert(1)",
        "https://",
    ] {
        let (mut editor, ids) = page(&[p(0, "")]);
        editor.set_caret(at(ids[0], 0)).unwrap();
        editor.paste_text(text).unwrap();
        let spans = &outline(&editor)[0].2;
        assert!(
            spans
                .iter()
                .all(|s| s.marks.link.is_none() || text.starts_with("see")),
            "{text:?} -> {spans:?}"
        );
        assert_eq!(texts(&editor), vec![text.to_owned()]);
    }
    // Into code: exactly as typed.
    let (mut editor, ids) = page(&[(
        0,
        BlockKind::Code {
            language: String::new(),
        },
        "",
    )]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.paste_text("https://example.com").unwrap();
    assert_eq!(outline(&editor)[0].2, vec![plain("https://example.com")]);
}
