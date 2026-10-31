//! Writes a small, hand-written workspace that shows Leaf off: a trip to Japan planned in pages with
//! icons and a cover, using most block kinds and marks, and links to its own pages. The README's screenshots are made from it:
//! `just demo <new folder>`, then `just screenshot <folder> <out.png>`. Due dates are set relative
//! to today, so one action item is always overdue and one always coming up.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use leaf_core::{BlockId, PageId};
use leaf_fixtures::{Fixture, FixtureError, save_to_folder};
use leaf_model::{
    BlockKind, DueDate, DueTime, Mark, Outcome, PageCommand, PageDoc, PageOutcome, Priority, Workspace,
    WorkspaceCommand,
};
use leaf_store::ViewState;

fn main() -> ExitCode {
    let Some(folder) = std::env::args().nth(1) else {
        eprintln!("usage: demo_workspace <new folder>");
        return ExitCode::FAILURE;
    };
    match demo().and_then(|(fixture, trip)| {
        let saved = save_to_folder(folder.as_ref(), &fixture)?;
        // Open the trip with its pages showing in the sidebar.
        let mut view = ViewState::default();
        view.set_expanded(&[trip]);
        view.set_open_page(Some(trip));
        view.save(&saved.view_state_path())?;
        Ok(())
    }) {
        Ok(()) => {
            println!("{folder}: demo workspace written");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("could not make the demo workspace: {err}");
            ExitCode::FAILURE
        }
    }
}

/// A page's icon and title.
type Page = (&'static str, &'static str);

/// The page tree: each top-level page with its children. The first page is the one Leaf opens.
const TREE: &[(Page, &[Page])] = &[
    (
        ("🗾", "Trip to Japan"),
        &[
            ("🚄", "Itinerary"),
            ("🏨", "Bookings"),
            ("🍜", "Food to try"),
            ("🎒", "Packing list"),
            ("💴", "Budget"),
        ],
    ),
    (("🗣️", "Japanese phrases"), &[]),
    (("📚", "Reading list"), &[]),
    (("🏡", "Home projects"), &[]),
    (("📝", "Journal"), &[("🌸", "Spring"), ("🍂", "Autumn")]),
];

/// The workspace and the page to open.
fn demo() -> Result<(Fixture, PageId), FixtureError> {
    let now = now_ms();
    let mut workspace = Workspace::new();
    let mut first = None;
    let mut made: Vec<(&str, PageId)> = Vec::new();
    for (index, ((icon, title), children)) in TREE.iter().enumerate() {
        let page = create(&mut workspace, None, index, icon, title, now)?;
        first.get_or_insert(page);
        for (child_index, (icon, title)) in children.iter().enumerate() {
            made.push((
                title,
                create(&mut workspace, Some(page), child_index, icon, title, now)?,
            ));
        }
    }
    let find = |wanted: &str| {
        made.iter()
            .find(|(title, _)| *title == wanted)
            .map(|(_, page)| *page)
            .ok_or_else(|| FixtureError::Unexpected(format!("no page {wanted}")))
    };
    let links = [find("Itinerary")?, find("Bookings")?, find("Budget")?];
    let trip = first.ok_or_else(|| FixtureError::Unexpected("no pages".into()))?;
    workspace.apply(WorkspaceCommand::SetPageCover {
        page: trip,
        cover: Some("builtin:travel".into()),
    })?;
    workspace.apply(WorkspaceCommand::AddFavourite { page: trip })?;
    let fixture = Fixture {
        workspace,
        pages: vec![(trip, trip_page(now, links)?)],
    };
    Ok((fixture, trip))
}

fn create(
    workspace: &mut Workspace,
    parent: Option<PageId>,
    index: usize,
    icon: &str,
    title: &str,
    now: i64,
) -> Result<PageId, FixtureError> {
    let Outcome::PageCreated(page) = workspace.apply(WorkspaceCommand::CreatePage {
        parent,
        index,
        title: title.into(),
        created_at: now,
    })?
    else {
        return Err(FixtureError::Unexpected("no page for CreatePage".into()));
    };
    workspace.apply(WorkspaceCommand::SetPageIcon {
        page,
        icon: Some(icon.into()),
    })?;
    Ok(page)
}

/// The page the screenshots show.
fn trip_page(now: i64, links: [PageId; 3]) -> Result<PageDoc, FixtureError> {
    let today = now.div_euclid(86_400_000);
    let mut page = Writer {
        doc: PageDoc::new(),
        count: 0,
    };
    page.add(
        BlockKind::Paragraph,
        "Two weeks in April for the cherry blossoms: Tokyo, Hakone, Kyoto and Osaka. Sakura season books up early, so hotels and the ryokan come first.",
        &[("Two weeks in April", Mark::Bold), ("Sakura season books up early", Mark::Italic)],
    )?;
    page.with_links(
        "Plan each day in @, keep confirmations in @ and track spending in @.",
        &links,
    )?;
    page.add(BlockKind::Heading { level: 2 }, "Before we go", &[])?;
    page.add(
        task(false, today - 2, None, Some(Priority::High))?,
        "Book the ryokan in Hakone",
        &[],
    )?;
    page.add(
        task(
            false,
            today + 3,
            Some(DueTime::new(10, 0)?),
            Some(Priority::Medium),
        )?,
        "Buy teamLab Planets tickets",
        &[],
    )?;
    page.add(task(true, today - 5, None, None)?, "Renew passports", &[])?;
    page.add(BlockKind::Heading { level: 2 }, "Route", &[])?;
    for (stop, rest) in [
        ("Tokyo", ": 5 nights in Shibuya, day trip to Nikko"),
        ("Hakone", ": 2 nights in a ryokan with an onsen, Mt Fuji views"),
        ("Kyoto", ": 4 nights near Gion, Fushimi Inari at sunrise"),
        ("Osaka", ": 3 nights, street food in Dotonbori, fly home from KIX"),
    ] {
        page.add(
            BlockKind::NumberedList,
            &format!("{stop}{rest}"),
            &[(stop, Mark::Bold)],
        )?;
    }
    page.add(
        BlockKind::Quote,
        "Leave one afternoon free in every city. The best days are the unplanned ones.",
        &[],
    )?;
    page.add(BlockKind::Heading { level: 2 }, "Budget", &[])?;
    page.table(&[
        ["Item", "Per person", "Notes"],
        ["Flights", "¥120,000", "Return to Narita"],
        ["Hotels", "¥180,000", "13 nights, ryokan included"],
        ["Rail", "¥50,000", "7-day JR Pass"],
        ["Food and fun", "¥90,000", "About ¥6,500 a day"],
    ])?;
    page.add(BlockKind::Heading { level: 2 }, "Packing", &[])?;
    page.add(
        BlockKind::Todo { checked: true },
        "Passports and printed hotel bookings",
        &[],
    )?;
    page.add(BlockKind::Todo { checked: true }, "Plug adapter (type A)", &[])?;
    page.add(
        BlockKind::Todo { checked: false },
        "Load the Suica card on our phones",
        &[("Suica", Mark::Bold)],
    )?;
    page.add(
        BlockKind::Todo { checked: false },
        "Comfortable walking shoes",
        &[],
    )?;
    Ok(page.doc)
}

fn task(
    done: bool,
    day: i64,
    time: Option<DueTime>,
    priority: Option<Priority>,
) -> Result<BlockKind, FixtureError> {
    let (year, month, day) = civil(day);
    Ok(BlockKind::Task {
        done,
        due: Some(DueDate::new(year, month, day)?),
        time,
        priority,
    })
}

/// Appends top-level blocks in order.
struct Writer {
    doc: PageDoc,
    count: usize,
}

impl Writer {
    /// Adds a block, then gives each of `marks`' words (their first appearance) its mark.
    fn add(&mut self, kind: BlockKind, text: &str, marks: &[(&str, Mark)]) -> Result<BlockId, FixtureError> {
        let id = self.created(PageCommand::InsertBlock {
            parent: None,
            index: self.count,
            kind,
            text: text.into(),
        })?;
        for (words, mark) in marks {
            let byte = text
                .find(words)
                .ok_or_else(|| FixtureError::Unexpected(format!("{words:?} not in {text:?}")))?;
            let start = text[..byte].chars().count();
            self.doc.apply(PageCommand::SetMark {
                id,
                range: start..start + words.chars().count(),
                mark: mark.clone(),
            })?;
        }
        Ok(id)
    }

    /// Adds a paragraph with a link to each of `pages` where `template` has `@`, in order.
    fn with_links(&mut self, template: &str, pages: &[PageId]) -> Result<(), FixtureError> {
        let id = self.add(BlockKind::Paragraph, &template.replace('@', ""), &[])?;
        let spots = template
            .chars()
            .enumerate()
            .filter(|(_, c)| *c == '@')
            .map(|(at, _)| at);
        // Each link takes the one place its `@` took, so the template's positions hold as they go in.
        for (pos, page) in spots.zip(pages) {
            self.doc
                .apply(PageCommand::InsertPageLink { id, pos, page: *page })?;
        }
        Ok(())
    }

    /// Adds a table with a header row and fills its cells.
    fn table<const N: usize>(&mut self, rows: &[[&str; N]]) -> Result<(), FixtureError> {
        let table = self.created(PageCommand::InsertTable {
            parent: None,
            index: self.count,
            rows: rows.len(),
            columns: N,
            header_row: true,
        })?;
        for (row, cells) in self.doc.children(table)?.into_iter().zip(rows) {
            for (cell, text) in self.doc.children(row)?.into_iter().zip(cells) {
                self.doc.apply(PageCommand::InsertText {
                    id: cell,
                    pos: 0,
                    text: (*text).into(),
                })?;
            }
        }
        Ok(())
    }

    fn created(&mut self, command: PageCommand) -> Result<BlockId, FixtureError> {
        let outcome = self.doc.apply(command)?;
        self.count += 1;
        match outcome {
            PageOutcome::BlockCreated(id) => Ok(id),
            other => Err(FixtureError::Unexpected(format!("{other:?} for a new block"))),
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// The calendar date of a day counted from 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (u16, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        u16::try_from(year).unwrap_or(2026),
        u8::try_from(month).unwrap_or(1),
        u8::try_from(day).unwrap_or(1),
    )
}
