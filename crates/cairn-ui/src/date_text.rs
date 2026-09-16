//! The date column's text.

/// `author_time` as `YYYY-MM-DD HH:MM` in UTC. Total over every `i64`.
pub fn utc_minutes(author_time: i64) -> String {
    let days = author_time.div_euclid(SECONDS_PER_DAY);
    let second_of_day = author_time.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = second_of_day / 3600;
    let minute = (second_of_day % 3600) / 60;

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

const SECONDS_PER_DAY: i64 = 86_400;

/// `(year, month, day)` for days since 1970-01-01: Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // Epoch shifted to 0000-03-01, so a leap day lands at the end of a cycle
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

    /// Reference values from `date -u -d @<seconds>`.
    #[test]
    fn matches_the_system_date_at_known_instants() {
        assert_eq!(utc_minutes(0), "1970-01-01 00:00");
        assert_eq!(utc_minutes(1_700_000_000), "2023-11-14 22:13");
        assert_eq!(utc_minutes(1_759_276_800), "2025-10-01 00:00");
    }

    /// The case a naive leap rule gets wrong.
    #[test]
    fn handles_the_four_hundred_year_leap_day() {
        assert_eq!(utc_minutes(951_782_400), "2000-02-29 00:00");
    }

    #[test]
    fn times_before_the_epoch_go_backwards_rather_than_wrapping() {
        assert_eq!(utc_minutes(-1), "1969-12-31 23:59");
        assert_eq!(utc_minutes(-86_400), "1969-12-31 00:00");
    }

    #[test]
    fn the_extremes_of_the_type_render_rather_than_panicking() {
        let low = utc_minutes(i64::MIN);
        let high = utc_minutes(i64::MAX);
        assert!(!low.is_empty());
        assert!(!high.is_empty());
        // Wider than the column; the fixed-width test below does not cover this.
        assert!(high.len() > "1970-01-01 00:00".len());
    }

    /// Four-digit years only: `{year:04}` pads but does not truncate.
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
