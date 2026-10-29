//! Property test for workspace undo and redo (E01-S06).
//!
//! Random sequences of every page command, valid and invalid. After each accepted command the
//! outline is recorded; undoing step by step must retrace those outlines exactly back to the empty
//! workspace, and redoing everything must reach the final one again.

use leaf_core::PageId;
use leaf_model::{Outcome, PageMeta, Workspace, WorkspaceCommand};
use proptest::prelude::*;

/// A random command; page numbers pick from every page created so far (deleted ones included, so
/// some commands fail with "not found", which must not create an undo step).
#[derive(Debug, Clone, Copy)]
enum Step {
    Create {
        parent: Option<usize>,
        index: usize,
    },
    Rename {
        page: usize,
        title: u8,
    },
    Icon {
        page: usize,
        icon: Option<u8>,
    },
    Cover {
        page: usize,
        cover: Option<u8>,
    },
    Move {
        page: usize,
        parent: Option<usize>,
        index: usize,
    },
    Trash {
        page: usize,
    },
    Restore {
        page: usize,
    },
    Delete {
        page: usize,
    },
}

fn step() -> impl Strategy<Value = Step> {
    let page = 0..12_usize;
    let parent = proptest::option::of(0..12_usize);
    prop_oneof![
        3 => (parent.clone(), 0..4_usize).prop_map(|(parent, index)| Step::Create { parent, index }),
        1 => (page.clone(), any::<u8>()).prop_map(|(page, title)| Step::Rename { page, title }),
        1 => (page.clone(), proptest::option::of(any::<u8>())).prop_map(|(page, icon)| Step::Icon { page, icon }),
        1 => (page.clone(), proptest::option::of(0..4_u8)).prop_map(|(page, cover)| Step::Cover { page, cover }),
        2 => (page.clone(), parent, 0..4_usize).prop_map(|(page, parent, index)| Step::Move { page, parent, index }),
        1 => page.clone().prop_map(|page| Step::Trash { page }),
        1 => page.clone().prop_map(|page| Step::Restore { page }),
        1 => page.prop_map(|page| Step::Delete { page }),
    ]
}

/// Every page with its parent and metadata: the visible tree, then each trashed page's subtree.
fn outline(ws: &Workspace) -> Result<Vec<(Option<PageId>, PageMeta)>, leaf_model::ModelError> {
    fn walk(
        ws: &Workspace,
        parent: Option<PageId>,
        pages: Vec<PageId>,
        out: &mut Vec<(Option<PageId>, PageMeta)>,
    ) -> Result<(), leaf_model::ModelError> {
        for page in pages {
            out.push((parent, ws.page(page)?));
            walk(ws, Some(page), ws.children(page)?, out)?;
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(ws, None, ws.root_pages()?, &mut out)?;
    for page in ws.trash()? {
        walk(ws, None, vec![page], &mut out)?;
    }
    Ok(out)
}

fn command(step: Step, pages: &[PageId]) -> Option<WorkspaceCommand> {
    let pick = |n: usize| pages.get(n % pages.len().max(1)).copied();
    Some(match step {
        Step::Create { parent, index } => WorkspaceCommand::CreatePage {
            parent: parent.and_then(pick),
            index,
            title: String::new(),
            created_at: 0,
        },
        Step::Rename { page, title } => WorkspaceCommand::RenamePage {
            page: pick(page)?,
            title: format!("t{title}"),
        },
        Step::Icon { page, icon } => WorkspaceCommand::SetPageIcon {
            page: pick(page)?,
            icon: icon.map(|i| format!("i{i}")),
        },
        Step::Cover { page, cover } => WorkspaceCommand::SetPageCover {
            page: pick(page)?,
            cover: cover.map(|c| format!("builtin:c{c}")),
        },
        Step::Move { page, parent, index } => WorkspaceCommand::MovePage {
            page: pick(page)?,
            new_parent: parent.and_then(pick),
            index,
        },
        Step::Trash { page } => WorkspaceCommand::TrashPage {
            page: pick(page)?,
            at: 1,
        },
        Step::Restore { page } => WorkspaceCommand::RestorePage { page: pick(page)? },
        Step::Delete { page } => WorkspaceCommand::DeletePagePermanently { page: pick(page)? },
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn undo_retraces_every_step_and_redo_replays_them(steps in proptest::collection::vec(step(), 1..30)) {
        let mut ws = Workspace::new();
        ws.set_undo_merge_interval(0);
        let mut pages: Vec<PageId> = Vec::new();
        let mut history = vec![outline(&ws).unwrap()];

        for step in steps {
            let Some(command) = command(step, &pages) else { continue };
            match ws.apply(command) {
                Ok(Outcome::PageCreated(page)) => pages.push(page),
                Ok(Outcome::Changed | Outcome::PagesDeleted(_)) => {}
                // Valid but changed nothing, or rejected: no undo step either way.
                Ok(Outcome::Unchanged) | Err(_) => {
                    prop_assert_eq!(&outline(&ws).unwrap(), history.last().unwrap(), "no-op left the workspace as it was");
                    continue;
                }
            }
            history.push(outline(&ws).unwrap());
        }

        let last = history.len() - 1;
        for expected in history[..last].iter().rev() {
            prop_assert!(ws.undo().unwrap());
            prop_assert_eq!(&outline(&ws).unwrap(), expected);
        }
        prop_assert!(!ws.undo().unwrap(), "nothing left to undo");
        prop_assert!(ws.is_empty());

        for expected in &history[1..] {
            prop_assert!(ws.redo().unwrap());
            prop_assert_eq!(&outline(&ws).unwrap(), expected);
        }
        prop_assert!(!ws.redo().unwrap());
    }
}
