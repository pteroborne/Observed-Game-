//! Pacing bounds assertions across modes and power policies.

use super::playtest_instrument::run_mode_playtest;
use super::*;
use crate::economy::PowerPolicy;

/// Pacing window (floor, ceiling) in beats for each architect mode.
///
/// Proposed windows are scaled by facility footprint:
/// - Pocket (6x5x1): [20, 80] beats
/// - Quick Climb (8x6x2): [80, 300] beats
/// - Full Ascent (10x8x2): [120, 450] beats
/// - Deep Stack (8x6x5): [200, 900] beats
#[must_use]
pub const fn pacing_window(mode: ArchitectMode) -> (u64, u64) {
    match mode {
        ArchitectMode::Pocket => (20, 80),
        ArchitectMode::QuickClimb => (80, 300),
        ArchitectMode::FullAscent => (120, 450),
        ArchitectMode::DeepStack => (200, 900),
    }
}

pub fn check_pacing(mode: ArchitectMode, policy: PowerPolicy) -> Result<u64, String> {
    let stats = run_mode_playtest(mode, policy, 1000);
    let (floor, ceiling) = pacing_window(mode);
    if stats.outcome == MatchOutcome::Running {
        Err(format!(
            "{:?} under {:?}: hit beat cap 1000 (stalemate)",
            mode, policy
        ))
    } else if stats.total_beats < floor {
        Err(format!(
            "{:?} under {:?}: ended at beat {} below floor {} (walkover, outcome: {:?})",
            mode, policy, stats.total_beats, floor, stats.outcome
        ))
    } else if stats.total_beats > ceiling {
        Err(format!(
            "{:?} under {:?}: ended at beat {} above ceiling {} (outcome: {:?})",
            mode, policy, stats.total_beats, ceiling, stats.outcome
        ))
    } else {
        Ok(stats.total_beats)
    }
}

#[ignore = "~2.5 minute 12-run pacing bounds sweep; full regression matrix. cargo dev-test-all"]
#[test]
fn test_all_modes_pacing_bounds() {
    let mut failures = Vec::new();
    let mut successes = Vec::new();

    for mode in ArchitectMode::ALL {
        for policy in PowerPolicy::ALL {
            match check_pacing(mode, policy) {
                Ok(beats) => {
                    let msg = format!("{:?} {:?} in {} beats", mode, policy, beats);
                    println!("PASS: {msg}");
                    successes.push(msg);
                }
                Err(err) => {
                    eprintln!("FAIL: {err}");
                    failures.push(err);
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Pacing bounds violated in {} of 12 runs ({} passed):\n{}",
        failures.len(),
        successes.len(),
        failures.join("\n")
    );
}

#[test]
fn test_pocket_restorable_pacing_gate() {
    if let Err(err) = check_pacing(ArchitectMode::Pocket, PowerPolicy::Restorable) {
        panic!("Pocket Restorable violated pacing bounds: {err}");
    }
}
