//! Turning a commit's timestamp into the text of the date column.
//!
//! **UTC, and the column says so.** A commit records its author time as seconds
//! since the epoch plus the offset the author was at; `CommitSummary` carries
//! the seconds and not the offset, and converting to the *reader's* local time
//! needs a timezone database, which is a dependency and therefore a decision
//! for the user rather than a default taken here. Rendering UTC and labelling
//! the column `Date (UTC)` is the honest version of what this can do today; a
//! column headed `Date` showing something that is not the reader's date is the
//! dishonest one.
//!
//! The civil-date arithmetic is Howard Hinnant's `civil_from_days`, which is
//! exact for every day the proleptic Gregorian calendar covers and needs no
//! table. Written out rather than pulled in: it is a dozen lines of arithmetic
//! against a dependency in a tree whose rule is that adding one is a decision.

/// `author_time` as `YYYY-MM-DD HH:MM` in UTC.
///
/// Total over every `i64`: times before the epoch land on the proleptic
/// Gregorian calendar, and the extremes of the type cannot overflow because the
/// seconds are divided down to a day count before anything is added to them. So
/// no timestamp a repository can hold — including a corrupt one — produces a
/// panic on a path a user can reach.
pub fn utc_minutes(author_time: i64) -> String {
    let days = author_time.div_euclid(SECONDS_PER_DAY);
    let second_of_day = author_time.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = second_of_day / 3600;
    let minute = (second_of_day % 3600) / 60;

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

const SECONDS_PER_DAY: i64 = 86_400;

/// `(year, month, day)` for a count of days since 1970-01-01, proleptic
/// Gregorian. Howard Hinnant's `civil_from_days`, transcribed.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // Shift the epoch to 0000-03-01 so a leap day lands at the end of a cycle
    // and every month before it has a fixed length.
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097); // 0..=146096
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // 0..=399
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153; // 0..=11, March-based
    let day = day_of_year - (153 * month_position + 2) / 5 + 1;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    };

    (year + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values taken from `date -u -d @<seconds>`, not from this
    /// implementation: a test written from the code it tests decides nothing.
    #[test]
    fn matches_the_system_date_at_known_instants() {
        assert_eq!(utc_minutes(0), "1970-01-01 00:00");
        assert_eq!(utc_minutes(1_700_000_000), "2023-11-14 22:13");
        assert_eq!(utc_minutes(1_759_276_800), "2025-10-01 00:00");
    }

    /// A leap day in a year divisible by 400 — the case a naive rule gets
    /// wrong, and the reason this is Hinnant's algorithm and not four lines of
    /// division.
    #[test]
    fn handles_the_four_hundred_year_leap_day() {
        assert_eq!(utc_minutes(951_782_400), "2000-02-29 00:00");
    }

    /// A commit can carry a timestamp before the epoch — an imported history,
    /// or a wrong clock. It renders as a date rather than as nonsense.
    #[test]
    fn times_before_the_epoch_go_backwards_rather_than_wrapping() {
        assert_eq!(utc_minutes(-1), "1969-12-31 23:59");
        assert_eq!(utc_minutes(-86_400), "1969-12-31 00:00");
    }

    /// The extremes of the type do not panic. A repository with a corrupt or
    /// hostile timestamp must not be able to take the window down; what it
    /// renders as is unimportant, that it renders at all is not.
    #[test]
    fn the_extremes_of_the_type_render_rather_than_panicking() {
        let low = utc_minutes(i64::MIN);
        let high = utc_minutes(i64::MAX);
        assert!(!low.is_empty());
        assert!(!high.is_empty());
        // And they are WIDER than the column, which is the boundary the
        // fixed-width test above deliberately does not claim to cover.
        assert!(high.len() > "1970-01-01 00:00".len());
    }

    /// Every field below the year is zero-padded, so the column is a column
    /// rather than a ragged edge that moves as the hour or the month changes
    /// digits.
    ///
    /// Scoped to four-digit years on purpose, and the name says so: `{year:04}`
    /// pads but does not truncate, so a timestamp near the ends of `i64`
    /// renders a twelve-digit year and is wider than this. That is not worth
    /// fixing — the column clips, and no repository holds such a commit — but a
    /// test called `the_rendering_is_fixed_width` would have claimed a property
    /// that is false and passed only because its fixture avoided the case.
    #[test]
    fn the_rendering_is_fixed_width_for_every_year_a_repository_holds() {
        for seconds in [0, 1_700_000_000, -86_400, 951_782_400] {
            assert_eq!(
                utc_minutes(seconds).len(),
                "1970-01-01 00:00".len(),
                "{seconds} rendered ragged: {}",
                utc_minutes(seconds)
            );
        }
    }
}
