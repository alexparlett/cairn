//! The dates the window draws, in the zone it is handed ([`LocalZone`]): the history's date
//! column, the Commit tab's dates and the activity popover's times — Fork's presentation, in
//! English, which needs no locale data.

use cairn_model::Timestamp;

use crate::local_time::LocalZone;

const SECONDS_PER_DAY: i64 = 86_400;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The wall clock `seconds` after the epoch reads at `offset` seconds east of UTC: its day,
/// month's name, year, hour, minute and second. Total over every `i64`.
fn wall_clock(seconds: i64, offset: i32) -> (i64, &'static str, i64, i64, i64, i64) {
    let local = seconds.saturating_add(i64::from(offset));
    let days = local.div_euclid(SECONDS_PER_DAY);
    let second_of_day = local.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let month_name = usize::try_from(month - 1)
        .ok()
        .and_then(|at| MONTHS.get(at))
        .copied()
        .unwrap_or("???");
    (
        day,
        month_name,
        year,
        second_of_day / 3600,
        (second_of_day % 3600) / 60,
        second_of_day % 60,
    )
}

/// `author_time` as the history's date column draws it: Fork's `d Mon yyyy HH:mm` —
/// `16 Oct 2026 12:12`, `6 Oct 2026 09:05` — at the offset `zone` gives that instant. Total
/// over every `i64`.
pub fn history_date(author_time: i64, zone: &LocalZone) -> String {
    let (day, month, year, hour, minute, _) = wall_clock(author_time, zone.offset_at(author_time));
    format!("{day} {month} {year:04} {hour:02}:{minute:02}")
}

/// `time` as the Commit tab draws it: Fork's `16 Oct 2026 12:12:33 +01:00`, at the offset
/// `zone` gives that instant. The day is two digits, as the hours are: Fork's Windows build is
/// .NET, whose `dd` pads. Total over every timestamp.
pub fn long_date(time: Timestamp, zone: &LocalZone) -> String {
    let offset = zone.offset_at(time.seconds);
    let (day, month, year, hour, minute, second) = wall_clock(time.seconds, offset);
    let sign = if offset < 0 { '-' } else { '+' };
    let east = offset.unsigned_abs();
    format!(
        "{day:02} {month} {year:04} {hour:02}:{minute:02}:{second:02} {sign}{:02}:{:02}",
        east / 3600,
        (east % 3600) / 60
    )
}

/// What the Commit tab says once, after the author's date, when the system's zone could not
/// be read and every date is drawn in UTC ([`LocalZone::fell_back_to_utc`]): ` (UTC)`; nothing
/// otherwise.
pub fn utc_fallback_note(zone: &LocalZone) -> &'static str {
    if zone.fell_back_to_utc() {
        " (UTC)"
    } else {
        ""
    }
}

/// The time of day `seconds` after the epoch reads in `zone`, `HH:MM:SS`: when an operation
/// started, as the activity popover says it.
pub fn time_of_day(seconds: i64, zone: &LocalZone) -> String {
    let (_, _, _, hour, minute, second) = wall_clock(seconds, zone.offset_at(seconds));
    format!("{hour:02}:{minute:02}:{second:02}")
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
    use jiff::tz::{Offset, TimeZone};

    fn fixed(seconds: i32) -> LocalZone {
        let offset = Offset::from_seconds(seconds).unwrap_or_else(|error| panic!("{error}"));
        LocalZone::read(TimeZone::fixed(offset))
    }

    fn utc() -> LocalZone {
        fixed(0)
    }

    /// Reference values from GNU `date -d @<seconds> '+%-d %b %Y %H:%M'` in a zone of each
    /// offset. Caught by: a padded day, the instant drawn in UTC whatever the zone, a 12-hour
    /// clock, or a month's name off by one.
    #[test]
    fn the_history_date_is_forks_at_the_zones_offset() {
        for (seconds, offset, shown) in [
            (0, 0, "1 Jan 1970 00:00"),
            (1_700_000_000, 0, "14 Nov 2023 22:13"),
            (1_700_000_000, 3_600, "14 Nov 2023 23:13"),
            (1_700_000_000, 5 * 3_600 + 30 * 60, "15 Nov 2023 03:43"),
            (1_759_276_800, -8 * 3_600, "30 Sep 2025 16:00"),
            (951_782_400, 0, "29 Feb 2000 00:00"),
            (-1, 0, "31 Dec 1969 23:59"),
            (1_791_882_000, 0, "13 Oct 2026 09:00"),
            (1_791_018_000, 3_600, "3 Oct 2026 10:00"),
        ] {
            assert_eq!(
                history_date(seconds, &fixed(offset)),
                shown,
                "{seconds} at {offset}s east of UTC"
            );
        }
    }

    /// A zone with daylight-saving time draws each date at the offset of its own instant.
    /// Caught by: the zone's offset read once, for now, and used for every date.
    #[test]
    fn each_date_takes_its_own_instants_offset() {
        let london = LocalZone::read(
            TimeZone::posix("GMT0BST,M3.5.0/1,M10.5.0").unwrap_or_else(|error| panic!("{error}")),
        );
        assert_eq!(history_date(1_782_907_200, &london), "1 Jul 2026 13:00");
        assert_eq!(history_date(1_767_268_800, &london), "1 Jan 2026 12:00");
        assert_eq!(
            long_date(Timestamp::new(1_782_907_200, -7 * 3_600), &london),
            "01 Jul 2026 13:00:00 +01:00",
            "the zone's offset, never the one the commit was recorded at"
        );
    }

    /// Fork's Commit-tab presentation, `25 Nov 2020 01:11:30 +01:00` — Fork's own example is
    /// the first row — at the zone's offset, whatever offset the commit was recorded at; the
    /// reference from GNU `date -d @<seconds> '+%d %b %Y %H:%M:%S %:z'` in a zone of that
    /// offset. Caught by: the recorded offset kept, the offset without its colon or its sign,
    /// an unpadded day, or a 12-hour clock.
    #[test]
    fn a_commit_tab_date_reads_as_fork_shows_it_in_the_zone() {
        for (seconds, offset, shown) in [
            (1_606_263_090, 3_600, "25 Nov 2020 01:11:30 +01:00"),
            (
                1_700_000_000,
                5 * 3_600 + 30 * 60,
                "15 Nov 2023 03:43:20 +05:30",
            ),
            (1_700_000_000, -8 * 3_600, "14 Nov 2023 14:13:20 -08:00"),
            (1_700_000_000, -30 * 60, "14 Nov 2023 21:43:20 -00:30"),
            (1_704_067_199, 14 * 3_600, "01 Jan 2024 13:59:59 +14:00"),
            (1_709_251_200, -12 * 3_600, "29 Feb 2024 12:00:00 -12:00"),
            (951_782_400, 0, "29 Feb 2000 00:00:00 +00:00"),
        ] {
            for recorded in [0, offset, -offset, 9 * 3_600] {
                assert_eq!(
                    long_date(Timestamp::new(seconds, recorded), &fixed(offset)),
                    shown,
                    "{seconds} in a zone {offset}s east of UTC, recorded at {recorded}"
                );
            }
        }
    }

    /// A zone the worker could not read draws every date in UTC, and the note says so; a zone
    /// not read yet draws them in UTC and the note says nothing; the dates themselves never
    /// say it, so it is said once, where the Commit tab puts the note. Caught by: the fallback
    /// said nowhere, said in every date, or said before the worker answered.
    #[test]
    fn only_the_note_says_the_zone_fell_back_to_utc() {
        let at = Timestamp::new(1_606_263_090, 3_600);
        assert_eq!(utc_fallback_note(&LocalZone::unreadable()), " (UTC)");
        assert_eq!(utc_fallback_note(&LocalZone::default()), "");
        assert_eq!(utc_fallback_note(&fixed(0)), "");
        assert_eq!(
            long_date(at, &LocalZone::unreadable()),
            "25 Nov 2020 00:11:30 +00:00"
        );
        assert_eq!(
            long_date(at, &LocalZone::default()),
            "25 Nov 2020 00:11:30 +00:00"
        );
        assert_eq!(
            history_date(1_606_263_090, &LocalZone::unreadable()),
            "25 Nov 2020 00:11"
        );
        assert_eq!(
            time_of_day(1_606_263_090, &LocalZone::unreadable()),
            "00:11:30"
        );
    }

    /// A commit recorded before 1970 shows its true instant — git itself does not agree with
    /// itself there (`%ad` prints nothing, `fuller` clamps to the epoch, some commands refuse
    /// the commit). Reference values from GNU `date`, as above. Caught by: truncating division
    /// in place of `div_euclid` and `rem_euclid`, which puts a negative instant on the wrong
    /// day with a negative hour — most visibly the epoch itself under a negative offset, which
    /// falls back into 1969.
    #[test]
    fn a_timestamp_before_the_epoch_reads_as_its_true_instant() {
        for (seconds, offset, shown) in [
            (-1, 0, "31 Dec 1969 23:59:59 +00:00"),
            (0, -3_600, "31 Dec 1969 23:00:00 -01:00"),
            (-1, -30 * 60, "31 Dec 1969 23:29:59 -00:30"),
            (-86_401, 5 * 3_600 + 30 * 60, "31 Dec 1969 05:29:59 +05:30"),
            (-1_000_000_000, -8 * 3_600, "24 Apr 1938 14:13:20 -08:00"),
            (-2_208_988_800, 0, "01 Jan 1900 00:00:00 +00:00"),
        ] {
            assert_eq!(
                long_date(Timestamp::new(seconds, offset), &fixed(offset)),
                shown,
                "{seconds} at {offset}s east of UTC"
            );
        }
    }

    /// The case a naive leap rule gets wrong.
    #[test]
    fn handles_the_four_hundred_year_leap_day() {
        assert_eq!(history_date(951_782_400, &utc()), "29 Feb 2000 00:00");
    }

    #[test]
    fn the_extremes_render_rather_than_panicking() {
        for seconds in [i64::MIN, i64::MAX] {
            assert!(!history_date(seconds, &utc()).is_empty());
            assert!(!time_of_day(seconds, &utc()).is_empty());
            for offset in [i32::MIN, 0, i32::MAX] {
                assert!(!long_date(Timestamp::new(seconds, offset), &utc()).is_empty());
            }
        }
    }

    /// The popover's times: the time of day at the zone's offset. Caught by: the time drawn in
    /// UTC whatever the zone.
    #[test]
    fn the_time_of_day_is_the_zones() {
        assert_eq!(time_of_day(3_723, &utc()), "01:02:03");
        assert_eq!(time_of_day(3_723, &fixed(-2 * 3_600)), "23:02:03");
        assert_eq!(time_of_day(3_723, &fixed(30 * 60)), "01:32:03");
    }
}
