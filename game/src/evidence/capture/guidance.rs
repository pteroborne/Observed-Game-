//! Deterministic in-match presentation proof. Roles use production Start/Loading;
//! power, charge, jail and void are explicit rules fixtures, not completed human play.
use super::{Shot, shot};
use crate::{
    GameState,
    hex_wfc::overlay::{MatchOverlayState, PausePage},
    play_setup::{PlayPreset, PlayRules, PlaySeat, PlaySetupDraft},
};

use crate::hex_wfc::GuidanceCaptureCase as Case;

pub(super) fn sweep() -> Vec<Shot> {
    use Case::*;
    let mut shots = Vec::new();
    for (seat, entry) in [
        (PlaySeat::Observer, "00_observer_launch"),
        (PlaySeat::Observer, "10_rogue_launch"),
        (PlaySeat::Observer, "13_spectator_launch"),
    ] {
        let spectate = entry == "13_spectator_launch";
        shots.push(Shot {
            setup: Some(PlaySetupDraft {
                rules: PlayRules::Ascent,
                seat,
                guardian: false,
                ..PlaySetupDraft::for_preset(if spectate {
                    PlayPreset::Spectate
                } else {
                    PlayPreset::TeamRace
                })
            }),
            ..shot(entry, GameState::Play)
        });
        shots.push(Shot {
            production_launch: true,
            extra_settle: 3.0,
            ..shot(
                if spectate {
                    "14_spectator_help"
                } else if entry == "10_rogue_launch" {
                    "11_rogue_entry"
                } else {
                    "01_observer_help"
                },
                GameState::HexWfc,
            )
        });
        let cases: &[(&str, Case)] = if spectate {
            &[
                ("15_spectator_chase", Human),
                ("16_spectator_overview", Overview),
                ("17_spectator_eyes", Eyes),
                ("18_spectator_next", Follow),
                ("19_spectator_map", Map),
                ("20_spectator_guide", Guide),
            ]
        } else if entry == "10_rogue_launch" {
            &[("12_rogue_board", Rogue)]
        } else {
            &[
                ("02_observer_hud", Human),
                ("03_observer_hud_large", Large),
                ("04_power_lost", Dark),
                ("05_tool_empty", Empty),
                ("06_teammate_rescue", Rescue),
                ("07_prison_hud", Jailed),
                ("08_prison_map", Map),
                ("09_prison_guide", Guide),
            ]
        };
        for &(label, case) in cases {
            shots.push(Shot {
                overlay: Some(if matches!(case, Map | Guide) {
                    MatchOverlayState::SurvivorMap
                } else {
                    MatchOverlayState::Playing
                }),
                guidance: Some(case),
                extra_settle: if matches!(case, Rogue) { 2.0 } else { 0.0 },
                ..shot(label, GameState::HexWfc)
            });
        }
        if spectate || entry == "10_rogue_launch" {
            shots.push(Shot {
                overlay: Some(MatchOverlayState::Pause(PausePage::Root)),
                ..shot(
                    if spectate {
                        "21_spectator_pause"
                    } else {
                        "12b_rogue_pause"
                    },
                    GameState::HexWfc,
                )
            });
            shots.push(Shot {
                guidance: Some(Review),
                ..shot(
                    if spectate {
                        "22_spectator_review"
                    } else {
                        "12c_rogue_review"
                    },
                    GameState::HexWfc,
                )
            });
            let labels = if spectate {
                [
                    "23_spectator_help_2",
                    "24_spectator_help_3",
                    "25_spectator_help_4",
                ]
            } else {
                ["12d_rogue_help_2", "12e_rogue_help_3", "12f_rogue_help_4"]
            };
            for label in labels {
                shots.push(Shot {
                    help_action: Some(crate::screens::onboarding::OnboardingAction::Next),
                    ..shot(label, GameState::HexWfc)
                });
            }
        }
    }
    shots
}
