//! The date column's text, and the Commit tab's.

use cairn_model::Timestamp;

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

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `time` as Fork shows a commit's date on Windows — `25 Nov 2020 01:11:30 +01:00` — at the
/// offset it was recorded with, so the instant and the offset both survive (user decision 1,
/// 2026-10-03: Fork's presentation, not git's, and in English, which needs no locale data).
/// The day is two digits, as the hours are: Fork's one example has a two-digit day, and its
/// Windows build is .NET, whose `dd` pads. Total over every timestamp.
pub fn long_date(time: Timestamp) -> String {
    let offset = i64::from(time.offset_seconds);
    let local = time.seconds.saturating_add(offset);
    let days = local.div_euclid(SECONDS_PER_DAY);
    let second_of_day = local.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let month_name = usize::try_from(month - 1)
        .ok()
        .and_then(|at| MONTHS.get(at))
        .unwrap_or(&"???");
    let hour = second_of_day / 3600;
    let minute = (second_of_day % 3600) / 60;
    let second = second_of_day % 60;
    let sign = if offset < 0 { '-' } else { '+' };
    let east = offset.unsigned_abs();

    format!(
        "{day:02} {month_name} {year:04} {hour:02}:{minute:02}:{second:02} {sign}{:02}:{:02}",
        east / 3600,
        (east % 3600) / 60
    )
}

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

    /// User decision 1 (2026-10-03): Fork's Windows presentation, `25 Nov 2020 01:11:30
    /// +01:00` (`docs/research/diff-engine/fork-detail-and-diff-ui.md`, Finding 3) — Fork's
    /// own example is the first row. Every row is the instant at the offset it was recorded
    /// with, the reference from GNU `date -d @<seconds> '+%d %b %Y %H:%M:%S %:z'` in a zone
    /// of that offset. Caught by: rendering in UTC (every offset-bearing case moves), the
    /// offset without its colon or its sign, an unpadded day, or a 12-hour clock.
    #[test]
    fn a_timestamp_reads_as_fork_shows_it_at_its_own_offset() {
        for (seconds, offset_seconds, shown) in [
            (1_606_263_090, 3600, "25 Nov 2020 01:11:30 +01:00"),
            (
                1_700_000_000,
                5 * 3600 + 30 * 60,
                "15 Nov 2023 03:43:20 +05:30",
            ),
            (1_700_000_000, -8 * 3600, "14 Nov 2023 14:13:20 -08:00"),
            (1_700_000_000, -30 * 60, "14 Nov 2023 21:43:20 -00:30"),
            (1_704_067_199, 14 * 3600, "01 Jan 2024 13:59:59 +14:00"),
            (1_709_251_200, -12 * 3600, "29 Feb 2024 12:00:00 -12:00"),
            (951_782_400, 0, "29 Feb 2000 00:00:00 +00:00"),
            (
                1_759_276_800,
                5 * 3600 + 45 * 60,
                "01 Oct 2025 05:45:00 +05:45",
            ),
        ] {
            assert_eq!(
                long_date(Timestamp::new(seconds, offset_seconds)),
                shown,
                "{seconds} at {offset_seconds}s east of UTC"
            );
        }
    }

    /// User decision 2: a commit recorded before 1970 shows its true instant — git itself
    /// does not agree with itself there (`%ad` prints nothing, `fuller` clamps to the epoch,
    /// some commands refuse the commit). Reference values from GNU `date`, as above. Caught
    /// by: truncating division in place of `div_euclid` and `rem_euclid`, which puts a
    /// negative instant on the wrong day with a negative hour — most visibly the epoch
    /// itself under a negative offset, which falls back into 1969.
    #[test]
    fn a_timestamp_before_the_epoch_reads_as_its_true_instant() {
        for (seconds, offset_seconds, shown) in [
            (-1, 0, "31 Dec 1969 23:59:59 +00:00"),
            (0, -3600, "31 Dec 1969 23:00:00 -01:00"),
            (-1, -30 * 60, "31 Dec 1969 23:29:59 -00:30"),
            (-86_401, 5 * 3600 + 30 * 60, "31 Dec 1969 05:29:59 +05:30"),
            (-1_000_000_000, -8 * 3600, "24 Apr 1938 14:13:20 -08:00"),
            (-2_208_988_800, 0, "01 Jan 1900 00:00:00 +00:00"),
        ] {
            assert_eq!(
                long_date(Timestamp::new(seconds, offset_seconds)),
                shown,
                "{seconds} at {offset_seconds}s east of UTC"
            );
        }
    }

    #[test]
    fn a_timestamp_at_the_extremes_renders_rather_than_panicking() {
        for seconds in [i64::MIN, i64::MAX] {
            for offset in [i32::MIN, 0, i32::MAX] {
                assert!(!long_date(Timestamp::new(seconds, offset)).is_empty());
            }
        }
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
