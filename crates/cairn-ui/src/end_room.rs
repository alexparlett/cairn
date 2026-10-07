//! Room after a list's last row, so the row is never under the horizontal scrollbar.
//!
//! Freya's `VirtualScrollView` (the fork at `caa46f8`, `scrollviews/virtual_scrollview.rs`)
//! draws its horizontal scrollbar as an overlay — an absolutely positioned bar 16 px tall over
//! the bottom of the viewport (`scrollviews/scrollbar.rs`) — and lays out no padding of its own:
//! it reads only its width and height bounds, never a padding. So a list that can scroll
//! sideways (a long path, a long line) would end with its last row flush against the bottom
//! edge, the bar drawn over it. Each such list counts empty rows after its last one, tall
//! enough together for the bar and a little more; an index past the list's own rows builds
//! an empty row, and nothing that places a row by its index (a search for a row, a scroll to
//! one, the Commit tab's `Expansion`) sees them, since they come after every real row.

/// The horizontal scrollbar's thickness in Freya's `ScrollBar`, in pixels: what it covers of
/// the viewport's bottom.
pub const SCROLLBAR_THICKNESS: f32 = 16.0;

/// The least room after a list's last row, in pixels: the horizontal scrollbar's thickness
/// and 4 px more.
pub const END_ROOM: f32 = SCROLLBAR_THICKNESS + 4.0;

/// How many rows a list of `rows` rows, each `row_height` tall, is laid out as: its own, then
/// as many empty ones as [`END_ROOM`] needs. An empty list stays empty.
pub fn with_end_room(rows: usize, row_height: f32) -> usize {
    if rows == 0 || row_height <= 0.0 {
        return rows;
    }
    rows + (END_ROOM / row_height).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_room_is_at_least_end_room_in_whole_rows_and_an_empty_list_has_none() {
        for height in [17.0f32, 20.0, 24.0, 26.0, 40.0] {
            let extra = with_end_room(10, height) - 10;
            assert!(extra as f32 * height >= END_ROOM, "{height}");
            assert!(
                (extra - 1) as f32 * height < END_ROOM,
                "{height}: a row more than needed"
            );
        }
        assert_eq!(with_end_room(0, 24.0), 0);
    }
}
