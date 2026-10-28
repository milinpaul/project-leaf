//! Property test for block text edits (E02-S03).
//!
//! Random inserts and deletes of mixed scripts (ASCII, emoji with modifiers, Devanagari, CJK) at
//! Unicode scalar positions, compared against a plain `Vec<char>` after every step and after reload.

use leaf_model::{BlockKind, ModelError, PageCommand, PageDoc, PageOutcome};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Step {
    Insert { pos: usize, text: String },
    Delete { start: usize, len: usize },
}

fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("a".to_owned()),
        Just("leaf ".to_owned()),
        Just("👋🏽".to_owned()),
        Just("👨‍👩‍👧".to_owned()),
        Just("नमस्ते".to_owned()),
        Just("葉".to_owned()),
        Just("\n".to_owned()),
    ]
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0..40_usize, piece()).prop_map(|(pos, text)| Step::Insert { pos, text }),
        (0..40_usize, 0..6_usize).prop_map(|(start, len)| Step::Delete { start, len }),
    ]
}

proptest! {
    #[test]
    fn text_edits_match_a_char_vector(steps in proptest::collection::vec(step(), 1..50)) {
        let mut page = PageDoc::new();
        let PageOutcome::BlockCreated(id) = page
            .apply(PageCommand::InsertBlock { parent: None, index: 0, kind: BlockKind::Paragraph, text: String::new() })
            .unwrap()
        else {
            panic!("expected a block");
        };
        let mut expected: Vec<char> = Vec::new();

        for step in steps {
            match step {
                Step::Insert { pos, text } => {
                    let result = page.apply(PageCommand::InsertText { id, pos, text: text.clone() });
                    if pos <= expected.len() {
                        prop_assert!(result.is_ok());
                        expected.splice(pos..pos, text.chars());
                    } else {
                        let out_of_range = matches!(result, Err(ModelError::TextOutOfRange { .. }));
                        prop_assert!(out_of_range);
                    }
                }
                Step::Delete { start, len } => {
                    let range = start..start + len;
                    let result = page.apply(PageCommand::DeleteText { id, range: range.clone() });
                    if range.end <= expected.len() {
                        prop_assert!(result.is_ok());
                        expected.drain(range);
                    } else {
                        let out_of_range = matches!(result, Err(ModelError::TextOutOfRange { .. }));
                        prop_assert!(out_of_range);
                    }
                }
            }
            prop_assert_eq!(page.plain_text(id).unwrap().unwrap(), expected.iter().collect::<String>());
        }

        let reloaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
        prop_assert_eq!(reloaded.plain_text(id).unwrap().unwrap(), expected.iter().collect::<String>());
    }
}
