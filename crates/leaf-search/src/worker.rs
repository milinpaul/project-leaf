//! The background thread that keeps a workspace's index current.
//!
//! Indexing never runs on the UI thread: the app only [`SearchWorker::poke`]s the worker after
//! autosave writes something, which is a channel send and never waits. The worker then
//! reconciles the index with what is saved (see [`crate::Syncer`]). Search is a convenience, so
//! any failure here is logged and repaired by rebuilding; it never reaches editing or saving.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::sync::{Finished, Syncer};
use crate::{IndexState, PageSource, SearchIndex};

/// How many times in a row the worker repairs the index straight away after an error, before it
/// waits for the next save to try again. Stops a lasting problem (a full disk) from spinning.
const RETRIES: u32 = 1;

/// Handle to a workspace's indexing thread. Dropping it stops the thread.
#[derive(Debug)]
pub struct SearchWorker {
    pokes: Option<Sender<()>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SearchWorker {
    /// Starts indexing the workspace folder at `workspace_root`, reading saved documents from
    /// `source`. Opens (or creates) the index, rebuilds it if needed, then waits for pokes.
    ///
    /// If the thread can't be started, search is simply unavailable: the failure is logged and the
    /// handle does nothing.
    pub fn start<S: PageSource + 'static>(workspace_root: PathBuf, source: S) -> Self {
        let (pokes, inbox) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let spawned = thread::Builder::new()
            .name("leaf-search".into())
            .spawn(move || run(&workspace_root, source, &inbox, &stopping));
        let thread = match spawned {
            Ok(thread) => Some(thread),
            Err(err) => {
                tracing::warn!(%err, "could not start the search indexer; search is unavailable");
                None
            }
        };
        Self {
            pokes: Some(pokes),
            stop,
            thread,
        }
    }

    /// Tells the worker that something was saved. Never blocks; pokes that arrive while it is busy
    /// are merged into one pass.
    pub fn poke(&self) {
        if let Some(pokes) = &self.pokes {
            // Fails only if the thread has ended (logged when it did); nothing more to do.
            let _ = pokes.send(());
        }
    }
}

impl Drop for SearchWorker {
    /// Stops a rebuild between pages (it reruns next time), but still applies pokes already sent,
    /// so the last edits before quitting are indexed. Waits for the thread to finish.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.pokes = None;
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("the search indexer stopped with a panic");
        }
    }
}

/// The thread's body: open, rebuild if needed, then reconcile after every burst of pokes.
fn run<S: PageSource>(root: &std::path::Path, source: S, inbox: &Receiver<()>, stop: &AtomicBool) {
    let (index, state) = match SearchIndex::open(root) {
        Ok(opened) => opened,
        Err(err) => {
            tracing::warn!(%err, "could not open the search index; search is unavailable");
            return;
        }
    };
    let mut syncer = Syncer::new(index, source);
    let mut rebuild = state == IndexState::NeedsRebuild;
    let mut failures = 0;
    let stopping = || stop.load(Ordering::Relaxed);
    loop {
        let result = if rebuild {
            syncer.rebuild(&stopping)
        } else {
            // Not stoppable: after a poke this is a handful of pages, and it carries the last
            // edits before quitting.
            syncer.reconcile(&|| false)
        };
        match result {
            Ok(Finished::Done) => {
                rebuild = false;
                failures = 0;
            }
            Ok(Finished::Stopped) => return,
            Err(err) => {
                tracing::warn!(%err, "search index error; rebuilding it");
                if let Err(err) = syncer.recreate() {
                    tracing::warn!(%err, "could not recreate the search index; search is unavailable");
                    return;
                }
                rebuild = true;
                failures += 1;
                if failures <= RETRIES {
                    continue;
                }
            }
        }
        // Wait for the next save; then take every poke already queued, so a burst is one pass.
        if inbox.recv().is_err() {
            return; // the app dropped its handle and every poke has been handled
        }
        while inbox.try_recv().is_ok() {}
    }
}
