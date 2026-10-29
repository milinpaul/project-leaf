//! The sidebar's page tree: which rows it shows, expanding and collapsing, and the actions its
//! context menu and drag-and-drop offer (add a page inside, move, trash).
//!
//! Listings hide pages in the trash, but command positions count them (`BLOCK_MODEL.md`). So every
//! place the owner points at ("above this row", "inside that page") is turned into a model index
//! here, using the page's full list of siblings, never the visible one.

use leaf_core::PageId;
use leaf_model::{Outcome, WorkspaceCommand};

use super::{EditorError, SidebarItem, UNTITLED, WorkspaceEditor};

/// Where a dragged page lands, relative to the row under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPlace {
    /// Just above the row's page, beside it.
    Before(PageId),
    /// Just below the row's page, beside it. Below an expanded page with children, the line sits
    /// above its first child, so the page goes there instead: what the owner sees is what they get.
    After(PageId),
    /// Inside the row's page, as its last child.
    Inside(PageId),
    /// Below every row, at the end of the top level: the empty space under the tree.
    End,
}

impl DropPlace {
    /// The place for a pointer `offset` pixels below the top of `target`'s row, which is `height`
    /// tall: the top quarter drops above it, the bottom quarter below it, the middle into it.
    #[must_use]
    pub fn at(target: PageId, offset: f32, height: f32) -> Self {
        if offset < height / 4.0 {
            Self::Before(target)
        } else if offset > height * 3.0 / 4.0 {
            Self::After(target)
        } else {
            Self::Inside(target)
        }
    }

    /// The page whose row this place is next to or inside; `None` for [`DropPlace::End`].
    #[must_use]
    pub fn target(self) -> Option<PageId> {
        match self {
            Self::Before(page) | Self::After(page) | Self::Inside(page) => Some(page),
            Self::End => None,
        }
    }
}

impl WorkspaceEditor {
    /// The page tree as sidebar rows, top to bottom: each page followed by its children when it is
    /// expanded. Pages in the trash, and everything inside them, are left out.
    pub fn sidebar(&self) -> Result<Vec<SidebarItem>, EditorError> {
        let mut items = Vec::new();
        // Depth-first with an explicit stack (children pushed in reverse), so a very deep tree can't
        // overflow the call stack.
        let mut stack: Vec<(PageId, usize)> = self
            .workspace
            .root_pages()?
            .into_iter()
            .rev()
            .map(|page| (page, 0))
            .collect();
        while let Some((page, depth)) = stack.pop() {
            let meta = self.workspace.page(page)?;
            let children = self.workspace.children(page)?;
            let expanded = self.expanded.contains(&page);
            if expanded {
                stack.extend(children.iter().rev().map(|&child| (child, depth + 1)));
            }
            items.push(SidebarItem {
                page,
                label: if meta.title.is_empty() {
                    UNTITLED.to_owned()
                } else {
                    meta.title
                },
                icon: meta.icon,
                selected: self.selected == Some(page),
                depth,
                has_children: !children.is_empty(),
                expanded,
            });
        }
        Ok(items)
    }

    /// Whether `page`'s children are listed in the sidebar.
    #[must_use]
    pub fn is_expanded(&self, page: PageId) -> bool {
        self.expanded.contains(&page)
    }

    /// Expands or collapses `page` in the sidebar. Returns whether that changed anything, so the
    /// caller knows when to save the view state.
    pub fn set_expanded(&mut self, page: PageId, expanded: bool) -> bool {
        if expanded {
            self.expanded.insert(page)
        } else {
            self.expanded.remove(&page)
        }
    }

    /// Flips `page` between expanded and collapsed.
    pub fn toggle_expanded(&mut self, page: PageId) {
        let expanded = !self.is_expanded(page);
        self.set_expanded(page, expanded);
    }

    /// The expanded pages that still exist, in a stable order, for saving the view state.
    #[must_use]
    pub fn expanded_pages(&self) -> Vec<PageId> {
        let mut pages: Vec<PageId> = self
            .expanded
            .iter()
            .copied()
            .filter(|&page| self.workspace.contains(page))
            .collect();
        pages.sort();
        pages
    }

    /// Expands `pages`, as saved last time. Pages no longer in the workspace are ignored.
    pub fn restore_expanded(&mut self, pages: impl IntoIterator<Item = PageId>) {
        let known: Vec<PageId> = pages
            .into_iter()
            .filter(|&page| self.workspace.contains(page))
            .collect();
        self.expanded.extend(known);
    }

    /// Adds an untitled page at the end of the top level and selects it, so the user can type its
    /// title straight away. `now_ms` is the current time in Unix milliseconds, from the app.
    pub fn new_page(&mut self, now_ms: i64) -> Result<PageId, EditorError> {
        self.new_page_in(None, now_ms)
    }

    /// Adds an untitled page as the last child of `parent` (or at the end of the top level), opens
    /// `parent` so the new page shows, and selects it.
    pub fn new_page_in(&mut self, parent: Option<PageId>, now_ms: i64) -> Result<PageId, EditorError> {
        // After every sibling, trashed ones included, so it lands below the last visible one.
        let index = self.workspace.child_slots(parent)?.len();
        let command = WorkspaceCommand::CreatePage {
            parent,
            index,
            title: String::new(),
            created_at: now_ms,
        };
        let Outcome::PageCreated(page) = self.workspace.apply(command)? else {
            unreachable!("CreatePage always reports the page it created");
        };
        if let Some(parent) = parent {
            self.set_expanded(parent, true);
        }
        self.selected = Some(page);
        Ok(page)
    }

    /// Moves `page`, with everything inside it, to the trash. If the open page went with it, nothing
    /// is open afterwards. `now_ms` is the current time in Unix milliseconds, from the app.
    pub fn trash(&mut self, page: PageId, now_ms: i64) -> Result<(), EditorError> {
        self.workspace
            .apply(WorkspaceCommand::TrashPage { page, at: now_ms })?;
        if let Some(open) = self.selected
            && self.workspace.is_in_trash(open)?
        {
            self.selected = None;
        }
        Ok(())
    }

    /// Whether `page` may be dropped at `place`: not onto itself, and not anywhere inside itself
    /// (a page can't contain itself).
    pub fn can_drop(&self, page: PageId, place: DropPlace) -> Result<bool, EditorError> {
        let Some(target) = place.target() else {
            return Ok(true); // the top level can take any page
        };
        if target == page {
            return Ok(false);
        }
        // Walk up from where the page would live; meeting it on the way means a loop.
        let mut cursor = Some(target);
        while let Some(current) = cursor {
            if current == page {
                return Ok(false);
            }
            cursor = self.workspace.parent(current)?;
        }
        Ok(true)
    }

    /// Moves `page`, with everything inside it, to `place`. Returns whether anything moved: a drop
    /// that is not allowed (see [`WorkspaceEditor::can_drop`]) or that leaves the page where it was
    /// changes nothing.
    pub fn move_page(&mut self, page: PageId, place: DropPlace) -> Result<bool, EditorError> {
        if !self.can_drop(page, place)? {
            return Ok(false);
        }
        let (parent, index) = self.drop_slot(page, place)?;
        let outcome = self.workspace.apply(WorkspaceCommand::MovePage {
            page,
            new_parent: parent,
            index,
        })?;
        if let Some(parent) = parent {
            // Dropping into a collapsed page opens it, so the moved page stays in sight.
            self.set_expanded(parent, true);
        }
        Ok(outcome == Outcome::Changed)
    }

    /// The parent and model index for `place`, counted (as `MovePage` wants) among the new siblings
    /// without `page` itself, pages in the trash included.
    fn drop_slot(&self, page: PageId, place: DropPlace) -> Result<(Option<PageId>, usize), EditorError> {
        let slots = |parent| -> Result<Vec<PageId>, EditorError> {
            let mut slots = self.workspace.child_slots(parent)?;
            slots.retain(|&slot| slot != page);
            Ok(slots)
        };
        let beside = |target: PageId, after: bool| -> Result<_, EditorError> {
            let parent = self.workspace.parent(target)?;
            let slots = slots(parent)?;
            let at = slots
                .iter()
                .position(|&slot| slot == target)
                .unwrap_or(slots.len());
            Ok((parent, if after { at + 1 } else { at }))
        };
        match place {
            DropPlace::Inside(target) => Ok((Some(target), slots(Some(target))?.len())),
            DropPlace::After(target)
                if self.is_expanded(target) && !self.workspace.children(target)?.is_empty() =>
            {
                Ok((Some(target), 0))
            }
            DropPlace::After(target) => beside(target, true),
            DropPlace::Before(target) => beside(target, false),
            DropPlace::End => Ok((None, slots(None)?.len())),
        }
    }
}

#[cfg(test)]
mod tests;
