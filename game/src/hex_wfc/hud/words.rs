//! What the in-play HUD says: pure functions from the match to each panel's words,
//! kept apart from the layout (`play`) so they can be tested without a renderer.
use observed_match::hex_wfc::{
    DUAL_STATION_HOLD_TICKS, HexInteraction, HexInteractionAction, HexMatchEventKind,
};

use crate::settings::{Settings, key_name};

/// How long a notice stays, and how long it takes to fade in and out, seconds.
pub(super) const NOTICE_SECONDS: f64 = 3.2;
const NOTICE_FADE: f64 = 0.35;
/// The most keystone pips drawn; a larger requirement is shown as numbers.
pub(super) const MAX_PIPS: u8 = 6;

/// Whether a notice is good news or not.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::hex_wfc) enum Tone {
    #[default]
    Good,
    Against,
}

/// What the objective panel says.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::hex_wfc) struct ObjectiveView {
    pub heading: String,
    pub goal: String,
    /// Keystones held and required, while keystones are the goal.
    pub pips: Option<(u8, u8)>,
}

/// What the prompt panel says.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::hex_wfc) struct PromptView {
    pub key: String,
    pub pad: &'static str,
    pub title: String,
    pub detail: &'static str,
    /// How far a held action has got, 0 to 1, while it is being held.
    pub progress: Option<f32>,
}

/// What the objective panel is drawn from.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::hex_wfc) struct ObjectiveFacts {
    pub floor: u8,
    pub team: u8,
    pub escaped: bool,
    /// Whether this match has objectives at all.
    pub enabled: bool,
    pub keystones: u8,
    pub required: u8,
    pub station_done: bool,
    /// A team of one holds a station alone.
    pub solo: bool,
    /// The match plays Architect Ascent: the summit, not the race's objectives.
    pub ascent: bool,
    /// The local body is in its team's prison maze.
    pub jailed: bool,
    /// Teammates in the prison maze, while the local body is free.
    pub teammates_jailed: u8,
}

/// The objective panel, from the team's progress.
#[must_use]
pub(in crate::hex_wfc) fn objective_view(facts: ObjectiveFacts) -> ObjectiveView {
    let ObjectiveFacts {
        floor,
        team,
        escaped,
        enabled,
        keystones,
        required,
        station_done,
        solo,
        ascent,
        jailed,
        teammates_jailed,
    } = facts;
    let heading = if jailed {
        format!("PRISON / TEAM {}", u16::from(team) + 1)
    } else {
        format!(
            "FLOOR {} / TEAM {}",
            u16::from(floor) + 1,
            u16::from(team) + 1
        )
    };
    let (goal, pips) = if jailed {
        ("Find the way out of the maze".to_owned(), None)
    } else if teammates_jailed > 0 {
        let who = if teammates_jailed == 1 {
            "a teammate".to_owned()
        } else {
            format!("{teammates_jailed} teammates")
        };
        (format!("Hold the prison lobby to free {who}"), None)
    } else if ascent {
        ("Climb to the summit".to_owned(), None)
    } else if escaped {
        ("Escaped. Waiting for the team".to_owned(), None)
    } else if enabled && keystones < required {
        let goal = if required > MAX_PIPS {
            format!("Find keystones  {keystones} / {required}")
        } else {
            "Find keystones".to_owned()
        };
        (
            goal,
            (required <= MAX_PIPS).then_some((keystones, required)),
        )
    } else if enabled && !station_done {
        let goal = if solo {
            "Find and hold a station console"
        } else {
            "Synchronize a station with your teammate"
        };
        (goal.to_owned(), None)
    } else {
        ("Regroup at the exit".to_owned(), None)
    };
    ObjectiveView {
        heading,
        goal,
        pips,
    }
}

/// The prompt panel for an interaction, with the key it is bound to.
#[must_use]
pub(in crate::hex_wfc) fn prompt_view(
    prompt: &HexInteraction,
    settings: &Settings,
    station_ticks: u16,
) -> PromptView {
    let (key, pad, hold) = match prompt.action {
        HexInteractionAction::Interact => (settings.bindings.interact, "X", false),
        HexInteractionAction::HoldInteract => (settings.bindings.interact, "X", true),
        HexInteractionAction::DeployLantern => (settings.bindings.torch, "LB", false),
        HexInteractionAction::RecoverLantern => (settings.bindings.recover_lantern, "B", false),
    };
    PromptView {
        key: key_name(key).to_owned(),
        pad,
        title: if hold {
            format!("Hold to {}", lowercase_first(prompt.title))
        } else {
            prompt.title.to_owned()
        },
        detail: prompt.detail,
        progress: (hold && station_ticks > 0).then(|| {
            (f32::from(station_ticks) / f32::from(DUAL_STATION_HOLD_TICKS)).clamp(0.0, 1.0)
        }),
    }
}

/// "Synchronize station" as it reads after "Hold to".
fn lowercase_first(title: &str) -> String {
    let mut chars = title.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// The notice for an event of the local player's, and whether it went for or against.
#[must_use]
pub(in crate::hex_wfc) const fn notice_for(
    kind: HexMatchEventKind,
) -> Option<(&'static str, Tone)> {
    Some(match kind {
        HexMatchEventKind::AnchorDeployed => ("Connection anchored", Tone::Good),
        HexMatchEventKind::AnchorRecovered => {
            ("Lantern recovered. Connection released", Tone::Good)
        }
        HexMatchEventKind::KeystoneCollected => ("Team keystone collected", Tone::Good),
        HexMatchEventKind::MonitorSurveyed => ("Team map updated", Tone::Good),
        HexMatchEventKind::LanternCacheCollected => ("Anchor lanterns collected", Tone::Good),
        HexMatchEventKind::DualStationCompleted => {
            ("Station synchronized. Head for the exit", Tone::Good)
        }
        HexMatchEventKind::PlayerRecovered => ("Recovered to safe ground", Tone::Against),
        HexMatchEventKind::ExitDenied => (
            "Complete the team's objectives before leaving",
            Tone::Against,
        ),
        HexMatchEventKind::GuardianCatch => ("The Guardian caught you. Regroup", Tone::Against),
        HexMatchEventKind::PlayerJailed => {
            ("Caught. Find the way out of the prison", Tone::Against)
        }
        HexMatchEventKind::PlayerReleased => ("Out of the prison", Tone::Good),
        HexMatchEventKind::Jailbreak => ("Your team broke you out", Tone::Good),
        HexMatchEventKind::PlayerLost => ("Lost to the void", Tone::Against),
        _ => return None,
    })
}

/// How visible a notice is, `left` seconds before it ends: faded in, held, faded out.
#[must_use]
pub(in crate::hex_wfc) fn notice_alpha(left: f64) -> f32 {
    let shown = NOTICE_SECONDS - left;
    #[allow(clippy::cast_possible_truncation)]
    let alpha = (shown / NOTICE_FADE)
        .min(left / NOTICE_FADE)
        .clamp(0.0, 1.0) as f32;
    alpha
}

#[cfg(test)]
mod tests {
    use observed_match::hex_wfc::{HexInteraction, HexInteractionAction, HexMatchEventKind};

    use super::{
        MAX_PIPS, NOTICE_SECONDS, ObjectiveFacts, Tone, notice_alpha, notice_for, objective_view,
        prompt_view,
    };
    use crate::settings::Settings;

    fn facts(keystones: u8, required: u8, station_done: bool, escaped: bool) -> ObjectiveFacts {
        ObjectiveFacts {
            floor: 6,
            team: 0,
            escaped,
            enabled: true,
            keystones,
            required,
            station_done,
            solo: false,
            ascent: false,
            jailed: false,
            teammates_jailed: 0,
        }
    }

    #[test]
    fn the_prison_comes_first_and_ascent_climbs() {
        let jailed = objective_view(ObjectiveFacts {
            jailed: true,
            teammates_jailed: 1,
            ..facts(1, 3, false, false)
        });
        assert_eq!(jailed.heading, "PRISON / TEAM 1");
        assert_eq!(jailed.goal, "Find the way out of the maze");
        let rescue = |teammates_jailed| {
            objective_view(ObjectiveFacts {
                teammates_jailed,
                ascent: true,
                ..facts(1, 3, false, false)
            })
            .goal
        };
        assert_eq!(rescue(1), "Hold the prison lobby to free a teammate");
        assert_eq!(rescue(2), "Hold the prison lobby to free 2 teammates");
        assert_eq!(
            rescue(0),
            "Climb to the summit",
            "the race's keystones are not Ascent's"
        );
    }

    #[test]
    fn the_objective_is_the_one_thing_to_do_next() {
        let view = |keys, station, escaped| objective_view(facts(keys, 3, station, escaped));
        assert_eq!(view(1, false, false).goal, "Find keystones");
        assert_eq!(view(1, false, false).pips, Some((1, 3)));
        assert_eq!(
            view(3, false, false).goal,
            "Synchronize a station with your teammate"
        );
        assert_eq!(view(3, false, false).pips, None);
        assert_eq!(view(3, true, false).goal, "Regroup at the exit");
        assert!(view(3, true, true).goal.starts_with("Escaped"));
        assert_eq!(view(0, false, false).heading, "FLOOR 7 / TEAM 1");
        let solo = objective_view(ObjectiveFacts {
            solo: true,
            ..facts(3, 3, false, false)
        });
        assert_eq!(solo.goal, "Find and hold a station console");
    }

    #[test]
    fn too_many_keystones_for_pips_are_counted_instead() {
        let many = objective_view(facts(2, MAX_PIPS + 1, false, false));
        assert_eq!(many.pips, None);
        assert!(many.goal.contains("2 / 7"), "{}", many.goal);
    }

    #[test]
    fn a_held_action_shows_progress_and_a_press_does_not() {
        let settings = Settings::default();
        let hold = HexInteraction {
            action: HexInteractionAction::HoldInteract,
            title: "Synchronize station",
            detail: "Release to cancel",
        };
        let press = HexInteraction {
            action: HexInteractionAction::Interact,
            title: "Collect keystone",
            detail: "Shared with your team",
        };
        let held = prompt_view(
            &hold,
            &settings,
            observed_match::hex_wfc::DUAL_STATION_HOLD_TICKS / 2,
        );
        assert!((held.progress.expect("held") - 0.5).abs() < 1e-3);
        assert_eq!(held.title, "Hold to synchronize station");
        assert_eq!(prompt_view(&hold, &settings, 0).progress, None);
        assert_eq!(prompt_view(&press, &settings, 40).progress, None);
        assert_eq!(prompt_view(&press, &settings, 0).pad, "X");
    }

    #[test]
    fn bad_news_is_amber_and_good_news_is_not() {
        assert_eq!(
            notice_for(HexMatchEventKind::GuardianCatch).map(|n| n.1),
            Some(Tone::Against)
        );
        assert_eq!(
            notice_for(HexMatchEventKind::ExitDenied).map(|n| n.1),
            Some(Tone::Against)
        );
        assert_eq!(
            notice_for(HexMatchEventKind::KeystoneCollected).map(|n| n.1),
            Some(Tone::Good)
        );
        assert_eq!(notice_for(HexMatchEventKind::MutationCommitted), None);
    }

    #[test]
    fn a_notice_fades_in_holds_and_fades_out() {
        assert!(notice_alpha(NOTICE_SECONDS) < 0.01, "starts invisible");
        assert!(
            (notice_alpha(NOTICE_SECONDS / 2.0) - 1.0).abs() < 1e-6,
            "held"
        );
        assert!(notice_alpha(0.0) < 0.01, "ends invisible");
        assert!(
            notice_alpha(0.1) > 0.0 && notice_alpha(0.1) < 1.0,
            "fading out"
        );
    }
}
