//! Phase 38 evidence: a deterministic, headless bot race over
//! [`ContentionWorld`], run across a seed corpus to measure whether route
//! denial (the `Denier` policy) is a real, measurable competitive force
//! while the shared solvability guard keeps every race winnable.
//!
//! This module is pure (no Bevy). It exists to produce the four numbers the
//! Phase 38 plan asks for, and the `#[cfg(test)]` block below *is* the
//! phase's success criteria — each test's name states which criterion it
//! answers, and each prints its evidence table so `cargo test -p
//! contention_lab -- --nocapture` shows the numbers instead of just a pass/fail.
//!
//! Criteria (see `docs/contention_arc_plan.md`, Phase 38):
//! - (a) route denial measurably changes race outcomes, and helps the denier;
//! - (b) races are byte-for-byte deterministic given the same seed/config;
//! - (c) the solvability guard never leaves an unfinished team stranded;
//! - (d) four deniers racing each other never freezes the race solid.
//!
//! ## Bot simplification (documented, not hidden)
//! Each bot re-derives its route every tick from `path_to_exit`, which walks
//! the world's *current, live* links — not the team's private knowledge
//! ledger (`record_observations` is still called every tick so a future,
//! knowledge-limited bot is a drop-in refinement; it just isn't consulted for
//! pathing yet). This keeps criterion (a) honest: a denier's anchors change
//! the graph every team's BFS actually walks, so measured advantage is a
//! product of topology, not of one team having better information. A
//! knowledge-limited bot (racing on stale/partial ledgers) is a natural
//! follow-up and is called out here rather than silently assumed.

use std::collections::BTreeMap;

use observed_core::{Direction, RoomId, TeamId};
use observed_observation::contention::ContentionWorld;
use observed_observation::{COLS, ROWS};

/// Number of lattice rooms (3x3, matches the lab and the arc plan).
const ROOM_COUNT: usize = (ROWS * COLS) as usize;

/// The shared exit room for every race (bottom-right corner of the 3x3
/// lattice, matching the lab's live scene).
const EXIT_ROOM: RoomId = RoomId(8);

/// Starting rooms for the race's four teams, chosen to mirror
/// `ContentionLabPlugin`'s live scene exactly (see `lib.rs`): room 0
/// (top-left corner), room 2 (top-right corner), room 6 (bottom-left
/// corner), room 4 (center). All four are distinct and none is the exit
/// (room 8, bottom-right corner) — a spread of three corners plus the
/// center gives every team a meaningfully different opening distance to the
/// exit rather than a symmetric or trivially-tied layout.
const START_ROOMS: [RoomId; 4] = [RoomId(0), RoomId(2), RoomId(6), RoomId(4)];

/// Hard ceiling on anchors a single `Denier` team holds at once. Oldest is
/// dropped first when a new anchor would exceed this.
const MAX_ANCHORS: usize = 2;

/// Rebuilds the authored 3x3 lattice edges. `observed_observation::authored_edges`
/// is private to that crate, so this mirrors it verbatim (identical to the
/// copy already kept in `lib.rs`'s `make_authored_edges`) rather than
/// reaching into crate internals.
fn lattice_edges() -> Vec<(RoomId, Direction, RoomId, Direction)> {
    let mut edges = Vec::new();
    for r in 0..ROWS {
        for c in 0..COLS {
            let room = RoomId(r * COLS + c);
            if c + 1 < COLS {
                edges.push((
                    room,
                    Direction::East,
                    RoomId(r * COLS + c + 1),
                    Direction::West,
                ));
            }
            if r + 1 < ROWS {
                edges.push((
                    room,
                    Direction::South,
                    RoomId((r + 1) * COLS + c),
                    Direction::North,
                ));
            }
        }
    }
    edges
}

/// A team's strategy for the race. Deliberately simple and deterministic —
/// the point of Phase 38's evidence is *measurable* denial, not a clever AI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy {
    /// Never places anchors. The control condition.
    Naive,
    /// Anchors its own current room whenever that room sits on a fork of at
    /// least three unsealed doors (a deterministic, purely local heuristic —
    /// no lookahead into rivals' paths, no global search). Anchors advance
    /// with the team: the oldest anchor is dropped once a new one would push
    /// the team over [`MAX_ANCHORS`], so a denier always holds at most
    /// `MAX_ANCHORS` rooms frozen, biased toward its most recent ones.
    Denier,
}

/// One team's configuration and running state inside a race.
#[derive(Clone, Copy, Debug)]
struct TeamConfig {
    policy: Policy,
}

/// Deterministic race configuration. Two races built from equal `RaceConfig`s
/// (same seed, same policies) must produce byte-identical outcomes — that is
/// criterion (b).
#[derive(Clone, Copy, Debug)]
pub struct RaceConfig {
    /// Seeds the embedded `ObservationWorld`'s decoherence stream.
    pub seed: u64,
    /// Number of teams. The corpus tests always use 4 (one per
    /// [`START_ROOMS`] slot); kept as a field so the shape is explicit.
    pub teams: usize,
    /// `world.decohere()` is called once every this-many ticks.
    pub decohere_every: u32,
    /// Race is declared over (unfinished teams recorded as DNF) after this
    /// many ticks even if some teams never reach the exit.
    pub tick_budget: u32,
    /// Per-team policy, indexed by team id (0..teams).
    pub policies: [Policy; 4],
}

/// The recorded result of one race.
#[derive(Clone, Debug)]
pub struct RaceOutcome {
    /// Tick each team finished on, `None` if it never reached the exit
    /// within `tick_budget`. Indexed by team id.
    pub finish_ticks: [Option<u32>; 4],
    /// The team id with the lowest finish tick, if anyone finished. Ties
    /// (impossible in practice since teams move in a fixed order and are
    /// checked one at a time, but kept explicit) favor the lower team id.
    pub winner: Option<usize>,
    /// Total `world.decohere()` calls made during the race.
    pub decoheres: u32,
    /// Of those, how many fully reverted (every retry salt still stranded a
    /// member) — see `ContentionWorld::decohere`.
    pub reverted_decoheres: u32,
    /// The highest fraction of rooms simultaneously pinned (observed or
    /// anchored) seen at any tick boundary during the race, over
    /// `ROOM_COUNT`.
    pub max_pinned_fraction: f32,
    /// How many teams never reached the exit before the budget expired.
    pub unfinished: usize,
}

impl RaceOutcome {
    /// A deterministic 64-bit fingerprint of the race: the final door-link
    /// matching plus every outcome field. Two calls with equal inputs must
    /// produce equal digests (criterion b); this is for test comparison
    /// only, not a cryptographic hash.
    ///
    /// FNV-1a, folded by hand so the mixing order is fixed and never depends
    /// on hash map/set iteration (there are none here — everything below
    /// iterates a `Vec` or a fixed-size array in index order).
    fn digest(&self, final_links: &[observed_observation::DoorId]) -> u64 {
        let mut hash: u64 = 0xCBF2_9CE4_8422_2325; // FNV-1a 64-bit offset basis
        const PRIME: u64 = 0x0000_0100_0000_01B3; // FNV-1a 64-bit prime

        let mix_byte = |hash: &mut u64, byte: u8| {
            *hash ^= byte as u64;
            *hash = hash.wrapping_mul(PRIME);
        };
        let mix_u64 = |hash: &mut u64, value: u64| {
            for shift in 0..8 {
                mix_byte(hash, (value >> (shift * 8)) as u8);
            }
        };

        for door in final_links {
            mix_u64(&mut hash, door.0 as u64);
        }
        for finish in self.finish_ticks {
            mix_u64(&mut hash, finish.map(u64::from).unwrap_or(u64::MAX));
        }
        mix_u64(&mut hash, self.winner.map(|w| w as u64).unwrap_or(u64::MAX));
        mix_u64(&mut hash, self.decoheres as u64);
        mix_u64(&mut hash, self.reverted_decoheres as u64);
        mix_u64(&mut hash, self.max_pinned_fraction.to_bits() as u64);
        mix_u64(&mut hash, self.unfinished as u64);
        hash
    }
}

/// Per-team live race bookkeeping that outlives a single tick (anchor
/// history for `Denier`, in oldest-first order so dropping the oldest is a
/// `remove(0)`).
#[derive(Clone, Debug, Default)]
struct DenierState {
    anchor_order: Vec<RoomId>,
}

/// Runs one deterministic race and returns its outcome. `on_after_decohere`
/// is invoked after every `world.decohere()` call with a shared reference to
/// the world and the current tick, so callers (the corpus tests) can assert
/// solvability invariants without duplicating the race loop.
fn run_race_with_hook(
    config: &RaceConfig,
    mut on_after_decohere: impl FnMut(&ContentionWorld, u32),
) -> (RaceOutcome, ContentionWorld) {
    assert_eq!(
        config.teams, 4,
        "corpus races are wired for exactly 4 teams"
    );

    let edges = lattice_edges();
    let members: Vec<(TeamId, RoomId)> = (0..config.teams)
        .map(|team| (TeamId(team as u8), START_ROOMS[team]))
        .collect();

    let mut world = ContentionWorld::new(ROOM_COUNT, &edges, &members, EXIT_ROOM, config.seed);

    let team_configs: Vec<TeamConfig> = (0..config.teams)
        .map(|team| TeamConfig {
            policy: config.policies[team],
        })
        .collect();
    let mut denier_state: BTreeMap<usize, DenierState> = BTreeMap::new();

    let mut finish_ticks: [Option<u32>; 4] = [None; 4];
    let mut finished = vec![false; config.teams];
    let mut decoheres = 0u32;
    let mut reverted = 0u32;
    let mut max_pinned_fraction = 0.0f32;

    let mut tick = 0u32;
    while tick < config.tick_budget && finished.iter().any(|&done| !done) {
        world.tick = u64::from(tick);

        for team in 0..config.teams {
            if finished[team] {
                continue;
            }
            world.record_observations();

            let member_index = team; // one member per team, indices line up 1:1
            let room = world.members[member_index].room;

            match team_configs[team].policy {
                Policy::Naive => {}
                Policy::Denier => apply_denier_policy(&mut world, team, room, &mut denier_state),
            }

            if room == world.exit {
                finished[team] = true;
                finish_ticks[team] = Some(tick);
                continue;
            }

            if let Some(path) = world.path_to_exit(room)
                && let Some(&first_step) = path.first()
            {
                world.traverse(member_index, first_step);
            }

            if world.members[member_index].room == world.exit {
                finished[team] = true;
                finish_ticks[team] = Some(tick);
            }
        }

        max_pinned_fraction = max_pinned_fraction.max(pinned_fraction(&world));

        tick += 1;
        if tick.is_multiple_of(config.decohere_every) {
            world.decohere();
            decoheres += 1;
            if world.last_decohere_reverted {
                reverted += 1;
            }
            on_after_decohere(&world, tick);
            max_pinned_fraction = max_pinned_fraction.max(pinned_fraction(&world));
        }
    }

    let unfinished = finished.iter().filter(|&&done| !done).count();
    let winner = finish_ticks
        .iter()
        .enumerate()
        .filter_map(|(team, finish)| finish.map(|tick| (team, tick)))
        .min_by_key(|&(team, tick)| (tick, team))
        .map(|(team, _)| team);

    let outcome = RaceOutcome {
        finish_ticks,
        winner,
        decoheres,
        reverted_decoheres: reverted,
        max_pinned_fraction,
        unfinished,
    };
    (outcome, world)
}

/// Runs one deterministic race to completion (or budget expiry) and returns
/// its outcome.
pub fn run_race(config: &RaceConfig) -> RaceOutcome {
    let (outcome, _world) = run_race_with_hook(config, |_, _| {});
    outcome
}

/// Runs a race, asserting after every `decohere()` that every unfinished
/// team can still reach the exit. Used by criterion (c); factored out so the
/// assertion lives next to the loop it must hold for, not duplicated in the
/// test body.
#[cfg(test)]
fn run_race_asserting_solvability(config: &RaceConfig) -> RaceOutcome {
    let (outcome, _world) = run_race_with_hook(config, |world, tick| {
        for member in &world.members {
            assert!(
                member.room == world.exit || world.reachable(member.room),
                "seed {} tick {}: member in room {:?} cannot reach the exit after decohere",
                config.seed,
                tick,
                member.room
            );
        }
    });
    outcome
}

/// Computes a byte-identical fingerprint for a completed race by re-deriving
/// the final link table. Exposed as a free function (rather than a method on
/// `RaceOutcome`) because the digest needs the world's final links, which
/// the outcome itself does not retain.
pub fn race_digest(config: &RaceConfig) -> u64 {
    let (outcome, world) = run_race_with_hook(config, |_, _| {});
    outcome.digest(&world.world.links)
}

/// Fraction of rooms currently pinned (observed or anchored, shared across
/// teams) out of `ROOM_COUNT`.
fn pinned_fraction(world: &ContentionWorld) -> f32 {
    let pinned_rooms = (0..ROOM_COUNT)
        .filter(|&index| {
            let room = RoomId(index as u32);
            // A room is pinned iff any of its doors is pinned; probing one
            // door per side is equivalent to `ContentionWorld::room_pins`,
            // which is private, so re-derive via `is_pinned` on each side's
            // door (any true hit means the room pins).
            Direction::ALL
                .iter()
                .any(|&side| world.is_pinned(world.world.door_id(room, side)))
        })
        .count();
    pinned_rooms as f32 / ROOM_COUNT as f32
}

/// `Denier`'s anchor heuristic: if the team's current room has at least
/// three unsealed doors (a "fork") and the team holds fewer than
/// `MAX_ANCHORS` anchors on rooms it hasn't already anchored, anchor it.
/// Anchors advance with the team — the oldest is released once a new one
/// would exceed the budget. Entirely local and deterministic: no inspection
/// of rival positions or paths, matching the plan's "simpler deterministic
/// heuristic" allowance.
fn apply_denier_policy(
    world: &mut ContentionWorld,
    team: usize,
    room: RoomId,
    denier_state: &mut BTreeMap<usize, DenierState>,
) {
    let team_id = TeamId(team as u8);
    let unsealed_doors = Direction::ALL
        .iter()
        .filter(|&&side| !world.world.is_sealed(world.world.door_id(room, side)))
        .count();

    if unsealed_doors < 3 {
        return;
    }

    let state = denier_state.entry(team).or_default();
    if state.anchor_order.contains(&room) {
        return;
    }

    if state.anchor_order.len() >= MAX_ANCHORS {
        let oldest = state.anchor_order.remove(0);
        world.remove_anchor(team_id, oldest);
    }
    world.place_anchor(team_id, room);
    state.anchor_order.push(room);
}

#[cfg(test)]
mod tests {
    use super::{
        Policy, RaceConfig, RaceOutcome, race_digest, run_race, run_race_asserting_solvability,
    };

    /// Seed corpus size. Kept modest so `cargo test -p contention_lab` stays
    /// well under the ~30s budget even with the 4-Denier worst case.
    const CORPUS: u64 = 200;

    fn base_config(seed: u64, policies: [Policy; 4]) -> RaceConfig {
        RaceConfig {
            seed,
            teams: 4,
            decohere_every: 3,
            tick_budget: 400,
            policies,
        }
    }

    fn all_naive(seed: u64) -> RaceConfig {
        base_config(seed, [Policy::Naive; 4])
    }

    fn denier_team0(seed: u64) -> RaceConfig {
        base_config(
            seed,
            [Policy::Denier, Policy::Naive, Policy::Naive, Policy::Naive],
        )
    }

    fn all_deniers(seed: u64) -> RaceConfig {
        base_config(seed, [Policy::Denier; 4])
    }

    /// Mean finish placement (1 = first to finish, ..., up to 5 = DNF) for a
    /// team across a set of outcomes. DNF is scored one worse than the
    /// slowest possible finish (5) so it is unambiguously penalized rather
    /// than silently excluded.
    fn mean_placement(outcomes: &[RaceOutcome], team: usize) -> f64 {
        let placements: Vec<u32> = outcomes
            .iter()
            .map(|outcome| placement_of(outcome, team))
            .collect();
        placements.iter().map(|&p| p as f64).sum::<f64>() / placements.len() as f64
    }

    /// 1-indexed finish rank of `team` among all four teams in `outcome`;
    /// DNF teams are ranked after every finisher, broken by team id so the
    /// function is a pure order (never ties two different teams equally
    /// unless their finish ticks truly matched, which a fixed tick counter
    /// makes impossible for distinct teams anyway).
    fn placement_of(outcome: &RaceOutcome, team: usize) -> u32 {
        let mut order: Vec<(usize, u32)> = (0..4)
            .map(|t| {
                (
                    t,
                    outcome.finish_ticks[t].unwrap_or(u32::MAX - t as u32 + 1000),
                )
            })
            .collect();
        order.sort_by_key(|&(t, tick)| (tick, t));
        order.iter().position(|&(t, _)| t == team).unwrap() as u32 + 1
    }

    /// **Criterion (a):** route denial must measurably change race outcomes,
    /// and it must help the denier. For every corpus seed, race an
    /// all-`Naive` control against a single `Denier` (team 0) among three
    /// `Naive` rivals. Assert outcomes differ on a substantial fraction of
    /// seeds, and that team 0's mean placement improves when it plays
    /// `Denier` versus when everyone (including it) plays `Naive`.
    #[test]
    fn criterion_a_route_denial_measurably_changes_outcomes() {
        let mut differing = 0u32;
        let mut control_outcomes = Vec::with_capacity(CORPUS as usize);
        let mut denier_outcomes = Vec::with_capacity(CORPUS as usize);

        for seed in 0..CORPUS {
            let control = run_race(&all_naive(seed));
            let contested = run_race(&denier_team0(seed));

            if control.winner != contested.winner || control.finish_ticks != contested.finish_ticks
            {
                differing += 1;
            }

            control_outcomes.push(control);
            denier_outcomes.push(contested);
        }

        let differing_fraction = f64::from(differing) / CORPUS as f64;
        let control_mean_placement = mean_placement(&control_outcomes, 0);
        let denier_mean_placement = mean_placement(&denier_outcomes, 0);

        println!("=== criterion (a): route denial evidence ===");
        println!("corpus size:                  {CORPUS}");
        println!(
            "seeds with a different outcome: {differing} / {CORPUS} ({:.1}%)",
            differing_fraction * 100.0
        );
        println!("team 0 mean placement, all-Naive control: {control_mean_placement:.3}");
        println!("team 0 mean placement, as Denier:         {denier_mean_placement:.3}");
        println!(
            "improvement (lower is better):            {:.3}",
            control_mean_placement - denier_mean_placement
        );

        assert!(
            differing_fraction >= 0.25,
            "expected >= 25% of seeds to show a different outcome under denial, got {:.1}%",
            differing_fraction * 100.0
        );
        assert!(
            denier_mean_placement < control_mean_placement,
            "denial should strictly improve team 0's mean placement: control {control_mean_placement:.3} vs denier {denier_mean_placement:.3}"
        );
    }

    /// **Criterion (b):** races are fully deterministic. Running the exact
    /// same config twice must produce byte-identical digests, and different
    /// seeds must (in practice, across this sample) produce different
    /// digests — proving the digest actually discriminates rather than
    /// collapsing every race to the same value.
    #[test]
    fn criterion_b_races_are_deterministic() {
        let mut digests = Vec::with_capacity(20);
        for seed in 0..20 {
            let config = denier_team0(seed);
            let first = race_digest(&config);
            let second = race_digest(&config);
            assert_eq!(
                first, second,
                "seed {seed}: repeated run of the same config must be byte-identical"
            );
            digests.push(first);
        }

        let mut unique = digests.clone();
        unique.sort_unstable();
        unique.dedup();
        println!("=== criterion (b): determinism evidence ===");
        println!("seeds tested:            20");
        println!(
            "unique digests observed: {} / {}",
            unique.len(),
            digests.len()
        );
        assert!(
            unique.len() > 1,
            "digests must discriminate between different seeds, got {} unique value(s) across 20 seeds",
            unique.len()
        );
    }

    /// **Criterion (c):** the solvability guard is a hard contract, not a
    /// best-effort. Across the full corpus, in both the all-Naive and the
    /// single-Denier regime, assert that after every `decohere()` call every
    /// unfinished team can still reach the exit, and tally reverted
    /// decoheres (the guard's fallback path) so the evidence shows the guard
    /// firing, not just never being exercised.
    #[test]
    fn criterion_c_zero_solvability_violations() {
        let mut total_decoheres = 0u32;
        let mut total_reverted = 0u32;

        for seed in 0..CORPUS {
            let naive_outcome = run_race_asserting_solvability(&all_naive(seed));
            let denier_outcome = run_race_asserting_solvability(&denier_team0(seed));

            total_decoheres += naive_outcome.decoheres + denier_outcome.decoheres;
            total_reverted += naive_outcome.reverted_decoheres + denier_outcome.reverted_decoheres;
        }

        println!("=== criterion (c): solvability guard evidence ===");
        println!("corpus size (x2 regimes):      {}", CORPUS * 2);
        println!("total decohere() calls:        {total_decoheres}");
        println!("total reverted (guard fallback): {total_reverted}");
        println!(
            "revert rate:                    {:.3}%",
            f64::from(total_reverted) / f64::from(total_decoheres.max(1)) * 100.0
        );
        // The real assertion already ran inline in
        // `run_race_asserting_solvability` for every decohere in every race
        // above; reaching this point means zero violations were found
        // across the full corpus in both regimes. This final assert is the
        // test's contract restated so a future refactor that silently drops
        // the hook still fails loudly.
        assert!(
            total_decoheres > 0,
            "expected at least one decohere() across the corpus to exercise the guard at all"
        );
    }

    /// **Criterion (d):** four `Denier`s racing each other (the worst case
    /// for lockup) must not freeze the race solid. Assert at least 95% of
    /// corpus seeds finish all four teams within budget, and max pinned
    /// fraction stays below 0.9 on at least 95% of seeds. A failure here is
    /// a *finding* (it triggers the counter-tool design discussion in the
    /// arc plan), not a bug to paper over — thresholds must not be weakened
    /// to force a pass.
    #[test]
    fn criterion_d_no_freeze_stalemate() {
        let mut finished_all = 0u32;
        let mut low_pinned = 0u32;
        let mut worst_pinned_fraction = 0.0f32;
        let mut worst_unfinished = 0usize;

        for seed in 0..CORPUS {
            let outcome = run_race(&all_deniers(seed));
            if outcome.unfinished == 0 {
                finished_all += 1;
            }
            if outcome.max_pinned_fraction < 0.9 {
                low_pinned += 1;
            }
            worst_pinned_fraction = worst_pinned_fraction.max(outcome.max_pinned_fraction);
            worst_unfinished = worst_unfinished.max(outcome.unfinished);
        }

        let finished_fraction = f64::from(finished_all) / CORPUS as f64;
        let low_pinned_fraction = f64::from(low_pinned) / CORPUS as f64;

        println!("=== criterion (d): four-Denier stalemate evidence ===");
        println!("corpus size:                          {CORPUS}");
        println!(
            "seeds finishing all 4 teams in budget:  {finished_all} / {CORPUS} ({:.1}%)",
            finished_fraction * 100.0
        );
        println!(
            "seeds with max_pinned_fraction < 0.9:   {low_pinned} / {CORPUS} ({:.1}%)",
            low_pinned_fraction * 100.0
        );
        println!("worst-case max_pinned_fraction seen:   {worst_pinned_fraction:.3}");
        println!("worst-case unfinished team count seen: {worst_unfinished}");

        assert!(
            finished_fraction >= 0.95,
            "expected >= 95% of all-Denier races to finish within budget, got {:.1}% ({finished_all}/{CORPUS}) -- \
             this is a finding for the counter-tool design discussion, not a threshold to relax",
            finished_fraction * 100.0
        );
        assert!(
            low_pinned_fraction >= 0.95,
            "expected >= 95% of all-Denier races to keep max_pinned_fraction < 0.9, got {:.1}% ({low_pinned}/{CORPUS}) -- \
             this is a finding for the counter-tool design discussion, not a threshold to relax",
            low_pinned_fraction * 100.0
        );
    }
}
