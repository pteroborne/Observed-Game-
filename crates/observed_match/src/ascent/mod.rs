//! Shared deterministic Architect Ascent rules, promoted from architect_lab.

pub mod economy;
pub mod facility;
pub mod falls;
pub mod placement;
pub mod prison;
pub mod requisition;
pub mod session;
pub mod sim;

/// Canonical loyal Observer bodies per team; the non-embodied Architect is separate.
pub const MAX_OBSERVERS_PER_TEAM: u8 = 3;
