//! The toolbar and what hangs from it: Fork's buttons, the status box in its middle, the
//! leftover lock's banner under it, and the activity popover the status box opens.
//!
//! Fork's buttons in Fork's order and groups (`docs/research/ui-review/fork-window-survey-2026-10-16.md`,
//! sections 1 and 2), each an icon over its label: the sidebar toggle, Quick Launch — Fetch, Pull,
//! Push — Stash and its ⌄ — the status box — Branch and its ⌄ — Appearance. A button for what
//! Cairn does not do yet keeps its place greyed, its tooltip naming what brings it ("Comes with
//! <feature>."); Fork's Open in, Console and Work, which nothing plans, are not drawn. Fetch's
//! tooltip is its name over its chord, read from the accelerator table; the sidebar toggle, the
//! status box and Branch have none, as Fork's. The commits ahead of the upstream are a badge on
//! Push and those behind on Pull, absent at zero, drawn on the greyed buttons too.
//!
//! The toolbar asks; the window does: every press is reported as a [`ToolbarPress`].

pub(crate) mod activity_popover;
mod button;
pub(crate) mod lock_banner;
pub(crate) mod status_box;

use cairn_model::AheadBehind;
use freya::prelude::*;

use crate::accelerators::{self, Action, Os};
use crate::dialogs::create_branch::COMES_WITH_STASHING;
use crate::history::list::NEW_BRANCH_CAPTION;
use crate::menu::{Item, context_menu};
use crate::style::{icons, metrics, tones};
use button::{Tip, ToolButton, chevron};
use status_box::StatusBox;

/// The sidebar toggle's name for assistive technology: Fork draws it as an icon alone.
pub const SIDEBAR_TOGGLE_NAME: &str = "Sidebar";
pub const QUICK_LAUNCH_CAPTION: &str = "Quick Launch";
pub const FETCH_CAPTION: &str = "Fetch";
pub const PULL_CAPTION: &str = "Pull";
pub const PUSH_CAPTION: &str = "Push";
pub const STASH_CAPTION: &str = "Stash";
pub const BRANCH_CAPTION: &str = "Branch";
pub const APPEARANCE_CAPTION: &str = "Appearance";
/// The name of Stash's ⌄ for assistive technology.
pub const STASH_MENU_NAME: &str = "Stash menu";
/// The name of Branch's ⌄ for assistive technology.
pub const BRANCH_MENU_NAME: &str = "Branch menu";
/// The name of the status box's list glyph, which opens the activity popover.
pub const ACTIVITY_GLYPH_NAME: &str = "Activity";

/// Quick Launch's tooltip while it is greyed.
pub const COMES_WITH_QUICK_LAUNCH: &str = "Comes with Quick Launch.";
/// Pull's and Push's tooltip while they are greyed.
pub const COMES_WITH_PULL_AND_PUSH: &str = "Comes with pull and push.";
/// Appearance's tooltip while it is greyed.
pub const COMES_WITH_PREFERENCES: &str = "Comes with Preferences.";

/// What a press on the toolbar asks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarPress {
    /// The sidebar toggle: hide the sidebar, or show it again.
    ToggleSidebar,
    /// Fetch the default remote.
    Fetch,
    /// Branch, or New Branch… in its ⌄'s menu: Create Branch at `HEAD`.
    NewBranch,
    /// The status box's list glyph: the activity popover.
    Activity,
}

/// The toolbar under the caption row.
pub struct Toolbar {
    status: StatusBox,
    counts: Option<AheadBehind>,
    fetch_refusal: Option<String>,
    branch_refusal: Option<String>,
    on_press: EventHandler<ToolbarPress>,
    on_status_sized: EventHandler<(f32, f32)>,
    key: DiffKey,
}

impl Toolbar {
    /// The toolbar around `status`.
    pub fn new(status: StatusBox) -> Self {
        Self {
            status,
            counts: None,
            fetch_refusal: None,
            branch_refusal: None,
            on_press: EventHandler::new(|_: ToolbarPress| {}),
            on_status_sized: EventHandler::new(|_: (f32, f32)| {}),
            key: DiffKey::None,
        }
    }

    /// The current branch's distance from its upstream: Push's and Pull's badges.
    pub fn counts(mut self, counts: Option<AheadBehind>) -> Self {
        self.counts = counts;
        self
    }

    /// Why Fetch cannot act now, greying it; `None` while it can.
    pub fn fetch_refusal(mut self, refusal: Option<String>) -> Self {
        self.fetch_refusal = refusal;
        self
    }

    /// Why Branch cannot act now, greying it and its ⌄; `None` while it can.
    pub fn branch_refusal(mut self, refusal: Option<String>) -> Self {
        self.branch_refusal = refusal;
        self
    }

    pub fn on_press(mut self, on_press: impl Into<EventHandler<ToolbarPress>>) -> Self {
        self.on_press = on_press.into();
        self
    }

    /// Where the status box's bottom left corner is laid out: where the popover hangs from.
    pub fn on_status_sized(mut self, on_sized: impl Into<EventHandler<(f32, f32)>>) -> Self {
        self.on_status_sized = on_sized.into();
        self
    }
}

// By what it draws: `EventHandler` never compares equal.
impl PartialEq for Toolbar {
    fn eq(&self, other: &Self) -> bool {
        self.status == other.status
            && self.counts == other.counts
            && self.fetch_refusal == other.fetch_refusal
            && self.branch_refusal == other.branch_refusal
            && self.key == other.key
    }
}

impl std::fmt::Debug for Toolbar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toolbar")
            .field("status", &self.status)
            .field("counts", &self.counts)
            .finish_non_exhaustive()
    }
}

impl KeyExt for Toolbar {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for Toolbar {
    fn render(&self) -> impl IntoElement {
        let press = |which: ToolbarPress| {
            let on_press = self.on_press.clone();
            EventHandler::new(move |()| on_press.call(which))
        };
        let never = || EventHandler::new(|()| {});
        let greyed = |icon, caption, sentence: &str, badge| ToolButton {
            icon,
            label: Some(caption),
            name: caption,
            enabled: false,
            tip: Tip::Sentence(sentence.to_owned()),
            badge,
            icon_menu: false,
            on_press: never(),
        };
        let (ahead, behind) = self
            .counts
            .map_or((0, 0), |counts| (counts.ahead, counts.behind));
        let fetch = ToolButton {
            icon: icons::fetch,
            label: Some(FETCH_CAPTION),
            name: FETCH_CAPTION,
            enabled: self.fetch_refusal.is_none(),
            tip: match &self.fetch_refusal {
                Some(refusal) => Tip::Sentence(refusal.clone()),
                None => Tip::Named {
                    name: FETCH_CAPTION.to_owned(),
                    chord: accelerators::chord_name(Action::Fetch, Os::current()),
                },
            },
            badge: 0,
            icon_menu: false,
            on_press: press(ToolbarPress::Fetch),
        };
        let branching = self.branch_refusal.is_none();
        let branch_tip = || match &self.branch_refusal {
            Some(refusal) => Tip::Sentence(refusal.clone()),
            None => Tip::None,
        };
        let branch = ToolButton {
            icon: icons::new_branch,
            label: Some(BRANCH_CAPTION),
            name: BRANCH_CAPTION,
            enabled: branching,
            tip: branch_tip(),
            badge: 0,
            icon_menu: false,
            on_press: press(ToolbarPress::NewBranch),
        };
        let branch_menu = {
            let on_press = self.on_press.clone();
            let refusal = self.branch_refusal.clone();
            EventHandler::new(move |()| {
                let on_press = on_press.clone();
                ContextMenu::open(context_menu(
                    vec![vec![
                        Item::new(NEW_BRANCH_CAPTION, ToolbarPress::NewBranch)
                            .chord(Action::NewBranch)
                            .greyed(refusal.clone()),
                    ]],
                    move |choice: ToolbarPress| on_press.call(choice),
                ));
            })
        };
        let gap = || rect().width(Size::px(metrics::TOOLBAR_GROUP_GAP));
        let on_sized = self.on_status_sized.clone();
        let activity = self.on_press.clone();

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(metrics::TOOLBAR))
            .padding(Gaps::new(0., 6., 0., 6.))
            .cross_align(Alignment::center())
            .background(tones::CHROME)
            .font_size(metrics::BODY)
            .child(
                ToolButton {
                    icon: icons::sidebar,
                    label: None,
                    name: SIDEBAR_TOGGLE_NAME,
                    enabled: true,
                    tip: Tip::None,
                    badge: 0,
                    icon_menu: false,
                    on_press: press(ToolbarPress::ToggleSidebar),
                }
                .element(),
            )
            .child(
                greyed(
                    icons::quick_launch,
                    QUICK_LAUNCH_CAPTION,
                    COMES_WITH_QUICK_LAUNCH,
                    0,
                )
                .element(),
            )
            .child(gap())
            .child(fetch.element())
            .child(greyed(icons::pull, PULL_CAPTION, COMES_WITH_PULL_AND_PUSH, behind).element())
            .child(greyed(icons::push, PUSH_CAPTION, COMES_WITH_PULL_AND_PUSH, ahead).element())
            .child(gap())
            .child(greyed(icons::archive, STASH_CAPTION, COMES_WITH_STASHING, 0).element())
            .child(chevron(
                STASH_MENU_NAME,
                false,
                Tip::Sentence(COMES_WITH_STASHING.to_owned()),
                never(),
            ))
            // The status box, ~300 px at 1024 px wide, centred in the room between the groups
            // and the first thing to shrink as the window narrows.
            .child(
                rect()
                    .width(Size::flex(1.))
                    .min_width(Size::px(
                        metrics::STATUS_BOX_MIN_WIDTH + 2. * metrics::TOOLBAR_GROUP_GAP,
                    ))
                    .height(Size::fill())
                    .padding(Gaps::new(
                        0.,
                        metrics::TOOLBAR_GROUP_GAP,
                        0.,
                        metrics::TOOLBAR_GROUP_GAP,
                    ))
                    .center()
                    .child(
                        rect()
                            .width(Size::fill())
                            .max_width(Size::px(metrics::STATUS_BOX_WIDTH))
                            .height(Size::px(metrics::STATUS_BOX_HEIGHT))
                            .corner_radius(4.)
                            .background(tones::STATUS_BOX)
                            .padding(Gaps::new(
                                0.,
                                metrics::STATUS_BOX_GLYPH + 12.,
                                0.,
                                metrics::STATUS_BOX_GLYPH + 12.,
                            ))
                            .on_sized(move |e: Event<SizedEventData>| {
                                on_sized.call((e.area.min_x(), e.area.max_y()));
                            })
                            // Fork's list glyph at the box's left, and only it opens the
                            // popover: a press on the box's centre opens nothing.
                            .child(
                                rect()
                                    .position(Position::new_absolute().left(4.).bottom(4.))
                                    .a11y_role(AccessibilityRole::Button)
                                    .a11y_builder(|node| node.set_value(ACTIVITY_GLYPH_NAME))
                                    .padding(2.)
                                    .on_press(move |_| activity.call(ToolbarPress::Activity))
                                    .child(icons::activity(
                                        metrics::STATUS_BOX_GLYPH,
                                        tones::TOOLBAR_TEXT,
                                    )),
                            )
                            .child(self.status.clone()),
                    ),
            )
            .child(branch.element())
            .child(chevron(
                BRANCH_MENU_NAME,
                branching,
                branch_tip(),
                branch_menu,
            ))
            .child(gap())
            .child(
                ToolButton {
                    icon_menu: true,
                    ..greyed(
                        icons::appearance,
                        APPEARANCE_CAPTION,
                        COMES_WITH_PREFERENCES,
                        0,
                    )
                }
                .element(),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
