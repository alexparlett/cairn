use cairn_model::CommitSummary;
use freya::prelude::*;

/// One row of the history list.
///
/// Takes the commit by value and renders it; selection and activation are the
/// caller's business, reported through `on_select`.
#[derive(PartialEq, Clone)]
pub struct CommitRow {
    commit: CommitSummary,
    selected: bool,
    on_select: EventHandler<()>,
    key: DiffKey,
}

impl CommitRow {
    pub fn new(commit: CommitSummary, on_select: EventHandler<()>) -> Self {
        Self {
            commit,
            selected: false,
            on_select,
            key: DiffKey::None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

// Hand-written because `EventHandler` is not `Debug`. Components stay printable
// anyway: a row whose fields you cannot inspect is unreadable in a devtools
// tree and in a panic message.
impl std::fmt::Debug for CommitRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommitRow")
            .field("commit", &self.commit.id)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl KeyExt for CommitRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl ComponentOwned for CommitRow {
    fn render(self) -> impl IntoElement {
        let on_select = self.on_select;

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new(6., 10., 6., 10.))
            .spacing(10.)
            .maybe(self.selected, |el| el.background((45, 45, 55)))
            .on_press(move |_| on_select.call(()))
            .child(
                label()
                    // `short()` formats into a stack buffer; the copy out of
                    // it is Freya's price, not the id's — `text` takes a
                    // `Cow<'static, str>`, which no borrow can satisfy. Going
                    // through `as_str` keeps it to one 7-byte copy rather than
                    // a trip through a formatter.
                    .text(self.commit.id.short().as_str().to_string())
                    .theme_color(),
            )
            .child(label().text(self.commit.summary).expanded().theme_color())
            .child(label().text(self.commit.author_name).theme_color())
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
