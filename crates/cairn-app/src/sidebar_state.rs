//! The sidebar as the window keeps it (refs-and-status R8): the rows a worker laid out with the
//! snapshot they index, what is open, the filter's text they were asked for, and the entry
//! last pressed with what its press is doing.
//!
//! The rows are asked again — `Request::FilterRefs`, on the repository thread — whenever what
//! they are laid out from changes: a snapshot arrives, the filter's text changes, a section or
//! folder is opened or closed. Until the answer comes the rows before it stay drawn. Each ask
//! supersedes the one before it, so the answer that arrives is the latest asked; the rows it
//! replaces, and the snapshot they may be the last hold on, go to a worker to free.
//!
//! The current branch's folders are opened when it changes, as Fork reveals the current branch
//! on a checkout.

use std::sync::Arc;

use cairn_model::{Disclosure, HeadState, RefsSnapshot, SidebarRow, SidebarSection};
use cairn_ui::{MainView, SidebarRefs, SidebarTarget, current_branch};
use freya::prelude::*;

use crate::ref_find::Find;
use crate::worker::{Request, Retired};

/// The sidebar's share of the window, in pixels, until it is dragged.
pub const SIDEBAR_WIDTH: f32 = 240.0;
/// The narrowest the sidebar is dragged or squeezed to.
pub const SIDEBAR_MIN_WIDTH: f32 = 140.0;

/// The sidebar's state handles, each a session's: what it keeps, the filter as typed, what the
/// main region shows, its width as last dragged, and the find a press began.
#[derive(Clone, Copy, PartialEq)]
pub struct SidebarView {
    pub state: State<SidebarState>,
    pub filter_text: State<String>,
    pub main: State<MainView>,
    pub width: State<f32>,
    pub finding: State<Option<Find>>,
}

impl SidebarView {
    /// Each handle made by its hook, in the window's root.
    pub fn used() -> Self {
        Self {
            state: use_state(SidebarState::default),
            filter_text: use_state(String::new),
            main: use_state(MainView::default),
            width: use_state(|| SIDEBAR_WIDTH),
            finding: use_state(|| None),
        }
    }

    /// Each handle made outside a component, for a test's view.
    #[cfg(test)]
    pub fn created() -> Self {
        Self {
            state: State::create(SidebarState::default()),
            filter_text: State::create(String::new()),
            main: State::create(MainView::default()),
            width: State::create(SIDEBAR_WIDTH),
            finding: State::create(None),
        }
    }
}

#[derive(Debug, Default)]
pub struct SidebarState {
    disclosure: Disclosure,
    shown: Option<SidebarRefs>,
    /// The filter's text the last ask was for.
    text: String,
    /// The snapshot the last ask was for.
    asked: Option<Arc<RefsSnapshot>>,
    /// `HEAD` as the last snapshot had it: a change reveals the current branch's folders.
    head: Option<HeadState>,
    /// The entry last pressed, drawn chosen.
    chosen: Option<SidebarTarget>,
    /// What the last press is doing, or why it found nothing.
    notice: Option<String>,
}

impl SidebarState {
    /// The rows answered last and the snapshot they index.
    pub fn shown(&self) -> Option<&SidebarRefs> {
        self.shown.as_ref()
    }

    pub fn chosen(&self) -> Option<&SidebarTarget> {
        self.chosen.as_ref()
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The entry pressed, and what its press says, if anything yet.
    pub fn pressed(&mut self, chosen: Option<SidebarTarget>, notice: Option<String>) {
        self.chosen = chosen;
        self.notice = notice;
    }

    /// What the press says now: the find it began ended, one way or another.
    pub fn say(&mut self, notice: Option<String>) {
        self.notice = notice;
    }

    /// A row chosen in the history: the entry pressed is no longer what is shown.
    pub fn let_go(&mut self) {
        self.chosen = None;
        self.notice = None;
    }

    /// A refresh's snapshot arrived: the rows are asked for it, the current branch's folders
    /// opened first when `HEAD` moved.
    pub fn refs_arrived(&mut self, refs: &Arc<RefsSnapshot>) -> Request {
        if self.head.as_ref() != Some(&refs.head) {
            if let Some(branch) = current_branch(&refs.head) {
                self.disclosure.reveal(branch.as_str());
            }
            self.head = Some(refs.head.clone());
        }
        self.ask(Arc::clone(refs))
    }

    /// The filter's text changed: the rows are asked for it, unless it is what was asked.
    pub fn filter(&mut self, text: &str) -> Option<Request> {
        if text == self.text {
            return None;
        }
        self.text = text.to_owned();
        let refs = self.asked.clone()?;
        Some(self.ask(refs))
    }

    /// A section's or folder's row pressed: it is opened or closed and the rows asked again.
    /// `None` for any other row.
    pub fn toggle(&mut self, row: SidebarRow) -> Option<Request> {
        let refs = self.shown.as_ref().map(|shown| Arc::clone(&shown.refs))?;
        match row {
            SidebarRow::Section { section, .. } => self.toggle_section(section),
            SidebarRow::Folder { first, depth, .. } => {
                let path = cairn_model::folder_path(&refs, first, depth).to_owned();
                self.disclosure.toggle_folder(&path);
            }
            SidebarRow::Ref { .. } | SidebarRow::DetachedHead | SidebarRow::Stash { .. } => {
                return None;
            }
        }
        Some(self.ask(refs))
    }

    fn toggle_section(&mut self, section: SidebarSection) {
        self.disclosure.toggle_section(section);
    }

    fn ask(&mut self, refs: Arc<RefsSnapshot>) -> Request {
        self.asked = Some(Arc::clone(&refs));
        Request::FilterRefs {
            refs,
            text: self.text.clone(),
            disclosure: Arc::new(self.disclosure.clone()),
        }
    }

    /// The rows a worker laid out arrived: kept with their snapshot, the ones they replace
    /// handed back to be freed on a worker.
    pub fn rows_arrived(
        &mut self,
        refs: Arc<RefsSnapshot>,
        rows: Vec<SidebarRow>,
    ) -> Option<Retired> {
        let replaced = self.shown.replace(SidebarRefs {
            refs,
            rows: Arc::new(rows),
        });
        replaced.map(|old| Retired::sidebar(old.refs, old.rows))
    }
}

#[cfg(test)]
mod tests {
    use cairn_model::{Oid, Ref, RefKind, RefName, RefTarget};

    use super::*;

    fn snapshot(head: &str) -> Arc<RefsSnapshot> {
        let id = Oid::from_bytes(&[1; 20]).unwrap();
        Arc::new(RefsSnapshot {
            refs: ["refs/heads/feature/login", "refs/heads/main"]
                .iter()
                .map(|name| Ref {
                    name: RefName::new(*name),
                    kind: RefKind::LocalBranch,
                    target: RefTarget::Commit(id),
                    symbolic: None,
                    upstream: None,
                })
                .collect(),
            head: HeadState::Branch(RefName::new(head)),
            stashes: Vec::new(),
            unreadable: 0,
        })
    }

    fn asked_open(request: &Request) -> Disclosure {
        match request {
            Request::FilterRefs { disclosure, .. } => (**disclosure).clone(),
            other => panic!("expected the rows asked, got {other:?}"),
        }
    }

    /// Fork reveals the current branch: its folders open when it becomes current — on the
    /// first snapshot and on a checkout — and a folder the user closed stays closed while the
    /// current branch is the same. Caught by: folders opened on every refresh (a closed one
    /// springs open on focus), or never.
    #[test]
    fn the_current_branchs_folders_open_when_it_becomes_current() {
        let mut state = SidebarState::default();
        let first = state.refs_arrived(&snapshot("refs/heads/feature/login"));
        assert!(asked_open(&first).is_folder_open("refs/heads/feature"));

        state.disclosure.toggle_folder("refs/heads/feature");
        let again = state.refs_arrived(&snapshot("refs/heads/feature/login"));
        assert!(!asked_open(&again).is_folder_open("refs/heads/feature"));

        let _ = state.refs_arrived(&snapshot("refs/heads/main"));
        let back = state.refs_arrived(&snapshot("refs/heads/feature/login"));
        assert!(asked_open(&back).is_folder_open("refs/heads/feature"));
    }

    /// The filter's text asks the rows again for the snapshot last asked, once per text; a
    /// folder's press opens it and asks again. Caught by: a keystroke that asks nothing, the
    /// same text asked twice, or a toggle that does not reach the ask.
    #[test]
    fn a_filter_or_a_toggle_asks_the_rows_again() {
        let mut state = SidebarState::default();
        assert!(state.filter("lo").is_none(), "nothing to lay out yet");
        let refs = snapshot("refs/heads/main");
        let _ = state.refs_arrived(&refs);
        match state.filter("log") {
            Some(Request::FilterRefs {
                text, refs: asked, ..
            }) => {
                assert_eq!(text, "log");
                assert!(Arc::ptr_eq(&asked, &refs));
            }
            other => panic!("expected the rows asked, got {other:?}"),
        }
        assert!(state.filter("log").is_none(), "the same text asked again");

        let _ = state.rows_arrived(Arc::clone(&refs), Vec::new());
        let toggled = state
            .toggle(SidebarRow::Folder {
                first: 0,
                depth: 0,
                open: false,
            })
            .unwrap_or_else(|| panic!("a folder's press asked nothing"));
        assert!(asked_open(&toggled).is_folder_open("refs/heads/feature"));
        assert!(state.toggle(SidebarRow::DetachedHead).is_none());
    }
}
