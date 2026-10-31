//! The shell's views of databases (E09-S09): one per database shown, on its own page or inline in a page, with the
//! active one the table actions work on. Making a view loads its database and reads the first screen of rows; the rest
//! are read a chunk at a time on a timer.

use gpui::{AppContext as _, Context, Window};
use gpui_component::calendar::CalendarState;
use gpui_component::input::InputState;
use gpui_component::table::TableState;
use leaf_core::{DatabaseId, PageId};
use leaf_editor::FIRST_SCREEN_ROWS;

use super::super::{Shell, report};
use super::table::DatabaseTable;
use super::{DatabasePage, READ_CHUNK, READ_TICK, query};

impl Shell {
    /// The view of the active database: the one the person is working in, on its own page or inline (E09-S09).
    pub(super) fn database_view(&self) -> Option<&DatabasePage> {
        let page = self.active_database?;
        self.databases.iter().find(|d| d.page == page)
    }

    pub(super) fn database_view_mut(&mut self) -> Option<&mut DatabasePage> {
        let page = self.active_database?;
        self.databases.iter_mut().find(|d| d.page == page)
    }

    /// Makes the database of `page` the one the table actions work on. Every press inside a database's view does
    /// this first, so with several inline databases on a page each acts on its own.
    pub(super) fn activate_database(&mut self, page: PageId) {
        if self.active_database != Some(page) && self.databases.iter().any(|d| d.page == page) {
            self.active_database = Some(page);
        }
    }

    /// Makes sure the database page `page` (database `id`) has a view, loading the database the first time.
    pub(super) fn ensure_database_view(
        &mut self,
        page: PageId,
        id: DatabaseId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.databases.iter().any(|d| d.page == page) {
            return;
        }
        self.load_database(id);
        let shell = cx.entity().downgrade();
        let text = cx.new(|cx| InputState::new(window, cx));
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search or create an option"));
        let calendar = cx.new(|cx| CalendarState::new(window, cx));
        let column_name = cx.new(|cx| InputState::new(window, cx).placeholder("Column name"));
        let option_name = super::option_menu::name_input(window, cx);
        let (filter_text, filter_calendar) = query::inputs(window, cx);
        let delegate = DatabaseTable {
            view: leaf_editor::TableView {
                columns: Vec::new(),
                rows: Vec::new(),
                complete: false,
                total_rows: 0,
            },
            editing: None,
            picker: None,
            text: text.clone(),
            query: query.clone(),
            calendar: calendar.clone(),
            column_menu: None,
            add_menu: None,
            column_name: column_name.clone(),
            option_menu: None,
            option_name: option_name.clone(),
            shell,
        };
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .cell_selectable(true)
                .row_selectable(false)
                .row_header(false)
                .col_movable(true)
                .col_resizable(true)
                .sortable(false)
        });
        // Each event marks its own database active first, so the handler acts on the view it came from.
        macro_rules! sub {
            ($entity:expr, $handler:path) => {
                cx.subscribe_in($entity, window, move |shell, e, event, window, cx| {
                    shell.activate_database(page);
                    $handler(shell, e, event, window, cx);
                })
            };
        }
        let subscriptions = vec![
            sub!(&table, Self::on_table_event),
            sub!(&text, Self::on_cell_text_event),
            sub!(&query, Self::on_option_query_event),
            sub!(&calendar, Self::on_calendar_event),
            sub!(&column_name, Self::on_column_name_event),
            cx.subscribe_in(&option_name, window, move |shell, _, event, window, cx| {
                shell.activate_database(page);
                shell.on_option_name_event(super::cells::PickerFor::Table, event, window, cx);
            }),
            sub!(&filter_text, Self::on_filter_text_event),
            sub!(&filter_calendar, Self::on_filter_calendar_event),
        ];
        self.databases.push(DatabasePage {
            page,
            id,
            table,
            text,
            query,
            calendar,
            column_name,
            query_menu: None,
            filter_text,
            filter_calendar,
            editing: None,
            reading: None,
            _subscriptions: subscriptions,
        });
        if let Err(err) = self.editor.read_database(id, FIRST_SCREEN_ROWS) {
            report("could not read the database", err);
        }
        let active = self.active_database.replace(page);
        self.refresh_database_table(cx);
        self.read_rest_of_database(cx);
        self.active_database = active.or(Some(page));
    }

    /// Hands the editor the database's saved document, if it has none in memory yet.
    fn load_database(&mut self, id: DatabaseId) {
        if self.editor.database(id).is_some() {
            return;
        }
        let Some(storage) = &self.storage else {
            return;
        };
        match storage.load_database(id) {
            Ok(database) => self.editor.insert_database(id, database.unwrap_or_default()),
            Err(err) => report("could not load the database", err),
        }
    }

    /// Reads the rest of the database a chunk at a time, redrawing as rows arrive.
    fn read_rest_of_database(&mut self, cx: &mut Context<Self>) {
        let Some(database) = self.database_view_mut() else {
            return;
        };
        let id = database.id;
        database.reading = Some(cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor().timer(READ_TICK).await;
                let done = shell.update(cx, |shell, cx| {
                    let done = shell.editor.read_database(id, READ_CHUNK).unwrap_or(true);
                    shell.refresh_database_table(cx);
                    done
                });
                if !matches!(done, Ok(false)) {
                    break;
                }
            }
        }));
    }
}
