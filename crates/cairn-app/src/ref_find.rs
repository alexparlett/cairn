//! Pressing a ref or a stash in the sidebar (refs-and-status R8.5, R8.6): its row is selected
//! and scrolled into view; one not loaded yet is found by paging the held walk forward — a
//! history-lane query, `Request::FindRow` — every page appended as a scroll's is, so the window
//! keeps what scrolling there would and no more. While it looks the sidebar says "Finding
//! <ref>…". The next press, a scroll of the list or a row chosen in it supersedes the find
//! (`Request::StopFinding`); a reopen of the history asks it again of the new walk.
//!
//! A ref that names no commit (a tag on a tree) is not in the walk, and says so at once. A row
//! the walk ends without — a stash whose base no ref reaches (R4.2), or a ref moved since the
//! walk began — says it is not in the graph; a stash's changes are shown all the same, with no
//! row selected.
//!
//! Where a pressed ref's row is, among those loaded, is looked up among the rows refs label,
//! the stashes' and `HEAD`'s (`History::labelled_position`) — a pass that grows with the
//! labelled rows, never with every loaded row: a few hundred for the whole of rust-lang/rust,
//! a fraction of a millisecond at 50,000 refs — since every ref labels its commit's row in a
//! walk from the snapshot the sidebar lists. A find then looks through each page as it arrives,
//! never a page twice. (A ref moved since the walk began labels no loaded row until the
//! refresh that saw it reopens the history, which it does at once; meanwhile its press pages
//! the walk, and finds the commit only in a page still to come.)

use cairn_model::{HeadState, History, Oid, RowId, SidebarRow};
use cairn_ui::{MainView, SidebarTarget, reveal_row};
use freya::prelude::*;

use crate::selection;
use crate::window::View;
use crate::worker::Request;

/// How many rows each page of a find walks.
pub const FIND_PAGE_ROWS: usize = 512;

/// A find in progress, as the window keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Find {
    /// The commit whose row is looked for: a commit's own, or a stash's stash commit.
    target: Oid,
    /// What the find is called in what the sidebar says.
    name: String,
    /// Whether it is a stash's, whose changes are shown when it has no row.
    stash: bool,
    /// The history the rows below are of, and how many of its rows have been looked through.
    serial: u64,
    scanned: usize,
    /// Where the list stood when the find began: a scroll from there ends it.
    scrolled_y: i32,
}

/// What the sidebar says while a find looks.
pub fn finding_text(name: &str) -> String {
    format!("Finding {name}…")
}

/// What the sidebar says of a row the walk does not hold.
pub fn not_in_graph_text(name: &str) -> String {
    format!("{name} is not in the graph")
}

/// What the sidebar says of a ref that names no commit: a tag on a tree or a blob.
pub fn no_commit_text(name: &str) -> String {
    format!("{name} names no commit, so it is not in the graph")
}

/// What a press finds: the commit whose row it looks for, and its name — or that the ref names
/// no commit.
enum Pressed {
    Row {
        target: Oid,
        name: String,
        stash: bool,
    },
    NoCommit {
        name: String,
    },
}

/// A row of the sidebar pressed: a section or folder opens or closes; a ref, a detached `HEAD`
/// or a stash is found in the history and selected.
pub fn press(row: SidebarRow, view: View, submit: Option<&dyn Fn(Request)>) {
    let mut sidebar = view.sidebar.state;
    let Some(refs) = sidebar.peek().shown().map(|shown| shown.refs.clone()) else {
        return;
    };
    if let Some(request) = sidebar.write().toggle(row) {
        ask(submit, request);
        return;
    }
    let chosen = SidebarTarget::of(row, &refs);
    let pressed = match row {
        SidebarRow::Ref { index, .. } => refs.refs.get(index as usize).map(|listed| {
            let name = listed.name.shorthand().to_owned();
            match listed.commit_id() {
                Some(target) => Pressed::Row {
                    target,
                    name,
                    stash: false,
                },
                None => Pressed::NoCommit { name },
            }
        }),
        SidebarRow::DetachedHead => match refs.head {
            HeadState::Detached(target) => Some(Pressed::Row {
                target,
                name: cairn_ui::DETACHED_HEAD_CAPTION.to_owned(),
                stash: false,
            }),
            HeadState::Branch(_) | HeadState::Unborn(_) => None,
        },
        SidebarRow::Stash { index } => refs.stashes.get(index as usize).map(|stash| Pressed::Row {
            target: stash.commit,
            name: format!("stash@{{{}}}", stash.index),
            stash: true,
        }),
        SidebarRow::Section { .. } | SidebarRow::Folder { .. } => None,
    };
    let Some(pressed) = pressed else {
        return;
    };
    let mut main = view.sidebar.main;
    main.set(MainView::AllCommits);
    // The press supersedes any find in flight, whatever it turns out to be.
    let was_finding = view.sidebar.finding.peek().is_some();
    let (target, name, stash) = match pressed {
        Pressed::NoCommit { name } => {
            end_find(view, submit, was_finding);
            sidebar.write().pressed(chosen, Some(no_commit_text(&name)));
            return;
        }
        Pressed::Row {
            target,
            name,
            stash,
        } => (target, name, stash),
    };
    sidebar.write().pressed(chosen, None);
    let (serial, loaded, found) = {
        let rows = view.rows.peek();
        (rows.serial(), rows.len(), rows.labelled_position(target))
    };
    if let Some(index) = found {
        end_find(view, submit, was_finding);
        bring_into_view(index, view, submit);
        return;
    }
    if view.progress.peek().complete() {
        end_find(view, submit, was_finding);
        not_in_graph(target, &name, stash, view, submit);
        return;
    }
    let (_, scrolled_y): (i32, i32) = view.history_scroll.into();
    let mut finding = view.sidebar.finding;
    finding.set(Some(Find {
        target,
        name: name.clone(),
        stash,
        serial,
        scanned: loaded,
        scrolled_y,
    }));
    sidebar.write().say(Some(finding_text(&name)));
    let mut progress = view.progress;
    progress.write().asked();
    ask(
        submit,
        Request::FindRow {
            target,
            rows: FIND_PAGE_ROWS,
        },
    );
}

/// Pages arrived while a find looks: the rows appended since the last look are looked
/// through, never a row twice. Found, the row is selected and brought into view and the find
/// stopped; the walk ended without it, the sidebar says so.
pub fn pages_arrived(view: View, submit: Option<&dyn Fn(Request)>) {
    let mut finding = view.sidebar.finding;
    let Some(find) = finding.peek().clone() else {
        return;
    };
    let (serial, loaded, found) = {
        let rows = view.rows.peek();
        let from = if rows.serial() == find.serial {
            find.scanned
        } else {
            0
        };
        (
            rows.serial(),
            rows.len(),
            place_of(&rows, find.target, from),
        )
    };
    if let Some(index) = found {
        finding.set(None);
        // The find may have walked on past a page laid out before it began.
        ask(submit, Request::StopFinding);
        let mut sidebar = view.sidebar.state;
        sidebar.write().say(None);
        bring_into_view(index, view, submit);
        return;
    }
    if view.progress.peek().complete() {
        finding.set(None);
        not_in_graph(find.target, &find.name, find.stash, view, submit);
        return;
    }
    if let Some(kept) = finding.write().as_mut() {
        kept.serial = serial;
        kept.scanned = loaded;
    }
}

/// The history's walk failed: the find ends with it, the failure said where the history is.
pub fn failed(view: View) {
    let mut finding = view.sidebar.finding;
    if finding.peek().is_some() {
        finding.set(None);
        let mut sidebar = view.sidebar.state;
        sidebar.write().say(None);
    }
}

/// The list was scrolled, or a row chosen in it, while a find looked: the find ends there.
pub fn superseded(view: View, submit: Option<&dyn Fn(Request)>) {
    let mut finding = view.sidebar.finding;
    if finding.peek().is_none() {
        return;
    }
    finding.set(None);
    let mut sidebar = view.sidebar.state;
    sidebar.write().say(None);
    let mut progress = view.progress;
    progress.write().stopped_finding();
    ask(submit, Request::StopFinding);
}

/// A row was chosen in the history: a find ends there, and the entry pressed in the sidebar
/// is no longer what is shown.
pub fn row_chosen(view: View, submit: Option<&dyn Fn(Request)>) {
    superseded(view, submit);
    let mut sidebar = view.sidebar.state;
    if sidebar.peek().chosen().is_some() || sidebar.peek().notice().is_some() {
        sidebar.write().let_go();
    }
}

/// Whether the list has scrolled since the find began.
pub fn scrolled_away(find: &Find, y: i32) -> bool {
    find.scrolled_y != y
}

/// The history was reopened while a find looked: it is asked again of the new walk.
pub fn reopened(view: View, submit: Option<&dyn Fn(Request)>) {
    let mut finding = view.sidebar.finding;
    let Some(target) = finding.peek().as_ref().map(|find| find.target) else {
        return;
    };
    if let Some(kept) = finding.write().as_mut() {
        kept.scanned = 0;
    }
    ask(
        submit,
        Request::FindRow {
            target,
            rows: FIND_PAGE_ROWS,
        },
    );
}

/// The find in flight, if any, is stopped: a press that needs no walk supersedes it too.
fn end_find(view: View, submit: Option<&dyn Fn(Request)>, was_finding: bool) {
    if !was_finding {
        return;
    }
    let mut finding = view.sidebar.finding;
    finding.set(None);
    let mut progress = view.progress;
    progress.write().stopped_finding();
    ask(submit, Request::StopFinding);
}

/// Where the row of `target` — a commit's or a stash's — is among the loaded rows from `from`.
fn place_of(rows: &History, target: Oid, from: usize) -> Option<usize> {
    (from..rows.len()).find(|&at| rows.id(at).map(RowId::oid) == Some(target))
}

/// The row at `index` selected, its changes asked, and brought into view.
fn bring_into_view(index: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(id) = view.rows.peek().id(index) else {
        return;
    };
    selection::choose(id, view, submit);
    // Told to the list, so its next arrow key starts here.
    let mut cursor = view.history_cursor;
    cursor.set(index);
    let mut scroll = view.history_scroll;
    reveal_row(&mut scroll, index);
}

/// The walk ended without the row: said, and a stash's changes shown with no row selected.
fn not_in_graph(
    target: Oid,
    name: &str,
    stash: bool,
    view: View,
    submit: Option<&dyn Fn(Request)>,
) {
    let mut sidebar = view.sidebar.state;
    sidebar.write().say(Some(not_in_graph_text(name)));
    if stash {
        selection::choose(RowId::Stash(target), view, submit);
    }
}

fn ask(submit: Option<&dyn Fn(Request)>, request: Request) {
    if let Some(submit) = submit {
        submit(request);
    }
}
