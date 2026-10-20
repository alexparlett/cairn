//! Every dialog's buttons in its platform's order: on Linux the primary first and Cancel last,
//! as Fork for Windows; on macOS Cancel first and the primary last, as Fork for macOS — the
//! confirmation, the Git Error dialog, Create Branch and the credential prompt alike — and a
//! destructive confirmation's focus on Cancel on both.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{Consequence, DiscardedFile, FileLoss, Oid, RepoPath};
use cairn_ui::accelerators::Os;
use cairn_ui::style::{metrics, tones};
use cairn_ui::{AboutDialog, LocalChoice};
use cairn_ui::{
    CANCEL_BRANCH_CAPTION, CANCEL_CAPTION, CLOSE_CAPTION, CREATE_CAPTION, ConfirmDialog,
    CreateBranchDialog, CredentialPrompt, GitErrorDialog, SKIP_HOOKS_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

#[path = "support/focus_marks.rs"]
mod focus_marks;

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

/// A discard of two files, whose confirmation offers Show files.
fn discard_two() -> Consequence {
    let Consequence::DiscardFiles { mut files } = discard() else {
        unreachable!("discard() is a discard of files");
    };
    let mut second = files[0].clone();
    second.path = RepoPath::from("b.rs");
    files.push(second);
    Consequence::DiscardFiles { files }
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
            ConfirmDialog::new(1, "Discard Changes", consequence.clone())
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
                ConfirmDialog::new(1, "Discard Changes", Rc::new(discard()))
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

/// ssh's host-key question, which the prompt asks as a yes or no.
const HOST_KEY: &str = "The authenticity of host 'git.example.com (203.0.113.7)' can't be \
                        established.\nED25519 key fingerprint is SHA256:abcdefg.\nAre you sure \
                        you want to continue connecting (yes/no/[fingerprint])? ";

/// Presses `key` once the dialog has settled, and lets the press land.
fn press_on_open(test: &mut TestingRunner, key: Key) {
    test.press_key(key);
    for _ in 0..2 {
        test.sync_and_update();
    }
}

/// Focus starts on the safe answer of anything risky, whatever the order: a freshly opened
/// host-key prompt and a Git Error dialog offering the hooks' skip start on Cancel and Close,
/// so Return or Space answers safely on both platforms; Create Branch starts in its name field,
/// so Space types. Caught by: focus left to the toolkit — behind a dialog that is not modal, on
/// a modal's frame, or on the first button, the primary on Linux — or on the risky answer.
#[test]
fn a_risky_dialog_opens_on_its_safe_answer_on_both_platforms() {
    for platform in [Os::Linux, Os::MacOs] {
        for key in [Key::Named(NamedKey::Enter), Key::Character(" ".into())] {
            let (accepted, cancelled) = (Rc::new(RefCell::new(0)), Rc::new(RefCell::new(0)));
            let mut test = render({
                let (accepted, cancelled) = (accepted.clone(), cancelled.clone());
                move || {
                    let (accepted, cancelled) = (accepted.clone(), cancelled.clone());
                    CredentialPrompt::new("origin", HOST_KEY)
                        .platform(platform)
                        .on_submit(move |_: String| *accepted.borrow_mut() += 1)
                        .on_cancel(move |()| *cancelled.borrow_mut() += 1)
                        .into_element()
                }
            });
            press_on_open(&mut test, key.clone());
            assert_eq!(
                (*accepted.borrow(), *cancelled.borrow()),
                (0, 1),
                "the host-key prompt on {platform:?}, {key:?}"
            );

            let (skipped, closed) = (Rc::new(RefCell::new(0)), Rc::new(RefCell::new(0)));
            let mut test = render({
                let (skipped, closed) = (skipped.clone(), closed.clone());
                move || {
                    let (skipped, closed) = (skipped.clone(), closed.clone());
                    GitErrorDialog::new(1, "git commit -q -F -", Rc::new(vec!["no".to_owned()]))
                        .skip(true)
                        .platform(platform)
                        .on_skip(move |()| *skipped.borrow_mut() += 1)
                        .on_close(move |()| *closed.borrow_mut() += 1)
                        .into_element()
                }
            });
            press_on_open(&mut test, key.clone());
            assert_eq!(
                (*skipped.borrow(), *closed.borrow()),
                (0, 1),
                "the Git Error dialog on {platform:?}, {key:?}"
            );
        }

        let created = Rc::new(RefCell::new(0));
        let typed = Rc::new(RefCell::new(String::new()));
        let mut test = render({
            let (created, typed) = (created.clone(), typed.clone());
            move || {
                let name = use_state(|| "topic".to_owned());
                *typed.borrow_mut() = name.read().clone();
                let created = created.clone();
                CreateBranchDialog::new(1, oid(0xab), "Fix the parser", name)
                    .ready(true)
                    .platform(platform)
                    .on_create(move |()| *created.borrow_mut() += 1)
                    .into_element()
            }
        });
        press_on_open(&mut test, Key::Character(" ".into()));
        assert_eq!(
            *created.borrow(),
            0,
            "Space created a branch on {platform:?}"
        );
        assert!(
            *typed.borrow() != "topic" && typed.borrow().contains(' '),
            "Space was not typed into the name on {platform:?}: {:?}",
            typed.borrow()
        );
    }
}

/// The bordered button under the label reading `caption`: its fill and its border's colour.
fn button_of(test: &TestingRunner, caption: &str) -> (Fill, Option<Color>) {
    let label = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == caption)
                .map(|_| node.layout().area)
        })
        .unwrap_or_else(|| panic!("no label reads {caption:?}"));
    // The smallest bordered rect round the label: the button itself.
    let mut found: Vec<(f32, Fill, Option<Color>)> = test.find_many(|node, element| {
        let rect = Rect::try_downcast(element)?;
        let area = node.layout().area;
        let holds = area.min_x() <= label.min_x()
            && area.max_x() >= label.max_x()
            && area.min_y() <= label.min_y()
            && area.max_y() >= label.max_y();
        (holds
            && rect.style.background != Fill::Color(Color::TRANSPARENT)
            && rect.style.background != Fill::Color(tones::DIALOG)
            && !rect.style.borders.is_empty())
        .then(|| {
            (
                area.width() * area.height(),
                rect.style.background.clone(),
                rect.style.borders.first().map(|border| border.fill),
            )
        })
    });
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (_, fill, border) = found
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no button holds {caption:?}"));
    (fill, border)
}

/// Fork Dark's dialog frame is drawn: the ground in its 1 px light-blue border, and the title
/// white and bold at the heading's size.
fn assert_forks_frame(test: &TestingRunner, title: &str, what: &str) {
    let framed = test.find(|_, element| {
        Rect::try_downcast(element).filter(|rect| {
            rect.style.background == Fill::Color(tones::DIALOG)
                && rect
                    .style
                    .borders
                    .iter()
                    .any(|border| border.fill == tones::DIALOG_BORDER)
        })
    });
    assert!(framed.is_some(), "{what} is not in Fork's frame");
    let heading = test
        .find(|_, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == title)
                .map(|label| {
                    (
                        label.text_style_data.font_size,
                        label.text_style_data.font_weight,
                        label.text_style_data.color.clone(),
                    )
                })
        })
        .unwrap_or_else(|| panic!("{what}'s title {title:?} is not drawn"));
    assert_eq!(
        heading,
        (
            Some(metrics::HEADING.into()),
            Some(FontWeight::BOLD),
            Some(Fill::Color(tones::DIALOG_TITLE))
        ),
        "{what}'s title"
    );
}

/// Every dialog takes Fork Dark's look (fork-copy-and-dialogs §6, cp60-cp76): the frame, the
/// title, and its primary drawn as Fork's default button — the button's fill in Fork's blue
/// border — beside the other answers in the button's grey border; the Git Error dialog has no
/// default, as Fork's has none. Caught by: the toolkit's 18 px title or rounded borderless
/// frame back, a primary filled or drawn as the other answers, or a Git Error answer drawn as a
/// default.
#[test]
fn every_dialogs_primary_button_is_forks_default_in_forks_frame() {
    let primary = (Fill::Color(tones::BUTTON), Some(tones::ACCENT));
    let other = (Fill::Color(tones::BUTTON), Some(tones::BUTTON_BORDER));
    let action = discard().action();

    let test =
        render(|| ConfirmDialog::new(1, "Discard Changes", Rc::new(discard())).into_element());
    assert_forks_frame(&test, "Discard Changes", "the confirmation");
    assert_eq!(
        button_of(&test, &action),
        primary,
        "the confirmation's primary"
    );
    assert_eq!(
        button_of(&test, CANCEL_CAPTION),
        other,
        "the confirmation's Cancel"
    );

    let test = render(|| {
        let name = use_state(|| "topic".to_owned());
        CreateBranchDialog::new(1, oid(0xab), "Fix the parser", name)
            .ready(true)
            .local_changes(Some(LocalChoice::Keep))
            .into_element()
    });
    assert_forks_frame(&test, "Create Branch", "Create Branch");
    assert_eq!(button_of(&test, CREATE_CAPTION), primary, "Create");
    assert_eq!(
        button_of(&test, CANCEL_BRANCH_CAPTION),
        other,
        "Create Branch's Cancel"
    );
    let test = render(|| {
        let name = use_state(String::new);
        CreateBranchDialog::new(1, oid(0xab), "Fix the parser", name).into_element()
    });
    assert_eq!(
        button_of(&test, CREATE_CAPTION),
        (
            Fill::Color(tones::DISABLED_FILL),
            Some(tones::DISABLED_BORDER)
        ),
        "Create with no name is greyed"
    );

    for (text, primary_caption) in [
        ("Password for 'https://ada@example.com': ", "Continue"),
        (HOST_KEY, "Yes, connect"),
    ] {
        let test = render(move || CredentialPrompt::new("origin", text).into_element());
        assert_forks_frame(
            &test,
            "origin is asking for a credential",
            "the credential prompt",
        );
        assert_eq!(
            button_of(&test, primary_caption),
            primary,
            "{primary_caption}"
        );
        assert_eq!(button_of(&test, "Cancel"), other, "the prompt's Cancel");
    }

    let test = render(|| {
        AboutDialog::new("0.1.0", "MIT", Some("git version 2.50.1".to_owned())).into_element()
    });
    assert_forks_frame(&test, "Cairn", "About Cairn");
    assert_eq!(
        button_of(&test, "Close"),
        primary,
        "About's Close, its one answer"
    );

    let test = render(|| {
        GitErrorDialog::new(
            1,
            "git commit -q -F -",
            Rc::new(vec!["hook failed".to_owned()]),
        )
        .skip(true)
        .into_element()
    });
    assert_forks_frame(&test, "Git Error", "the Git Error dialog");
    assert_eq!(button_of(&test, SKIP_HOOKS_CAPTION), other, "the skip");
    assert_eq!(button_of(&test, CLOSE_CAPTION), other, "Git Error's Close");
}

/// A dialog as the focus walk reads it: what it is, the fewest Tab stops it has, and the app
/// that draws it.
type Dialog = (&'static str, usize, Box<dyn Fn() -> Element>);

/// Every Tab stop of every dialog shows where focus is as focus reaches it — a button by the
/// style's ring inside its border, its primary's blue border notwithstanding, a check box, a
/// radio and Show files by the same ring, a field by its blue border. Caught by: a stop that draws
/// nothing as it takes focus, or a default button whose own border is taken for its focus.
#[test]
fn every_dialogs_tab_stop_shows_focus() {
    let dialogs: Vec<Dialog> = vec![
        (
            "the confirmation",
            3,
            Box::new(|| {
                ConfirmDialog::new(1, "Discard Changes", Rc::new(discard_two())).into_element()
            }),
        ),
        (
            "Create Branch",
            6,
            Box::new(|| {
                let name = use_state(|| "topic".to_owned());
                CreateBranchDialog::new(1, oid(0xab), "Fix the parser", name)
                    .ready(true)
                    .checkout(true)
                    .local_changes(Some(LocalChoice::Keep))
                    .into_element()
            }),
        ),
        (
            "the credential prompt",
            3,
            Box::new(|| {
                CredentialPrompt::new("origin", "Password for 'https://ada@example.com': ")
                    .into_element()
            }),
        ),
        (
            "the Git Error dialog",
            3,
            Box::new(|| {
                GitErrorDialog::new(1, "git commit -q -F -", Rc::new(vec!["no".to_owned()]))
                    .skip(true)
                    .remove_lock(true)
                    .into_element()
            }),
        ),
        (
            "About Cairn",
            1,
            Box::new(|| AboutDialog::new("0.1.0", "MIT", None).into_element()),
        ),
    ];
    for (what, least, app) in dialogs {
        // Under the window's launch theme, whose fields take Fork's blue as their focus.
        let mut test = render(move || {
            use_init_theme(cairn_ui::style::tones::theme);
            app()
        });
        let stops = focus_marks::walk_tab_stops(&mut test, 20);
        assert!(
            stops.len() >= least,
            "{what}: {} Tab stops reached, {least} at least: {stops:?}",
            stops.len()
        );
        for stop in &stops {
            assert!(
                stop.shows,
                "{what}: a Tab stop at {:?} shows no focus: {stops:?}",
                stop.area
            );
        }
    }
}
