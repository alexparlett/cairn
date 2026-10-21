//! The time zone the window draws dates in: read once by the application's worker and handed
//! to the render layer as a value, so no view reads the system's zone itself
//! (`the_render_layer_is_handed_its_time_zone`). Cheap to clone — a zone read from the system
//! is shared, not copied — so each row, the Commit tab and the activity popover are handed it.

use jiff::Timestamp;
use jiff::tz::TimeZone;

/// The zone dates are drawn in, and how it was had. Compared by which read it is, never by the
/// zone's contents: every row, the Commit tab and the popover compare the one they hold on
/// each render, and a zone read from the system is a whole zone file.
#[derive(Debug, Clone)]
pub struct LocalZone {
    zone: TimeZone,
    had: Had,
    /// Which read of the system's zone this is; 0 for UTC had without one.
    read: u64,
}

impl PartialEq for LocalZone {
    fn eq(&self, other: &Self) -> bool {
        self.had == other.had && self.read == other.read
    }
}

impl Eq for LocalZone {}

/// The number the next zone read is given.
static NEXT_READ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Had {
    /// Not read yet: dates are drawn in UTC, and nothing says so.
    Waiting,
    /// The system's zone.
    Read,
    /// The system's zone could not be read: dates are drawn in UTC, and the Commit tab says so.
    Unreadable,
}

impl Default for LocalZone {
    /// Before the worker has read the zone: UTC, said nowhere.
    fn default() -> Self {
        Self {
            zone: TimeZone::UTC,
            had: Had::Waiting,
            read: 0,
        }
    }
}

impl LocalZone {
    /// The zone the worker read from the system.
    pub fn read(zone: TimeZone) -> Self {
        Self {
            zone,
            had: Had::Read,
            read: NEXT_READ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// The system's zone could not be read: UTC, which the Commit tab's date says.
    pub fn unreadable() -> Self {
        Self {
            zone: TimeZone::UTC,
            had: Had::Unreadable,
            read: 0,
        }
    }

    /// Whether dates are drawn in UTC because the system's zone could not be read — which the
    /// Commit tab's date says, once.
    pub fn fell_back_to_utc(&self) -> bool {
        self.had == Had::Unreadable
    }

    /// The offset east of UTC, in seconds, the zone gives the instant `seconds` after the Unix
    /// epoch: its daylight-saving time where that instant falls in it. An instant past the
    /// range the zone can place is drawn at UTC's.
    pub fn offset_at(&self, seconds: i64) -> i32 {
        Timestamp::from_second(seconds).map_or(0, |at| self.zone.to_offset(at).seconds())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::tz::Offset;

    /// A zone with daylight-saving time gives each instant its own offset, a fixed one gives
    /// every instant the same, and an instant past jiff's range falls back to UTC's rather
    /// than failing. Caught by: one offset read for the zone and used for every date (the
    /// summer's in winter), or a panic on a far date.
    #[test]
    fn each_instant_takes_the_offset_its_zone_gives_it() {
        let london =
            TimeZone::posix("GMT0BST,M3.5.0/1,M10.5.0").unwrap_or_else(|error| panic!("{error}"));
        let zone = LocalZone::read(london);
        // 2026-07-01 and 2026-01-01, noon UTC.
        assert_eq!(zone.offset_at(1_782_907_200), 3_600, "summer");
        assert_eq!(zone.offset_at(1_767_268_800), 0, "winter");
        let fixed = LocalZone::read(TimeZone::fixed(Offset::constant(-8)));
        assert_eq!(fixed.offset_at(0), -8 * 3_600);
        for far in [i64::MIN, i64::MAX] {
            assert_eq!(zone.offset_at(far), 0, "{far}");
        }
    }

    /// Only an unreadable zone says it fell back: one waiting for the worker draws in UTC too,
    /// but says nothing, and a read one is the system's.
    #[test]
    fn only_an_unreadable_zone_says_it_fell_back_to_utc() {
        assert!(LocalZone::unreadable().fell_back_to_utc());
        assert!(!LocalZone::default().fell_back_to_utc());
        assert!(!LocalZone::read(TimeZone::UTC).fell_back_to_utc());
        assert_eq!(LocalZone::default().offset_at(1_782_907_200), 0);
        assert_ne!(LocalZone::default(), LocalZone::unreadable());
    }

    /// A zone and its clones compare equal, by the read they came from and not by the zone's
    /// contents, and another read is another zone. Caught by: a clone comparing unequal, which
    /// would draw every row again on each render.
    #[test]
    fn a_zone_compares_by_the_read_it_came_from() {
        let zone = LocalZone::read(TimeZone::fixed(Offset::constant(1)));
        assert_eq!(zone, zone.clone());
        assert_ne!(zone, LocalZone::read(TimeZone::fixed(Offset::constant(1))));
        assert_ne!(zone, LocalZone::default());
        assert_eq!(LocalZone::unreadable(), LocalZone::unreadable());
    }
}
