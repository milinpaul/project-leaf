//! Tests for the search bar: typing and the pause before searching, merging full-text hits with title
//! matches, ignoring stale hits, keys and the Cmd/Ctrl+K decision.

#![allow(
    clippy::single_range_in_vec_init,
    reason = "lists holding one matched range are the data under test"
)]

use std::fmt::Write as _;
use std::ops::Range;

use leaf_core::{BlockId, PageId};
use leaf_model::Workspace;
use proptest::prelude::*;

use super::{
    BlockMatch, Destination, Marked, SEARCH_DELAY_MS, SearchBar, SearchHit, SearchKey, SearchOutcome,
    SearchRequest, SearchRow, Shortcut, ShortcutAction, shortcut_action,
};
use crate::WorkspaceEditor;

const NOW: i64 = 1_700_000_000_000;

/// The sample's pages by name.
struct Pages {
    projects: PageId,
    leaf: PageId,
    garden: PageId,
    journal: PageId,
    recipes: PageId,
    old: PageId,
}

fn page(editor: &mut WorkspaceEditor, parent: Option<PageId>, title: &str) -> PageId {
    let page = editor.new_page_in(parent, NOW).unwrap();
    editor.rename(page, title).unwrap();
    page
}

/// Projects (Leaf app, Garden plan), Journal, Recipes, and "Old garden notes" in the trash.
fn sample() -> (WorkspaceEditor, Pages) {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let projects = page(&mut editor, None, "Projects");
    let leaf = page(&mut editor, Some(projects), "Leaf app");
    let garden = page(&mut editor, Some(projects), "Garden plan");
    let journal = page(&mut editor, None, "Journal");
    let recipes = page(&mut editor, None, "Recipes");
    let old = page(&mut editor, None, "Old garden notes");
    editor.trash(old, NOW).unwrap();
    let pages = Pages {
        projects,
        leaf,
        garden,
        journal,
        recipes,
        old,
    };
    (editor, pages)
}

fn marked(text: &str, ranges: &[Range<usize>]) -> Marked {
    Marked {
        text: text.to_owned(),
        ranges: ranges.to_vec(),
    }
}

fn hit(page: PageId, title: Marked, title_matched: bool, blocks: &[(BlockId, Marked)]) -> SearchHit {
    SearchHit {
        page,
        title,
        title_matched,
        blocks: blocks
            .iter()
            .map(|(block, excerpt)| BlockMatch {
                block: *block,
                excerpt: excerpt.clone(),
            })
            .collect(),
    }
}

/// `text` with the `ranges` (bytes) in brackets.
fn bracket(text: &str, ranges: &[Range<usize>]) -> String {
    let mut out = String::new();
    let mut at = 0;
    for range in ranges {
        out.push_str(&text[at..range.start]);
        write!(out, "[{}]", &text[range.clone()]).unwrap();
        at = range.end;
    }
    out.push_str(&text[at..]);
    out
}

/// Rows as text: a `>` marks the highlighted row; page rows say where they open.
fn render(bar: &SearchBar) -> String {
    let mut out = String::new();
    for (ix, row) in bar.rows().iter().enumerate() {
        let mark = if bar.selected() == Some(ix) { '>' } else { ' ' };
        match row {
            SearchRow::Page(page) => {
                let icon = page.icon.as_deref().unwrap_or("·");
                let path = if page.path.is_empty() {
                    String::new()
                } else {
                    format!("  —  {}", page.path)
                };
                let opens = if page.opens_at.is_some() {
                    "  (opens at block)"
                } else {
                    ""
                };
                writeln!(
                    out,
                    "{mark} {icon} {}{path}{opens}",
                    bracket(&page.label, &page.highlights)
                )
                .unwrap();
            }
            SearchRow::Excerpt(excerpt) => {
                writeln!(out, "{mark}     {}", bracket(&excerpt.text, &excerpt.highlights)).unwrap();
            }
        }
    }
    out
}

/// Types `query` at `now` and lets the pause pass: returns the search the app should run.
fn type_and_wait(bar: &mut SearchBar, query: &str, now: i64) -> SearchRequest {
    bar.set_query(query, now);
    bar.due(now + SEARCH_DELAY_MS).unwrap()
}

#[test]
fn an_empty_query_lists_recent_pages_first_and_never_searches() {
    let (editor, pages) = sample();
    let mut bar = SearchBar::open(editor.workspace(), &[pages.recipes, pages.garden]).unwrap();
    insta::assert_snapshot!("recent", render(&bar));
    assert_eq!(bar.next_deadline(), None);
    assert!(!bar.searching());
    bar.set_query("   ", NOW);
    assert_eq!(bar.next_deadline(), None, "spaces are not a query");
    assert_eq!(bar.due(NOW + 10 * SEARCH_DELAY_MS), None);
    assert_eq!(
        bar.rows().len(),
        5,
        "live pages only: the trashed page is not listed"
    );
}

#[test]
fn titles_match_at_once_and_the_full_text_search_waits_for_a_pause() {
    let (editor, _) = sample();
    let mut bar = SearchBar::open(editor.workspace(), &[]).unwrap();
    bar.set_query("g", NOW);
    bar.set_query("ga", NOW + 100);
    insta::assert_snapshot!(render(&bar), @"> · [Ga]rden plan  —  Projects");
    assert!(bar.searching());
    assert_eq!(
        bar.next_deadline(),
        Some(NOW + 100 + SEARCH_DELAY_MS),
        "each key restarts the pause"
    );
    assert_eq!(bar.due(NOW + 100 + SEARCH_DELAY_MS - 1), None);
    let request = bar.due(NOW + 100 + SEARCH_DELAY_MS).unwrap();
    assert_eq!(request.query, "ga");
    assert_eq!(bar.due(NOW + 10_000), None, "asked for once");
    assert_eq!(bar.next_deadline(), None);
    bar.set_query("ga", NOW + 20_000);
    assert_eq!(bar.next_deadline(), None, "the same query again changes nothing");
}

#[test]
fn full_text_hits_are_grouped_by_page_title_matches_first() {
    let (editor, pages) = sample();
    let mut bar = SearchBar::open(editor.workspace(), &[]).unwrap();
    let request = type_and_wait(&mut bar, "garden", NOW);
    let (tomatoes, beds, soup) = (BlockId::new(), BlockId::new(), BlockId::new());
    let hits = vec![
        // Text-only first in the index's order, to show title matches still lead.
        hit(
            pages.journal,
            marked("Journal", &[]),
            false,
            &[
                (
                    tomatoes,
                    marked("planted tomatoes in the garden today", &[24..30]),
                ),
                (beds, marked("…raised garden beds", &[8..14])),
            ],
        ),
        hit(pages.garden, marked("Garden plan", &[0..6]), true, &[]),
        // In the trash: the index may not know yet.
        hit(pages.old, marked("Old garden notes", &[4..10]), true, &[]),
        // A title that doesn't match: only the text did.
        hit(
            pages.recipes,
            marked("Recipes", &[]),
            false,
            &[(soup, marked("garden soup", &[0..6]))],
        ),
        // A page that no longer exists.
        hit(PageId::new(), marked("Gone", &[]), false, &[]),
    ];
    assert!(bar.deliver(request.id, hits));
    assert!(!bar.searching());
    insta::assert_snapshot!("grouped", render(&bar));
    let destinations: Vec<Destination> = bar.rows().iter().map(SearchRow::destination).collect();
    assert_eq!(
        destinations,
        vec![
            Destination {
                page: pages.garden,
                block: None
            },
            Destination {
                page: pages.journal,
                block: Some(tomatoes)
            },
            Destination {
                page: pages.journal,
                block: Some(tomatoes)
            },
            Destination {
                page: pages.journal,
                block: Some(beds)
            },
            Destination {
                page: pages.recipes,
                block: Some(soup)
            },
            Destination {
                page: pages.recipes,
                block: Some(soup)
            },
        ]
    );
}

#[test]
fn a_title_the_index_matched_but_the_switcher_did_not_is_highlighted_from_the_index() {
    let (editor, pages) = sample();
    let mut bar = SearchBar::open(editor.workspace(), &[]).unwrap();
    let request = type_and_wait(&mut bar, "project", NOW);
    let hits = vec![
        hit(pages.projects, marked("Projects", &[0..8]), true, &[]),
        // The index's title is out of date: the live title is shown, without the index's marks.
        hit(pages.leaf, marked("Leaf", &[0..4]), true, &[]),
    ];
    assert!(bar.deliver(request.id, hits));
    insta::assert_snapshot!(render(&bar), @r"
    > · [Projects]
      · Leaf app  —  Projects
    ");
}

#[test]
fn hits_for_an_older_query_never_replace_newer_ones() {
    let (editor, pages) = sample();
    let mut bar = SearchBar::open(editor.workspace(), &[]).unwrap();
    let old = type_and_wait(&mut bar, "jour", NOW);
    let new = type_and_wait(&mut bar, "recipe", NOW + 1_000);
    assert!(new.id > old.id);
    let journal = vec![hit(pages.journal, marked("Journal", &[0..4]), true, &[])];
    assert!(!bar.deliver(old.id, journal.clone()), "stale");
    assert!(bar.searching(), "still waiting for the newest");
    insta::assert_snapshot!(render(&bar), @"> · [Recipe]s");
    let recipes = vec![hit(pages.recipes, marked("Recipes", &[0..6]), true, &[])];
    assert!(bar.deliver(new.id, recipes));
    assert!(
        !bar.deliver(old.id, journal),
        "still stale after the newest arrived"
    );
    insta::assert_snapshot!(render(&bar), @"> · [Recipe]s");
    bar.set_query("", NOW + 2_000);
    assert!(!bar.deliver(new.id, Vec::new()), "the query was cleared");
}

#[test]
fn hits_arriving_keep_the_highlight_on_the_same_row() {
    let (editor, pages) = sample();
    let mut bar = SearchBar::open(editor.workspace(), &[]).unwrap();
    let request = type_and_wait(&mut bar, "p", NOW);
    bar.press(SearchKey::Down); // Garden plan, under Projects
    let chosen = bar.rows()[bar.selected().unwrap()].destination();
    let block = BlockId::new();
    let hits = vec![hit(
        pages.journal,
        marked("Journal", &[]),
        false,
        &[(block, marked("a plan", &[2..3]))],
    )];
    assert!(bar.deliver(request.id, hits));
    assert_eq!(bar.rows()[bar.selected().unwrap()].destination(), chosen);
    // A row that disappears sends the highlight back to the top.
    let request = type_and_wait(&mut bar, "plan", NOW + 1_000);
    bar.press(SearchKey::Down);
    assert!(bar.deliver(request.id, Vec::new()));
    assert_eq!(bar.selected(), Some(0));
}

#[test]
fn up_and_down_wrap_enter_opens_and_escape_closes() {
    let (editor, pages) = sample();
    let mut bar = SearchBar::open(editor.workspace(), &[]).unwrap();
    assert_eq!(bar.selected(), Some(0));
    assert_eq!(bar.press(SearchKey::Up), SearchOutcome::Nothing);
    assert_eq!(bar.selected(), Some(4), "up from the first wraps to the last");
    assert_eq!(bar.press(SearchKey::Down), SearchOutcome::Nothing);
    assert_eq!(bar.selected(), Some(0), "down from the last wraps to the first");
    bar.select(3);
    assert_eq!(
        bar.press(SearchKey::Enter),
        SearchOutcome::Open(Destination {
            page: pages.journal,
            block: None
        })
    );
    bar.select(99);
    assert_eq!(bar.selected(), Some(3), "no such row: ignored");
    assert_eq!(bar.press(SearchKey::Escape), SearchOutcome::Close);
    bar.set_query("zzz", NOW);
    assert_eq!(bar.rows(), &[] as &[SearchRow]);
    assert_eq!(bar.selected(), None);
    assert_eq!(bar.press(SearchKey::Down), SearchOutcome::Nothing);
    assert_eq!(bar.press(SearchKey::Up), SearchOutcome::Nothing);
    assert_eq!(
        bar.press(SearchKey::Enter),
        SearchOutcome::Nothing,
        "nothing to open"
    );
    assert_eq!(bar.press(SearchKey::Escape), SearchOutcome::Close);
}

#[test]
fn mod_k_links_selected_text_and_otherwise_searches_mod_p_always_searches() {
    assert_eq!(shortcut_action(Shortcut::ModK, true), ShortcutAction::Link);
    assert_eq!(shortcut_action(Shortcut::ModK, false), ShortcutAction::OpenSearch);
    assert_eq!(shortcut_action(Shortcut::ModP, true), ShortcutAction::OpenSearch);
    assert_eq!(shortcut_action(Shortcut::ModP, false), ShortcutAction::OpenSearch);
}

#[test]
fn a_longer_pause_can_be_chosen() {
    let (editor, _) = sample();
    let mut bar = SearchBar::with_delay(editor.workspace(), &[], 500).unwrap();
    bar.set_query("leaf", NOW);
    assert_eq!(bar.next_deadline(), Some(NOW + 500));
    assert_eq!(bar.query(), "leaf");
}

/// One step of a session: typing, the clock moving on, or the app delivering the hits of some
/// earlier search (possibly late or out of order).
#[derive(Debug, Clone)]
enum Step {
    Type(String),
    Wait(i64),
    Deliver(usize),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        "[a-c ]{0,3}".prop_map(Step::Type),
        (0..400i64).prop_map(Step::Wait),
        (0..8usize).prop_map(Step::Deliver),
    ]
}

proptest! {
    /// However searches interleave with typing, the excerpts shown are always for the current query,
    /// and a search is only asked for after a pause.
    #[test]
    fn shown_hits_always_belong_to_the_current_query(steps in prop::collection::vec(step(), 1..40)) {
        let (editor, pages) = sample();
        let mut bar = SearchBar::open(editor.workspace(), &[]).unwrap();
        let mut now = NOW;
        let mut last_typed = NOW;
        let mut asked: Vec<SearchRequest> = Vec::new();
        for step in steps {
            match step {
                Step::Type(query) => {
                    if query != bar.query() {
                        last_typed = now;
                    }
                    bar.set_query(&query, now);
                }
                Step::Wait(ms) => now += ms,
                Step::Deliver(which) => {
                    if let Some(request) = asked.get(which % asked.len().max(1)) {
                        // Each search's hit carries its own query as the excerpt.
                        let hits = vec![hit(
                            pages.journal,
                            marked("Journal", &[]),
                            false,
                            &[(BlockId::new(), marked(&request.query, &[]))],
                        )];
                        bar.deliver(request.id, hits);
                    }
                }
            }
            if let Some(request) = bar.due(now) {
                prop_assert!(now - last_typed >= SEARCH_DELAY_MS, "searched before the pause");
                prop_assert_eq!(&request.query, bar.query());
                asked.push(request);
            }
            for row in bar.rows() {
                if let SearchRow::Excerpt(excerpt) = row {
                    prop_assert_eq!(excerpt.text.as_str(), bar.query());
                }
            }
        }
    }
}
