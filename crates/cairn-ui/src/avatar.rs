//! An author's avatar, as Fork draws one before a name: a rounded square of the author's
//! initials, its colour chosen from the name out of a fixed set (`style::tones::AVATARS`).
//! Drawn from the name alone: no network is asked, and no picture is fetched.

use freya::prelude::*;

use crate::style::{metrics, tones};

/// The initials an avatar draws for `name`: the first letter or digit of its first word and of
/// its last, upper-cased — `Ada Lovelace` is `AL` — one for a name of one word, and `?` for a
/// name with none.
pub fn initials(name: &str) -> String {
    let lead = |word: &str| word.chars().find(|c| c.is_alphanumeric());
    let mut words = name.split_whitespace().filter_map(lead);
    let first = words.next();
    let last = words.next_back();
    let mut drawn: String = first
        .into_iter()
        .chain(last)
        .flat_map(char::to_uppercase)
        .collect();
    if drawn.is_empty() {
        drawn.push('?');
    }
    drawn
}

/// The colour `name`'s avatar is drawn in: one of [`tones::AVATARS`], the same for the same name
/// every time and on every machine (FNV-1a over its bytes, which no build changes).
pub fn avatar_colour(name: &str) -> Color {
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let at = usize::try_from(hash % tones::AVATARS.len() as u64).unwrap_or(0);
    tones::AVATARS
        .get(at)
        .copied()
        .unwrap_or(tones::AVATAR_TEXT)
}

/// `name`'s avatar: its initials, white and bold, on its colour, in a square
/// [`metrics::AVATAR`] across.
pub fn avatar(name: &str) -> Rect {
    rect()
        .width(Size::px(metrics::AVATAR))
        .height(Size::px(metrics::AVATAR))
        .corner_radius(metrics::AVATAR_RADIUS)
        .background(avatar_colour(name))
        .center()
        .child(
            label()
                .text(initials(name))
                .max_lines(1)
                .font_size(metrics::AVATAR_TEXT)
                .font_weight(FontWeight::BOLD)
                .color(tones::AVATAR_TEXT),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: the last word's letter dropped, a lower-case letter kept, punctuation taken
    /// for a letter, or nothing drawn for an empty name.
    #[test]
    fn initials_are_the_first_and_last_words_letters() {
        for (name, drawn) in [
            ("Ada Lovelace", "AL"),
            ("A U Thor", "AT"),
            ("grace hopper", "GH"),
            ("Margaret", "M"),
            ("  Zoë   Ångström ", "ZÅ"),
            ("(bot) deploy", "BD"),
            ("", "?"),
            ("—", "?"),
        ] {
            assert_eq!(initials(name), drawn, "{name:?}");
        }
    }

    /// One name, one colour, every time; and the set is used, not one colour for all. Caught
    /// by: a colour chosen at random or by a hasher keyed per process, or every name in one.
    #[test]
    fn a_name_always_takes_the_same_colour_of_the_set() {
        assert_eq!(avatar_colour("Ada Lovelace"), avatar_colour("Ada Lovelace"));
        let names = [
            "Ada Lovelace",
            "Grace Hopper",
            "Margaret Hamilton",
            "Alan Turing",
            "Zoë",
        ];
        let colours: std::collections::HashSet<_> = names
            .iter()
            .map(|name| {
                let colour = avatar_colour(name);
                assert!(tones::AVATARS.contains(&colour), "{name}'s is off the set");
                format!("{colour:?}")
            })
            .collect();
        assert!(colours.len() > 1, "every name took one colour");
    }
}
