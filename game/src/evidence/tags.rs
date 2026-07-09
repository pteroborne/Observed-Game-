//! Presentation-facing audit tags: components and small helpers that other
//! presentation modules (`screens::hud`, `screens::place::shell`, `screens::input`)
//! attach or query to drive the `OBSERVED2_VIS_AUDIT` / `OBSERVED2_FREECAM` debug
//! visuals. Kept separate from the audit runner (`evidence::audit`) and the
//! snapshot collectors (`evidence::snapshot`) so those two can depend on this
//! module instead of each other.

use bevy::prelude::*;
use observed_core::RoomId;

use crate::teleport::{DoorGap, GapKind, ThresholdLink};

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticThresholdVisualKind {
    Frame,
    Leaf,
    FrameLight,
    Label,
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticThresholdStatus {
    Passage,
    TetheredPassage,
    Locked,
    Collapsed,
    Sealed,
}

impl DiagnosticThresholdStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            DiagnosticThresholdStatus::Passage => "passage",
            DiagnosticThresholdStatus::TetheredPassage => "tethered_passage",
            DiagnosticThresholdStatus::Locked => "locked",
            DiagnosticThresholdStatus::Collapsed => "collapsed_rubble",
            DiagnosticThresholdStatus::Sealed => "sealed",
        }
    }
}

#[derive(Clone, Component, Debug)]
pub(crate) struct DiagnosticThresholdVisual {
    pub(crate) threshold: ThresholdLink,
    pub(crate) kind: DiagnosticThresholdVisualKind,
    pub(crate) status: DiagnosticThresholdStatus,
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticTacMapRole {
    Route,
    Room,
    Exit,
    Keystone,
    Rival,
    Player,
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) struct DiagnosticTacMapVisual {
    pub(crate) role: DiagnosticTacMapRole,
    pub(crate) room: Option<RoomId>,
    pub(crate) other_room: Option<RoomId>,
}

impl DiagnosticTacMapVisual {
    pub(crate) fn route(a: RoomId, b: RoomId) -> Self {
        Self {
            role: DiagnosticTacMapRole::Route,
            room: Some(a),
            other_room: Some(b),
        }
    }

    pub(crate) fn room(room: RoomId) -> Self {
        Self {
            role: DiagnosticTacMapRole::Room,
            room: Some(room),
            other_room: None,
        }
    }

    pub(crate) fn one(role: DiagnosticTacMapRole, room: Option<RoomId>) -> Self {
        Self {
            role,
            room,
            other_room: None,
        }
    }
}

pub(crate) fn visual_audit_enabled() -> bool {
    std::env::var("OBSERVED2_VIS_AUDIT").is_ok()
}

pub(crate) fn freecam_enabled() -> bool {
    std::env::var("OBSERVED2_FREECAM").is_ok()
}

/// Whether the match spawns the debug status HUD (top-left readouts) and the legend.
/// Off by default (Phase 50): normal play communicates diegetically and through the
/// tac-map. `OBSERVED2_DEBUG_HUD` turns it on explicitly; a visual-audit or freecam
/// session implies it, since those flows read the on-screen readouts.
pub(crate) fn debug_hud_enabled() -> bool {
    std::env::var("OBSERVED2_DEBUG_HUD").is_ok() || visual_audit_enabled() || freecam_enabled()
}

pub(crate) fn threshold_label(threshold: &ThresholdLink) -> String {
    format!(
        "R{}:S{} -> H{}-{}:{}:S{}",
        threshold.room.room.0,
        threshold.room.slot.0,
        threshold.hall.hall.a.0,
        threshold.hall.hall.b.0,
        threshold.hall.side.0,
        threshold.hall.slot.0
    )
}

pub(crate) fn threshold_status(gap: &DoorGap, tethered: bool) -> DiagnosticThresholdStatus {
    if tethered && gap.kind.is_passage() {
        DiagnosticThresholdStatus::TetheredPassage
    } else if gap.kind == GapKind::LockedExit {
        DiagnosticThresholdStatus::Locked
    } else if gap.kind == GapKind::Collapsed {
        DiagnosticThresholdStatus::Collapsed
    } else if gap.kind.is_passage() {
        DiagnosticThresholdStatus::Passage
    } else {
        DiagnosticThresholdStatus::Sealed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teleport;

    #[test]
    fn threshold_label_uses_stable_domain_ids() {
        let link = ThresholdLink {
            room: teleport::RoomThreshold {
                room: RoomId(3),
                slot: teleport::ThresholdSlotId(1),
            },
            hall: teleport::HallThreshold {
                hall: teleport::HallId::new(RoomId(3), RoomId(4)),
                side: RoomId(3),
                slot: teleport::ThresholdSlotId(0),
            },
            local_side: teleport::ThresholdLocalSide::Room,
        };
        assert_eq!(threshold_label(&link), "R3:S1 -> H3-4:3:S0");
    }
}
