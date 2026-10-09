//! Every dialog's buttons in its platform's order (the user's decision E, 2026-10-09): on Linux
//! the primary first and Cancel last, as Fork for Windows; on macOS Cancel first and the primary
//! last, as Fork for macOS — the confirmation, the Git Error dialog, Create Branch and the
//! credential prompt alike — and a destructive confirmation's focus on Cancel on both.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{Consequence, DiscardedFile, FileLoss, Oid, RepoPath};
use cairn_ui::accelerators::Os;
use cairn_ui::{
    CANCEL_BRANCH_CAPTION, CANCEL_CAPTION, CLOSE_CAPTION, CREATE_CAPTION, ConfirmDialog,
    CreateBranchDialog, CredentialPrompt, GitErrorDialog, SKIP_HOOKS_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 700.;

fn oid(byte: u8) -> Oid {
    Oid::from_bytes(&[byte; 20]).unwrap_or_else(|e| panic!("{e}"))
}

fn discard() -> Consequence {
    Consequence::DiscardFiles {
        files: vec![DiscardedFile {
            path: RepoPath::from("a.rs"),
            loss: FileLoss::Modified {
                index: oid(1),
                working_tree: Some(oid(2)),
                executable: false,
                lines: Some(3),
                mode: None,
            },
        }],
    }
}

fn render(app: impl Fn() -> Element + 'static) -> TestingRunner {
    let (mut test, _) = TestingRunner::new(app, (WIDTH, HEIGHT).into(), |_| {}, 1.);
    for _ in 0..4 {
        test.sync_and_update();
    }
    test
}

/// Where `caption`'s label starts across the dialog.
fn left_of(test: &TestingRunner, caption: &str) -> f32 {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == caption)
            .map(|_| node.layout().area.min_x())
    })
    .unwrap_or_else(|| panic!("no label reads {caption:?}"))
}

/// Whether `first` is drawn left of `then`, in one row.
fn in_order(test: &TestingRunner, first: &str, then: &str) -> bool {
    left_of(test, first) < left_of(test, then)
}

/// Each dialog on each platform: the primary left of Cancel (or Close) on Linux, right of it on
/// macOS. Caught by: one order for both platforms, or a dialog left out of the rule.
#[test]
fn every_dialogs_buttons_follow_the_platforms_order() {
    let action = discard().action();
    for platform in [Os::Linux, Os::MacOs] {
        let linux = platform == Os::Linux;
        let consequence = Rc::new(discard());
        let test = render(move || {
            ConfirmDialog::new(1, "Discard changes", consequence.clone())
                .platform(platform)
                .into_element()
        });
        assert_eq!(
            in_order(&test, &action, CANCEL_CAPTION),
            linux,
            "the confirmation on {platform:?}"
        );

        let test = render(move || {
            GitErrorDialog::new(
                1,
                "git commit -q -F -",
                Rc::new(vec!["hook failed".to_owned()]),
            )
            .skip(true)
            .platform(platform)
            .into_element()
        });
        assert_eq!(
            in_order(&test, SKIP_HOOKS_CAPTION, CLOSE_CAPTION),
            linux,
            "the Git Error dialog on {platform:?}"
        );

        let test = render(move || {
            let name = use_state(|| "topic".to_owned());
            CreateBranchDialog::new(1, oid(0xab), "Fix the parser", name)
                .ready(true)
                .platform(platform)
                .into_element()
        });
        assert_eq!(
            in_order(&test, CREATE_CAPTION, CANCEL_BRANCH_CAPTION),
            linux,
            "Create Branch on {platform:?}"
        );

        let test = render(move || {
            CredentialPrompt::new("origin", "Password for 'https://ada@example.com': ")
                .platform(platform)
                .into_element()
        });
        assert_eq!(
            in_order(&test, "Continue", "Cancel"),
            linux,
            "the credential prompt on {platform:?}"
        );
    }
}

/// Cairn's rule beside Fork's order: a destructive confirmation starts with Cancel focused on
/// both platforms, so a Return pressed by habit cancels wherever Cancel is drawn. Caught by:
/// focus following the button's place (the primary first on Linux taking it).
#[test]
fn a_confirmation_starts_on_cancel_on_both_platforms() {
    for platform in [Os::Linux, Os::MacOs] {
        let (confirmed, cancelled) = (Rc::new(RefCell::new(0)), Rc::new(RefCell::new(0)));
        let mut test = render({
            let (confirmed, cancelled) = (confirmed.clone(), cancelled.clone());
            move || {
                let (confirmed, cancelled) = (confirmed.clone(), cancelled.clone());
                ConfirmDialog::new(1, "Discard changes", Rc::new(discard()))
                    .platform(platform)
                    .on_confirm(move |_| *confirmed.borrow_mut() += 1)
                    .on_cancel(move |()| *cancelled.borrow_mut() += 1)
                    .into_element()
            }
        });
        test.press_key(Key::Named(NamedKey::Enter));
        for _ in 0..2 {
            test.sync_and_update();
        }
        assert_eq!(
            (*confirmed.borrow(), *cancelled.borrow()),
            (0, 1),
            "Return on {platform:?}"
        );
    }
}
