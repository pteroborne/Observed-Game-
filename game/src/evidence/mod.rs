//! The game's **evidence pipelines**, consolidated (Arc G5): every opt-in way the
//! game photographs or audits itself for agent-readable proof.
//!
//! - [`capture`] — the `OBSERVED2_CAPTURE*` screenshot/GIF drivers (showcase shots,
//!   staged scenario captures, the overhead tour, the bot-POV walkthrough).
//! - [`audit`] — the `OBSERVED2_VIS_AUDIT` visual audit: staged inspection scenarios,
//!   the freecam, debug match coercion, and the audit phase machine.
//! - [`snapshot`] — the world → `observed_diagnostics` JSON collectors the audit
//!   writes its machine-readable findings through.
//! - [`tags`] — the presentation-facing components/helpers the renderer attaches so
//!   the audit (and debug HUD) can identify what it is looking at.
//! - [`driver`] — small helpers shared by every scripted driver.
//!
//! Everything here is opt-in via environment variables and a no-op in normal play.

pub(crate) mod audit;
pub(crate) mod capture;
pub(crate) mod driver;
pub(crate) mod snapshot;
pub(crate) mod tags;

pub(crate) use tags::{
    DiagnosticTacMapRole, DiagnosticTacMapVisual, DiagnosticThresholdVisual,
    DiagnosticThresholdVisualKind, freecam_enabled, threshold_label, threshold_status,
    visual_audit_enabled,
};

use bevy::prelude::*;

/// Wire every opt-in evidence pipeline (each a no-op unless its environment
/// variable is set).
pub(crate) fn configure(app: &mut App) {
    capture::configure_captures(app);
    audit::configure_audit(app);
}
