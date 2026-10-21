//! The system's time zone, read once on a worker as a repository opens, and handed to the
//! window as a value (`Update::Started`): the one place the application reads it
//! (`the_render_layer_is_handed_its_time_zone`), since reading it reads files — `TZ`'s,
//! `/etc/localtime` and the zone database — which only a worker may wait on.

use jiff::tz::TimeZone;

/// The system's zone, or `None` when it cannot be read — `TZ` naming no zone, no
/// `/etc/localtime` — so the window draws dates in UTC and says so, rather than drawing UTC
/// as though it were the system's, as `TimeZone::system` would.
pub(crate) fn read() -> Option<TimeZone> {
    TimeZone::try_system().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zone is read whatever this host is set to — the test host has a zone database and a
    /// `/etc/localtime`, or a `TZ` — and it places an instant as the system does. Caught by:
    /// a read that always fails over to UTC.
    #[test]
    fn the_systems_zone_is_read() {
        let read = read();
        assert!(read.is_some(), "this host's zone could not be read");
    }
}
