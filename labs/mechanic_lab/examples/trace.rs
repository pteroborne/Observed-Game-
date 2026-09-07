//! One scripted match, turn by turn. `cargo run -p mechanic_lab --example trace -- base`

use observed_mechanics::spec::{ModeSpec, Rules, deal};
use observed_mechanics::{bot, step::step};

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "plant".into());
    let spec = match which.as_str() {
        "base" => ModeSpec::base(),
        _ => ModeSpec::plant(),
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    println!("mode  : {}", spec.name);
    println!("rules : {}", rules.summary());
    println!("bases : {:?}", spec.board.spawns);
    println!("prisons: {:?}", spec.board.prisons);
    println!("flags : {:?}", spec.board.flags);

    for turn in 0..spec.turn_limit {
        if state.outcome.is_some() {
            break;
        }
        let intents = bot::intents(&state, &rules);
        step(&mut state, &rules, &intents);
        println!(
            "t{turn:02} pawns={:?} jail={:?} fresh={:?} guards={:?} rewired={} held={} took={:?} planted={:?} {:?}",
            state
                .pawns
                .iter()
                .map(|p| (p.at.q, p.at.r))
                .collect::<Vec<_>>(),
            state
                .pawns
                .iter()
                .map(|p| u8::from(p.jailed))
                .collect::<Vec<_>>(),
            state
                .pawns
                .iter()
                .map(|p| {
                    let f = state.freshness(p.id);
                    if f == u16::MAX { -1_i32 } else { f as i32 }
                })
                .collect::<Vec<_>>(),
            state
                .guardians
                .iter()
                .map(|g| (g.at.q, g.at.r))
                .collect::<Vec<_>>(),
            state.report.rewired.len(),
            state.report.refused_rewires.len(),
            state.report.taken,
            state.report.planted,
            state.outcome,
        );
    }
    println!(
        "flags: {:?}",
        state.flags.iter().map(|f| f.planted_by).collect::<Vec<_>>()
    );
}
