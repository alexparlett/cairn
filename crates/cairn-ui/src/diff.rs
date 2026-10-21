//! A file's diff as the views draw it: the diff view and its rows, unified or side by side,
//! the notice in place of rows, the bar over it and its settings, the diff's colours and
//! typeface, the terminal columns a tab stops by, and Local Changes' staging gesture over
//! the drawn rows.

pub(crate) mod columns;
pub(crate) mod header;
pub(crate) mod line_text;
pub(crate) mod notice;
pub mod palette;
pub(crate) mod row_parts;
pub(crate) mod settings;
pub(crate) mod side_by_side_rows;
pub(crate) mod staging_gesture;
pub(crate) mod unified_rows;
pub(crate) mod view;
