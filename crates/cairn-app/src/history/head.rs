//! `HEAD`'s commit as the last refresh read it, and Show HEAD: its row selected and brought into
//! view, as a press of its ref in the sidebar finds it.

use cairn_model::{HeadState, Oid, Ref};

use crate::window::View;
use crate::worker::Request;

/// Why `HEAD` names no commit to go to, as a greyed menu item says it.
pub const REFS_UNREAD: &str = "The refs have not been read yet.";
/// An unborn `HEAD`, in git's word.
pub const HEAD_UNBORN: &str = "HEAD is unborn: it names no commit yet.";

/// `HEAD`'s commit as the last refresh read it — a branch's tip or a detached `HEAD` — or why
/// there is none: the refs not read yet, or an unborn `HEAD`.
pub fn head_commit(view: View) -> Result<Oid, &'static str> {
    let refreshed = view.refreshed.peek();
    let refs = refreshed.refs().ok_or(REFS_UNREAD)?;
    match &refs.head {
        HeadState::Branch(name) => refs.find(name).and_then(Ref::commit_id).ok_or(HEAD_UNBORN),
        HeadState::Detached(at) => Ok(*at),
        HeadState::Unborn(_) => Err(HEAD_UNBORN),
    }
}

/// Show HEAD: All Commits shown, `HEAD`'s row selected and brought into view, or found by paging
/// the walk; nothing while `HEAD` names no commit.
pub fn show(view: View, submit: Option<&dyn Fn(Request)>) {
    if let Ok(at) = head_commit(view) {
        crate::history::ref_find::find_commit(at, "HEAD".to_owned(), view, submit);
    }
}
