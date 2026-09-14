use architect_lab::sim::{ArchitectLab, ArchitectMode, MatchOutcome};
fn main() {
    for mode in ArchitectMode::ALL {
        for bot in [false, true] {
            let mut sim = ArchitectLab::for_mode(mode).unwrap();
            sim.bot_architect = bot;
            for _ in 0..180 {
                sim.step_beat();
                if sim.outcome != MatchOutcome::Running {
                    break;
                }
            }
            println!(
                "{mode:?} bot={bot} tick={} outcome={:?} plays={} observers={:?} guardian={:?} unstable={} retracted={}",
                sim.tick,
                sim.outcome,
                sim.command_log.len(),
                sim.observers,
                sim.guardians,
                sim.contradictions.len(),
                sim.retracted.len()
            );
        }
    }
}
