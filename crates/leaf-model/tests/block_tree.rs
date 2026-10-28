//! Property test for the structural block commands (E02-S02).
//!
//! Random inserts, deletes and multi-block moves, valid and invalid, run against both `PageDoc` and
//! a small reference model. After every step both must agree on which commands are rejected and on
//! every block's children in order; a save and load at the end must change nothing.

use std::collections::{HashMap, HashSet};

use leaf_core::BlockId;
use leaf_model::{BlockKind, ModelError, PageCommand, PageDoc, PageOutcome};
use proptest::prelude::*;

/// Block numbers pick from every block created so far (deleted ones too), wrapping around.
#[derive(Debug, Clone)]
enum Step {
    Insert {
        parent: Option<usize>,
        index: usize,
        heading: bool,
    },
    Delete {
        blocks: Vec<usize>,
    },
    Move {
        blocks: Vec<usize>,
        parent: Option<usize>,
        index: usize,
    },
}

fn step() -> impl Strategy<Value = Step> {
    let some = || proptest::collection::vec(0..16_usize, 0..4);
    prop_oneof![
        4 => (proptest::option::of(0..16_usize), 0..4_usize, proptest::bool::weighted(0.25))
            .prop_map(|(parent, index, heading)| Step::Insert { parent, index, heading }),
        1 => some().prop_map(|blocks| Step::Delete { blocks }),
        3 => (some(), proptest::option::of(0..16_usize), 0..4_usize)
            .prop_map(|(blocks, parent, index)| Step::Move { blocks, parent, index }),
    ]
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Ok,
    NotFound,
    ChildrenNotAllowed,
    OutOfRange,
    IntoOwnSubtree,
}

/// What the page should look like.
#[derive(Default)]
struct Reference {
    children: HashMap<Option<BlockId>, Vec<BlockId>>,
    parent: HashMap<BlockId, Option<BlockId>>,
    leaf_only: HashSet<BlockId>, // headings: no children allowed
}

impl Reference {
    fn kids(&self, parent: Option<BlockId>) -> Vec<BlockId> {
        self.children.get(&parent).cloned().unwrap_or_default()
    }

    fn document_order(&self) -> Vec<BlockId> {
        fn walk(r: &Reference, parent: Option<BlockId>, out: &mut Vec<BlockId>) {
            for b in r.kids(parent) {
                out.push(b);
                walk(r, Some(b), out);
            }
        }
        let mut out = Vec::new();
        walk(self, None, &mut out);
        out
    }

    fn ancestors(&self, block: BlockId) -> Vec<BlockId> {
        let mut out = Vec::new();
        let mut cursor = self.parent[&block];
        while let Some(b) = cursor {
            out.push(b);
            cursor = self.parent[&b];
        }
        out
    }

    fn container_ok(&self, parent: Option<BlockId>) -> Result<(), Verdict> {
        match parent {
            Some(p) if !self.parent.contains_key(&p) => Err(Verdict::NotFound),
            Some(p) if self.leaf_only.contains(&p) => Err(Verdict::ChildrenNotAllowed),
            _ => Ok(()),
        }
    }

    fn outermost(&self, ids: &[BlockId]) -> Result<Vec<BlockId>, Verdict> {
        if ids.iter().any(|b| !self.parent.contains_key(b)) {
            return Err(Verdict::NotFound);
        }
        let listed: HashSet<BlockId> = ids.iter().copied().collect();
        Ok(self
            .document_order()
            .into_iter()
            .filter(|b| listed.contains(b) && !self.ancestors(*b).iter().any(|a| listed.contains(a)))
            .collect())
    }

    fn insert(&mut self, block: BlockId, parent: Option<BlockId>, index: usize, heading: bool) -> Verdict {
        if let Err(v) = self.container_ok(parent) {
            return v;
        }
        if index > self.kids(parent).len() {
            return Verdict::OutOfRange;
        }
        self.children.entry(parent).or_default().insert(index, block);
        self.parent.insert(block, parent);
        if heading {
            self.leaf_only.insert(block);
        }
        Verdict::Ok
    }

    fn insert_verdict(&self, parent: Option<BlockId>, index: usize) -> Verdict {
        match self.container_ok(parent) {
            Err(v) => v,
            Ok(()) if index > self.kids(parent).len() => Verdict::OutOfRange,
            Ok(()) => Verdict::Ok,
        }
    }

    fn delete(&mut self, ids: &[BlockId]) -> Verdict {
        let roots = match self.outermost(ids) {
            Ok(roots) => roots,
            Err(v) => return v,
        };
        for root in roots {
            let mut gone = vec![root];
            while let Some(b) = gone.pop() {
                gone.extend(self.kids(Some(b)));
                self.children.remove(&Some(b));
                let parent = self.parent.remove(&b).flatten();
                if let Some(list) = self.children.get_mut(&parent) {
                    list.retain(|x| *x != b);
                }
                self.leaf_only.remove(&b);
            }
        }
        Verdict::Ok
    }

    fn move_blocks(&mut self, ids: &[BlockId], new_parent: Option<BlockId>, index: usize) -> Verdict {
        let roots = match self.outermost(ids) {
            Ok(roots) => roots,
            Err(v) => return v,
        };
        if let Err(v) = self.container_ok(new_parent) {
            return v;
        }
        if roots.is_empty() {
            return Verdict::Ok;
        }
        if let Some(p) = new_parent
            && (roots.contains(&p) || self.ancestors(p).iter().any(|a| roots.contains(a)))
        {
            return Verdict::IntoOwnSubtree;
        }
        let mut others: Vec<BlockId> = self
            .kids(new_parent)
            .into_iter()
            .filter(|b| !roots.contains(b))
            .collect();
        if index > others.len() {
            return Verdict::OutOfRange;
        }
        for &root in &roots {
            let old = self.parent[&root];
            self.children.entry(old).or_default().retain(|b| *b != root);
            self.parent.insert(root, new_parent);
        }
        others.splice(index..index, roots);
        self.children.insert(new_parent, others);
        Verdict::Ok
    }

    fn assert_matches(&self, page: &PageDoc) {
        assert_eq!(page.root_blocks(), self.kids(None), "top level");
        for &block in self.parent.keys() {
            assert_eq!(
                page.children(block).ok(),
                Some(self.kids(Some(block))),
                "children of {block}"
            );
        }
        assert_eq!(page.len(), self.parent.len());
    }
}

fn verdict(result: &Result<PageOutcome, ModelError>) -> Verdict {
    match result {
        Ok(_) => Verdict::Ok,
        Err(ModelError::BlockNotFound(_)) => Verdict::NotFound,
        Err(ModelError::ChildrenNotAllowed { .. }) => Verdict::ChildrenNotAllowed,
        Err(ModelError::IndexOutOfRange { .. }) => Verdict::OutOfRange,
        Err(ModelError::BlockMoveIntoOwnSubtree { .. }) => Verdict::IntoOwnSubtree,
        Err(other) => panic!("unexpected error: {other}"),
    }
}

proptest! {
    #[test]
    fn block_commands_match_the_reference_and_survive_reload(steps in proptest::collection::vec(step(), 1..40)) {
        let mut page = PageDoc::new();
        let mut reference = Reference::default();
        let mut created: Vec<BlockId> = Vec::new();
        let pick = |created: &[BlockId], n: usize| created.get(n % created.len().max(1)).copied();

        for step in steps {
            match step {
                Step::Insert { parent, index, heading } => {
                    let parent = parent.and_then(|n| pick(&created, n));
                    let kind = if heading { BlockKind::Heading { level: 1 } } else { BlockKind::Paragraph };
                    let expected = reference.insert_verdict(parent, index);
                    let result = page.apply(PageCommand::InsertBlock { parent, index, kind, text: String::new() });
                    prop_assert_eq!(verdict(&result), expected);
                    if let Ok(PageOutcome::BlockCreated(block)) = result {
                        prop_assert_eq!(reference.insert(block, parent, index, heading), Verdict::Ok);
                        created.push(block);
                    }
                }
                Step::Delete { blocks } => {
                    let ids: Vec<BlockId> = blocks.iter().filter_map(|&n| pick(&created, n)).collect();
                    let result = page.apply(PageCommand::DeleteBlocks { ids: ids.clone() });
                    prop_assert_eq!(verdict(&result), reference.delete(&ids));
                }
                Step::Move { blocks, parent, index } => {
                    let ids: Vec<BlockId> = blocks.iter().filter_map(|&n| pick(&created, n)).collect();
                    let new_parent = parent.and_then(|n| pick(&created, n));
                    let result = page.apply(PageCommand::MoveBlocks { ids: ids.clone(), new_parent, index });
                    prop_assert_eq!(verdict(&result), reference.move_blocks(&ids, new_parent, index));
                }
            }
            reference.assert_matches(&page);
        }

        let reloaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
        reference.assert_matches(&reloaded);
    }
}
