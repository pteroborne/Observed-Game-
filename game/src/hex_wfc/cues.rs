//! Semantic cue catalog for the hex facility's authoritative gameplay events.
//!
//! The hex match's event set: the observation-safe relayout beats (warning /
//! committed / cancelled — the arc's central "observation holds structure"
//! mechanic), an escape, a deterministic out-of-bounds body recovery, and the
//! final match-complete beat. Each maps to a legend-backed marker colour and an
//! audio one-shot. The relayout beats mirror the square lattice's `Mutation*`
//! presentation exactly.
//!
//! `marker` and `sound` are the whole player-facing contract: what colour the
//! world answers an event with (`feedback.rs`, a world-space beacon at the
//! event's cell) and what it sounds like from there (`audio.rs`, spatialized
//! at the same place). There is no on-screen text anymore — a player reads the
//! event by looking and listening at the place it happened, not by reading a
//! banner that could be anywhere.
//!
//! `label` survives only as plumbing for two pre-existing developer/debug
//! readouts outside this packet's file list — `hud.rs`'s debug-gated status
//! line and `sim.rs`'s LAN-desync status line, neither shown in normal play —
//! and is not part of the diegetic contract described above.

use observed_match::hex_wfc::HexMatchEventKind;
use observed_style::MarkerRole;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HexWfcSound {
    Reroute,
    Hold,
    Recover,
    Escape,
    Complete,
    Guardian,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CueDefinition {
    /// Developer-status text only; see the module doc.
    pub label: &'static str,
    pub marker: MarkerRole,
    pub sound: HexWfcSound,
}

#[cfg(test)]
pub(super) const ALL_EVENTS: [HexMatchEventKind; 17] = [
    HexMatchEventKind::MutationWarning,
    HexMatchEventKind::MutationCommitted,
    HexMatchEventKind::MutationCancelled,
    HexMatchEventKind::PlayerRecovered,
    HexMatchEventKind::PlayerEscaped,
    HexMatchEventKind::LanternCacheCollected,
    HexMatchEventKind::KeystoneCollected,
    HexMatchEventKind::DualStationProgress,
    HexMatchEventKind::DualStationCompleted,
    HexMatchEventKind::MonitorSurveyed,
    HexMatchEventKind::ExitDenied,
    HexMatchEventKind::AnchorDeployed,
    HexMatchEventKind::AnchorRecovered,
    HexMatchEventKind::PadDeployed,
    HexMatchEventKind::PadTraversed,
    HexMatchEventKind::GuardianCatch,
    HexMatchEventKind::MatchFinished,
];

pub(super) fn cue_for(kind: HexMatchEventKind) -> CueDefinition {
    match kind {
        HexMatchEventKind::MutationWarning => {
            cue("STRUCTURE BREATHING", MarkerRole::NextRoom, HexWfcSound::Reroute)
        }
        HexMatchEventKind::MutationCommitted => {
            cue("ROOMS MUTATED", MarkerRole::Collapse, HexWfcSound::Reroute)
        }
        HexMatchEventKind::MutationCancelled => {
            cue("MUTATION HELD", MarkerRole::You, HexWfcSound::Hold)
        }
        HexMatchEventKind::PlayerRecovered => {
            cue("BODY RECOVERED", MarkerRole::NextRoom, HexWfcSound::Recover)
        }
        HexMatchEventKind::PlayerEscaped => {
            cue("RUNNER ESCAPED", MarkerRole::Exit, HexWfcSound::Escape)
        }
        HexMatchEventKind::LanternCacheCollected => cue(
            "LANTERNS RECOVERED",
            MarkerRole::Teammate,
            HexWfcSound::Recover,
        ),
        HexMatchEventKind::KeystoneCollected => {
            cue("KEYSTONE CLAIMED", MarkerRole::NextRoom, HexWfcSound::Hold)
        }
        HexMatchEventKind::DualStationProgress => cue(
            "TEAM LINK HOLDING",
            MarkerRole::Teammate,
            HexWfcSound::Hold,
        ),
        HexMatchEventKind::DualStationCompleted => cue(
            "TEAM STATION COMPLETE",
            MarkerRole::Control,
            HexWfcSound::Complete,
        ),
        HexMatchEventKind::MonitorSurveyed => cue(
            "LOCAL SCHEMATIC SURVEYED",
            MarkerRole::Control,
            HexWfcSound::Recover,
        ),
        HexMatchEventKind::ExitDenied => cue(
            "EXIT REQUIRES 2 KEYS + TEAM STATION",
            MarkerRole::Exit,
            HexWfcSound::Recover,
        ),
        HexMatchEventKind::AnchorDeployed => {
            cue("THRESHOLD ANCHORED", MarkerRole::Control, HexWfcSound::Hold)
        }
        HexMatchEventKind::PadDeployed => {
            cue("PLATE SET", MarkerRole::Control, HexWfcSound::Hold)
        }
        // Arrival is the loud half of a jump: the departure is behind you and the
        // room you land in is the thing you need to reorient in.
        HexMatchEventKind::PadTraversed => {
            cue("PLATE LINK", MarkerRole::NextRoom, HexWfcSound::Reroute)
        }
        HexMatchEventKind::AnchorRecovered => cue(
            "LANTERN RECOVERED",
            MarkerRole::Control,
            HexWfcSound::Recover,
        ),
        HexMatchEventKind::GuardianCatch => {
            cue("GUARDIAN SETBACK", MarkerRole::Collapse, HexWfcSound::Guardian)
        }
        HexMatchEventKind::MatchFinished => {
            cue("MATCH COMPLETE", MarkerRole::Exit, HexWfcSound::Complete)
        }
    }
}

const fn cue(label: &'static str, marker: MarkerRole, sound: HexWfcSound) -> CueDefinition {
    CueDefinition {
        label,
        marker,
        sound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The new contract: every event still has an audio one-shot and a
    /// legend-backed (signal-tier) marker colour — the pair `feedback.rs` and
    /// `audio.rs` turn into a world-space beacon and a spatialized sound. This
    /// replaces the old glyph/label screen-banner assertion; label itself is
    /// no longer part of the player-facing contract (see the module doc).
    #[test]
    fn every_hex_event_has_audio_and_a_legend_backed_world_marker() {
        for kind in ALL_EVENTS {
            let definition = cue_for(kind);
            assert!(!definition.label.is_empty());
            assert!(observed_style::marker(definition.marker).signal);
            let _audio = definition.sound;
        }
    }
}
