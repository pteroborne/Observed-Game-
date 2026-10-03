//! Contextual next steps, shared by the HUD and the team's discovered map.
use observed_match::hex_wfc::HexBodyPlace;

use super::words::{ObjectiveFacts, ObjectiveView, objective_view};
use crate::hex_wfc::sim::HexWfcRuntime;

pub(in crate::hex_wfc) fn objective(runtime: &HexWfcRuntime) -> ObjectiveView {
    objective_view(facts(runtime))
}

pub(in crate::hex_wfc) fn facts(runtime: &HexWfcRuntime) -> ObjectiveFacts {
    let game = &runtime.match_state;
    let player = runtime.local();
    let team = &game.teams[&player.team];
    ObjectiveFacts {
        floor: player.cell.level,
        team: player.team.0,
        escaped: player.escaped,
        enabled: game.objectives.enabled,
        keystones: team.objectives.keystones,
        required: game.objectives.keystones_required,
        station_done: team.objectives.dual_station_complete,
        solo: team.members.len() == 1,
        ascent: runtime.ascent.is_some(),
        jailed: player.place == HexBodyPlace::Prison,
        corrupted: runtime.ascent.as_ref().is_some_and(|ascent| {
            ascent
                .observer_for(player.id)
                .and_then(|id| ascent.rules().observers.get(&id))
                .is_some_and(|observer| {
                    observer.state == observed_match::ascent::sim::ObserverState::Corrupted
                })
        }),
        teammates_jailed: u8::try_from(
            team.members
                .iter()
                .filter(|&&id| id != player.id && game.players[&id].place == HexBodyPlace::Prison)
                .count(),
        )
        .unwrap_or(u8::MAX),
    }
}

pub(in crate::hex_wfc) fn next_step(runtime: &HexWfcRuntime) -> &'static str {
    let charge = runtime.ascent.as_ref().and_then(|ascent| {
        Some(
            ascent
                .rules()
                .economy
                .charge(ascent.observer_for(runtime.local_player)?),
        )
    });
    detail(
        facts(runtime),
        crate::hex_wfc::power::local_floor_powered(runtime),
        charge,
    )
}

fn detail(facts: ObjectiveFacts, powered: Option<bool>, charge: Option<u32>) -> &'static str {
    if facts.escaped {
        "Your run is complete. The remaining teammates are still playing."
    } else if facts.corrupted {
        "A fall into true void changes your side. You now play for the Rogue AI."
    } else if facts.jailed {
        "Follow the prison passages to the exit, or ask your Architect for rescue."
    } else if facts.teammates_jailed > 0 {
        "Stay in the prison lobby until the rescue completes."
    } else if !facts.ascent {
        "Explore with your team. The map records only what your team has discovered."
    } else if powered == Some(false) {
        "Find this floor's generator to restore power, or ask your Architect for help."
    } else if charge == Some(0) {
        "Your kinetic tool is empty. Stand at a powered station to recharge."
    } else {
        "Find a route upward. Ask your Architect when you need a way forward."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confinement_and_rescue_take_priority_over_power_and_charge() {
        let mut facts = ObjectiveFacts {
            ascent: true,
            ..Default::default()
        };
        assert!(detail(facts, Some(false), Some(0)).contains("generator"));
        assert!(detail(facts, Some(true), Some(0)).contains("recharge"));
        facts.teammates_jailed = 1;
        assert!(detail(facts, Some(false), Some(0)).contains("prison lobby"));
        facts.jailed = true;
        assert!(detail(facts, Some(false), Some(0)).contains("prison passages"));
        facts.corrupted = true;
        assert!(detail(facts, Some(false), Some(0)).contains("Rogue AI"));
        assert!(objective_view(facts).goal.contains("loyal Observer"));
        facts.escaped = true;
        assert!(detail(facts, Some(false), Some(0)).contains("run is complete"));
        assert_eq!(objective_view(facts).goal, "Escaped. Waiting for the team");
    }
}
