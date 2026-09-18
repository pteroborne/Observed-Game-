//! Playtest instrumentation: observation probe for bot matches across all ArchitectModes.
//! Measures Step B economy, emergency requisition, prison self-escape, and unsafe falls.
//!
//! Note: This module is an instrumentation harness for playtest data collection,
//! not a behavioral assertion test suite.

use std::collections::BTreeMap;

use super::*;
use crate::economy::{ShoveError, ShoveOutcome};

#[derive(Debug, Default)]
pub struct ShoveDiagnostic {
    pub attempts: u64,
    pub success_void: u64,
    pub success_retraction: u64,
    pub success_displaced: u64,
    pub blocked_by_wall: u64,
    pub err_target_not_adjacent: u64,
    pub err_target_not_detected: u64,
    pub err_insufficient_charge: u64,
    pub err_observer_not_active: u64,
    pub err_other: u64,
}

#[derive(Debug)]
pub struct ModeRunStats {
    pub mode: &'static str,
    pub total_beats: u64,
    pub final_tick: u64,
    pub outcome: MatchOutcome,

    // Cards & Architect
    pub cards_played: usize,
    pub contradiction_cards: usize,
    pub times_legal_commands_empty: u64,
    pub times_legal_empty_while_off_cooldown: u64,
    pub cooldown_waits: u64,

    // Disturbance & Waves
    pub max_disturbance: BTreeMap<u8, u32>,
    pub final_disturbance: BTreeMap<u8, u32>,
    pub waves_released: BTreeMap<u8, u32>,
    pub total_minors_allocated: u16,
    pub peak_active_minors: usize,
    pub peak_active_majors: usize,

    // Economy: Charge & Shoves
    pub initial_charge: BTreeMap<ObserverId, u32>,
    pub min_charge: BTreeMap<ObserverId, u32>,
    pub final_charge: BTreeMap<ObserverId, u32>,
    pub shove_diag: ShoveDiagnostic,
    pub recharge_events: u64,

    // Power
    pub beats_unpowered: BTreeMap<u8, u64>,
    pub power_toggles: u64,
    pub toggle_generator_intents: u64,
    pub contest_generator_intents: u64,

    // Requisition
    pub requisitions_taken: u32,

    // Jail & Prison Escape
    pub jail_events: u64,
    pub escape_steps_taken: u64,
    pub completed_escapes: u64,

    // Falls & Corruptions
    pub fall_landings: u64,
    pub corruptions: u64,

    // Observer intents summary
    pub observer_intents: BTreeMap<String, u64>,
    pub architect_intents: BTreeMap<String, u64>,

    // Observer final states
    pub observer_final_states: BTreeMap<ObserverId, ObserverState>,
    pub observer_final_positions: BTreeMap<ObserverId, HexCoord>,
}

impl Default for ModeRunStats {
    fn default() -> Self {
        Self {
            mode: "",
            total_beats: 0,
            final_tick: 0,
            outcome: MatchOutcome::Running,
            cards_played: 0,
            contradiction_cards: 0,
            times_legal_commands_empty: 0,
            times_legal_empty_while_off_cooldown: 0,
            cooldown_waits: 0,
            max_disturbance: BTreeMap::new(),
            final_disturbance: BTreeMap::new(),
            waves_released: BTreeMap::new(),
            total_minors_allocated: 0,
            peak_active_minors: 0,
            peak_active_majors: 0,
            initial_charge: BTreeMap::new(),
            min_charge: BTreeMap::new(),
            final_charge: BTreeMap::new(),
            shove_diag: ShoveDiagnostic::default(),
            recharge_events: 0,
            beats_unpowered: BTreeMap::new(),
            power_toggles: 0,
            toggle_generator_intents: 0,
            contest_generator_intents: 0,
            requisitions_taken: 0,
            jail_events: 0,
            escape_steps_taken: 0,
            completed_escapes: 0,
            fall_landings: 0,
            corruptions: 0,
            observer_intents: BTreeMap::new(),
            architect_intents: BTreeMap::new(),
            observer_final_states: BTreeMap::new(),
            observer_final_positions: BTreeMap::new(),
        }
    }
}

pub fn run_mode_playtest(mode: ArchitectMode, max_beats: u64) -> ModeRunStats {
    let mut sim = ArchitectLab::for_mode(mode).expect("scenario boots");
    sim.bot_architect = true;

    let mut stats = ModeRunStats {
        mode: mode.short_label(),
        ..Default::default()
    };

    for (id, _) in &sim.observers {
        let c = sim.economy.charge(*id);
        stats.initial_charge.insert(*id, c);
        stats.min_charge.insert(*id, c);
    }

    let mut prev_observer_states: BTreeMap<ObserverId, ObserverState> =
        sim.observers.iter().map(|(&id, o)| (id, o.state)).collect();

    let mut prev_commands_len = sim.command_log.len();
    let mut prev_requisition_count = sim.requisition.count;
    let mut prev_wave_counts = sim.economy.wave_counts.clone();
    let mut prev_power = sim.economy.power.clone();

    for beat in 0..max_beats {
        if sim.outcome != MatchOutcome::Running {
            break;
        }

        let legal = sim.legal_commands();
        if legal.is_empty() {
            stats.times_legal_commands_empty += 1;
            if sim.cooldown == 0 {
                stats.times_legal_empty_while_off_cooldown += 1;
            }
        }
        if sim.cooldown > 0 {
            stats.cooldown_waits += 1;
        }

        let charge_before: BTreeMap<ObserverId, u32> = sim
            .observers
            .keys()
            .map(|&id| (id, sim.economy.charge(id)))
            .collect();

        // Peek intents of observers to diagnose shove calls before step_beat executes them
        let mut shove_calls: Vec<(ObserverId, GuardianId)> = Vec::new();
        for (&id, observer) in &sim.observers {
            if observer.state == ObserverState::Active {
                let (intent, _) = sim.observer_intent(id);
                if let ObserverIntent::Shove(g_id) = intent {
                    shove_calls.push((id, g_id));
                }
            }
        }

        for &(obs_id, g_id) in &shove_calls {
            let mut clone = sim.clone();
            stats.shove_diag.attempts += 1;
            match clone.shove(obs_id, g_id) {
                Ok(ShoveOutcome::CommittedToVoid { .. }) => stats.shove_diag.success_void += 1,
                Ok(ShoveOutcome::CommittedToRetraction { .. }) => {
                    stats.shove_diag.success_retraction += 1
                }
                Ok(ShoveOutcome::Displaced { .. }) => stats.shove_diag.success_displaced += 1,
                Ok(ShoveOutcome::BlockedByWall { .. }) => stats.shove_diag.blocked_by_wall += 1,
                Err(ShoveError::TargetNotAdjacent) => stats.shove_diag.err_target_not_adjacent += 1,
                Err(ShoveError::TargetNotDetected) => stats.shove_diag.err_target_not_detected += 1,
                Err(ShoveError::InsufficientCharge) => {
                    stats.shove_diag.err_insufficient_charge += 1
                }
                Err(ShoveError::ObserverNotActive) => stats.shove_diag.err_observer_not_active += 1,
                Err(_) => stats.shove_diag.err_other += 1,
            }
        }

        sim.step_beat();
        stats.total_beats = beat + 1;
        stats.final_tick = sim.tick;

        let current_commands_len = sim.command_log.len();
        if current_commands_len > prev_commands_len {
            for (_, cmd) in &sim.command_log[prev_commands_len..current_commands_len] {
                match cmd {
                    ArchitectCommand::Play { target, .. } => {
                        stats.cards_played += 1;
                        if sim.contradictions.contains(target) {
                            stats.contradiction_cards += 1;
                        }
                    }
                    ArchitectCommand::Requisition => {
                        stats.requisitions_taken += 1;
                    }
                }
            }
            prev_commands_len = current_commands_len;
        }

        if sim.requisition.count > prev_requisition_count {
            stats.requisitions_taken += sim.requisition.count - prev_requisition_count;
            prev_requisition_count = sim.requisition.count;
        }

        for level in 0..sim.world.config.levels {
            let d = sim.economy.disturbance(level);
            let entry = stats.max_disturbance.entry(level).or_insert(0);
            if d > *entry {
                *entry = d;
            }
            stats.final_disturbance.insert(level, d);

            let waves = sim.economy.wave_count(level);
            let prev_w = prev_wave_counts.get(&level).copied().unwrap_or(0);
            if waves > prev_w {
                *stats.waves_released.entry(level).or_insert(0) += waves - prev_w;
                prev_wave_counts.insert(level, waves);
            }

            if !sim.economy.is_powered(level) {
                *stats.beats_unpowered.entry(level).or_insert(0) += 1;
            }
            let prev_p = prev_power.get(&level).copied().unwrap_or(true);
            let curr_p = sim.economy.is_powered(level);
            if curr_p != prev_p {
                stats.power_toggles += 1;
                prev_power.insert(level, curr_p);
            }
        }

        let minors = sim
            .guardians
            .values()
            .filter(|g| g.kind == GuardianKind::Minor)
            .count();
        let majors = sim
            .guardians
            .values()
            .filter(|g| g.kind == GuardianKind::Major)
            .count();
        stats.peak_active_minors = stats.peak_active_minors.max(minors);
        stats.peak_active_majors = stats.peak_active_majors.max(majors);

        for (id, observer) in &sim.observers {
            let c = sim.economy.charge(*id);
            let min_entry = stats.min_charge.entry(*id).or_insert(c);
            if c < *min_entry {
                *min_entry = c;
            }
            stats.final_charge.insert(*id, c);

            let before = charge_before.get(id).copied().unwrap_or(c);
            if c > before {
                stats.recharge_events += 1;
            }

            let prev_state = prev_observer_states
                .get(id)
                .copied()
                .unwrap_or(ObserverState::Active);
            if prev_state != ObserverState::Jailed && observer.state == ObserverState::Jailed {
                stats.jail_events += 1;
            } else if prev_state == ObserverState::Jailed && observer.state == ObserverState::Active
            {
                stats.completed_escapes += 1;
            } else if prev_state != ObserverState::Corrupted
                && observer.state == ObserverState::Corrupted
            {
                stats.corruptions += 1;
            }
            prev_observer_states.insert(*id, observer.state);
        }

        for (role, trace) in &sim.traces {
            if let Some(selected) = trace.selected {
                if role.starts_with("Observer") {
                    *stats
                        .observer_intents
                        .entry(selected.to_string())
                        .or_default() += 1;
                    if selected == "escape jail" {
                        stats.escape_steps_taken += 1;
                    }
                    if selected.contains("generator") {
                        stats.toggle_generator_intents += 1;
                    }
                } else if role == "Architect" {
                    *stats
                        .architect_intents
                        .entry(selected.to_string())
                        .or_default() += 1;
                    if selected.contains("generator") {
                        stats.contest_generator_intents += 1;
                    }
                }
            }
        }

        for event in &sim.events {
            if event.tick == sim.tick {
                match event.kind {
                    LabEventKind::Fell => stats.fall_landings += 1,
                    LabEventKind::Corrupted => stats.corruptions += 1,
                    _ => {}
                }
            }
        }
    }

    stats.outcome = sim.outcome;
    stats.total_minors_allocated = sim.economy.next_minor_guardian_id.saturating_sub(1000);
    for (&id, o) in &sim.observers {
        stats.observer_final_states.insert(id, o.state);
        stats.observer_final_positions.insert(id, o.cell);
    }

    stats
}

#[test]
fn playtest_instrument_runs_all_modes() {
    println!("\n=================== ARCHITECT LAB PLAYTEST ===================");

    for mode in ArchitectMode::ALL {
        let stats = run_mode_playtest(mode, 1000);
        println!("\n--------------------------------------------------------------");
        println!("MODE: {} ({:?})", stats.mode, mode);
        println!(
            "  Duration: {} beats ({} ticks)",
            stats.total_beats, stats.final_tick
        );
        println!("  Outcome: {:?}", stats.outcome);
        println!("  Observer Final States: {:?}", stats.observer_final_states);
        println!(
            "  Observer Final Positions: {:?}",
            stats.observer_final_positions
        );
        println!(
            "  Cards played: {} (contradictions: {})",
            stats.cards_played, stats.contradiction_cards
        );
        println!(
            "  Times legal commands empty: {} (empty while off cooldown: {})",
            stats.times_legal_commands_empty, stats.times_legal_empty_while_off_cooldown
        );
        println!("  Cooldown waits: {}", stats.cooldown_waits);
        println!("  Requisitions taken: {}", stats.requisitions_taken);
        println!("  Disturbance max per floor: {:?}", stats.max_disturbance);
        println!(
            "  Disturbance final per floor: {:?}",
            stats.final_disturbance
        );
        println!("  Waves released per floor: {:?}", stats.waves_released);
        println!(
            "  Minors allocated (total spawned): {}",
            stats.total_minors_allocated
        );
        println!(
            "  Peak active minors: {}, peak active majors: {}",
            stats.peak_active_minors, stats.peak_active_majors
        );
        println!(
            "  Power toggles: {}, beats unpowered: {:?}",
            stats.power_toggles, stats.beats_unpowered
        );
        println!(
            "  Toggle generator intents: {}, contest generator intents: {}",
            stats.toggle_generator_intents, stats.contest_generator_intents
        );
        println!("  Initial charge: {:?}", stats.initial_charge);
        println!("  Min charge: {:?}", stats.min_charge);
        println!("  Final charge: {:?}", stats.final_charge);
        println!("  Shove diagnostic: {:?}", stats.shove_diag);
        println!("  Recharge events: {}", stats.recharge_events);
        println!(
            "  Jail events: {}, escape steps: {}, completed escapes: {}",
            stats.jail_events, stats.escape_steps_taken, stats.completed_escapes
        );
        println!(
            "  Fall landings: {}, Corruptions: {}",
            stats.fall_landings, stats.corruptions
        );
        println!("  Observer intents: {:?}", stats.observer_intents);
        println!("  Architect intents: {:?}", stats.architect_intents);
    }
    println!("\n==============================================================\n");
}
