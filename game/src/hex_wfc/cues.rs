//! Semantic cue catalog for the hex facility's authoritative gameplay events.
//!
//! The hex match's event set: the observation-safe relayout beats (warning /
//! committed / cancelled — the arc's central "observation holds structure"
//! mechanic), an escape, a deterministic out-of-bounds body recovery, and the
//! final match-complete beat. Each maps to a legend-backed marker colour and an
//! audio one-shot. The relayout beats mirror the square lattice's `Mutation*`
//! presentation exactly.

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
    pub glyph: &'static str,
    pub label: &'static str,
    pub marker: MarkerRole,
    pub sound: HexWfcSound,
}

#[cfg(test)]
pub(super) const ALL_EVENTS: [HexMatchEventKind; 10] = [
    HexMatchEventKind::MutationWarning,
    HexMatchEventKind::MutationCommitted,
    HexMatchEventKind::MutationCancelled,
    HexMatchEventKind::PlayerRecovered,
    HexMatchEventKind::PlayerEscaped,
    HexMatchEventKind::LanternCacheCollected,
    HexMatchEventKind::AnchorDeployed,
    HexMatchEventKind::AnchorRecovered,
    HexMatchEventKind::GuardianCatch,
    HexMatchEventKind::MatchFinished,
];

pub(super) fn cue_for(kind: HexMatchEventKind) -> CueDefinition {
    match kind {
        HexMatchEventKind::MutationWarning => cue(
            "~",
            "STRUCTURE BREATHING",
            MarkerRole::NextRoom,
            HexWfcSound::Reroute,
        ),
        HexMatchEventKind::MutationCommitted => cue(
            "X",
            "ROOMS MUTATED",
            MarkerRole::Collapse,
            HexWfcSound::Reroute,
        ),
        HexMatchEventKind::MutationCancelled => {
            cue("=", "MUTATION HELD", MarkerRole::You, HexWfcSound::Hold)
        }
        HexMatchEventKind::PlayerRecovered => cue(
            "~",
            "BODY RECOVERED",
            MarkerRole::NextRoom,
            HexWfcSound::Recover,
        ),
        HexMatchEventKind::PlayerEscaped => {
            cue(">", "RUNNER ESCAPED", MarkerRole::Exit, HexWfcSound::Escape)
        }
        HexMatchEventKind::LanternCacheCollected => cue(
            "+",
            "LANTERNS RECOVERED",
            MarkerRole::Teammate,
            HexWfcSound::Recover,
        ),
        HexMatchEventKind::AnchorDeployed => cue(
            "#",
            "THRESHOLD ANCHORED",
            MarkerRole::Control,
            HexWfcSound::Hold,
        ),
        HexMatchEventKind::AnchorRecovered => cue(
            "^",
            "LANTERN RECOVERED",
            MarkerRole::Control,
            HexWfcSound::Recover,
        ),
        HexMatchEventKind::GuardianCatch => cue(
            "!",
            "GUARDIAN SETBACK",
            MarkerRole::Collapse,
            HexWfcSound::Guardian,
        ),
        HexMatchEventKind::MatchFinished => cue(
            "#",
            "MATCH COMPLETE",
            MarkerRole::Exit,
            HexWfcSound::Complete,
        ),
    }
}

const fn cue(
    glyph: &'static str,
    label: &'static str,
    marker: MarkerRole,
    sound: HexWfcSound,
) -> CueDefinition {
    CueDefinition {
        glyph,
        label,
        marker,
        sound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hex_event_has_audio_and_legend_backed_visual_semantics() {
        for kind in ALL_EVENTS {
            let definition = cue_for(kind);
            assert!(!definition.glyph.is_empty());
            assert!(!definition.label.is_empty());
            assert!(observed_style::marker(definition.marker).signal);
            let _audio = definition.sound;
        }
    }
}
