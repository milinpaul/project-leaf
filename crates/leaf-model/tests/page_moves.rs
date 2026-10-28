//! Property test for page creates and moves (E01-S04).
//!
//! Random command sequences, valid and invalid, run against both `Workspace` and a tiny reference
//! model that spells out what should happen. After every step both must agree on which commands are
//! rejected and on the order of pages under every parent, and a save and load must change nothing.

use std::collections::HashMap;

use leaf_core::PageId;
use leaf_model::{ModelError, Outcome, Workspace, WorkspaceCommand};
use proptest::prelude::*;

/// A random step. Page and parent numbers pick from the pages created so far, wrapping around.
#[derive(Debug, Clone)]
enum Step {
    Create {
        parent: Option<usize>,
        index: usize,
    },
    Move {
        page: usize,
        parent: Option<usize>,
        index: usize,
    },
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (proptest::option::of(0..10_usize), 0..5_usize)
            .prop_map(|(parent, index)| Step::Create { parent, index }),
        (0..10_usize, proptest::option::of(0..10_usize), 0..5_usize)
            .prop_map(|(page, parent, index)| Step::Move { page, parent, index }),
    ]
}

/// What the workspace should look like: each parent's children in order, and each page's parent.
#[derive(Default)]
struct Reference {
    children: HashMap<Option<PageId>, Vec<PageId>>,
    parent: HashMap<PageId, Option<PageId>>,
}

/// Which way a command should go; error details are checked by the unit tests.
#[derive(Debug, PartialEq)]
enum Verdict {
    Ok,
    OutOfRange,
    IntoOwnSubtree,
}

impl Reference {
    fn create(&mut self, page: PageId, parent: Option<PageId>, index: usize) {
        self.children.entry(parent).or_default().insert(index, page);
        self.parent.insert(page, parent);
    }

    fn create_verdict(&self, parent: Option<PageId>, index: usize) -> Verdict {
        if index > self.children.get(&parent).map_or(0, Vec::len) {
            Verdict::OutOfRange
        } else {
            Verdict::Ok
        }
    }

    fn move_page(&mut self, page: PageId, new_parent: Option<PageId>, index: usize) -> Verdict {
        let mut cursor = new_parent;
        while let Some(current) = cursor {
            if current == page {
                return Verdict::IntoOwnSubtree;
            }
            cursor = self.parent[&current];
        }
        let old_parent = self.parent[&page];
        let len = self
            .children
            .get(&new_parent)
            .map_or(0, |c| c.iter().filter(|&&p| p != page).count());
        if index > len {
            return Verdict::OutOfRange;
        }
        self.children
            .entry(old_parent)
            .or_default()
            .retain(|&p| p != page);
        self.children.entry(new_parent).or_default().insert(index, page);
        self.parent.insert(page, new_parent);
        Verdict::Ok
    }

    /// Checks every parent's children, in order, against the workspace.
    fn assert_matches(&self, ws: &Workspace) {
        assert_eq!(
            ws.root_pages(),
            self.children.get(&None).cloned().unwrap_or_default(),
            "top level"
        );
        for &page in self.parent.keys() {
            let expected = self.children.get(&Some(page)).cloned().unwrap_or_default();
            assert_eq!(ws.children(page).ok(), Some(expected), "children of {page}");
        }
        assert_eq!(ws.len(), self.parent.len());
    }
}

fn verdict(result: Result<Outcome, ModelError>) -> (Verdict, Option<PageId>) {
    match result {
        Ok(Outcome::PageCreated(page)) => (Verdict::Ok, Some(page)),
        Ok(Outcome::Changed) => (Verdict::Ok, None),
        Err(ModelError::IndexOutOfRange { .. }) => (Verdict::OutOfRange, None),
        Err(ModelError::MoveIntoOwnSubtree { .. }) => (Verdict::IntoOwnSubtree, None),
        Err(other) => panic!("unexpected error: {other}"),
    }
}

proptest! {
    #[test]
    fn creates_and_moves_match_the_reference_and_survive_reload(steps in proptest::collection::vec(step(), 1..40)) {
        let mut ws = Workspace::new();
        let mut reference = Reference::default();
        let mut pages: Vec<PageId> = Vec::new();
        let pick = |pages: &[PageId], n: usize| pages[n % pages.len()];

        for step in steps {
            match step {
                Step::Create { parent, index } => {
                    let parent = parent.filter(|_| !pages.is_empty()).map(|n| pick(&pages, n));
                    let expected = reference.create_verdict(parent, index);
                    let command = WorkspaceCommand::CreatePage { parent, index, title: String::new(), created_at: 0 };
                    let (got, created) = verdict(ws.apply(command));
                    prop_assert_eq!(&got, &expected);
                    if let Some(page) = created {
                        reference.create(page, parent, index);
                        pages.push(page);
                    }
                }
                Step::Move { page, parent, index } => {
                    if pages.is_empty() {
                        continue;
                    }
                    let page = pick(&pages, page);
                    let parent = parent.map(|n| pick(&pages, n));
                    let expected = reference.move_page(page, parent, index);
                    let (got, _) = verdict(ws.apply(WorkspaceCommand::MovePage { page, new_parent: parent, index }));
                    prop_assert_eq!(got, expected);
                }
            }
            reference.assert_matches(&ws);
        }

        let reloaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
        reference.assert_matches(&reloaded);
    }
}
