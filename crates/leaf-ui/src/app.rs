//! A window's top view: the welcome screen or a workspace, and the moves between them.
//!
//! At launch it opens the workspace named on the command line, else the one opened last; when there
//! is none (first launch) or it can't be opened, the welcome screen shows, saying why. Switching
//! workspace drops the open one first, which saves everything and releases its folder.

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Subscription, Window, div,
};
use gpui_component::Root;

use crate::menus::{NewWorkspace, OpenRecent, OpenWorkspace, SwitchWorkspace};
use crate::shell::{self, Shell, Storage};
use crate::welcome::{Welcome, WelcomeEvent, model};
use crate::{menus, settings};

/// What the window shows.
enum Screen {
    /// Only while the view is being built.
    Starting,
    /// The welcome screen, and the subscription to what it asks for (kept alive with it).
    Welcome {
        view: Entity<Welcome>,
        _events: Subscription,
    },
    Workspace(Entity<Shell>),
}

/// The window's top view.
pub struct AppView {
    screen: Screen,
    /// The folder of the workspace showing, if one is.
    open_path: Option<PathBuf>,
    /// Where new workspaces go by default (`~/Documents/Leaf`).
    default_location: PathBuf,
}

/// Builds a window's root: the workspace at `named` (from the command line) or the last one opened,
/// else the welcome screen. `default_location` is where the welcome screen creates workspaces unless
/// the owner picks another folder.
pub fn app_root(
    named: Option<PathBuf>,
    default_location: PathBuf,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Root> {
    let view = cx.new(|cx| AppView::launch(named, default_location, window, cx));
    cx.new(|cx| Root::new(view, window, cx))
}

/// Rebuilds the menu bar, so File → Open Recent lists the current recent workspaces.
pub fn refresh_menus(cx: &mut App) {
    let recent = settings::get(cx).recent_workspaces(Path::exists);
    cx.set_menus(menus::menus(&recent));
}

impl AppView {
    pub(crate) fn launch(
        named: Option<PathBuf>,
        default_location: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut app = Self {
            screen: Screen::Starting,
            open_path: None,
            default_location,
        };
        let last = settings::get(cx).last_workspace();
        match named.or(last) {
            Some(path) => {
                if let Err(problem) = app.open(&path, window, cx) {
                    app.show_welcome(Some(problem), window, cx);
                }
            }
            None => {
                app.show_welcome(None, window, cx);
            }
        }
        app
    }

    /// Opens the workspace at `path` (never creating one), or says why it can't. On failure the
    /// screen stays as it was, so the welcome screen can show the problem.
    pub fn open(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        if self.open_path.as_deref() == Some(path) {
            return Ok(()); // already showing
        }
        let name = display_name(path);
        let (storage, workspace) = leaf_store::Workspace::open(path)
            .and_then(Storage::open)
            .map_err(|err| model::open_problem(&name, &err))?;
        // Remembered at once (not only on quit), so a crash still reopens this workspace next time.
        settings::update(cx, |settings| settings.remember_workspace(path, shell::now_ms()));
        settings::save(cx);
        refresh_menus(cx);
        let shell = cx.new(|cx| Shell::with_storage(storage, workspace, window, cx));
        // Replacing the screen drops the previous workspace, which saves it and releases its folder.
        self.screen = Screen::Workspace(shell);
        self.open_path = Some(path.to_owned());
        cx.notify();
        Ok(())
    }

    /// Creates a workspace at `path` (a new folder, or an existing empty one) and opens it.
    fn create(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        let name = display_name(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("Leaf couldn't make the folder for “{name}”: {err}"))?;
        }
        let created = leaf_store::Workspace::create(path).map_err(|err| match err {
            leaf_store::StoreError::AlreadyExists(_) => {
                format!("There's already something called “{name}” there.")
            }
            other => format!("Leaf couldn't create “{name}”: {other}"),
        })?;
        drop(created); // releases the folder lock so `open` can take it
        self.open(path, window, cx)
    }

    /// Shows the welcome screen, explaining `reason` if given. Any open workspace is closed (saved).
    pub fn show_welcome(
        &mut self,
        reason: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Welcome> {
        let default_location = self.default_location.clone();
        let welcome = cx.new(|cx| Welcome::new(reason, default_location, cx));
        let events = cx.subscribe_in(
            &welcome,
            window,
            |app, welcome, event: &WelcomeEvent, window, cx| {
                let result = match event {
                    WelcomeEvent::Open(path) => app.open(path, window, cx),
                    WelcomeEvent::Create(path) => app.create(path, window, cx),
                };
                if let Err(problem) = result {
                    welcome.update(cx, |welcome, cx| {
                        welcome.reload(cx);
                        welcome.show_problem(Some(problem), cx);
                    });
                }
            },
        );
        window.focus(&welcome.read(cx).focus_handle(cx), cx);
        self.screen = Screen::Welcome {
            view: welcome.clone(),
            _events: events,
        };
        self.open_path = None;
        cx.notify();
        welcome
    }

    /// The open workspace's shell, if a workspace is showing.
    #[must_use]
    pub fn shell(&self) -> Option<&Entity<Shell>> {
        match &self.screen {
            Screen::Workspace(shell) => Some(shell),
            _ => None,
        }
    }

    /// The welcome screen, if it is showing.
    #[must_use]
    pub fn welcome(&self) -> Option<&Entity<Welcome>> {
        match &self.screen {
            Screen::Welcome { view, .. } => Some(view),
            _ => None,
        }
    }
}

/// The folder name without `.leaf`, for messages.
fn display_name(path: &Path) -> String {
    path.file_stem().map_or_else(
        || path.display().to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match &self.screen {
            Screen::Starting => div().into_any_element(),
            Screen::Welcome { view, .. } => view.clone().into_any_element(),
            Screen::Workspace(shell) => shell.clone().into_any_element(),
        };
        div()
            .size_full()
            .on_action(cx.listener(|app, _: &SwitchWorkspace, window, cx| {
                app.show_welcome(None, window, cx);
            }))
            .on_action(cx.listener(|app, _: &NewWorkspace, window, cx| {
                let welcome = app.show_welcome(None, window, cx);
                welcome.update(cx, |welcome, cx| welcome.start_create(window, cx));
            }))
            .on_action(cx.listener(|app, _: &OpenWorkspace, window, cx| {
                let welcome = app.show_welcome(None, window, cx);
                welcome.update(cx, Welcome::choose_folder_to_open);
            }))
            .on_action(cx.listener(|app, action: &OpenRecent, window, cx| {
                if let Err(problem) = app.open(&action.path, window, cx) {
                    app.show_welcome(Some(problem), window, cx);
                }
            }))
            .child(content)
    }
}

#[cfg(test)]
mod tests;
