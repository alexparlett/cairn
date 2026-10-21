//! Every size, tone and icon the chrome draws, in one place, so the window reads as one
//! instrument: Fork's sizes, measured at 1024×768 ([`metrics`]), Fork Dark's tones and the
//! theme the window launches with ([`tones`]), the Lucide icons ([`icons`]), and how a control
//! shows keyboard focus ([`focus`]). A component
//! reads these and never writes a size or a colour of its own
//! (`chrome_sizes_and_tones_come_from_the_style_module`); the diff's colours stay in
//! `diff::palette` and the lanes' in `history::lane_palette`, each reading its tones from here
//! where Fork Dark gives one.

pub mod focus;
pub mod icons;
pub mod metrics;
pub mod tones;
