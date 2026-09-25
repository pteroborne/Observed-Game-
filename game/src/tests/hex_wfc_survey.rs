//! Production-corpus traversal gates for the autonomous spectator.
//!
//! The broad survey is intentionally ignored for duration. The small regression
//! matrix runs normally so a catalog change cannot quietly reintroduce the two
//! traversal failures that motivated the survey.

use std::collections::{BTreeSet, VecDeque};
use std::fmt::Write as _;

use bevy::prelude::{Vec2, Vec3};
use observed_hex::HexCoord;
use observed_match::hex_wfc::{
    HexBotDriver, HexInputFrame, HexMatchConfig, HexMatchEventKind, HexMatchStatus, HexPlayerState,
    HexWfcMatch,
};
use observed_traversal::FpsConfig;

const TICK_BUDGET: u64 = 36_000;
const RECENT_SAMPLE_INTERVAL: u64 = 300;
const RECENT_SAMPLE_CAPACITY: usize = 12;
const FORMERLY_FAILING_SEED: u64 = 10_000_031;

#[derive(Clone, Debug)]
struct ProgressSample {
    tick: u64,
    cell: HexCoord,
    position: Vec3,
}

#[derive(Clone, Debug)]
struct SurveyReport {
    requested_seed: u64,
    selected_seed: u64,
    completion_tick: u64,
}

#[derive(Clone, Debug)]
struct TraversalStall {
    requested_seed: u64,
    selected_seed: u64,
    tick: u64,
    reason: &'static str,
    details: Box<TraversalStallDetails>,
}

#[derive(Clone, Debug)]
struct TraversalStallDetails {
    cell: HexCoord,
    placement: String,
    resolved_module_family: Vec<String>,
    guide_edge: String,
    position: Vec3,
    last_progress_tick: u64,
    recent_progress: Vec<ProgressSample>,
}

impl std::fmt::Display for TraversalStall {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            formatter,
            "requested seed {} selected stable seed {} stalled at tick {}: {}",
            self.requested_seed, self.selected_seed, self.tick, self.reason
        )?;
        writeln!(
            formatter,
            "  logical cell: {:?}; placement: {}",
            self.details.cell, self.details.placement
        )?;
        writeln!(
            formatter,
            "  resolved module instance/family (compat tile identity): {:?}",
            self.details.resolved_module_family
        )?;
        writeln!(formatter, "  guide edge: {}", self.details.guide_edge)?;
        writeln!(formatter, "  position: {:?}", self.details.position)?;
        writeln!(
            formatter,
            "  last progress tick: {}; recent progress:",
            self.details.last_progress_tick
        )?;
        for sample in &self.details.recent_progress {
            writeln!(
                formatter,
                "    tick {} cell {:?} position {:?}",
                sample.tick, sample.cell, sample.position
            )?;
        }
        Ok(())
    }
}

fn run_spectator_seed(seed: u64) -> Result<SurveyReport, TraversalStall> {
    run_spectator_seed_with_budget(seed, TICK_BUDGET)
}

fn run_spectator_seed_with_budget(
    requested_seed: u64,
    tick_budget: u64,
) -> Result<SurveyReport, TraversalStall> {
    let prototypes = crate::hex_wfc::sim::load_prototypes();
    let (mut game, selected_seed) = (0..64u64)
        .find_map(|offset| {
            let selected_seed = requested_seed.wrapping_add(offset);
            let config = HexMatchConfig {
                teams: 1,
                members_per_team: 1,
                ..Default::default()
            };
            HexWfcMatch::new(selected_seed, config, &prototypes)
                .ok()
                .map(|game| (game, selected_seed))
        })
        .expect("the production corpus contains a solvable nearby showcase seed");
    let player = game.players.values().next().expect("solo runner");
    let mut recent_progress = VecDeque::from([sample(&game, player)]);
    let mut progress_anchor = player.position;
    let mut last_cell = player.cell;
    let mut last_progress_tick = 0;
    let mut driver = HexBotDriver::new();

    for _ in 0..tick_budget {
        step_all_bots(&mut game, &mut driver);
        let player = game.players.values().next().expect("solo runner");
        if player.cell != last_cell || player.position.distance_squared(progress_anchor) >= 0.25 {
            last_progress_tick = game.tick;
            last_cell = player.cell;
            progress_anchor = player.position;
        }
        if game.tick % RECENT_SAMPLE_INTERVAL == 0 {
            push_sample(&mut recent_progress, sample(&game, player));
        }
        if game
            .recent_events
            .iter()
            .any(|event| event.kind == HexMatchEventKind::PlayerRecovered)
        {
            return Err(stall(
                requested_seed,
                selected_seed,
                &game,
                "PlayerRecovered fired during ordinary traversal",
                last_progress_tick,
                recent_progress,
            ));
        }
        if game.status == HexMatchStatus::Finished {
            return Ok(SurveyReport {
                requested_seed,
                selected_seed,
                completion_tick: game.tick,
            });
        }
    }

    Err(stall(
        requested_seed,
        selected_seed,
        &game,
        "tick budget exhausted before escape",
        last_progress_tick,
        recent_progress,
    ))
}

fn step_all_bots(game: &mut HexWfcMatch, driver: &mut HexBotDriver) {
    let mut frame = HexInputFrame {
        tick: game.tick + 1,
        ..Default::default()
    };
    for id in game.players.keys().copied().collect::<Vec<_>>() {
        frame.commands.insert(id, driver.command(game, id));
    }
    game.step(&frame);
}

fn sample(game: &HexWfcMatch, player: &HexPlayerState) -> ProgressSample {
    ProgressSample {
        tick: game.tick,
        cell: player.cell,
        position: player.position,
    }
}

fn push_sample(samples: &mut VecDeque<ProgressSample>, sample: ProgressSample) {
    if samples.len() == RECENT_SAMPLE_CAPACITY {
        samples.pop_front();
    }
    samples.push_back(sample);
}

fn stall(
    requested_seed: u64,
    selected_seed: u64,
    game: &HexWfcMatch,
    reason: &'static str,
    last_progress_tick: u64,
    mut recent_progress: VecDeque<ProgressSample>,
) -> TraversalStall {
    let player = game.players.values().next().expect("solo runner");
    if recent_progress
        .back()
        .is_none_or(|sample| sample.tick != game.tick)
    {
        push_sample(&mut recent_progress, sample(game, player));
    }
    let mut resolved_module_family = game
        .geometry
        .pieces
        .iter()
        .filter(|piece| piece.source_cell == player.cell)
        .filter_map(|piece| {
            piece
                .tile
                .as_ref()
                .map(|tile| format!("anchor={:?} tile={tile:?}", piece.anchor))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if resolved_module_family.is_empty() {
        resolved_module_family
            .push("<whole-room blueprint; prototype identity is not projected yet>".to_string());
    }
    TraversalStall {
        requested_seed,
        selected_seed,
        tick: game.tick,
        reason,
        details: Box::new(TraversalStallDetails {
            cell: player.cell,
            placement: format!("{:?}", game.facility.placements.get(&player.cell)),
            resolved_module_family,
            guide_edge: guide_edge(game, player),
            position: player.position,
            last_progress_tick,
            recent_progress: recent_progress.into_iter().collect(),
        }),
    }
}

fn guide_edge(game: &HexWfcMatch, player: &HexPlayerState) -> String {
    let next = game
        .facility
        .route_between_cells(player.cell, game.facility.config.exit())
        .and_then(|route| route.cells.get(1).copied());
    let half_height = FpsConfig::deliberate_rapier().half_height;
    let feet = Vec3::new(
        player.position.x,
        player.position.y - half_height,
        player.position.z,
    );
    let vertical_candidates = [
        Some(player.cell),
        (player.cell.level > 0).then_some(HexCoord {
            level: player.cell.level.saturating_sub(1),
            ..player.cell
        }),
        player.cell.level.checked_add(1).map(|level| HexCoord {
            level,
            ..player.cell
        }),
    ];
    for cell in vertical_candidates.into_iter().flatten() {
        if let Some((segment, progress)) = game
            .geometry
            .climbs
            .get(&cell)
            .and_then(|spine| spine.locate(feet))
        {
            return format!(
                "facility {:?}->{next:?}; climb cell {cell:?} segment {segment} progress {progress:.3}",
                player.cell
            );
        }
    }
    if let Some(deck) = game.geometry.decks.get(&player.cell)
        && let Some((segment, progress)) = nearest_plan_segment(&deck.nodes, feet)
    {
        return format!(
            "facility {:?}->{next:?}; deck cell {:?} segment {segment} progress {progress:.3}",
            player.cell, player.cell
        );
    }
    format!(
        "facility {:?}->{next:?}; direct cell-center handoff (no local compatibility guide)",
        player.cell
    )
}

fn nearest_plan_segment(nodes: &[Vec3], point: Vec3) -> Option<(usize, f32)> {
    let point = Vec2::new(point.x, point.z);
    nodes
        .windows(2)
        .enumerate()
        .map(|(index, edge)| {
            let from = Vec2::new(edge[0].x, edge[0].z);
            let to = Vec2::new(edge[1].x, edge[1].z);
            let delta = to - from;
            let progress = if delta.length_squared() <= f32::EPSILON {
                0.0
            } else {
                ((point - from).dot(delta) / delta.length_squared()).clamp(0.0, 1.0)
            };
            let distance = point.distance_squared(from + delta * progress);
            (index, progress, distance)
        })
        .min_by(|left, right| left.2.total_cmp(&right.2))
        .map(|(index, progress, _)| (index, progress))
}

fn format_stalls(stalls: &[TraversalStall]) -> String {
    let mut output = String::new();
    for stall in stalls {
        writeln!(output, "{stall}").expect("writing into a String cannot fail");
    }
    output
}

#[test]
fn production_corpus_spectator_regression_seeds_complete_without_recovery() {
    let reports = [crate::flow::MATCH_SEED, FORMERLY_FAILING_SEED]
        .into_iter()
        .map(run_spectator_seed)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|stall| panic!("spectator traversal regression:\n{stall}"));

    assert_eq!(reports.len(), 2);
    for report in reports {
        assert!(report.completion_tick > 0);
        assert!(report.completion_tick <= TICK_BUDGET);
        assert_eq!(
            report.selected_seed, report.requested_seed,
            "the pinned regression seeds must remain exact, not nearby fallbacks"
        );
    }
}

/// Sweep the gate across 24 deterministic production-corpus facilities. Kept
/// ignored for duration, but unlike the old diagnostic it fails on any stall.
#[test]
#[ignore = "24-seed, 36k-tick traversal survey; run explicitly"]
fn survey_spectator_routes_across_seeds() {
    let stalls = (0..24u64)
        .filter_map(|index| {
            let seed = crate::flow::MATCH_SEED.wrapping_add(index.wrapping_mul(1_000_003));
            run_spectator_seed(seed).err()
        })
        .collect::<Vec<_>>();
    assert!(
        stalls.is_empty(),
        "{} of 24 spectator seeds stalled:\n{}",
        stalls.len(),
        format_stalls(&stalls)
    );
}

#[test]
fn spectator_stall_diagnostic_names_the_traversal_state() {
    let stall = run_spectator_seed_with_budget(crate::flow::MATCH_SEED, 0)
        .expect_err("a zero-tick budget must produce a diagnostic stall");
    let diagnostic = stall.to_string();
    for label in [
        "stable seed",
        "logical cell",
        "resolved module instance/family",
        "guide edge",
        "position",
        "recent progress",
    ] {
        assert!(
            diagnostic.contains(label),
            "stall diagnostic omitted {label:?}:\n{diagnostic}"
        );
    }
}

/// What more void does to a match, not just to a view: a solo spectator bot on the
/// production lattice under the committed catalog, with only the void share changed.
///
/// Printed rather than asserted, because it is the measurement a composition change
/// is decided on: per share, how many runs finish inside the budget, their median
/// completion tick, and how many recoveries (falls out of the world or off a roof
/// onto one the runner cannot leave) the bot needed on the way.
#[test]
#[ignore = "composition playability survey: void shares x production matches, ~50 s each; prints"]
fn survey_void_share_playability() {
    use observed_facility::hex_wfc::HexWfcConfig;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/tiles");
    let slugs = observed_content::ArchitectureRegister::ALL
        .map(observed_content::ArchitectureRegister::slug);
    let catalog =
        observed_authoring::RuntimeHexCatalog::load(&root, &slugs).expect("committed catalog");
    // `OBSERVED2_VOID_SURVEY=300,2000` and `OBSERVED2_VOID_SURVEY_SEEDS=10` widen it.
    let voids: Vec<f64> = std::env::var("OBSERVED2_VOID_SURVEY")
        .ok()
        .map(|list| {
            list.split(',')
                .filter_map(|v| v.trim().parse().ok())
                .collect()
        })
        .unwrap_or_else(|| vec![300.0, 1_000.0, 2_000.0]);
    let seeds: u64 = std::env::var("OBSERVED2_VOID_SURVEY_SEEDS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(4);
    eprintln!("void  finished  median tick  recoveries  (per seed: tick/recoveries)");
    for void in voids {
        let mut catalog = catalog.clone();
        catalog.composition.space_mix.void = void;
        let content = std::sync::Arc::new(
            observed_match::hex_wfc::HexMatchContent::from_runtime_catalog(catalog),
        );
        let (mut ticks, mut recoveries, mut rows) = (Vec::new(), 0, Vec::new());
        for index in 0..seeds {
            let seed = crate::flow::MATCH_SEED.wrapping_add(index.wrapping_mul(1_000_003));
            let config = HexMatchConfig {
                teams: 1,
                members_per_team: 1,
                wfc: HexWfcConfig::arc_default(),
                ..Default::default()
            };
            let Ok(mut game) = HexWfcMatch::new_with_content(seed, config, content.clone()) else {
                rows.push("unsolved".to_string());
                continue;
            };
            let mut driver = HexBotDriver::new();
            let mut recovered = 0;
            for _ in 0..TICK_BUDGET * 2 {
                step_all_bots(&mut game, &mut driver);
                recovered += game
                    .recent_events
                    .iter()
                    .filter(|event| event.kind == HexMatchEventKind::PlayerRecovered)
                    .count();
                if game.status == HexMatchStatus::Finished {
                    break;
                }
            }
            recoveries += recovered;
            if game.status == HexMatchStatus::Finished {
                ticks.push(game.tick);
                rows.push(format!("{}/{recovered}", game.tick));
            } else {
                let runner = game.players.values().next().expect("runner");
                rows.push(format!(
                    "stalled/{recovered}[{:?} {:?}]",
                    runner.cell,
                    game.bot_behaviour(runner.id)
                ));
            }
        }
        ticks.sort_unstable();
        let median = ticks.get(ticks.len() / 2).copied().unwrap_or(0);
        eprintln!(
            "{void:5}  {}/{seeds}      {median:11}  {recoveries:10}  {}",
            ticks.len(),
            rows.join("  ")
        );
    }
}

/// Open edges at production scale: the first production run with them walked the
/// spectator off an unrailed level-7 walkway, onto the roof below, and back again for
/// the rest of the match. It was following the full-width hall's authored deck path
/// across a span that keeps none of that hall's floor. On the arc lattice, where the
/// top storeys are bare, the runner must get where it is going without a recovery.
#[test]
fn production_runner_crosses_unrailed_open_edges_without_falling() {
    use observed_facility::hex_wfc::HexWfcConfig;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/tiles");
    let slugs = observed_content::ArchitectureRegister::ALL
        .map(observed_content::ArchitectureRegister::slug);
    let catalog =
        observed_authoring::RuntimeHexCatalog::load(&root, &slugs).expect("committed catalog");
    let content = std::sync::Arc::new(
        observed_match::hex_wfc::HexMatchContent::from_runtime_catalog(catalog),
    );
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: 1,
        wfc: HexWfcConfig::arc_default(),
        ..Default::default()
    };
    let mut game = HexWfcMatch::new_with_content(crate::flow::MATCH_SEED, config, content)
        .expect("the production match solves");
    let mut driver = HexBotDriver::new();
    let mut spans_crossed = 0;
    let mut last = game.players.values().next().expect("runner").cell;
    for _ in 0..20_000 {
        step_all_bots(&mut game, &mut driver);
        assert!(
            !game
                .recent_events
                .iter()
                .any(|event| event.kind == HexMatchEventKind::PlayerRecovered),
            "the runner needed a recovery at tick {}",
            game.tick
        );
        let cell = game.players.values().next().expect("runner").cell;
        if cell != last
            && observed_match::hex_wfc::open_edges(&game.facility, cell)
                .is_some_and(|open| open.span.is_some() && !open.railed)
        {
            spans_crossed += 1;
        }
        last = cell;
    }
    assert!(spans_crossed > 0, "the route never met an unrailed span");
}
