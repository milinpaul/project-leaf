//! Property test for workspace undo and redo (E01-S06), favourites included (E18-S02).
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
    Width {
        page: usize,
        full: bool,
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
    Favourite {
        page: usize,
        on: bool,
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
        1 => (page.clone(), any::<bool>()).prop_map(|(page, full)| Step::Width { page, full }),
        2 => (page.clone(), parent, 0..4_usize).prop_map(|(page, parent, index)| Step::Move { page, parent, index }),
        1 => page.clone().prop_map(|page| Step::Trash { page }),
        1 => page.clone().prop_map(|page| Step::Restore { page }),
        1 => page.clone().prop_map(|page| Step::Delete { page }),
        2 => (page, any::<bool>()).prop_map(|(page, on)| Step::Favourite { page, on }),
    ]
}

/// Every page with its parent and metadata (the visible tree, then each trashed page's subtree),
/// and the favourites with whether each page is starred.
type Outline = (Vec<(Option<PageId>, PageMeta, bool)>, Vec<PageId>);

fn outline(ws: &Workspace) -> Result<Outline, leaf_model::ModelError> {
    fn walk(
        ws: &Workspace,
        parent: Option<PageId>,
        pages: Vec<PageId>,
        out: &mut Vec<(Option<PageId>, PageMeta, bool)>,
    ) -> Result<(), leaf_model::ModelError> {
        for page in pages {
            out.push((parent, ws.page(page)?, ws.is_favourite(page)));
            walk(ws, Some(page), ws.children(page)?, out)?;
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(ws, None, ws.root_pages()?, &mut out)?;
    for page in ws.trash()? {
        walk(ws, None, vec![page], &mut out)?;
    }
    Ok((out, ws.favourites()?))
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
        Step::Width { page, full } => WorkspaceCommand::SetPageFullWidth {
            page: pick(page)?,
            full_width: full,
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
        Step::Favourite { page, on: true } => WorkspaceCommand::AddFavourite { page: pick(page)? },
        Step::Favourite { page, on: false } => WorkspaceCommand::RemoveFavourite { page: pick(page)? },
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
