//! Finding a control by the name assistive technology reads off it — a glyph button's value or
//! a control's name — for the test binaries that press or measure controls with no text of their
//! own. Reached by `#[path]`.

use freya::elements::rect::RectElement;
use freya::prelude::*;
use freya_testing::TestingRunner;

/// Every control assistive technology reads as `name`, with where it is laid out.
pub fn named(test: &TestingRunner, name: &str) -> Vec<(RectElement, Area)> {
    test.find_many(|node, element| {
        Rect::try_downcast(element)
            .filter(|rect| {
                let builder = &rect.accessibility.builder;
                builder.value() == Some(name) || builder.label() == Some(name)
            })
            .map(|rect| (rect, node.layout().area))
    })
}

/// The one control named `name`: a second of that name fails.
pub fn one_named(test: &TestingRunner, name: &str) -> (RectElement, Area) {
    let mut found = named(test, name);
    assert_eq!(
        found.len(),
        1,
        "{} controls are named {name:?}",
        found.len()
    );
    found.remove(0)
}
