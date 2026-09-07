//! Two kinds of test carry this lab.
//!
//! The **conflict table** tests pin the simultaneous rules written down in
//! `docs/mechanic_lab_plan.md`, one row each, because that table is the part
//! that bites if code comes first.
//!
//! The **seam** tests assert that swapping one strategy actually changes the
//! match. A seam no test can distinguish is not a seam — it is a speculative
//! abstraction with two names, which is exactly what this lab must not become.

use observed_hex::coords::{HexCoord, lateral_distance};
use observed_hex::faces::HexFace;
use observed_hex::ports::PortClass;

use crate::sim::board::Edge;
use crate::sim::bot;
use crate::sim::objective::{PlantRule, PlantWin};
use crate::sim::state::{Action, ChangeSource, Intent, MatchState, PawnId, TeamId};
use crate::sim::step::step;
use crate::sim::threat::{ConeInteraction, GuardianTarget};
use crate::sim::tiles::{Refusal, TilePlay, TileShape};
use crate::spec::{
    ConeTiming, ModeSpec, MutationKind, MutationPreview, ObjectiveKind, ResolutionKind, Rules,
    SetbackKind, ThreatKind, VisionKind, deal,
};

const fn at(q: u16, r: u16) -> HexCoord {
    HexCoord { q, r, level: 0 }
}

/// A quiet board: no guardians, no reseal, so a conflict test measures only the
/// resolution strategy.
///
/// Stacking is pinned `Forbidden` on purpose. The shipped default is `Allowed`,
/// and under it most of the conflict table simply does not apply — teammates
/// stop contending for space at all. These rows describe the *forbidden* rule,
/// so they name it rather than inheriting whichever way the default happens to
/// point today.
fn still_spec() -> ModeSpec {
    ModeSpec {
        pawns_per_team: 3,
        threats: vec![ThreatKind::None],
        mutation: MutationKind::None,
        stacking: crate::spec::Stacking::Forbidden,
        ..ModeSpec::plant()
    }
}

fn place(state: &mut MatchState, positions: &[(u8, HexCoord)]) {
    for &(id, coord) in positions {
        let pawn = state.pawn_mut(PawnId(id));
        pawn.at = coord;
        pawn.prev_at = coord;
    }
}

fn intent(id: u8, face: HexFace) -> Intent {
    Intent {
        pawn: PawnId(id),
        facing: face,
        action: Action::Step(face),
    }
}

/// Run the scripted driver and fingerprint the result.
fn drive(spec: &ModeSpec, turns: u16) -> u64 {
    let rules = Rules::from_spec(spec);
    let mut state = deal(spec);
    for _ in 0..turns {
        if state.outcome.is_some() {
            break;
        }
        let intents = bot::intents(&state, &rules);
        step(&mut state, &rules, &intents);
    }
    state.digest()
}

// --- the board ------------------------------------------------------------

#[test]
fn radius_three_hexagon_holds_thirty_seven_cells() {
    let state = deal(&ModeSpec::plant());
    assert_eq!(state.board.cells().count(), 37, "3r^2 + 3r + 1 for r = 3");
    // The rhombic corners are masked out, not merely sealed.
    assert!(!state.board.on_board(at(0, 0)));
    assert!(state.board.on_board(at(3, 3)));
}

// --- the conflict table ---------------------------------------------------

#[test]
fn two_pawns_wanting_one_hex_are_both_refused() {
    let spec = still_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(2, 3)), (1, at(4, 3)), (2, at(0, 3))]);

    step(
        &mut state,
        &rules,
        &[intent(0, HexFace::East), intent(1, HexFace::West)],
    );

    assert_eq!(state.pawn(PawnId(0)).at, at(2, 3), "refused, symmetrically");
    assert_eq!(state.pawn(PawnId(1)).at, at(4, 3));
    assert_eq!(state.report.refused_moves.len(), 2);
}

#[test]
fn a_straight_swap_is_refused_because_bodies_do_not_pass_through_each_other() {
    let spec = still_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(2, 3)), (1, at(3, 3)), (2, at(0, 3))]);

    step(
        &mut state,
        &rules,
        &[intent(0, HexFace::East), intent(1, HexFace::West)],
    );

    assert_eq!(state.pawn(PawnId(0)).at, at(2, 3));
    assert_eq!(state.pawn(PawnId(1)).at, at(3, 3));
}

#[test]
fn a_pawn_may_follow_one_that_is_leaving() {
    let spec = still_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(2, 3)), (1, at(3, 3)), (2, at(0, 3))]);

    step(
        &mut state,
        &rules,
        &[intent(0, HexFace::East), intent(1, HexFace::East)],
    );

    assert_eq!(state.pawn(PawnId(0)).at, at(3, 3), "convoy");
    assert_eq!(state.pawn(PawnId(1)).at, at(4, 3));
}

#[test]
fn a_pawn_is_blocked_by_one_that_stays_put() {
    let spec = still_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(2, 3)), (1, at(3, 3)), (2, at(0, 3))]);

    let hold = Intent {
        pawn: PawnId(1),
        facing: HexFace::East,
        action: Action::Hold,
    };
    step(&mut state, &rules, &[intent(0, HexFace::East), hold]);

    assert_eq!(state.pawn(PawnId(0)).at, at(2, 3));
    assert_eq!(state.pawn(PawnId(1)).at, at(3, 3));
}

#[test]
fn a_full_cycle_rotates_because_it_is_consistent() {
    let spec = still_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(3, 3)), (1, at(4, 3)), (2, at(3, 4))]);

    step(
        &mut state,
        &rules,
        &[
            intent(0, HexFace::East),
            intent(1, HexFace::SouthWest),
            intent(2, HexFace::NorthWest),
        ],
    );

    assert_eq!(state.pawn(PawnId(0)).at, at(4, 3));
    assert_eq!(state.pawn(PawnId(1)).at, at(3, 4));
    assert_eq!(state.pawn(PawnId(2)).at, at(3, 3));
}

// --- the seams ------------------------------------------------------------

/// The seam the lab was asked for first, on the case that isolates it: under
/// sequential resolution the lower id takes the contested hex, so the two
/// strategies disagree about the same intents.
#[test]
fn sequential_and_simultaneous_disagree_on_a_contested_hex() {
    let mut spec = still_spec();
    spec.resolution = ResolutionKind::Sequential;
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(2, 3)), (1, at(4, 3)), (2, at(0, 3))]);

    step(
        &mut state,
        &rules,
        &[intent(0, HexFace::East), intent(1, HexFace::West)],
    );

    assert_eq!(state.pawn(PawnId(0)).at, at(3, 3), "first id wins the hex");
    assert_eq!(state.pawn(PawnId(1)).at, at(4, 3));
}

#[test]
fn every_seam_changes_the_match_it_is_swapped_into() {
    // Five pawns rather than the shipped three, and the reason is worth
    // recording. Once boundaries carry the walls, corridors force single file
    // and a three-pawn squad almost never wants the same hex — so resolution
    // order stops being observable in driver play at exactly the shipped
    // configuration, while differing at every other pawn count and wall count
    // measured. The seam is real (see the crafted contested-hex test below); it
    // is the *board* that hides it, which is a finding about walls rather than
    // a fault in the strategies.
    // A fixture chosen so the driver can actually exercise every mechanic, and
    // both departures from the shipped mode are findings in their own right.
    //
    // Five pawns rather than three: once boundaries carry walls, corridors
    // force single file and a three-pawn squad almost never wants the same hex,
    // so resolution order stops being observable at exactly the shipped
    // configuration while differing at every other pawn and wall count
    // measured. The seam is real - see the crafted contested-hex test - and it
    // is the *board* that hides it.
    //
    // `StandOnly` rather than `Overwatch`: with cone occlusion an escort must
    // now see the flag *through a doorway*, which the driver almost never
    // manages, so no flag is ever planted and the objective seam collapses to
    // "two matches in which nothing happened".
    // A single guardian rather than two: measured across pawn counts, cadences
    // and guardian counts, two guardians end the match before any flag is
    // planted at four and five pawns, which collapses the objective seam to
    // "two matches in which nothing happened". One guardian still catches
    // people - the setback seam needs that - while leaving the squad alive long
    // enough to reach an objective.
    //
    // `ConeInteraction::Ignores` rather than the shipped shield, and that is
    // the third departure worth recording: with the shield on, a three-face
    // cone stops the guardians taking anybody at all in driver play, so
    // `Setback` never fires and Prison and RespawnAtStart become the same
    // match. The seam is real; the shipped rule is what silences it.
    //
    // And a capped churn rather than the shipped `AllUnobserved`: when every
    // unheld boundary rewires each turn, the board reshapes faster than a
    // twenty-four turn drive can express anything, and matches end the same way
    // whatever else is swapped. A mild churn still exercises the mutation seam
    // while leaving the others room to differ.
    let base = ModeSpec {
        pawns_per_team: 5,
        guardian_count: 1,
        turn_limit: 24,
        cone_interaction: ConeInteraction::Ignores,
        mutation: MutationKind::TelegraphedRewire {
            scope: crate::sim::mutation::Scope::Capped { base: 2, cap: 6 },
        },
        objective: ObjectiveKind::PlantFlags {
            rule: PlantRule::StandOnly,
            win: PlantWin::All,
        },
        ..ModeSpec::plant()
    };
    let reference = drive(&base, 24);

    let variants: [(&str, ModeSpec); 6] = [
        (
            "resolution",
            ModeSpec {
                resolution: ResolutionKind::Sequential,
                ..base.clone()
            },
        ),
        (
            "vision",
            ModeSpec {
                vision: VisionKind::Radius { range: 1 },
                ..base.clone()
            },
        ),
        (
            "threat",
            ModeSpec {
                threats: vec![ThreatKind::None],
                ..base.clone()
            },
        ),
        (
            "setback",
            ModeSpec {
                setback: SetbackKind::RespawnAtStart,
                ..base.clone()
            },
        ),
        (
            "mutation",
            ModeSpec {
                mutation: MutationKind::None,
                ..base.clone()
            },
        ),
        (
            "objective",
            ModeSpec {
                objective: ObjectiveKind::ReachExit,
                ..base.clone()
            },
        ),
    ];

    for (seam, spec) in variants {
        assert_ne!(
            drive(&spec, 24),
            reference,
            "swapping `{seam}` produced an identical match: that seam is not a seam",
        );
    }
}

#[test]
fn cone_timing_relocates_the_lock_and_changes_the_match() {
    let base = ModeSpec::plant();
    let moved = ModeSpec {
        cone_timing: ConeTiming::PreMove,
        ..base.clone()
    };
    assert_ne!(drive(&moved, 12), drive(&base, 12));
}

#[test]
fn each_cone_interaction_reading_plays_differently() {
    // At the shipped three-face cone all three readings play differently. At a
    // one-face cone they do not: the arc is so narrow that a guardian
    // essentially never steps into it, and the shipped configuration loses
    // every pawn whether the cone blocks or is ignored. That measurement is why
    // the default cone is three faces wide.
    // Cadence 1, so the guardians actually make contact inside the drive; at
    // the shipped cadence 2 an evading squad can simply outrun them on a walled
    // board and none of the three readings gets a chance to differ.
    let base = ModeSpec {
        guardian_count: 3,
        guardian_cadence: 1,
        cone_interaction: ConeInteraction::Ignores,
        mutation: MutationKind::TelegraphedRewire {
            scope: crate::sim::mutation::Scope::Capped { base: 2, cap: 6 },
        },
        ..ModeSpec::plant()
    };
    let ignores = drive(&base, 12);
    let blocked = drive(
        &ModeSpec {
            cone_interaction: ConeInteraction::Blocked,
            ..base.clone()
        },
        12,
    );
    let slowed = drive(
        &ModeSpec {
            cone_interaction: ConeInteraction::Slowed,
            ..base.clone()
        },
        12,
    );
    assert_ne!(ignores, blocked, "the shield did nothing");
    // `Slowed` is deliberately not asserted here. Once the shield reads
    // coverage rather than occupancy it only fires when a *teammate* is looking
    // at the threatened cell, and the driver does not coordinate coverage — it
    // faces wherever it is walking. The crafted case below is what proves it.
    let _ = slowed;
}

/// The shield is a **teammate** mechanic, and this is where that is pinned.
///
/// A pawn's own cell is held but not *covered*, so looking after yourself is
/// not a defence — somebody else has to be watching you. That is a much better
/// rule than the one it replaced, where every pawn was permanently unreachable
/// and the guardians spent whole matches parked next to the squad, and it is
/// squarely the co-operation the north star asks for within a team.
#[test]
fn a_watched_teammate_is_shielded_and_an_unwatched_one_is_not() {
    use crate::spec::Stacking;

    fn caught(cone: ConeInteraction, escort_looks: bool) -> (bool, HexCoord) {
        let spec = ModeSpec {
            pawns_per_team: 2,
            guardian_count: 1,
            guardian_cadence: 1,
            guardian_target: GuardianTarget::NearestPawn,
            cone_interaction: cone,
            mutation: MutationKind::None,
            stacking: Stacking::Forbidden,
            ..ModeSpec::plant()
        };
        let rules = Rules::from_spec(&spec);
        let mut state = deal(&spec);

        let (target, escort, post) = (at(3, 3), at(2, 3), at(4, 3));
        for edge in [
            Edge {
                cell: escort,
                face: HexFace::East,
            },
            Edge {
                cell: target,
                face: HexFace::East,
            },
        ] {
            state.board.set_port(edge, PortClass::Door);
        }
        place(&mut state, &[(0, target), (1, escort)]);
        state.guardians[0].at = post;

        // The escort either watches its teammate or looks away.
        let watch = if escort_looks {
            HexFace::East
        } else {
            HexFace::West
        };
        step(
            &mut state,
            &rules,
            &[
                Intent {
                    pawn: PawnId(0),
                    facing: HexFace::East,
                    action: Action::Hold,
                },
                Intent {
                    pawn: PawnId(1),
                    facing: watch,
                    action: Action::Hold,
                },
            ],
        );
        (state.pawn(PawnId(0)).jailed, state.guardians[0].at)
    }

    let (taken, _) = caught(ConeInteraction::Ignores, true);
    assert!(
        taken,
        "without a shield the guardian walks in and takes you"
    );

    let (taken, where_it_stopped) = caught(ConeInteraction::Blocked, true);
    assert!(!taken, "a watched teammate is safe");
    assert_ne!(where_it_stopped, at(3, 3), "and the guardian is kept out");

    let (taken, where_it_went) = caught(ConeInteraction::Slowed, true);
    assert!(!taken, "slowed forfeits the catch");
    assert_eq!(where_it_went, at(3, 3), "but it still walks in");

    let (taken, _) = caught(ConeInteraction::Blocked, false);
    assert!(
        taken,
        "looking after yourself is not a defence - somebody must watch you"
    );
}

#[test]
fn guardian_targets_are_strategies_not_numbers() {
    let base = ModeSpec::plant();
    let hunting = drive(&base, 12);
    for target in [
        GuardianTarget::CampNearestUnplantedFlag,
        GuardianTarget::GuardPrison,
        GuardianTarget::FixedPatrol,
    ] {
        assert_ne!(
            drive(
                &ModeSpec {
                    guardian_target: target,
                    ..base.clone()
                },
                12
            ),
            hunting,
            "{target:?} played the same match as NearestPawn",
        );
    }
}

// --- the rules that would otherwise be vacuous ----------------------------

#[test]
fn overwatch_needs_a_second_pair_of_eyes_and_stand_only_does_not() {
    // A pawn always holds the cell it stands in, so "the planter must observe
    // the flag" would be satisfied by definition. Overwatch is the reading
    // where the gate bites.
    let solo = ModeSpec {
        pawns_per_team: 1,
        threats: vec![ThreatKind::None],
        mutation: MutationKind::None,
        ..ModeSpec::plant()
    };

    let rules = Rules::from_spec(&solo);
    let mut state = deal(&solo);
    let flag = state.flags[0].at;
    place(&mut state, &[(0, flag)]);
    let plant = Intent {
        pawn: PawnId(0),
        facing: HexFace::East,
        action: Action::Plant,
    };
    step(&mut state, &rules, &[plant]);
    assert!(
        state.flags[0].planted_by.is_none(),
        "one pawn cannot overwatch itself"
    );

    let relaxed = ModeSpec {
        objective: ObjectiveKind::PlantFlags {
            rule: PlantRule::StandOnly,
            win: PlantWin::All,
        },
        ..solo
    };
    let rules = Rules::from_spec(&relaxed);
    let mut state = deal(&relaxed);
    place(&mut state, &[(0, flag)]);
    step(&mut state, &rules, &[plant]);
    assert!(state.flags[0].planted_by.is_some());
}

#[test]
fn an_escort_facing_the_flag_lets_the_planter_plant() {
    let spec = ModeSpec {
        pawns_per_team: 2,
        threats: vec![ThreatKind::None],
        mutation: MutationKind::None,
        ..ModeSpec::plant()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    // Planter on the centre-adjacent cell, escort west of it looking east.
    let target = at(3, 3);
    state.flags[0].at = target;
    place(&mut state, &[(0, target), (1, at(2, 3))]);

    step(
        &mut state,
        &rules,
        &[
            Intent {
                pawn: PawnId(0),
                facing: HexFace::East,
                action: Action::Plant,
            },
            Intent {
                pawn: PawnId(1),
                facing: HexFace::East,
                action: Action::Hold,
            },
        ],
    );

    assert!(state.flags[0].planted_by.is_some(), "overwatch satisfied");
}

// --- holding ground -------------------------------------------------------

#[test]
fn standing_beside_a_telegraphed_boundary_refuses_the_rewire() {
    let spec = ModeSpec {
        pawns_per_team: 1,
        threats: vec![ThreatKind::None],
        ..ModeSpec::plant()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);

    let hold = Intent {
        pawn: PawnId(0),
        facing: HexFace::East,
        action: Action::Hold,
    };
    // Stand on one end of whatever is marked, and the change must be refused.
    for _ in 0..24 {
        let Some(change) = state.telegraph.first().copied() else {
            break;
        };
        place(&mut state, &[(0, change.edge.cell)]);
        let before = state.board.port(change.edge);
        step(&mut state, &rules, &[hold]);
        if state.report.refused_rewires.contains(&change) {
            assert_eq!(
                state.board.port(change.edge),
                before,
                "a held boundary changed anyway"
            );
            return;
        }
    }
    panic!("no telegraphed boundary was ever held in 24 turns");
}

#[test]
fn the_telegraph_never_marks_a_flag_or_prison_boundary() {
    let spec = ModeSpec::plant();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    for _ in 0..40 {
        let locks = rules.vision.locks(&state);
        rules.mutation.telegraph(&mut state, &locks);
        for change in &state.telegraph {
            let other = state
                .board
                .size()
                .neighbor(change.edge.cell, change.edge.face);
            for cell in [Some(change.edge.cell), other].into_iter().flatten() {
                assert!(
                    !state.prisons.contains(&cell),
                    "a prison boundary was marked"
                );
                assert!(
                    !state.flags.iter().any(|flag| flag.at == cell),
                    "a flag boundary was marked"
                );
            }
        }
        state.turn += 1;
    }
}

// --- the prison -----------------------------------------------------------

#[test]
fn a_rescue_grants_immunity_or_the_jailbreak_is_dead_on_arrival() {
    let spec = ModeSpec {
        pawns_per_team: 2,
        guardian_count: 1,
        guardian_target: GuardianTarget::GuardPrison,
        mutation: MutationKind::None,
        ..ModeSpec::plant()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);

    let prison = state.prison_for(TeamId(0));
    state.pawn_mut(PawnId(0)).jailed = true;
    state.pawn_mut(PawnId(0)).at = prison;
    place(&mut state, &[(1, prison)]);
    state.pawn_mut(PawnId(1)).jailed = false;
    state.guardians[0].at = prison;

    let hold = Intent {
        pawn: PawnId(1),
        facing: HexFace::East,
        action: Action::Hold,
    };
    step(&mut state, &rules, &[hold]);

    assert!(
        !state.pawn(PawnId(0)).jailed,
        "a guardian sitting on the prison re-took the rescue on the same turn"
    );
}

// --- determinism ----------------------------------------------------------

#[test]
fn a_mode_and_an_intent_log_reproduce_the_same_match() {
    let spec = ModeSpec::plant();
    assert_eq!(drive(&spec, 16), drive(&spec, 16));
}

#[test]
fn every_shipped_mode_runs_to_a_verdict_without_panicking() {
    for spec in [
        ModeSpec::plant(),
        ModeSpec {
            resolution: ResolutionKind::Sequential,
            vision: VisionKind::Radius { range: 2 },
            threats: vec![ThreatKind::None],
            setback: SetbackKind::RespawnAtStart,
            mutation: MutationKind::None,
            objective: ObjectiveKind::ReachExit,
            ..ModeSpec::plant()
        },
    ] {
        let rules = Rules::from_spec(&spec);
        let mut state = deal(&spec);
        for _ in 0..spec.turn_limit + 1 {
            if state.outcome.is_some() {
                break;
            }
            let intents = bot::intents(&state, &rules);
            step(&mut state, &rules, &intents);
        }
        assert!(state.outcome.is_some(), "{} never resolved", spec.name);
    }
}

// --- stink base -----------------------------------------------------------

/// A quiet contested board: two teams, no guardians, no reseal, so a recency
/// test measures only recency.
fn duel_spec() -> ModeSpec {
    ModeSpec {
        threats: vec![ThreatKind::RivalPawns],
        guardian_count: 0,
        mutation: MutationKind::None,
        ..ModeSpec::base()
    }
}

/// Put two rivals in contact off their bases with the given stamps, run a turn,
/// and report who is held.
fn contact(mine: u16, theirs: u16) -> (bool, bool) {
    let spec = duel_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(2, 3)), (3, at(3, 3))]);
    state.pawn_mut(PawnId(0)).left_base_at = mine;
    state.pawn_mut(PawnId(3)).left_base_at = theirs;

    step(&mut state, &rules, &[intent(0, HexFace::East)]);
    (state.pawn(PawnId(0)).jailed, state.pawn(PawnId(3)).jailed)
}

#[test]
fn the_fresher_pawn_takes_the_staler_one() {
    assert_eq!(contact(9, 1), (false, true), "fresher walks in and takes");
    assert_eq!(contact(1, 9), (true, false), "staler walks in and is taken");
}

#[test]
fn equal_recency_is_a_standoff() {
    assert_eq!(contact(4, 4), (false, false));
}

#[test]
fn a_pawn_in_its_own_base_cannot_be_taken() {
    let spec = duel_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    let their_base = state.base_of(TeamId(1));
    let approach = state
        .board
        .open_neighbours(their_base)
        .map(|(_, cell)| cell)
        .next()
        .expect("the base has a neighbour");

    place(&mut state, &[(0, approach), (3, their_base)]);
    // The intruder is as fresh as anything off-base can be.
    state.pawn_mut(PawnId(0)).left_base_at = u16::MAX - 1;
    state.pawn_mut(PawnId(3)).left_base_at = 0;

    let face = HexFace::LATERAL
        .into_iter()
        .find(|&face| state.board.size().neighbor(approach, face) == Some(their_base))
        .expect("adjacent");
    step(&mut state, &rules, &[intent(0, face)]);

    assert!(!state.pawn(PawnId(3)).jailed, "home is safe");
}

#[test]
fn stepping_off_base_stamps_recency_and_standing_on_it_does_not() {
    let spec = duel_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    let base = state.base_of(TeamId(0));
    place(&mut state, &[(0, base)]);
    state.turn = 7;

    assert_eq!(state.freshness(PawnId(0)), u16::MAX, "home is maximal");

    let face = state
        .board
        .open_neighbours(base)
        .map(|(face, _)| face)
        .next()
        .expect("the base has a neighbour");
    step(&mut state, &rules, &[intent(0, face)]);

    assert_eq!(state.pawn(PawnId(0)).left_base_at, 7, "stamped on leaving");
}

#[test]
fn rivals_may_share_a_hex_although_teammates_may_not() {
    // The conflict table's amendment: contention is team-local, because the
    // contact it would otherwise refuse is exactly what recency adjudicates.
    let spec = ModeSpec {
        threats: vec![ThreatKind::None],
        guardian_count: 0,
        mutation: MutationKind::None,
        ..ModeSpec::base()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(2, 3)), (3, at(4, 3))]);

    step(
        &mut state,
        &rules,
        &[intent(0, HexFace::East), intent(3, HexFace::West)],
    );

    assert_eq!(state.pawn(PawnId(0)).at, at(3, 3));
    assert_eq!(state.pawn(PawnId(3)).at, at(3, 3), "rivals meet");
}

#[test]
fn a_taken_pawn_is_held_in_the_rivals_prison() {
    let spec = duel_spec();
    let mut state = deal(&spec);
    assert_eq!(state.prison_for(TeamId(0)), spec.board.prisons[1]);
    assert_eq!(state.prison_for(TeamId(1)), spec.board.prisons[0]);

    let rules = Rules::from_spec(&spec);
    place(&mut state, &[(0, at(2, 3)), (3, at(3, 3))]);
    state.pawn_mut(PawnId(0)).left_base_at = 9;
    step(&mut state, &rules, &[intent(0, HexFace::East)]);

    assert!(state.pawn(PawnId(3)).jailed);
    assert_eq!(state.pawn(PawnId(3)).at, spec.board.prisons[0]);
}

#[test]
fn guardian_cadence_is_what_makes_a_guardian_escapable() {
    // At cadence 1 a guardian closes one hex per turn and so does a pawn, so it
    // never gives distance back. The two cadences must play differently or the
    // setting is a lie.
    let base = ModeSpec::plant();
    assert_ne!(
        drive(
            &ModeSpec {
                guardian_cadence: 1,
                ..base.clone()
            },
            12
        ),
        drive(&base, 12)
    );
}

#[test]
fn the_contested_board_is_symmetric_so_neither_side_starts_closer() {
    let spec = ModeSpec::base();
    let state = deal(&spec);
    let (a, b) = (state.base_of(TeamId(0)), state.base_of(TeamId(1)));
    let mut from_a: Vec<u32> = spec
        .board
        .flags
        .iter()
        .map(|&flag| observed_hex::coords::lateral_distance(a, flag))
        .collect();
    let mut from_b: Vec<u32> = spec
        .board
        .flags
        .iter()
        .map(|&flag| observed_hex::coords::lateral_distance(b, flag))
        .collect();
    from_a.sort_unstable();
    from_b.sort_unstable();
    assert_eq!(from_a, from_b, "one side had a shorter route to the flags");
}

#[test]
fn the_telegraph_is_visible_before_a_turn_is_played() {
    // The marks have to exist while the player is choosing orders. An earlier
    // draft generated the telegraph at the top of `step` and applied it in the
    // same call, so it existed only between two statements and no human ever
    // saw it. Bot play could not catch this: the driver does not read it.
    let spec = ModeSpec::plant();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    assert!(
        !state.telegraph.is_empty(),
        "a fresh match must open with its first telegraph already showing"
    );

    let marked = state.telegraph.clone();
    let hold = Intent {
        pawn: PawnId(0),
        facing: HexFace::East,
        action: Action::Hold,
    };
    step(&mut state, &rules, &[hold]);

    // What was showing is what resolved, and a fresh set is showing for next.
    for change in &marked {
        assert!(
            state.report.rewired.contains(change) || state.report.refused_rewires.contains(change),
            "a telegraphed boundary neither changed nor was held"
        );
    }
    assert!(
        !state.telegraph.is_empty(),
        "the next turn is telegraphed too"
    );
}

// --- walls ----------------------------------------------------------------

#[test]
fn a_wall_stops_a_pawn_and_a_doorway_does_not() {
    let spec = still_spec();
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    let from = at(3, 3);
    place(&mut state, &[(0, from)]);

    let edge = Edge {
        cell: from,
        face: HexFace::East,
    };
    state.board.set_port(edge, PortClass::Door);
    step(&mut state, &rules, &[intent(0, HexFace::East)]);
    assert_eq!(state.pawn(PawnId(0)).at, at(4, 3), "a doorway is passable");

    place(&mut state, &[(0, from)]);
    state.board.set_port(edge, PortClass::Sealed);
    step(&mut state, &rules, &[intent(0, HexFace::East)]);
    assert_eq!(state.pawn(PawnId(0)).at, from, "a wall is not");
}

#[test]
fn both_sides_of_a_boundary_always_agree() {
    // Ports that drift apart across a boundary are the classic WFC bug, and
    // `ports_compatible` only bonds faces offering the same class.
    let mut board = crate::sim::board::Board::hexagon(3);
    let edge = Edge {
        cell: at(3, 3),
        face: HexFace::East,
    };
    board.set_port(edge, PortClass::Sealed);
    let mirror = Edge {
        cell: at(4, 3),
        face: HexFace::West,
    };
    assert_eq!(board.port(mirror), PortClass::Sealed);
    assert!(!board.passable(at(4, 3), HexFace::West));
}

#[test]
fn a_wall_occludes_the_vision_cone() {
    let spec = ModeSpec {
        vision: VisionKind::Cone {
            width: 3,
            range: 2,
            lock_own_hex: true,
        },
        ..still_spec()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    let from = at(3, 3);
    place(&mut state, &[(0, from)]);

    let near = Edge {
        cell: from,
        face: HexFace::East,
    };
    state.board.set_port(near, PortClass::Door);
    let far = Edge {
        cell: at(4, 3),
        face: HexFace::East,
    };
    state.board.set_port(far, PortClass::Door);
    assert!(
        rules.vision.covers(&state, from, HexFace::East, at(5, 3)),
        "two doorways: sight reaches two cells"
    );

    state.board.set_port(near, PortClass::Sealed);
    assert!(
        !rules.vision.covers(&state, from, HexFace::East, at(5, 3)),
        "a wall in between must stop it"
    );
}

#[test]
fn a_board_is_never_dealt_disconnected() {
    // A lab that can deal an unwinnable match wastes the tester's time, so a
    // wall that would strand part of the board is simply not put up.
    for walls in [0_u16, 10, 22, 40, 200] {
        let spec = ModeSpec {
            walls,
            ..ModeSpec::plant()
        };
        let state = deal(&spec);
        assert!(
            state.board.fully_connected(),
            "{walls} walls disconnected the board"
        );
    }
}

// --- the preview is a display setting, not a rule --------------------------

#[test]
fn showing_the_coming_change_cannot_change_the_match() {
    // The same pin `tactics_lab` puts on its whole-map view: a display setting
    // that quietly moved the rules would be measuring a different game than the
    // one being judged.
    let hidden = drive(
        &ModeSpec {
            preview: MutationPreview::Hidden,
            ..ModeSpec::plant()
        },
        16,
    );
    for preview in [MutationPreview::Location, MutationPreview::Outcome] {
        assert_eq!(
            drive(
                &ModeSpec {
                    preview,
                    ..ModeSpec::plant()
                },
                16
            ),
            hidden,
            "{preview:?} changed the simulation",
        );
    }
}

#[test]
fn the_telegraph_says_what_each_boundary_will_become() {
    // `MutationPreview::Outcome` can only draw the future if the simulation
    // records it, and the recorded target must be the opposite of what is there.
    let spec = ModeSpec::plant();
    let state = deal(&spec);
    assert!(!state.telegraph.is_empty());
    for change in &state.telegraph {
        let now = state.board.port(change.edge);
        assert_ne!(now, change.to, "a change that changes nothing");
        assert!(matches!(change.to, PortClass::Door | PortClass::Sealed));
    }
}

// --- stacking -------------------------------------------------------------

/// Two teammates ordered onto one cell, under a given stacking rule.
fn crowd(stacking: crate::spec::Stacking) -> Vec<HexCoord> {
    let spec = ModeSpec {
        stacking,
        ..still_spec()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(2, 3)), (1, at(4, 3)), (2, at(0, 3))]);
    for face in [HexFace::East, HexFace::West] {
        let edge = Edge {
            cell: at(3, 3),
            face,
        };
        state.board.set_port(edge, PortClass::Door);
    }

    step(
        &mut state,
        &rules,
        &[intent(0, HexFace::East), intent(1, HexFace::West)],
    );
    vec![state.pawn(PawnId(0)).at, state.pawn(PawnId(1)).at]
}

#[test]
fn teammates_share_a_cell_only_when_the_mode_allows_it() {
    use crate::spec::Stacking;

    assert_eq!(
        crowd(Stacking::Forbidden),
        vec![at(2, 3), at(4, 3)],
        "both refused, as the conflict table says"
    );
    assert_eq!(
        crowd(Stacking::Allowed),
        vec![at(3, 3), at(3, 3)],
        "both arrive and pile up"
    );
}

#[test]
fn stacking_is_a_seam_that_changes_the_match() {
    use crate::spec::Stacking;

    let base = ModeSpec {
        pawns_per_team: 5,
        guardian_count: 1,
        turn_limit: 24,
        objective: ObjectiveKind::PlantFlags {
            rule: PlantRule::StandOnly,
            win: PlantWin::All,
        },
        ..ModeSpec::plant()
    };
    assert_ne!(
        drive(
            &ModeSpec {
                stacking: Stacking::Forbidden,
                ..base.clone()
            },
            24
        ),
        drive(&base, 24),
        "the shipped rule is Allowed, so Forbidden is the variant under test",
    );
}

#[test]
fn a_stacked_cell_reports_everyone_standing_in_it() {
    // The view fans a stack and `Next` walks it, so both need the full list —
    // an `occupant` that answers with only the first pawn leaves the ones
    // underneath unreachable.
    use crate::spec::Stacking;
    let spec = ModeSpec {
        stacking: Stacking::Allowed,
        ..still_spec()
    };
    let mut state = deal(&spec);
    place(&mut state, &[(0, at(3, 3)), (1, at(3, 3)), (2, at(0, 3))]);
    assert_eq!(state.occupants(at(3, 3)), vec![PawnId(0), PawnId(1)]);
    assert_eq!(state.occupants(at(0, 3)), vec![PawnId(2)]);
}

// --- watching it play ------------------------------------------------------

#[test]
fn two_bots_play_every_shipped_mode_to_a_verdict() {
    // Spectate is the cheapest way to see whether a mode has a shape, so every
    // preset has to survive being driven from both sides — including the ones
    // with two teams, where an earlier driver planned both squads as one and
    // had team 1 escorting team 0's planter.
    for spec in ModeSpec::presets() {
        let rules = Rules::from_spec(&spec);
        let mut state = deal(&spec);
        let mut turns = 0;
        while state.outcome.is_none() && turns < spec.turn_limit + 1 {
            let mut intents = Vec::new();
            for team in state.teams() {
                intents.extend(bot::team_intents(&state, team));
            }
            intents.sort_by_key(|intent| intent.pawn);
            step(&mut state, &rules, &intents);
            turns += 1;
        }
        assert!(
            state.outcome.is_some(),
            "{} never reached a verdict in {turns} turns",
            spec.name
        );
    }
}

#[test]
fn a_driven_team_only_ever_orders_its_own_pawns() {
    // The bug that made two-team play look broken: one plan for every free
    // pawn on the board, regardless of side.
    let spec = ModeSpec::base();
    let state = deal(&spec);
    for team in state.teams() {
        for intent in bot::team_intents(&state, team) {
            assert_eq!(
                state.pawn(intent.pawn).team,
                team,
                "team {} was given an order for someone else's pawn",
                team.0
            );
        }
    }
}

// --- the architect ---------------------------------------------------------

fn architect_spec() -> ModeSpec {
    ModeSpec {
        architect: Some(0),
        hand_size: 7,
        plays_per_turn: 2,
        threats: vec![ThreatKind::None],
        mutation: MutationKind::Architect {
            rogue: crate::sim::mutation::Scope::Capped { base: 0, cap: 0 },
        },
        ..ModeSpec::plant()
    }
}

/// Declare plays, run a turn, and report what was refused and what landed.
fn lay(spec: &ModeSpec, plays: &[TilePlay]) -> (Vec<Refusal>, Vec<TilePlay>) {
    let rules = Rules::from_spec(spec);
    let mut state = deal(spec);
    state.architect_queue = plays.to_vec();
    let hold: Vec<Intent> = state
        .free_pawns()
        .map(|pawn| Intent {
            pawn: pawn.id,
            facing: pawn.facing,
            action: Action::Hold,
        })
        .collect();
    step(&mut state, &rules, &hold);
    (
        state.refusals.iter().map(|(_, why)| *why).collect(),
        state.report.tiles_played.clone(),
    )
}

#[test]
fn an_architect_may_not_rebuild_what_its_own_team_is_holding() {
    // Observe-to-freeze from the other side of the table. The operatives'
    // attention is the price of their safety, and it is paid to their own
    // architect as much as to anyone else's — which is what forces an architect
    // to build *ahead* of the squad rather than underneath it.
    let spec = architect_spec();
    let underfoot = TilePlay {
        cell: at(0, 4),
        shape: TileShape::Corridor,
        rotation: 1,
    };
    let (refused, played) = lay(&spec, &[underfoot]);
    assert_eq!(refused, vec![Refusal::Held]);
    assert!(played.is_empty());

    // Two hexes clear of the squad, the same tile lands.
    let ahead = TilePlay {
        cell: at(4, 4),
        shape: TileShape::Corridor,
        rotation: 1,
    };
    let (refused, played) = lay(&spec, &[ahead]);
    assert!(refused.is_empty(), "unexpected refusal: {refused:?}");
    assert_eq!(played, vec![ahead]);
}

#[test]
fn no_accepted_play_can_ever_strand_the_objective() {
    // Rather than hand-pick a cell that happens to be a bridge — which depends
    // on a board that rewires every turn — assert the invariant itself over
    // every cell and shape: whatever the architect is allowed to play, the
    // flags and the squad remain mutually reachable afterwards. A lab that can
    // deal an unwinnable match wastes the tester's time.
    let spec = ModeSpec {
        hand_size: 24,
        plays_per_turn: 1,
        ..architect_spec()
    };
    let rules = Rules::from_spec(&spec);

    for shape in TileShape::ALL {
        for rotation in 0..6_u8 {
            let mut state = deal(&spec);
            let cells: Vec<_> = state.board.cells().collect();
            for cell in cells {
                let mut trial = state.clone();
                trial.architect_queue = vec![TilePlay {
                    cell,
                    shape,
                    rotation,
                }];
                let hold: Vec<Intent> = trial
                    .free_pawns()
                    .map(|pawn| Intent {
                        pawn: pawn.id,
                        facing: pawn.facing,
                        action: Action::Hold,
                    })
                    .collect();
                step(&mut trial, &rules, &hold);
                if trial.report.tiles_played.is_empty() {
                    continue;
                }
                let anchor = trial.pawns[0].at;
                for flag in &trial.flags {
                    assert!(
                        trial.board.connected(anchor, flag.at),
                        "{shape:?} rot {rotation} at {:?} stranded a flag",
                        (cell.q, cell.r)
                    );
                }
            }
            state.architect_queue.clear();
        }
    }
}

#[test]
fn a_hand_is_a_cadence_and_the_allowance_is_enforced() {
    let spec = ModeSpec {
        plays_per_turn: 1,
        ..architect_spec()
    };
    let plays = [
        TilePlay {
            cell: at(4, 4),
            shape: TileShape::Corridor,
            rotation: 1,
        },
        TilePlay {
            cell: at(3, 4),
            shape: TileShape::Corridor,
            rotation: 1,
        },
    ];
    let (refused, played) = lay(&spec, &plays);
    assert_eq!(played.len(), 1, "only one play per turn");
    assert_eq!(refused, vec![Refusal::NoPlaysLeft]);
}

#[test]
fn a_tile_not_in_hand_cannot_be_played() {
    let spec = ModeSpec {
        hand_size: 1,
        ..architect_spec()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    let held = state.hands[0].cards[0];
    let absent = TileShape::ALL
        .into_iter()
        .find(|shape| *shape != held)
        .expect("more than one shape exists");
    state.architect_queue = vec![TilePlay {
        cell: at(4, 4),
        shape: absent,
        rotation: 0,
    }];
    step(&mut state, &rules, &[]);
    assert_eq!(
        state
            .refusals
            .iter()
            .map(|(_, why)| *why)
            .collect::<Vec<_>>(),
        vec![Refusal::NotInHand]
    );
}

#[test]
fn a_played_tile_is_marked_as_a_decision_and_churn_is_not() {
    // If a player cannot tell a deliberate play from background noise, an
    // architect is only an expensive random number generator. The source is
    // what the view draws differently, so it has to be right in the simulation.
    let spec = ModeSpec {
        mutation: MutationKind::Architect {
            rogue: crate::sim::mutation::Scope::Capped { base: 3, cap: 3 },
        },
        ..architect_spec()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    // Play something the hand actually holds; the deck is shuffled, so naming
    // a shape up front would be testing the draw rather than the rule.
    let card = state.hands[0].cards[0];
    state.architect_queue = vec![TilePlay {
        cell: at(4, 4),
        shape: card,
        rotation: 0,
    }];
    let hold: Vec<Intent> = state
        .free_pawns()
        .map(|pawn| Intent {
            pawn: pawn.id,
            facing: pawn.facing,
            action: Action::Hold,
        })
        .collect();
    step(&mut state, &rules, &hold);

    let mine: Vec<_> = state
        .telegraph
        .iter()
        .filter(|change| matches!(change.source, ChangeSource::Architect(_)))
        .collect();
    let rogue: Vec<_> = state
        .telegraph
        .iter()
        .filter(|change| change.source == ChangeSource::Rogue)
        .collect();
    assert!(
        state.refusals.is_empty(),
        "the play was refused: {:?}",
        state.refusals
    );
    assert!(!mine.is_empty(), "the play left no marked footprint");
    assert!(!rogue.is_empty(), "the rogue churn stopped running");
    // And the played footprint is contiguous around one cell, which is what
    // makes it read as intent rather than scatter.
    assert!(
        mine.iter()
            .all(|change| lateral_distance(change.edge.cell, at(4, 4)) <= 1),
        "a play's footprint should sit on the cell it was played at"
    );
}

#[test]
fn an_architect_plays_a_different_match_than_a_generator() {
    let spec = architect_spec();
    let plain = ModeSpec {
        architect: None,
        mutation: MutationKind::TelegraphedRewire {
            scope: crate::sim::mutation::Scope::Capped { base: 2, cap: 4 },
        },
        ..spec.clone()
    };
    assert_ne!(drive(&spec, 12), drive(&plain, 12));
}

#[test]
fn the_squad_earns_the_architects_cards() {
    // Nothing refills on a clock. An architect whose operatives achieve nothing
    // runs out of tiles, which is what makes the two seats need each other
    // rather than merely coexist.
    let spec = ModeSpec {
        hand_size: 3,
        plays_per_turn: 1,
        ..architect_spec()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);
    assert_eq!(state.hands[0].cards.len(), 3, "an opening hand is dealt");
    assert_eq!(state.hands[0].owed, 0, "and nothing is owed on top of it");

    // Spend one and idle: no clock draw puts it back.
    let card = state.hands[0].cards[0];
    state.architect_queue = vec![TilePlay {
        cell: at(4, 4),
        shape: card,
        rotation: 0,
    }];
    let hold: Vec<Intent> = state
        .free_pawns()
        .map(|pawn| Intent {
            pawn: pawn.id,
            facing: pawn.facing,
            action: Action::Hold,
        })
        .collect();
    step(&mut state, &rules, &hold);
    assert_eq!(
        state.hands[0].cards.len(),
        2,
        "a spent tile is not replaced for free"
    );

    // Planting pays. The pawn has to actually be on the flag for `claim` to do
    // anything, so put it there.
    let flag = state.flags[0].at;
    place(&mut state, &[(0, flag)]);
    let before = state.hands[0].cards.len();
    rules.objective.claim(&mut state, PawnId(0));
    assert!(
        state.hands[0].owed > 0 || state.hands[0].cards.len() > before,
        "planting a flag must pay the architect"
    );
}

#[test]
fn an_idle_squad_starves_its_architect() {
    // The end state the economy is meant to produce, asserted rather than
    // assumed: a squad that holds nothing and achieves nothing eventually
    // leaves its architect with no tiles at all.
    let spec = ModeSpec {
        hand_size: 2,
        plays_per_turn: 1,
        mutation: MutationKind::Architect {
            rogue: crate::sim::mutation::Scope::Capped { base: 0, cap: 0 },
        },
        ..architect_spec()
    };
    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);

    for _ in 0..6 {
        if let Some(&card) = state.hands[0].cards.first() {
            state.architect_queue = vec![TilePlay {
                cell: at(4, 4),
                shape: card,
                rotation: 0,
            }];
        }
        let hold: Vec<Intent> = state
            .free_pawns()
            .map(|pawn| Intent {
                pawn: pawn.id,
                facing: pawn.facing,
                action: Action::Hold,
            })
            .collect();
        step(&mut state, &rules, &hold);
    }
    assert!(
        state.hands[0].cards.is_empty(),
        "an architect spending on an idle squad should run dry, still holds {:?}",
        state.hands[0].cards
    );
}
