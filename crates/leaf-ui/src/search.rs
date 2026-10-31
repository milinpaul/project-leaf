//! The search bar's popup (Cmd/Ctrl+K with no text selected, or Cmd/Ctrl+P): a search field over
//! results grouped by page, near the top of the window like Notion's.
//!
//! It draws `leaf_editor::SearchBar` and forwards typing, Up/Down, Enter and clicks to it. Titles
//! match as the owner types; when typing pauses, the bar asks for a full-text search, which runs on a
//! background thread against the workspace's index (`leaf_search::Searcher`) and is handed back. The
//! bar ignores answers to anything but the newest query. Choosing a result opens the page at the
//! matching block. Drawing is in `view`.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable, KeyDownEvent, ParentElement as _, ScrollHandle,
    Styled as _, Subscription, Task, WeakEntity, Window, px,
};
use gpui_component::WindowExt as _;
use gpui_component::input::{InputEvent, InputState};
use leaf_editor::{BlockMatch, Marked, SearchBar, SearchHit, SearchKey, SearchOutcome, WorkspaceEditor};
use leaf_search::{Highlighted, PageHit, Searcher};

use crate::shell::{Shell, now_ms};

mod view;

/// The popup's width and its distance from the top of the window, in pixels.
const WIDTH: f32 = 640.0;
const TOP: f32 = 88.0;

/// The popup's content: the bar's state, its search field, and the search in flight.
pub struct SearchPanel {
    bar: SearchBar,
    input: Entity<InputState>,
    shell: WeakEntity<Shell>,
    /// The workspace folder whose index is searched; `None` for an in-memory workspace, which has
    /// titles only.
    root: Option<PathBuf>,
    scroll: ScrollHandle,
    /// Fires when typing has paused long enough to search; replaced on every keystroke.
    timer: Option<Task<()>>,
    /// The full-text search running, if any; dropping it stops waiting for the answer.
    running: Option<Task<()>>,
    /// Whether a search failure was logged already, so a missing index is reported once.
    reported: bool,
    _events: Subscription,
}

/// Opens the search bar over `editor`'s pages, searching the index in the folder `root`. Choosing a
/// result opens it in `shell`. Returns the popup's content (tests read it).
pub fn open(
    shell: WeakEntity<Shell>,
    editor: &WorkspaceEditor,
    root: Option<PathBuf>,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Option<Entity<SearchPanel>> {
    let bar = match SearchBar::open(editor.workspace(), editor.recent_pages()) {
        Ok(bar) => bar,
        Err(err) => {
            crate::log::log(format_args!("could not list pages for search: {err}"));
            return None;
        }
    };
    let panel = cx.new(|cx| SearchPanel::new(bar, shell, root, window, cx));
    let focus = panel.read(cx).input.read(cx).focus_handle(cx);
    let content = panel.clone();
    window.open_dialog(cx, move |dialog, _, _| {
        let (panel, chooser) = (content.clone(), content.clone());
        dialog
            .p_0()
            .w(px(WIDTH))
            .margin_top(px(TOP))
            .close_button(false)
            // Enter is the dialog's "OK": open the highlighted result. The panel closes the dialog
            // itself, and only when there is something to open, so Enter with no results stays.
            .on_ok(move |_, window, cx| {
                chooser.update(cx, |panel, cx| panel.choose(window, cx));
                false
            })
            .child(panel.clone())
    });
    window.focus(&focus, cx);
    Some(panel)
}

impl SearchPanel {
    fn new(
        bar: SearchBar,
        shell: WeakEntity<Shell>,
        root: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search pages and their text…"));
        let events = cx.subscribe_in(&input, window, |panel, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                let query = input.read(cx).value().to_string();
                panel.set_query(&query, cx);
            }
        });
        Self {
            bar,
            input,
            shell,
            root,
            scroll: ScrollHandle::new(),
            timer: None,
            running: None,
            reported: false,
            _events: events,
        }
    }

    /// The bar's state, for tests.
    #[cfg(test)]
    pub(crate) fn bar(&self) -> &SearchBar {
        &self.bar
    }

    fn set_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.bar.set_query(query, now_ms());
        self.scroll.scroll_to_item(0);
        self.schedule(cx);
        cx.notify();
    }

    /// Sets the timer for the bar's next deadline (replacing any earlier one).
    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.timer = self.bar.next_deadline().map(|deadline| {
            let wait = u64::try_from(deadline - now_ms()).unwrap_or(0);
            cx.spawn(async move |panel, cx| {
                cx.background_executor().timer(Duration::from_millis(wait)).await;
                let _ = panel.update(cx, |panel, cx| panel.search_now(deadline, cx));
            })
        });
    }

    /// The timer fired: runs the search that is due. The timer is trusted over the clock, so a
    /// timer that fires a hair early still searches.
    fn search_now(&mut self, deadline: i64, cx: &mut Context<Self>) {
        let Some(request) = self.bar.due(now_ms().max(deadline)) else {
            return;
        };
        let Some(root) = self.root.clone() else {
            self.receive(request.id, Ok(Vec::new()), cx);
            return;
        };
        self.running = Some(cx.spawn(async move |panel, cx| {
            let query = request.query;
            let found = cx.background_spawn(async move { full_text(&root, &query) }).await;
            let _ = panel.update(cx, |panel, cx| panel.receive(request.id, found, cx));
        }));
    }

    /// A search's answer arrived. Errors (no index yet, a damaged one being rebuilt) show as no
    /// full-text results; titles still match.
    fn receive(&mut self, id: u64, found: Result<Vec<SearchHit>, String>, cx: &mut Context<Self>) {
        let hits = found.unwrap_or_else(|err| {
            if !self.reported {
                crate::log::log(format_args!("search is unavailable for now: {err}"));
                self.reported = true;
            }
            Vec::new()
        });
        if self.bar.deliver(id, hits) {
            self.reveal_selected();
            cx.notify();
        }
    }

    /// Up and Down move the highlight (the field would otherwise move its caret) and keep it in view.
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = match event.keystroke.key.as_str() {
            "up" => SearchKey::Up,
            "down" => SearchKey::Down,
            _ => return,
        };
        cx.stop_propagation();
        self.bar.press(key);
        self.reveal_selected();
        cx.notify();
    }

    fn reveal_selected(&self) {
        if let Some(selected) = self.bar.selected() {
            self.scroll.scroll_to_item(selected);
        }
    }

    /// Opens the highlighted result and closes the popup. With no results, Enter does nothing.
    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let SearchOutcome::Open(destination) = self.bar.press(SearchKey::Enter) else {
            return;
        };
        window.close_dialog(cx);
        let _ = self
            .shell
            .update(cx, |shell, cx| shell.open_at(destination, window, cx));
    }
}

impl Focusable for SearchPanel {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

/// Runs `query` against the index in the workspace folder `root`. A read-only connection is opened
/// per search: it is quick, and it never waits for the indexer, which writes on its own connection.
fn full_text(root: &std::path::Path, query: &str) -> Result<Vec<SearchHit>, String> {
    let hits = Searcher::open(root)
        .and_then(|searcher| searcher.search(query))
        .map_err(|err| err.to_string())?;
    Ok(hits.into_iter().map(search_hit).collect())
}

/// The index's result as the editor's plain data.
fn search_hit(hit: PageHit) -> SearchHit {
    SearchHit {
        page: hit.page,
        title: marked(hit.title),
        title_matched: hit.title_matched,
        blocks: hit
            .blocks
            .into_iter()
            .map(|block| BlockMatch {
                block: block.block,
                excerpt: marked(block.excerpt),
            })
            .collect(),
    }
}

fn marked(text: Highlighted) -> Marked {
    Marked {
        text: text.text,
        ranges: text.ranges,
    }
}

#[cfg(test)]
mod tests;
