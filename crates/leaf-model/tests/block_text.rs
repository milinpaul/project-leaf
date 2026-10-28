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

/// A step on a page of top-level paragraphs; block numbers wrap around the current blocks.
#[derive(Debug, Clone)]
enum PageStep {
    Type { block: usize, pos: usize, text: String },
    Split { block: usize, pos: usize },
    Merge { block: usize },
}

fn page_step() -> impl Strategy<Value = PageStep> {
    prop_oneof![
        3 => (0..8_usize, 0..30_usize, piece()).prop_map(|(block, pos, text)| PageStep::Type { block, pos, text }),
        2 => (0..8_usize, 0..30_usize).prop_map(|(block, pos)| PageStep::Split { block, pos }),
        2 => (0..8_usize).prop_map(|block| PageStep::Merge { block }),
    ]
}

proptest! {
    #[test]
    fn splits_and_merges_match_a_list_of_strings(steps in proptest::collection::vec(page_step(), 1..40)) {
        let mut page = PageDoc::new();
        let PageOutcome::BlockCreated(first) = page
            .apply(PageCommand::InsertBlock { parent: None, index: 0, kind: BlockKind::Paragraph, text: String::new() })
            .unwrap()
        else {
            panic!("expected a block");
        };
        let mut ids = vec![first];
        let mut expected: Vec<Vec<char>> = vec![Vec::new()];

        for step in steps {
            match step {
                PageStep::Type { block, pos, text } => {
                    let i = block % ids.len();
                    let pos = pos.min(expected[i].len());
                    page.apply(PageCommand::InsertText { id: ids[i], pos, text: text.clone() }).unwrap();
                    expected[i].splice(pos..pos, text.chars());
                }
                PageStep::Split { block, pos } => {
                    let i = block % ids.len();
                    let pos = pos.min(expected[i].len());
                    let PageOutcome::BlockCreated(new) = page.apply(PageCommand::SplitBlock { id: ids[i], pos }).unwrap() else {
                        panic!("split must create a block");
                    };
                    let tail = expected[i].split_off(pos);
                    ids.insert(i + 1, new);
                    expected.insert(i + 1, tail);
                }
                PageStep::Merge { block } => {
                    let i = block % ids.len();
                    let outcome = page.apply(PageCommand::MergeIntoPrevious { id: ids[i] }).unwrap();
                    if i == 0 {
                        prop_assert_eq!(outcome, PageOutcome::Unchanged);
                    } else {
                        prop_assert_eq!(outcome, PageOutcome::BlockMerged { into: ids[i - 1], at: expected[i - 1].len() });
                        let moved = expected.remove(i);
                        expected[i - 1].extend(moved);
                        ids.remove(i);
                    }
                }
            }
            prop_assert_eq!(&page.root_blocks(), &ids);
            for (id, chars) in ids.iter().zip(&expected) {
                prop_assert_eq!(page.plain_text(*id).unwrap().unwrap(), chars.iter().collect::<String>());
            }
        }
    }
}
