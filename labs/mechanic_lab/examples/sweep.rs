//! Run one scripted match per variant and report how each ended.
//!
//! The point of the bench in miniature: change one thing, see whether the match
//! changes. `cargo run -p mechanic_lab --example sweep`

use observed_mechanics::objective::{PlantRule, PlantWin};
use observed_mechanics::spec::{ModeSpec, ObjectiveKind, Rules, ThreatKind, deal};
use observed_mechanics::threat::ConeInteraction;
use observed_mechanics::{bot, step::step};

fn play(label: &str, spec: &ModeSpec) {
    let rules = Rules::from_spec(spec);
    let mut state = deal(spec);
    let mut turns = 0;
    while state.outcome.is_none() && turns < spec.turn_limit {
        let intents = bot::intents(&state, &rules);
        step(&mut state, &rules, &intents);
        turns += 1;
    }
    let planted = state
        .flags
        .iter()
        .filter(|f| f.planted_by.is_some())
        .count();
    let jailed = state.pawns.iter().filter(|p| p.jailed).count();
    println!(
        "{label:<34} {turns:>3} turns  flags {planted}/{}  jailed {jailed}/{}  {:?}",
        state.flags.len(),
        state.pawns.len(),
        state.outcome,
    );
}

fn main() {
    println!("--- mode 1: Plant -------------------------------------------");
    play("as shipped", &ModeSpec::plant());
    play(
        "stand-only plant",
        &ModeSpec {
            objective: ObjectiveKind::PlantFlags {
                rule: PlantRule::StandOnly,
                win: PlantWin::All,
            },
            ..ModeSpec::plant()
        },
    );
    play(
        "no guardians",
        &ModeSpec {
            threats: vec![ThreatKind::None],
            ..ModeSpec::plant()
        },
    );
    play(
        "cone blocks guardians",
        &ModeSpec {
            cone_interaction: ConeInteraction::Blocked,
            ..ModeSpec::plant()
        },
    );

    println!("\n--- mode 2: Base --------------------------------------------");
    play("as shipped", &ModeSpec::base());
    play(
        "rivals only, no guardians",
        &ModeSpec {
            threats: vec![ThreatKind::RivalPawns],
            guardian_count: 0,
            ..ModeSpec::base()
        },
    );
    play(
        "stand-only plant",
        &ModeSpec {
            objective: ObjectiveKind::PlantFlags {
                rule: PlantRule::StandOnly,
                win: PlantWin::Majority,
            },
            ..ModeSpec::base()
        },
    );
    play(
        "stand-only, rivals only",
        &ModeSpec {
            threats: vec![ThreatKind::RivalPawns],
            guardian_count: 0,
            objective: ObjectiveKind::PlantFlags {
                rule: PlantRule::StandOnly,
                win: PlantWin::Majority,
            },
            ..ModeSpec::base()
        },
    );
    play(
        "four pawns a side",
        &ModeSpec {
            pawns_per_team: 4,
            ..ModeSpec::base()
        },
    );
}
