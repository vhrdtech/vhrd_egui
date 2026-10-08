//! Small egui basics every vhrd app wants.
//!
//! - [`BuildInfo`] / [`build_info!`] / [`build_info_label`]: version, git SHA, build time and a
//!   debug / release marker, with a tooltip spelling it out (BAS-1).
//! - [`setup_labels`] / [`copyable_label`]: labels are not selectable by default, text worth copying is (BAS-2).
//! - [`hover_link`]: hovering one item highlights every item registered under the same key (BAS-3).
//! - [`StartupGuard`]: a start that died before its first frame moves the persisted state aside (PLT-4).
//!
//! The UI guide rule behind these (tooltips everywhere, highlight related items) is in the repo's `AGENTS.md`.

mod build_info;
mod hover_link;
mod labels;
mod startup_guard;

pub use build_info::{BuildInfo, build_info_label};
pub use hover_link::hover_link;
pub use labels::{copyable_label, setup_labels};
pub use startup_guard::StartupGuard;
