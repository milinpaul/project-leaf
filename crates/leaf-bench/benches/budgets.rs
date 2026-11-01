//! The performance budgets in `PRODUCT_SPEC.md`, measured: `just bench`.
//!
//! - Open a 5,000-block page: < 100 ms (`open_page`).
//! - Keystroke to paint on a 10,000-block page: < 16 ms. Drawing happens in the window; here
//!   `keystroke` times one typed character, which includes the editor bringing its view model up to
//!   date. `view_update` times that update on its own, and `view_rebuild` a full rebuild, to show
//!   what the incremental update saves.
//! - Saving a keystroke's change (`save`), which autosave does in the background.
//! - A 10,000-row database (E09-S10, budgets in `PRODUCT_SPEC.md`):
//!   - opening it to its first screen: its saved document loaded, the first rows read and the table built, < 100 ms
//!     (`database_open`);
//!   - reading every row after that, which the app does in the background, < 1 s (`database_read_all`);
//!   - changing the sort, or the filter, and building the table again, < 50 ms (`database_sort`, `database_filter`);
//!   - editing one cell and building the table again, < 16 ms (`database_edit_cell`);
//!   - and, from E09-S07, the sorted and filtered table built (`database_view`) and one character typed into a text
//!     filter (`database_filter_keystroke`).
//!
//! Benches that change the page take the change outside the timing (`with_inputs`) and use one
//! iteration per sample, so each timed call sees exactly one new keystroke.

#![allow(clippy::unwrap_used, reason = "a bench whose setup fails should stop loudly")]

use std::cell::RefCell;

use divan::Bencher;
use leaf_bench::{
    DATABASE_ROWS, OPEN_PAGE_BLOCKS, SAVED_AT, SEED, SavedPage, ViewedPage, caret_mid_page, saved_database,
    sorted_filtered_database, typing_editor,
};
use leaf_fixtures::BIG_PAGE_BLOCKS;
use leaf_model::CellValue;
use leaf_store::DocKey;

fn main() {
    divan::main();
}

/// Load a saved 5,000-block page, start an editor on it and build the view model the window draws.
#[divan::bench(sample_count = 50)]
fn open_page(bencher: Bencher) {
    let saved = SavedPage::new(SEED, OPEN_PAGE_BLOCKS).unwrap();
    bencher.bench_local(|| saved.open().unwrap());
}

/// Type one character in the middle of a 10,000-block page. Ten per sample, so the paragraph grows
/// by at most a thousand characters over the run, as real typing would.
#[divan::bench(sample_count = 100, sample_size = 10)]
fn keystroke(bencher: Bencher) {
    let editor = RefCell::new(typing_editor(SEED, BIG_PAGE_BLOCKS).unwrap());
    bencher.bench_local(|| editor.borrow_mut().insert_text("a").unwrap());
}

/// Bring a 10,000-block page's view model up to date after one typed character (typed through the
/// model, outside the timing). The editor does this inside every keystroke, so `keystroke` already
/// includes it; this shows its share.
#[divan::bench(sample_count = 200, sample_size = 1)]
fn view_update(bencher: Bencher) {
    let page = RefCell::new(ViewedPage::new(SEED, BIG_PAGE_BLOCKS).unwrap());
    bencher
        .with_inputs(|| page.borrow_mut().type_char().unwrap())
        .bench_local_values(|()| page.borrow_mut().refresh().unwrap());
}

/// Build a 10,000-block page's view model from scratch, for comparison with `view_update`.
#[divan::bench(sample_count = 20)]
fn view_rebuild(bencher: Bencher) {
    let editor = typing_editor(SEED, BIG_PAGE_BLOCKS).unwrap();
    bencher.bench_local(|| editor.rows().unwrap());
}

/// Save one keystroke's change to a 10,000-block page (typed outside the timing). Over 200 saves
/// this includes one compaction of the saved updates, as real autosaves would.
#[divan::bench(sample_count = 200, sample_size = 1)]
fn save(bencher: Bencher) {
    let mut saved = SavedPage::new(SEED, BIG_PAGE_BLOCKS).unwrap();
    let mut editor = saved.open().unwrap();
    caret_mid_page(&mut editor).unwrap();
    let key = DocKey::Page(saved.page);
    let editor = RefCell::new(editor);
    let store = RefCell::new(saved.folder.store_mut());
    bencher
        .with_inputs(|| editor.borrow_mut().insert_text("a").unwrap())
        .bench_local_values(|_| {
            store
                .borrow_mut()
                .save(key, editor.borrow().doc(), SAVED_AT)
                .unwrap()
        });
}

/// Build the table of a 10,000-row database, sorted by price and filtered to one status (every row already read).
#[divan::bench(sample_count = 50)]
fn database_view(bencher: Bencher) {
    let (editor, _) = sorted_filtered_database(DATABASE_ROWS).unwrap();
    let editor = RefCell::new(editor);
    bencher.bench_local(|| editor.borrow_mut().view(|_| String::new(), 0).unwrap());
}

/// Type one character into a text filter on that database and build the table it then shows.
#[divan::bench(sample_count = 50)]
fn database_filter_keystroke(bencher: Bencher) {
    let (editor, _) = sorted_filtered_database(DATABASE_ROWS).unwrap();
    let editor = RefCell::new(editor);
    let typed = RefCell::new(String::from("Item "));
    bencher.bench_local(|| {
        let mut typed = typed.borrow_mut();
        typed.push('1');
        let mut editor = editor.borrow_mut();
        editor.type_filter_text(1, &typed).unwrap();
        editor.view(|_| String::new(), 0).unwrap()
    });
}

/// Open a saved 10,000-row database as the app does: load the document, read the first screen of rows, build the table.
#[divan::bench(sample_count = 30)]
fn database_open(bencher: Bencher) {
    let saved = saved_database(DATABASE_ROWS).unwrap();
    bencher.bench_local(|| {
        let mut editor = saved.open().unwrap();
        editor.view(|_| String::new(), 0).unwrap()
    });
}

/// Read every row of that database once its first screen is shown (opened outside the timing).
#[divan::bench(sample_count = 10, sample_size = 1)]
fn database_read_all(bencher: Bencher) {
    let saved = saved_database(DATABASE_ROWS).unwrap();
    bencher
        .with_inputs(|| saved.open().unwrap())
        .bench_local_values(|mut editor| editor.read_more(usize::MAX).unwrap());
}

/// Turn the sort on the sorted, filtered database around and build the table it then shows.
#[divan::bench(sample_count = 50)]
fn database_sort(bencher: Bencher) {
    let saved = saved_database(DATABASE_ROWS).unwrap();
    let editor = RefCell::new(saved.sorted_filtered().unwrap());
    let descending = RefCell::new(false);
    bencher.bench_local(|| {
        let mut descending = descending.borrow_mut();
        *descending = !*descending;
        let mut editor = editor.borrow_mut();
        editor.set_sort_descending(0, *descending).unwrap();
        editor.view(|_| String::new(), 0).unwrap()
    });
}

/// Pick another status in the filter of that database and build the table it then shows.
#[divan::bench(sample_count = 50)]
fn database_filter(bencher: Bencher) {
    let saved = saved_database(DATABASE_ROWS).unwrap();
    let editor = RefCell::new(saved.sorted_filtered().unwrap());
    let next = RefCell::new(0_usize);
    bencher.bench_local(|| {
        let mut next = next.borrow_mut();
        *next = (*next + 1) % saved.options.len();
        let mut editor = editor.borrow_mut();
        editor
            .set_filter_value(0, Some(CellValue::Option(saved.options[*next])))
            .unwrap();
        editor.view(|_| String::new(), 0).unwrap()
    });
}

/// Type a new name into one cell of that database and build the table it then shows.
#[divan::bench(sample_count = 100)]
fn database_edit_cell(bencher: Bencher) {
    let saved = saved_database(DATABASE_ROWS).unwrap();
    let editor = RefCell::new(saved.sorted_filtered().unwrap());
    let count = RefCell::new(0_usize);
    bencher.bench_local(|| {
        let mut count = count.borrow_mut();
        *count += 1;
        let row = saved.rows[*count % saved.rows.len()];
        let mut editor = editor.borrow_mut();
        editor
            .commit_text(row, saved.name, &format!("Renamed {count}"))
            .unwrap();
        editor.view(|_| String::new(), 0).unwrap()
    });
}
