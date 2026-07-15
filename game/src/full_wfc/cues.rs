//! Exhaustive semantic cue catalog for authoritative gameplay events.

use observed_match::full_wfc::{GameplayEventKind, GameplayEventKind::*};
use observed_style::MarkerRole;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FullWfcSound {
    Reroute,
    Tool,
    Keystone,
    Unlock,
    Guardian,
    Collapse,
    Escape,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CueDefinition {
    pub glyph: &'static str,
    pub label: &'static str,
    pub marker: MarkerRole,
    pub sound: FullWfcSound,
}

pub(super) fn cue_for(kind: GameplayEventKind) -> CueDefinition {
    match kind {
        MutationWarning => cue(
            "~",
            "STRUCTURE BREATHING",
            MarkerRole::NextRoom,
            FullWfcSound::Reroute,
        ),
        MutationCommitted => cue(
            "X",
            "ROOMS MUTATED",
            MarkerRole::Collapse,
            FullWfcSound::Reroute,
        ),
        MutationCancelled => cue("=", "MUTATION HELD", MarkerRole::You, FullWfcSound::Tool),
        AnchorDeployed => cue(
            "A",
            "ANCHOR LOCKED",
            MarkerRole::Control,
            FullWfcSound::Tool,
        ),
        AnchorRecovered => cue(
            "A",
            "ANCHOR RECOVERED",
            MarkerRole::Control,
            FullWfcSound::Tool,
        ),
        PadDeployed => cue(
            "O",
            "TELEPORT PAD DEPLOYED",
            MarkerRole::Teammate,
            FullWfcSound::Tool,
        ),
        PadRecovered => cue(
            "O",
            "TELEPORT PAD RECOVERED",
            MarkerRole::Teammate,
            FullWfcSound::Tool,
        ),
        PadUsed => cue(
            "><",
            "TELEPORT LINK",
            MarkerRole::Teammate,
            FullWfcSound::Reroute,
        ),
        KeystoneCollected => cue(
            "K",
            "KEYSTONE SECURED",
            MarkerRole::NextRoom,
            FullWfcSound::Keystone,
        ),
        DualStationProgress => cue(
            "2",
            "DUAL STATION SYNC",
            MarkerRole::Control,
            FullWfcSound::Tool,
        ),
        DualStationCompleted => cue(
            "2",
            "EXIT AUTHORIZED",
            MarkerRole::Exit,
            FullWfcSound::Unlock,
        ),
        MonitorSurveyed => cue(
            "M",
            "FACILITY SURVEY COPIED",
            MarkerRole::Teammate,
            FullWfcSound::Tool,
        ),
        GuardianCatch => cue(
            "!",
            "GUARDIAN CATCH",
            MarkerRole::Collapse,
            FullWfcSound::Collapse,
        ),
        GuardianRedirected => cue(
            "!",
            "GUARDIAN REDIRECTED",
            MarkerRole::Director,
            FullWfcSound::Guardian,
        ),
        TeamEscaped => cue(">", "TEAM ESCAPED", MarkerRole::Exit, FullWfcSound::Escape),
        MatchFinished => cue(
            "#",
            "MATCH COMPLETE",
            MarkerRole::Exit,
            FullWfcSound::Unlock,
        ),
    }
}

const fn cue(
    glyph: &'static str,
    label: &'static str,
    marker: MarkerRole,
    sound: FullWfcSound,
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
    fn every_gameplay_event_has_audio_and_legend_backed_visual_semantics() {
        for kind in GameplayEventKind::ALL {
            let definition = cue_for(kind);
            assert!(!definition.glyph.is_empty());
            assert!(!definition.label.is_empty());
            assert!(observed_style::marker(definition.marker).signal);
            let _audio = definition.sound;
        }
    }
}
