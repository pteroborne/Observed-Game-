//! The headless answer: run the script and say what happened, in one table.

use crate::script;

/// Run every phase and print a report. Returns false if any phase failed, so
/// the binary can exit non-zero.
#[must_use]
pub fn run() -> bool {
    println!("PLUMB / feasibility\n");
    println!(
        "{:<52} {:>8} {:>10} {:>9} {:>8}",
        "phase", "settled", "surface", "walked", "verdict"
    );
    let mut all = true;
    for (phase, outcome) in script::report() {
        let settled = outcome
            .settled
            .map_or_else(|| "never".to_string(), |ticks| format!("{ticks}t"));
        let surface = outcome
            .surface
            .map_or_else(|| "-".to_string(), |surface| format!("{surface:?}"));
        let ok =
            outcome.ended_planted && outcome.surface == Some(phase.expect) && outcome.walked > 2.0;
        all &= ok;
        println!(
            "{:<52} {:>8} {:>10} {:>8.1}m {:>8}",
            phase.caption,
            settled,
            surface,
            outcome.walked,
            if ok { "ok" } else { "FAILED" }
        );
    }
    println!(
        "\n{}",
        if all {
            "A redirected gravity puts the subject on every surface of the room,\n\
             holds it there, and it walks along each one."
        } else {
            "At least one phase did not behave as expected; see the table."
        }
    );
    all
}
