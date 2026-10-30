//! Property test: two devices starring and unstarring pages at the same time end up with the same
//! favourites once they exchange changes, and a page starred on both is listed once (E18-S02).

use leaf_core::PageId;
use leaf_model::{ModelError, Workspace, WorkspaceCommand};
use proptest::prelude::*;

const PAGES: usize = 5;

/// Stars (`true`) or unstars a page, picked by number.
type Step = (usize, bool);

fn edit(ws: &mut Workspace, pages: &[PageId], steps: Vec<Step>) -> Result<(), ModelError> {
    for (page, on) in steps {
        let page = pages[page % pages.len()];
        let command = if on {
            WorkspaceCommand::AddFavourite { page }
        } else {
            WorkspaceCommand::RemoveFavourite { page }
        };
        ws.apply(command)?;
    }
    Ok(())
}

/// Swaps changes both ways, as two devices syncing.
fn sync(a: &mut Workspace, b: &mut Workspace) -> Result<(), ModelError> {
    let from_a = a.to_bytes()?;
    let from_b = b.to_bytes()?;
    a.import_changes(&from_b)?;
    b.import_changes(&from_a)
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    proptest::collection::vec((0..PAGES, any::<bool>()), 0..8)
}

proptest! {
    #[test]
    fn two_devices_agree_on_the_favourites(rounds in proptest::collection::vec((steps(), steps()), 1..4)) {
        let mut a = Workspace::new();
        let pages: Vec<PageId> = (0..PAGES)
            .map(|_| match a.apply(WorkspaceCommand::CreatePage { parent: None, index: 0, title: String::new(), created_at: 0 }) {
                Ok(leaf_model::Outcome::PageCreated(page)) => page,
                other => panic!("expected a new page, got {other:?}"),
            })
            .collect();
        let mut b = Workspace::from_bytes(&a.to_bytes().unwrap()).unwrap();
        for (on_a, on_b) in rounds {
            edit(&mut a, &pages, on_a).unwrap();
            edit(&mut b, &pages, on_b).unwrap();
            sync(&mut a, &mut b).unwrap();
            let listed = a.favourites().unwrap();
            prop_assert_eq!(&listed, &b.favourites().unwrap(), "both devices list the same favourites");
            let mut unique = listed.clone();
            unique.sort_unstable();
            unique.dedup();
            prop_assert_eq!(unique.len(), listed.len(), "each page at most once");
            for &page in &pages {
                prop_assert_eq!(a.is_favourite(page), listed.contains(&page));
            }
        }
    }
}

#[test]
fn a_page_starred_on_both_devices_is_listed_once() {
    let mut a = Workspace::new();
    let Ok(leaf_model::Outcome::PageCreated(page)) = a.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index: 0,
        title: String::new(),
        created_at: 0,
    }) else {
        panic!("page not created");
    };
    let mut b = Workspace::from_bytes(&a.to_bytes().unwrap()).unwrap();
    a.apply(WorkspaceCommand::AddFavourite { page }).unwrap();
    b.apply(WorkspaceCommand::AddFavourite { page }).unwrap();
    sync(&mut a, &mut b).unwrap();
    assert_eq!(a.favourites().unwrap(), vec![page]);
    assert_eq!(b.favourites().unwrap(), vec![page]);
}
