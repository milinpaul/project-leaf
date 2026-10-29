//! The welcome screen: where workspaces are created, opened and switched. It shows on the first
//! launch, when the last workspace can't be opened (saying why), and from File → Switch Workspace….
//!
//! It never opens a workspace itself: it tells the app view which folder to open or create
//! ([`WelcomeEvent`]), and shows any problem the app view reports back. Choices that need no window
//! (how a path is shortened, what an error means) are in `model`.

use std::path::{Path, PathBuf};

use gpui::{
    AppContext as _, Context, EventEmitter, FocusHandle, Focusable, KeyDownEvent, PathPromptOptions, Window,
};
use gpui_component::input::{InputEvent, InputState};
use leaf_store::{FolderKind, RecentWorkspace};

use crate::settings;

pub(crate) mod model;
mod view;

/// What the welcome screen asks the app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WelcomeEvent {
    /// Open the workspace in this folder.
    Open(PathBuf),
    /// Create a workspace in this folder (new, or an existing empty one), then open it.
    Create(PathBuf),
}

/// A line under the actions: a problem, or a question with a button that answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Message {
    pub(crate) text: String,
    /// "Create a workspace here" for an empty folder that was picked to open.
    pub(crate) create_here: Option<PathBuf>,
}

/// The create form: a name and the folder the workspace goes in.
pub(crate) struct CreateForm {
    pub(crate) name: gpui::Entity<InputState>,
    pub(crate) location: PathBuf,
    pub(crate) problem: Option<String>,
}

/// The welcome screen's state.
pub struct Welcome {
    pub(crate) recent: Vec<RecentWorkspace>,
    /// The highlighted recent workspace (arrow keys move it, Enter opens it).
    pub(crate) selected: Option<usize>,
    pub(crate) message: Option<Message>,
    pub(crate) create: Option<CreateForm>,
    /// Where new workspaces go unless the owner picks somewhere else (`~/Documents/Leaf`).
    pub(crate) default_location: PathBuf,
    pub(crate) focus: FocusHandle,
    pub(crate) now: i64,
}

impl EventEmitter<WelcomeEvent> for Welcome {}

impl Focusable for Welcome {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Welcome {
    /// The welcome screen, explaining `reason` if it is showing because a workspace couldn't open.
    pub fn new(reason: Option<String>, default_location: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut welcome = Self {
            recent: Vec::new(),
            selected: None,
            message: reason.map(|text| Message {
                text,
                create_here: None,
            }),
            create: None,
            default_location,
            focus: cx.focus_handle(),
            now: crate::shell::now_ms(),
        };
        welcome.reload(cx);
        welcome
    }

    /// Reads the recent list again (after opening, removing or locating a workspace).
    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        self.recent = settings::get(cx).recent_workspaces(Path::exists);
        self.now = crate::shell::now_ms();
        self.selected = self.selected.filter(|&at| at < self.recent.len());
        cx.notify();
    }

    /// Shows a problem the app hit while opening or creating (`None` clears it).
    pub fn show_problem(&mut self, problem: Option<String>, cx: &mut Context<Self>) {
        match (&mut self.create, problem) {
            (Some(form), problem) => form.problem = problem,
            (None, problem) => {
                self.message = problem.map(|text| Message {
                    text,
                    create_here: None,
                });
            }
        }
        cx.notify();
    }

    /// Opens recent workspace `index` if its folder is there.
    pub(crate) fn open_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.recent.get(index).filter(|entry| !entry.missing) {
            cx.emit(WelcomeEvent::Open(entry.path.clone()));
        }
    }

    /// "Remove" on a missing workspace: forgets it (the folder, wherever it is, is not touched).
    pub(crate) fn remove_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.recent.get(index) {
            let path = entry.path.clone();
            settings::update(cx, |settings| settings.forget_workspace(&path));
            settings::save(cx);
            crate::app::refresh_menus(cx);
            self.reload(cx);
        }
    }

    /// "Locate…" on a missing workspace: the owner points at where it went; the old entry is replaced.
    pub(crate) fn locate_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(old) = self.recent.get(index).map(|entry| entry.path.clone()) else {
            return;
        };
        Self::pick_folder("Locate", cx, move |welcome, folder, cx| {
            settings::update(cx, |settings| settings.forget_workspace(&old));
            welcome.open_folder(folder, cx);
        });
    }

    /// "Open existing folder": the system's folder picker, then [`Welcome::open_folder`].
    pub fn choose_folder_to_open(&mut self, cx: &mut Context<Self>) {
        self.message = None; // an old problem doesn't apply to the next folder
        Self::pick_folder("Open", cx, Self::open_folder);
    }

    /// Opens `folder` if it is a workspace; otherwise says why not, offering to create a workspace
    /// there if the folder is empty.
    pub(crate) fn open_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        let name = folder.file_name().map_or_else(
            || folder.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        self.message = match leaf_store::inspect(&folder) {
            FolderKind::Workspace => {
                cx.emit(WelcomeEvent::Open(folder));
                None
            }
            FolderKind::Empty => Some(Message {
                text: format!("“{name}” is empty. Create a new workspace in it?"),
                create_here: Some(folder),
            }),
            FolderKind::Other => Some(Message {
                text: format!(
                    "“{name}” isn't a Leaf workspace. Pick a .leaf folder, or an empty folder to start one in."
                ),
                create_here: None,
            }),
            FolderKind::Missing => Some(Message {
                text: format!("Leaf can't find “{name}”."),
                create_here: None,
            }),
        };
        cx.notify();
    }

    /// "Create new workspace": shows the form with a name to start from.
    pub fn start_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| InputState::new(window, cx).default_value("My Workspace"));
        name.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.subscribe_in(&name, window, |welcome, _, event, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                welcome.submit_create(cx);
            }
        })
        .detach();
        self.create = Some(CreateForm {
            name,
            location: self.default_location.clone(),
            problem: None,
        });
        self.message = None;
        cx.notify();
    }

    /// The create form's Create button (or Enter in its name field).
    pub(crate) fn submit_create(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.create else {
            return;
        };
        let name = form.name.read(cx).value().to_string();
        match leaf_store::new_workspace_path(&form.location, &name) {
            Ok(path) => {
                form.problem = None;
                cx.emit(WelcomeEvent::Create(path));
            }
            Err(problem) => form.problem = Some(problem.to_string()),
        }
        cx.notify();
    }

    /// Leaves the create form.
    pub(crate) fn cancel_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.create = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// "Change…" next to the create form's location.
    pub(crate) fn choose_location(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.create {
            form.problem = None;
        }
        Self::pick_folder("Choose", cx, |welcome, folder, cx| {
            if let Some(form) = &mut welcome.create {
                form.location = folder;
                form.problem = None;
            }
            cx.notify();
        });
    }

    /// Shows the system's folder picker and hands the chosen folder to `then`. If the system has no
    /// picker (some Linux desktops), says so instead.
    fn pick_folder(
        button: &'static str,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, PathBuf, &mut Context<Self>) + 'static,
    ) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(button.into()),
        });
        cx.spawn(async move |welcome, cx| {
            let result = chosen.await;
            let _ = welcome.update(cx, |welcome, cx| match result {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => then(welcome, paths.swap_remove(0), cx),
                Ok(Ok(_)) | Err(_) => {} // cancelled, or the picker went away
                Ok(Err(err)) => {
                    // Some Linux desktops have no folder picker service; the details go to the log.
                    eprintln!("leaf: no folder picker: {err:#}");
                    welcome.show_problem(
                        Some(
                            "This system has no folder picker. Start Leaf with the folder instead: leaf <folder>"
                                .to_owned(),
                        ),
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    /// Arrow keys move through the recent list and Enter opens; Escape leaves the create form.
    pub(crate) fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.create.is_some() {
            if event.keystroke.key == "escape" {
                cx.stop_propagation();
                self.cancel_create(window, cx);
            }
            return;
        }
        match event.keystroke.key.as_str() {
            "down" | "up" => {
                self.selected = model::step(self.selected, self.recent.len(), event.keystroke.key == "down");
                cx.notify();
            }
            "enter" => {
                if let Some(index) = self.selected {
                    self.open_recent(index, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

#[cfg(test)]
mod tests;
