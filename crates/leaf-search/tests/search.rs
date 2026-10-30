//! Searching: matching, ranking, grouping and highlights (E11-S03).

mod common;

use common::TempDir;
use leaf_core::{BlockId, PageId};
use leaf_search::{
    BLOCKS_PER_PAGE, BlockText, MAX_PAGES, PageHit, PageText, PageVersion, RANKED_BLOCKS, SearchIndex,
    Searcher,
};
use proptest::prelude::*;

fn open(dir: &TempDir) -> SearchIndex {
    common::ok(SearchIndex::open(&dir.0)).0
}

fn text(title: &str, blocks: &[&str]) -> PageText {
    PageText {
        page: PageId::new(),
        title: title.to_owned(),
        blocks: blocks
            .iter()
            .map(|b| BlockText {
                block: BlockId::new(),
                kind: "paragraph".to_owned(),
                text: (*b).to_owned(),
            })
            .collect(),
    }
}

fn put(index: &mut SearchIndex, page: &PageText, saved_at: i64) {
    common::ok(index.index_page(
        page,
        PageVersion {
            saved_at,
            seq: saved_at,
        },
    ));
}

fn pages(hits: &[PageHit]) -> Vec<PageId> {
    hits.iter().map(|hit| hit.page).collect()
}

#[test]
fn words_match_as_prefixes_ignoring_case_and_accents() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    let page = text("Plans", &["The Café PROJECT"]);
    put(&mut index, &page, 1);

    for query in ["pro", "project", "PROJ", "cafe", "café", "CAFE pro", "the proj"] {
        assert_eq!(pages(&index.search(query).unwrap()), [page.page], "{query:?}");
    }
    assert!(
        index.search("projects").unwrap().is_empty(),
        "a longer word is not a prefix"
    );
    assert!(
        index.search("cafe nothing").unwrap().is_empty(),
        "every word must match"
    );
}

#[test]
fn quoted_words_match_as_a_phrase() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    let together = text("A", &["a red apple pie"]);
    let apart = text("B", &["an apple that is red"]);
    put(&mut index, &together, 1);
    put(&mut index, &apart, 1);

    assert_eq!(pages(&index.search(r#""red apple""#).unwrap()), [together.page]);
    assert_eq!(index.search("red apple").unwrap().len(), 2);
}

#[test]
fn query_syntax_in_the_input_is_searched_as_text_not_obeyed() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    put(&mut index, &text("Notes", &["keep NOT this"]), 1);

    for query in [
        "\"",
        "(",
        "NOT",
        "a AND",
        "title:keep",
        "*",
        "^keep",
        "keep -",
        "NEAR(keep this)",
        "keep\"s",
    ] {
        assert!(index.search(query).is_ok(), "{query:?}");
    }
    assert_eq!(index.search("NOT").unwrap().len(), 1, "NOT is a word here");
}

#[test]
fn an_empty_query_returns_nothing() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    put(&mut index, &text("Anything", &["at all"]), 1);
    for query in ["", "   ", "!!", "\"\""] {
        assert!(index.search(query).unwrap().is_empty(), "{query:?}");
    }
}

#[test]
fn title_matches_rank_above_body_matches() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    let body = text("Diary", &["garden garden garden garden"]);
    let titled = text("Garden ideas", &["nothing relevant"]);
    put(&mut index, &body, 9);
    put(&mut index, &titled, 1);

    let hits = index.search("garden").unwrap();
    assert_eq!(pages(&hits), [titled.page, body.page]);
    assert!(hits[0].title_matched);
    assert!(hits[0].blocks.is_empty(), "only the title matched");
    assert!(!hits[1].title_matched);
    assert_eq!(hits[1].title.text, "Diary");
    assert!(hits[1].title.ranges.is_empty(), "its title didn't match");
}

#[test]
fn equal_matches_put_the_most_recently_edited_page_first() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    let old = text("Same", &["identical words here"]);
    let new = text("Same", &["identical words here"]);
    put(&mut index, &old, 100);
    put(&mut index, &new, 200);

    assert_eq!(pages(&index.search("identical").unwrap()), [new.page, old.page]);
    assert_eq!(pages(&index.search("same").unwrap()), [new.page, old.page]);
}

#[test]
fn results_are_grouped_by_page_and_capped() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    let many = text(
        "Apple many",
        &[
            "apple one",
            "apple two",
            "apple three",
            "apple four",
            "apple five",
        ],
    );
    put(&mut index, &many, 1);
    for n in 0..MAX_PAGES + 10 {
        put(&mut index, &text(&format!("Page {n}"), &["apple"]), 1);
    }

    let hits = index.search("apple").unwrap();
    assert_eq!(hits.len(), MAX_PAGES);
    // Its title matches too, so it ranks first and is surely in the capped list.
    assert_eq!(hits[0].page, many.page);
    assert_eq!(hits[0].blocks.len(), BLOCKS_PER_PAGE, "best blocks only");
    assert!(hits.iter().all(|hit| hit.blocks.len() <= BLOCKS_PER_PAGE));

    let only = index.search("apple three").unwrap();
    assert_eq!(pages(&only), [many.page]);
    assert_eq!(only[0].blocks.len(), 1);
    assert_eq!(only[0].blocks[0].block, many.blocks[2].block);
    assert_eq!(only[0].blocks[0].kind, "paragraph");
}

#[test]
fn hits_carry_excerpts_with_the_matched_words_marked() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    let long: String =
        (0..60).map(|n| format!("word{n}")).collect::<Vec<_>>().join(" ") + " the crème brûlée recipe";
    let page = text("Desserts and crème", &[&long]);
    put(&mut index, &page, 1);

    let hit = &index.search("creme").unwrap()[0];
    let title: Vec<String> = hit
        .title
        .ranges
        .iter()
        .map(|r| chars(&hit.title.text, r))
        .collect();
    assert_eq!(title, ["crème"]);

    let excerpt = &hit.blocks[0].excerpt;
    assert!(
        excerpt.text.starts_with('…'),
        "cut at the start: {:?}",
        excerpt.text
    );
    assert!(excerpt.text.len() < long.len());
    let marked: Vec<String> = excerpt.ranges.iter().map(|r| chars(&excerpt.text, r)).collect();
    assert_eq!(marked, ["crème"]);
}

fn chars(text: &str, range: &std::ops::Range<usize>) -> String {
    text.chars().skip(range.start).take(range.len()).collect()
}

#[test]
fn a_searcher_reads_while_the_index_writes() {
    let dir = TempDir::new();
    assert!(Searcher::open(&dir.0).is_err(), "no index yet");
    let mut index = open(&dir);
    let searcher = Searcher::open(&dir.0).unwrap();
    assert!(
        searcher.search("hello").unwrap().is_empty(),
        "an empty index finds nothing"
    );

    let page = text("Hello", &["world"]);
    put(&mut index, &page, 1);
    assert_eq!(
        pages(&searcher.search("hello world").unwrap()),
        Vec::<PageId>::new(),
        "title and body words apart"
    );
    assert_eq!(pages(&searcher.search("world").unwrap()), [page.page]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Whatever is typed, searching never fails.
    #[test]
    fn any_input_searches_without_error(input in prop_oneof![any::<String>(), "[\"*^():+\\-~ a-zé]{0,24}"]) {
        let dir = TempDir::new();
        let mut index = open(&dir);
        put(&mut index, &text("Café NOT AND", &["a (quoted) \"text\" with * and ^ signs"]), 1);
        prop_assert!(index.search(&input).is_ok(), "{input:?} failed: {:?}", index.search(&input).err());
    }
}

#[test]
fn only_the_newest_matching_blocks_are_ranked() {
    let dir = TempDir::new();
    let mut index = open(&dir);
    // The best possible match, indexed first...
    let old = text("Old", &["harbour"]);
    put(&mut index, &old, 1);
    // ...then `RANKED_BLOCKS` weaker matches, newer, spread over pages.
    let per_page = 1_000;
    let filler: Vec<String> = (0..per_page)
        .map(|n| format!("a long line {n} about the harbour and many other words besides"))
        .collect();
    let filler: Vec<&str> = filler.iter().map(String::as_str).collect();
    let page_count = usize::try_from(RANKED_BLOCKS).unwrap() / per_page;
    for n in 0..page_count {
        put(&mut index, &text(&format!("Page {n}"), &filler), 2);
    }

    let hits = index.search("harbour").unwrap();
    assert!(!hits.is_empty());
    assert!(
        !pages(&hits).contains(&old.page),
        "older than the newest {RANKED_BLOCKS}"
    );

    // Editing it makes it new again, and as the best match it comes first.
    put(&mut index, &old, 3);
    assert_eq!(index.search("harbour").unwrap()[0].page, old.page);
    // Its title is always searched, however many blocks match.
    assert_eq!(index.search("old").unwrap()[0].page, old.page);
}
