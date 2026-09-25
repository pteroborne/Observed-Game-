//! Playtest instrumentation: observation probe for bot matches across all ArchitectModes.
//! Measures Step B economy, emergency requisition, prison self-escape, and unsafe falls.
//!
//! Note: This module is an instrumentation harness for playtest data collection,
//! not a behavioral assertion test suite.

use std::collections::BTreeMap;

use super::*;
use crate::ascent::economy::{ShoveError, ShoveOutcome};

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

#[derive(Clone, Debug, Default)]
pub struct FallDiagnostic {
    pub total_retraction_commits: u64,
    pub retractions_on_occupied_cell: u64,
    pub retractions_on_observed_cell: u64,
    pub beats_observer_on_telegraphed: u64,
    pub beats_observer_on_raw_candidate: u64,
    pub beats_observer_on_contradiction: u64,
    pub intents_on_raw_candidate: BTreeMap<String, u64>,
    pub intents_on_contradiction: BTreeMap<String, u64>,
    pub min_dist_observer_to_retraction: BTreeMap<u32, u64>,
    pub all_dists_observer_to_retraction: BTreeMap<u32, u64>,
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
    pub policy: PowerPolicy,
    pub beats_unpowered: BTreeMap<u8, u64>,
    pub power_toggles: u64,
    pub power_cuts: u64,
    pub power_restorations: u64,
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
    pub fall_diag: FallDiagnostic,

    // Darkness: the facility going unwitnessed
    /// Beats on which at least one Observer was Active and none lit the cell they faced.
    pub dark_beats: u64,
    /// Beats on which somebody was looking outward.
    pub lit_beats: u64,
    /// The longest run of consecutive dark beats, completed or not.
    pub longest_dark_streak: u64,
    /// How often each streak length occurred, so a near miss is distinguishable from a
    /// mechanic that cannot be reached at all. Keyed by streak length in beats.
    pub dark_streak_histogram: BTreeMap<u64, u64>,
    /// The tick Darkness completed its hold, whether or not it was the selected objective.
    pub darkness_completed_at: Option<u64>,

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
            policy: PowerPolicy::Restorable,
            beats_unpowered: BTreeMap::new(),
            power_toggles: 0,
            power_cuts: 0,
            power_restorations: 0,
            toggle_generator_intents: 0,
            contest_generator_intents: 0,
            requisitions_taken: 0,
            jail_events: 0,
            escape_steps_taken: 0,
            completed_escapes: 0,
            fall_landings: 0,
            corruptions: 0,
            fall_diag: FallDiagnostic::default(),
            dark_beats: 0,
            lit_beats: 0,
            longest_dark_streak: 0,
            dark_streak_histogram: BTreeMap::new(),
            darkness_completed_at: None,
            observer_intents: BTreeMap::new(),
            architect_intents: BTreeMap::new(),
            observer_final_states: BTreeMap::new(),
            observer_final_positions: BTreeMap::new(),
        }
    }
}

pub fn raw_next_retraction(lab: &ArchitectLab) -> Option<HexCoord> {
    lab.contradictions
        .iter()
        .copied()
        .filter(|&cell| {
            !lab.prison_core.contains(&cell)
                && !lab.anchored.contains(&cell)
                && !lab.doors.iter().any(|(&key, &state)| {
                    state == DoorState::Open && threshold_touches(key, cell, &lab.world)
                })
        })
        .min_by_key(|&cell| {
            (
                lab.instability_origin
                    .map_or(0, |origin| travel_distance(origin, cell)),
                cell,
            )
        })
}

pub fn run_mode_playtest(mode: ArchitectMode, policy: PowerPolicy, max_beats: u64) -> ModeRunStats {
    let mut sim = ArchitectLab::for_mode_with_policy(mode, policy).expect("scenario boots");
    sim.bot_architect = true;

    let mut stats = ModeRunStats {
        mode: mode.short_label(),
        policy,
        ..Default::default()
    };

    for id in sim.observers.keys() {
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
    let mut prev_dark_streak = sim.darkness.streak;

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

        let telegraphed = sim.next_retraction();
        let raw_telegraphed = raw_next_retraction(&sim);
        for (&id, observer) in &sim.observers {
            if observer.state == ObserverState::Active {
                if Some(observer.cell) == telegraphed {
                    stats.fall_diag.beats_observer_on_telegraphed += 1;
                }
                if Some(observer.cell) == raw_telegraphed {
                    stats.fall_diag.beats_observer_on_raw_candidate += 1;
                    let (_, trace) = sim.observer_intent(id);
                    if let Some(selected) = trace.selected {
                        *stats
                            .fall_diag
                            .intents_on_raw_candidate
                            .entry(selected.to_string())
                            .or_default() += 1;
                    }
                }
                if sim.contradictions.contains(&observer.cell) {
                    stats.fall_diag.beats_observer_on_contradiction += 1;
                    let (_, trace) = sim.observer_intent(id);
                    if let Some(selected) = trace.selected {
                        *stats
                            .fall_diag
                            .intents_on_contradiction
                            .entry(selected.to_string())
                            .or_default() += 1;
                    }
                }
            }
        }

        let target_tick = sim.tick + u64::from(ACTOR_BEAT_TICKS);
        while sim.tick < target_tick && sim.outcome == MatchOutcome::Running {
            let pre_obs: BTreeMap<ObserverId, HexCoord> = sim
                .observers
                .iter()
                .filter(|(_, o)| o.state == ObserverState::Active)
                .map(|(&id, o)| (id, o.cell))
                .collect();
            let pre_observed = sim.observed.clone();
            let pre_tick = sim.tick;
            sim.tick();
            if sim.tick > pre_tick {
                for event in &sim.events {
                    if event.tick == sim.tick {
                        match event.kind {
                            LabEventKind::Retracted => {
                                stats.fall_diag.total_retraction_commits += 1;
                                if let Some(cell) = event.cell {
                                    if pre_obs.values().any(|&c| c == cell) {
                                        stats.fall_diag.retractions_on_occupied_cell += 1;
                                    }
                                    if pre_observed.contains(&cell) {
                                        stats.fall_diag.retractions_on_observed_cell += 1;
                                    }
                                    let mut min_d = None;
                                    for &c in pre_obs.values() {
                                        let d = travel_distance(c, cell);
                                        *stats
                                            .fall_diag
                                            .all_dists_observer_to_retraction
                                            .entry(d)
                                            .or_default() += 1;
                                        min_d = Some(min_d.map_or(d, |curr: u32| curr.min(d)));
                                    }
                                    if let Some(d) = min_d {
                                        *stats
                                            .fall_diag
                                            .min_dist_observer_to_retraction
                                            .entry(d)
                                            .or_default() += 1;
                                    }
                                }
                            }
                            LabEventKind::Fell => stats.fall_landings += 1,
                            LabEventKind::Corrupted => stats.corruptions += 1,
                            _ => {}
                        }
                    }
                }
            }
        }
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
                if curr_p {
                    stats.power_restorations += 1;
                } else {
                    stats.power_cuts += 1;
                }
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

        // A streak that dropped back to zero has ended; record how long it got. Without
        // the distribution, "Darkness never fired" and "Darkness came within one beat
        // every match" are the same line of output.
        if sim.darkness.streak == 0 && prev_dark_streak > 0 {
            *stats
                .dark_streak_histogram
                .entry(prev_dark_streak)
                .or_default() += 1;
        }
        if sim.darkness.streak > prev_dark_streak {
            stats.dark_beats += 1;
        } else {
            stats.lit_beats += 1;
        }
        prev_dark_streak = sim.darkness.streak;
    }

    // A streak still running when the match ended never returns to zero, so it would
    // otherwise be missing from the distribution entirely.
    if prev_dark_streak > 0 {
        *stats
            .dark_streak_histogram
            .entry(prev_dark_streak)
            .or_default() += 1;
    }
    stats.longest_dark_streak = sim.darkness.longest;
    stats.darkness_completed_at = sim.darkness.completed_at;

    stats.outcome = sim.outcome;
    stats.total_minors_allocated = sim.economy.next_minor_guardian_id.saturating_sub(1000);
    for (&id, o) in &sim.observers {
        stats.observer_final_states.insert(id, o.state);
        stats.observer_final_positions.insert(id, o.cell);
    }

    stats
}

#[ignore = "multi-minute instrumentation harness; prints playtest data, asserts nothing. cargo dev-test-all"]
#[test]
fn playtest_instrument_runs_all_modes() {
    println!("\n=================== ARCHITECT LAB PLAYTEST ===================");

    let mut all_stats = Vec::new();

    for mode in ArchitectMode::ALL {
        for policy in PowerPolicy::ALL {
            let stats = run_mode_playtest(mode, policy, 1000);
            println!("\n--------------------------------------------------------------");
            println!("MODE: {} ({:?}) | POLICY: {:?}", stats.mode, mode, policy);
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
                "  Power cuts: {}, restorations: {}, toggles: {}, beats unpowered: {:?}",
                stats.power_cuts,
                stats.power_restorations,
                stats.power_toggles,
                stats.beats_unpowered
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
            println!("  Fall Diagnostic: {:#?}", stats.fall_diag);
            println!(
                "  Darkness: {} dark beats / {} lit, longest streak {} of {} needed, completed at {:?}",
                stats.dark_beats,
                stats.lit_beats,
                stats.longest_dark_streak,
                DARKNESS_BEATS,
                stats.darkness_completed_at
            );
            println!(
                "  Dark streak distribution (length -> count): {:?}",
                stats.dark_streak_histogram
            );
            println!("  Observer intents: {:?}", stats.observer_intents);
            println!("  Architect intents: {:?}", stats.architect_intents);
            all_stats.push(stats);
        }
    }

    println!("\n=================== COMPARISON TABLE ===================");
    println!(
        "| Mode | Policy | Outcome | Beats | Cuts | Restorations | Unpowered Beats/Floor | Dark Beats | Lit Beats | Longest Dark Streak | Darkness Completed |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for s in &all_stats {
        let unpowered_str = format!("{:?}", s.beats_unpowered);
        let dark_comp_str = match s.darkness_completed_at {
            Some(t) => format!("Tick {t}"),
            None => "Never".to_string(),
        };
        println!(
            "| {} | {:?} | {:?} | {} | {} | {} | {} | {} | {} | {} | {} |",
            s.mode,
            s.policy,
            s.outcome,
            s.total_beats,
            s.power_cuts,
            s.power_restorations,
            unpowered_str,
            s.dark_beats,
            s.lit_beats,
            s.longest_dark_streak,
            dark_comp_str,
        );
    }
    println!("========================================================\n");
}

#[ignore = "~20 minute 40-run soak; instrumentation, not an assertion. cargo dev-test-all"]
#[test]
fn test_fall_reachability_across_many_seeds() {
    println!("\n=================== MULTI-SEED FALL REACHABILITY ===================");
    let mut total_retraction_commits = 0u64;
    let mut total_retractions_on_occupied = 0u64;
    let mut total_retractions_on_observed = 0u64;
    let mut total_beats_on_telegraphed = 0u64;
    let mut total_beats_on_raw_candidate = 0u64;
    let mut total_beats_on_contradiction = 0u64;
    let mut total_fall_landings = 0u64;
    let mut total_corruptions = 0u64;
    let mut aggregated_raw_intents: BTreeMap<String, u64> = BTreeMap::new();
    let mut aggregated_min_dists: BTreeMap<u32, u64> = BTreeMap::new();

    for mode in ArchitectMode::ALL {
        let mut mode_retractions = 0u64;
        let mut mode_falls = 0u64;
        for seed in 1..=10 {
            if let Ok(mut sim) = ArchitectLab::generate(mode, seed) {
                sim.bot_architect = true;
                for _beat in 0..500 {
                    if sim.outcome != MatchOutcome::Running {
                        break;
                    }
                    let telegraphed = sim.next_retraction();
                    let raw_telegraphed = raw_next_retraction(&sim);
                    for (&id, observer) in &sim.observers {
                        if observer.state == ObserverState::Active {
                            if Some(observer.cell) == telegraphed {
                                total_beats_on_telegraphed += 1;
                            }
                            if Some(observer.cell) == raw_telegraphed {
                                total_beats_on_raw_candidate += 1;
                                let (_, trace) = sim.observer_intent(id);
                                if let Some(selected) = trace.selected {
                                    *aggregated_raw_intents
                                        .entry(selected.to_string())
                                        .or_default() += 1;
                                }
                            }
                            if sim.contradictions.contains(&observer.cell) {
                                total_beats_on_contradiction += 1;
                            }
                        }
                    }

                    let target_tick = sim.tick + u64::from(ACTOR_BEAT_TICKS);
                    while sim.tick < target_tick && sim.outcome == MatchOutcome::Running {
                        let pre_obs: Vec<HexCoord> = sim
                            .observers
                            .values()
                            .filter(|o| o.state == ObserverState::Active)
                            .map(|o| o.cell)
                            .collect();
                        let pre_observed = sim.observed.clone();
                        let pre_tick = sim.tick;
                        sim.tick();
                        if sim.tick > pre_tick {
                            for event in &sim.events {
                                if event.tick == sim.tick {
                                    match event.kind {
                                        LabEventKind::Retracted => {
                                            total_retraction_commits += 1;
                                            mode_retractions += 1;
                                            if let Some(cell) = event.cell {
                                                if pre_obs.contains(&cell) {
                                                    total_retractions_on_occupied += 1;
                                                }
                                                if pre_observed.contains(&cell) {
                                                    total_retractions_on_observed += 1;
                                                }
                                                let min_d = pre_obs
                                                    .iter()
                                                    .map(|&c| travel_distance(c, cell))
                                                    .min();
                                                if let Some(d) = min_d {
                                                    *aggregated_min_dists.entry(d).or_default() +=
                                                        1;
                                                }
                                            }
                                        }
                                        LabEventKind::Fell => {
                                            total_fall_landings += 1;
                                            mode_falls += 1;
                                        }
                                        LabEventKind::Corrupted => {
                                            total_corruptions += 1;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!(
            "Mode {:?}: retractions={}, falls={}",
            mode, mode_retractions, mode_falls
        );
    }

    println!("\nAGGREGATED OVER 40 RUNS (10 SEEDS x 4 MODES):");
    println!("  Total Retraction Commits: {}", total_retraction_commits);
    println!(
        "  Retractions on Occupied Cell: {}",
        total_retractions_on_occupied
    );
    println!(
        "  Retractions on Observed Cell: {}",
        total_retractions_on_observed
    );
    println!(
        "  Beats Observer on Telegraphed Cell: {}",
        total_beats_on_telegraphed
    );
    println!(
        "  Beats Observer on Raw Candidate Cell: {}",
        total_beats_on_raw_candidate
    );
    println!(
        "  Beats Observer on Any Contradiction Cell: {}",
        total_beats_on_contradiction
    );
    println!(
        "  Intents Chosen when on Raw Candidate: {:?}",
        aggregated_raw_intents
    );
    println!(
        "  Min Distance from Observer to Retracting Cell at Commit: {:?}",
        aggregated_min_dists
    );
    println!("  Total Fall Landings: {}", total_fall_landings);
    println!("  Total Corruptions: {}", total_corruptions);
    println!("====================================================================\n");
}

/// Why the facility is dark: is nobody looking because the lights are off, or because
/// every Observer is facing a wall?
///
/// The Darkness measurement came back at 82-92% of beats dark in every mode, which is a
/// claim about Observers, not about the objective. This separates the two causes so the
/// finding can be stated as a fact rather than a guess.
#[ignore = "~140s diagnostic probe for backlog #44; prints, asserts nothing. cargo dev-test-all"]
#[test]
fn why_the_facility_is_dark() {
    println!("\n=================== DARKNESS CAUSE PROBE ===================");
    for mode in ArchitectMode::ALL {
        let mut sim = ArchitectLab::for_mode(mode).expect("scenario boots");
        sim.bot_architect = true;

        let mut beats = 0u64;
        let mut active_samples = 0u64;
        let mut unpowered = 0u64;
        let mut facing_wall = 0u64;
        let mut lit = 0u64;

        for _ in 0..1000 {
            if sim.outcome != MatchOutcome::Running {
                break;
            }
            sim.step_beat();
            beats += 1;
            let snapshot: Vec<(HexCoord, HexFace)> = sim
                .observers
                .values()
                .filter(|o| o.state == ObserverState::Active)
                .map(|o| (o.cell, o.facing))
                .collect();
            for (cell, facing) in snapshot {
                active_samples += 1;
                if !sim.economy.is_powered(cell.level) {
                    unpowered += 1;
                } else if sim.step_through(cell, facing).is_none() {
                    facing_wall += 1;
                } else {
                    lit += 1;
                }
            }
        }

        let pct = |n: u64| {
            if active_samples == 0 {
                0.0
            } else {
                100.0 * n as f64 / active_samples as f64
            }
        };
        println!(
            "{:>12}: {beats} beats, {active_samples} active-Observer samples -> \
             unpowered {unpowered} ({:.1}%), facing a wall {facing_wall} ({:.1}%), \
             lit {lit} ({:.1}%)",
            mode.short_label(),
            pct(unpowered),
            pct(facing_wall),
            pct(lit)
        );
    }
    println!("============================================================\n");
}

/// Does the generator orphan cells, or does the lab do it to itself?
///
/// `docs/facility_orphans.md` blamed the WFC. An assertion added to
/// `observed_facility` -- every passable cell in one component, four scenario shapes,
/// twelve seeds each -- passes, which says the solver ships a connected facility. The
/// lab then punches one to five route cells to `Void` as scenario damage. This measures
/// connectivity on both sides of that step.
#[test]
fn who_actually_orphans_the_cells() {
    use observed_facility::hex_wfc::disconnected_cells;
    println!("\n============== ORPHANS: BEFORE AND AFTER ==============");
    for mode in ArchitectMode::ALL {
        let lab = ArchitectLab::for_mode(mode).expect("scenario boots");
        let config = lab.world.config;

        // The same seed and config the lab used, without the lab's damage step.
        let pristine = observed_facility::hex_wfc::HexWfcWorld::generate(mode.seed(), config)
            .expect("the pinned mode solves");
        let before = disconnected_cells(config, &pristine.placements);
        let after = disconnected_cells(config, &lab.world.placements);

        let voids_added = lab
            .world
            .placements
            .iter()
            .filter(|(coord, placement)| {
                placement.space == observed_facility::hex_wfc::HexSpace::Void
                    && pristine
                        .placements
                        .get(coord)
                        .is_some_and(|p| p.space != observed_facility::hex_wfc::HexSpace::Void)
            })
            .count();

        println!(
            "{:>12}: as solved {} orphans; after {voids_added} scenario gaps, {} orphans",
            mode.short_label(),
            before.len(),
            after.len()
        );
        if !after.is_empty() {
            println!("              orphaned: {after:?}");
        }
    }
    println!("=======================================================\n");
}

/// How much of the facility has an alternative route around it?
///
/// Relaxing the gap spacing from 3 to 2 to 1 found no extra non-stranding candidates,
/// which says spacing was never the constraint. This counts, over every passable cell,
/// how many can be removed without cutting anything off — the facility's redundancy.
#[test]
fn how_many_cells_have_a_way_around_them() {
    use observed_facility::hex_wfc::{HexSpace, disconnected_cells};
    println!("\n============== FACILITY REDUNDANCY ==============");
    for mode in ArchitectMode::ALL {
        let lab = ArchitectLab::for_mode(mode).expect("scenario boots");
        let config = lab.world.config;
        let baseline = disconnected_cells(config, &lab.world.placements);
        let passable: Vec<HexCoord> = lab
            .world
            .placements
            .iter()
            .filter(|(_, p)| p.space.built())
            .map(|(&c, _)| c)
            .collect();

        let mut removable = 0usize;
        let mut cut_vertices = 0usize;
        for &cell in &passable {
            let mut probe = lab.world.placements.clone();
            let tile = probe.get_mut(&cell).expect("passable");
            tile.space = HexSpace::Void;
            tile.doors = 0;
            tile.up = observed_hex::PortClass::Sealed;
            tile.down = observed_hex::PortClass::Sealed;
            if disconnected_cells(config, &probe)
                .difference(&baseline)
                .next()
                .is_some()
            {
                cut_vertices += 1;
            } else {
                removable += 1;
            }
        }
        let total = passable.len();
        println!(
            "{:>12}: {total} passable cells, {cut_vertices} are cut vertices ({:.0}%), \
             {removable} have a way around",
            mode.short_label(),
            100.0 * cut_vertices as f64 / total as f64
        );
    }
    println!("================================================\n");
}

/// Why the reverse gear never engages.
///
/// The Observer bot already has both halves of power restoration -- "restore floor power
/// at generator" and "seek generator to restore power" in `behavior.rs` -- and the
/// instrument has recorded zero of either. This finds out which precondition fails, by
/// sampling every Active Observer on an unpowered floor every beat.
#[test]
fn why_nobody_restores_the_power() {
    println!("\n============== POWER RESTORATION PROBE ==============");
    for mode in ArchitectMode::ALL {
        let mut sim = ArchitectLab::for_mode(mode).expect("scenario boots");
        sim.bot_architect = true;

        let mut on_unpowered = 0u64;
        let mut no_generator_on_level = 0u64;
        let mut standing_on_generator = 0u64;
        let mut no_route_to_generator = 0u64;
        let mut route_exists = 0u64;
        let mut selected_a_generator_branch = 0u64;
        let mut preempted_by: BTreeMap<String, u64> = BTreeMap::new();
        let mut levels_seen: BTreeSet<u8> = BTreeSet::new();
        let mut levels_with_generator: BTreeSet<u8> = BTreeSet::new();

        for _ in 0..1000 {
            if sim.outcome != MatchOutcome::Running {
                break;
            }
            sim.step_beat();

            let snapshot: Vec<(ObserverId, HexCoord)> = sim
                .observers
                .values()
                .filter(|o| o.state == ObserverState::Active)
                .map(|o| (o.id, o.cell))
                .collect();

            for (id, cell) in snapshot {
                levels_seen.insert(cell.level);
                if sim.economy.generators.contains_key(&cell.level) {
                    levels_with_generator.insert(cell.level);
                }
                if sim.economy.is_powered(cell.level) {
                    continue;
                }
                on_unpowered += 1;

                let Some(generator) = sim.economy.generators.get(&cell.level).copied() else {
                    no_generator_on_level += 1;
                    continue;
                };
                if generator == cell {
                    standing_on_generator += 1;
                } else if sim.route(cell, generator).is_none() {
                    no_route_to_generator += 1;
                } else {
                    route_exists += 1;
                }

                // What the bot actually chose this beat, when it had a reason to go.
                if let Some(trace) = sim.traces.get(&format!("Observer {}", id.0))
                    && let Some(selected) = trace.selected
                {
                    if selected.contains("generator") {
                        selected_a_generator_branch += 1;
                    } else {
                        *preempted_by.entry(selected.to_string()).or_default() += 1;
                    }
                }
            }
        }

        println!("\n{}:", mode.short_label());
        println!(
            "  levels occupied {levels_seen:?}, of which have a generator {levels_with_generator:?}"
        );
        println!("  Active-Observer samples on an unpowered floor: {on_unpowered}");
        println!("    no generator exists on that level: {no_generator_on_level}");
        println!("    standing on the generator already: {standing_on_generator}");
        println!("    generator exists but no route to it: {no_route_to_generator}");
        println!("    generator exists and is routable:   {route_exists}");
        println!("  chose a generator branch: {selected_a_generator_branch}");
        println!("  otherwise chose: {preempted_by:?}");
    }
    println!("=====================================================\n");
}

/// Is the generator even connected to the facility?
///
/// `EconomyState::new` picks `candidates[0]` -- the lowest-sorted non-Void coordinate on
/// each level -- as that floor's generator, which is a choice made by coordinate order and
/// not by reachability. This checks, at match start, whether anyone could ever walk there.
#[test]
fn is_the_generator_reachable_at_all() {
    println!("\n============== GENERATOR REACHABILITY ==============");
    for mode in ArchitectMode::ALL {
        let sim = ArchitectLab::for_mode(mode).expect("scenario boots");
        println!("\n{}:", mode.short_label());
        let starts: Vec<HexCoord> = sim.observers.values().map(|o| o.cell).collect();
        for (&level, &generator) in &sim.economy.generators {
            let exits = sim.exits(generator).len();
            let reachable = starts
                .iter()
                .filter(|&&start| sim.route(start, generator).is_some())
                .count();
            println!(
                "  level {level}: generator {generator:?}, {exits} exits, \
                 reachable from {reachable}/{} Observer starts",
                starts.len()
            );
            assert!(
                exits > 0,
                "mode {:?} level {level} generator has 0 exits",
                mode
            );
            assert!(
                reachable >= 1,
                "mode {:?} level {level} generator {generator:?} unreachable from any Observer start",
                mode
            );
        }
        // And the same question for the other two economy fixtures, which are chosen the
        // same way and would fail the same way.
        let unreachable_stations = sim
            .economy
            .stations
            .iter()
            .filter(|&&cell| !starts.iter().any(|&s| sim.route(s, cell).is_some()))
            .count();
        let unreachable_pads = sim
            .economy
            .pads
            .iter()
            .filter(|&&cell| !starts.iter().any(|&s| sim.route(s, cell).is_some()))
            .count();
        println!(
            "  stations unreachable from every start: {unreachable_stations}/{}, \
             pads: {unreachable_pads}/{}",
            sim.economy.stations.len(),
            sim.economy.pads.len()
        );
        assert_eq!(
            unreachable_stations,
            0,
            "mode {:?} has unreachable stations: {unreachable_stations}/{}",
            mode,
            sim.economy.stations.len()
        );
        assert_eq!(
            unreachable_pads,
            0,
            "mode {:?} has unreachable pads: {unreachable_pads}/{}",
            mode,
            sim.economy.pads.len()
        );
    }
    println!("====================================================\n");
}

/// How much of the hand is playable, as the interface computes it?
///
/// Reported from play: "the card in hand often has no legal target". `legal_commands` is
/// simulation-level, so whatever this finds applies to the desktop cutaway and the browser
/// build alike -- they are two presentations of one rule set.
///
/// Sampled the way the interface does. `RogueGame::preview` zeroes the cooldown before
/// listing targets, deliberately: the glowing dots show what a card could reach, not what
/// the clock currently permits. So this ignores the cooldown too, and asks per beat how
/// many of the five cards have a legal target anywhere, and how many have one on the floor
/// the player is most likely watching -- the floor an Observer is standing on.
#[ignore = "~22s hand-availability sweep over four live matches. cargo dev-test-all"]
#[test]
fn how_much_of_the_hand_is_playable() {
    println!("\n============== PLAYABLE HAND OVER A LIVE HUNT ==============");
    for mode in ArchitectMode::ALL {
        let mut sim = ArchitectLab::for_mode(mode).expect("scenario boots");
        sim.bot_architect = true;

        let mut beats = 0u64;
        let mut cards_playable_anywhere: BTreeMap<usize, u64> = BTreeMap::new();
        let mut cards_playable_on_an_observer_floor: BTreeMap<usize, u64> = BTreeMap::new();

        while beats < 400 && sim.outcome == MatchOutcome::Running {
            sim.step_beat();
            beats += 1;

            let mut probe = sim.clone();
            probe.cooldown = 0;
            let targets = probe.mutable_targets();
            let observer_floors: BTreeSet<u8> = probe
                .observers
                .values()
                .filter(|o| o.state == ObserverState::Active)
                .map(|o| o.cell.level)
                .collect();

            // `selected_command` addresses the hand by position, which is also how the
            // interface numbers the cards 1-5.
            let hand_len = probe.deck.hand.len();
            let mut anywhere = 0usize;
            let mut on_floor = 0usize;
            for card in 0..hand_len {
                let mut reachable = false;
                let mut near = false;
                for &cell in &targets {
                    if (0..6).any(|rot| {
                        probe
                            .selected_command(card, cell, rot)
                            .is_some_and(|command| probe.refusal(command).is_none())
                    }) {
                        reachable = true;
                        if observer_floors.contains(&cell.level) {
                            near = true;
                            break;
                        }
                    }
                }
                if reachable {
                    anywhere += 1;
                }
                if near {
                    on_floor += 1;
                }
            }
            *cards_playable_anywhere.entry(anywhere).or_default() += 1;
            *cards_playable_on_an_observer_floor
                .entry(on_floor)
                .or_default() += 1;
        }

        println!("\n{:>12}: {beats} beats sampled", mode.short_label());
        println!(
            "              cards playable anywhere (count -> beats):        {cards_playable_anywhere:?}"
        );
        println!(
            "              cards playable on an Observer floor (count -> beats): {cards_playable_on_an_observer_floor:?}"
        );
    }
    println!("===========================================================\n");
}

/// Does an Observer now see further than it wards, in an actual match?
///
/// Seeing and warding were separated deliberately: sight crosses open air up to
/// `OBSERVER_SIGHT_RANGE`, protection stays the cell underfoot and one step onward. The
/// point of the split is that protection should not triple, so this measures both and
/// says whether the ward held still.
#[ignore = "~1 minute sight-versus-ward sweep over four matches. cargo dev-test-all"]
#[test]
fn does_sight_reach_further_than_the_ward() {
    println!("\n============== SEEN vs WARDED OVER A MATCH ==============");
    for mode in ArchitectMode::ALL {
        let mut sim = ArchitectLab::for_mode(mode).expect("scenario boots");
        sim.bot_architect = true;

        let mut beats = 0u64;
        let mut warded_total = 0usize;
        let mut seen_total = 0usize;
        let mut beats_sight_exceeded_ward = 0u64;
        let mut air_cells = 0usize;
        for placement in sim.world.placements.values() {
            if placement.space == observed_facility::hex_wfc::HexSpace::Air {
                air_cells += 1;
            }
        }

        while beats < 400 && sim.outcome == MatchOutcome::Running {
            sim.step_beat();
            beats += 1;
            warded_total += sim.observed.len();
            seen_total += sim.seen.len();
            if sim.seen.len() > sim.observed.len() {
                beats_sight_exceeded_ward += 1;
            }
            assert!(
                sim.observed.is_subset(&sim.seen),
                "an Observer must see everything it wards"
            );
        }

        let ratio = if warded_total == 0 {
            0.0
        } else {
            seen_total as f64 / warded_total as f64
        };
        println!(
            "{:>12}: {beats} beats, {air_cells} air cells, warded {warded_total}, \
             seen {seen_total} ({ratio:.2}x), sight beat the ward on {beats_sight_exceeded_ward} beats",
            mode.short_label()
        );
    }
    println!("========================================================\n");
}
