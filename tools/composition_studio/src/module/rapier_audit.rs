//! Optional selected-module traversal through the production follower and Rapier.
//!
//! This is deliberately not another route planner. It enumerates only the
//! climb/deck legs declared by a [`ProjectedTraversalGuide`], feeds the exact
//! [`PlayerIntent`](player_input::PlayerIntent) emitted by `follow_stateless`
//! into `step_character`, and reports what happened. Modules without a guide
//! are not applicable; the geometric preflight remains their fast diagnostic.

use bevy::prelude::*;
use observed_authoring::TilePrototype;
use observed_hex::HexCoord;
use observed_match::hex_wfc::ProjectedTraversalGuide;
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{
    DeckHandoff, FIXED_DT, FollowState, FollowTarget, FollowerPose, FpsBody, TraversalDirection,
    TraversalRuntimeProfile, follow_stateless,
};

const MAX_TICKS_PER_LEG: u32 = 6_000;
const TRACE_EVERY_TICKS: u32 = 12;
const HANDOFF_DISTANCE: f32 = 1.0;

/// Whether the optional selected-module audit is active and what it found.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum RapierAuditState {
    #[default]
    Off,
    NotApplicable,
    Complete(RapierAuditReport),
}

impl RapierAuditState {
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// Which declared compatibility annotation one physical run exercised.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuideLegKind {
    Climb,
    Deck,
}

/// Why a declared leg did not complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RapierAuditFailure {
    FollowerUnavailable,
    NonFiniteBody,
    PlayerRecovered,
    TimedOut,
}

/// Result of one direction through one declared guide leg.
#[derive(Clone, Debug, PartialEq)]
pub struct RapierLegReport {
    pub kind: GuideLegKind,
    pub direction: TraversalDirection,
    pub ticks: u32,
    pub final_state: FollowState,
    pub final_feet: Vec3,
    pub last_target: Option<Vec3>,
    pub trace: Vec<Vec3>,
    pub failure: Option<RapierAuditFailure>,
}

impl RapierLegReport {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failure.is_none()
    }
}

/// Exact runtime identity and all declared-leg results for one selected module.
#[derive(Clone, Debug, PartialEq)]
pub struct RapierAuditReport {
    pub profile_hash: [u8; 32],
    pub legs: Vec<RapierLegReport>,
}

impl RapierAuditReport {
    #[must_use]
    pub fn passed(&self) -> bool {
        !self.legs.is_empty() && self.legs.iter().all(RapierLegReport::passed)
    }

    #[must_use]
    pub fn profile_hash_prefix(&self) -> String {
        self.profile_hash[..6]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// Identity-project a standalone authored module into the same atomic guide
/// value the facility projection owns. No route is inferred here.
#[must_use]
pub fn projected_guide(prototype: &TilePrototype) -> Option<ProjectedTraversalGuide> {
    let climb = (!prototype.spine.is_empty()).then(|| prototype.spine.clone());
    let deck = (!prototype.deck.is_empty()).then(|| prototype.deck.clone());
    (climb.is_some() || deck.is_some()).then_some(ProjectedTraversalGuide {
        source_cell: HexCoord {
            q: 0,
            r: 0,
            level: 0,
        },
        climb,
        deck,
    })
}

/// Run every direction declared by an already-projected guide.
#[must_use]
pub fn audit_projected_guide(
    prototype: &TilePrototype,
    guide: &ProjectedTraversalGuide,
) -> RapierAuditReport {
    let profile = TraversalRuntimeProfile::canonical_hex();
    let scene = RapierTraversalScene::from_arena_spec(&prototype.arena_spec());
    let mut legs = Vec::new();
    if let Some(spine) = guide.climb.as_ref() {
        for direction in [TraversalDirection::Forward, TraversalDirection::Reverse] {
            legs.push(audit_climb(
                &scene,
                spine,
                guide.deck.as_ref(),
                direction,
                &profile,
            ));
        }
    }
    if let Some(deck) = guide.deck.as_ref() {
        for direction in [TraversalDirection::Forward, TraversalDirection::Reverse] {
            legs.push(audit_deck(&scene, deck, direction, &profile));
        }
    }
    RapierAuditReport {
        profile_hash: profile.profile_hash(),
        legs,
    }
}

fn audit_climb(
    scene: &RapierTraversalScene,
    spine: &observed_traversal::StairSpine,
    approach: Option<&observed_traversal::DeckPath>,
    direction: TraversalDirection,
    profile: &TraversalRuntimeProfile,
) -> RapierLegReport {
    let start = match direction {
        TraversalDirection::Forward => spine.nodes.first(),
        TraversalDirection::Reverse => spine.nodes.last(),
    }
    .copied()
    .expect("projected climb guides are non-empty");
    run_leg(
        scene,
        GuideLegKind::Climb,
        direction,
        start,
        profile,
        |pose| {
            follow_stateless(
                pose,
                FollowTarget::Climb {
                    spine,
                    approach,
                    direction,
                },
                profile,
            )
        },
        |state| state == FollowState::PassedClimbTerminal,
    )
}

fn audit_deck(
    scene: &RapierTraversalScene,
    deck: &observed_traversal::DeckPath,
    direction: TraversalDirection,
    profile: &TraversalRuntimeProfile,
) -> RapierLegReport {
    let nodes = &deck.nodes;
    let (start, before_goal, goal) = match direction {
        TraversalDirection::Forward => (nodes[0], nodes[nodes.len() - 2], nodes[nodes.len() - 1]),
        TraversalDirection::Reverse => (nodes[nodes.len() - 1], nodes[1], nodes[0]),
    };
    let outward = Vec2::new(goal.x - before_goal.x, goal.z - before_goal.z).normalize_or_zero();
    let destination = goal + Vec3::new(outward.x, 0.0, outward.y) * HANDOFF_DISTANCE;
    let handoff = DeckHandoff {
        threshold: goal,
        outward,
        destination,
    };
    run_leg(
        scene,
        GuideLegKind::Deck,
        direction,
        start,
        profile,
        |pose| {
            follow_stateless(
                pose,
                FollowTarget::Deck {
                    path: deck,
                    goal,
                    handoff: Some(handoff),
                },
                profile,
            )
        },
        |state| state == FollowState::PastDeckHandoff,
    )
}

fn run_leg(
    scene: &RapierTraversalScene,
    kind: GuideLegKind,
    direction: TraversalDirection,
    start_feet: Vec3,
    profile: &TraversalRuntimeProfile,
    decide: impl Fn(FollowerPose) -> observed_traversal::FollowDecision,
    completed: impl Fn(FollowState) -> bool,
) -> RapierLegReport {
    let config = profile.controller();
    let spawn = start_feet + Vec3::Y * config.half_height;
    let pristine = FpsBody::spawned(spawn, 0.0);
    let mut body = pristine;
    let mut trace = vec![start_feet];
    let mut last_state = FollowState::Unavailable;
    let mut last_target = None;

    for tick in 0..MAX_TICKS_PER_LEG {
        let feet = body.position - Vec3::Y * config.half_height;
        let decision = decide(FollowerPose {
            feet,
            yaw: body.yaw,
        });
        last_state = decision.state;
        last_target = decision.target;
        if completed(decision.state) {
            push_final_trace(&mut trace, feet);
            return leg_report(
                kind,
                direction,
                tick,
                decision.state,
                feet,
                decision.target,
                trace,
                None,
            );
        }
        let Some(intent) = decision.intent else {
            push_final_trace(&mut trace, feet);
            return leg_report(
                kind,
                direction,
                tick,
                decision.state,
                feet,
                decision.target,
                trace,
                Some(RapierAuditFailure::FollowerUnavailable),
            );
        };

        step_character(scene, &mut body, intent, &config, FIXED_DT);
        let feet = body.position - Vec3::Y * config.half_height;
        if tick % TRACE_EVERY_TICKS == 0 {
            trace.push(feet);
        }
        if !body.position.is_finite() || !body.velocity.is_finite() {
            push_final_trace(&mut trace, feet);
            return leg_report(
                kind,
                direction,
                tick + 1,
                decision.state,
                feet,
                decision.target,
                trace,
                Some(RapierAuditFailure::NonFiniteBody),
            );
        }
        // `step_character` resets a body that leaves the scene safety volume.
        // Treat that exact reset as a failed audit and stop immediately; an
        // audit never turns recovery into apparent traversal progress.
        if body == pristine {
            push_final_trace(&mut trace, feet);
            return leg_report(
                kind,
                direction,
                tick + 1,
                decision.state,
                feet,
                decision.target,
                trace,
                Some(RapierAuditFailure::PlayerRecovered),
            );
        }
    }

    let feet = body.position - Vec3::Y * config.half_height;
    push_final_trace(&mut trace, feet);
    leg_report(
        kind,
        direction,
        MAX_TICKS_PER_LEG,
        last_state,
        feet,
        last_target,
        trace,
        Some(RapierAuditFailure::TimedOut),
    )
}

#[allow(clippy::too_many_arguments)]
fn leg_report(
    kind: GuideLegKind,
    direction: TraversalDirection,
    ticks: u32,
    final_state: FollowState,
    final_feet: Vec3,
    last_target: Option<Vec3>,
    trace: Vec<Vec3>,
    failure: Option<RapierAuditFailure>,
) -> RapierLegReport {
    RapierLegReport {
        kind,
        direction,
        ticks,
        final_state,
        final_feet,
        last_target,
        trace,
        failure,
    }
}

fn push_final_trace(trace: &mut Vec<Vec3>, feet: Vec3) {
    if trace.last() != Some(&feet) {
        trace.push(feet);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use observed_authoring::load_tile;

    use super::*;

    fn tile(stem: &str) -> TilePrototype {
        load_tile(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/tiles/authored")
                .join(format!("{stem}.map")),
        )
        .expect("authored tile parses")
    }

    #[test]
    fn guide_less_modules_report_not_applicable() {
        assert!(projected_guide(&tile("hall_straight")).is_none());
    }

    #[test]
    fn perimeter_ramp_walks_its_declared_climb_in_both_directions() {
        let tile = tile("hall_ramp_perimeter_120");
        let guide = projected_guide(&tile).expect("perimeter ramp has a guide");
        let report = audit_projected_guide(&tile, &guide);

        assert_eq!(report.legs.len(), 2, "one climb in both directions");
        assert!(report.passed(), "{:#?}", report.legs);
        assert_eq!(report.legs[0].direction, TraversalDirection::Forward);
        assert_eq!(report.legs[1].direction, TraversalDirection::Reverse);
        assert!(
            report
                .legs
                .iter()
                .all(|leg| leg.final_state == FollowState::PassedClimbTerminal)
        );
    }

    #[test]
    fn repeated_perimeter_ramp_audits_are_bit_identical() {
        let tile = tile("hall_ramp_perimeter_120");
        let guide = projected_guide(&tile).expect("perimeter ramp has a guide");
        let first = audit_projected_guide(&tile, &guide);
        let second = audit_projected_guide(&tile, &guide);

        assert_eq!(first, second);
    }

    #[test]
    fn declared_climb_and_deck_legs_run_forward_and_reverse() {
        let tile = tile("stair_tower_helix_0");
        let guide = projected_guide(&tile).expect("tower has climb and deck guides");
        let report = audit_projected_guide(&tile, &guide);

        assert_eq!(
            report
                .legs
                .iter()
                .map(|leg| (leg.kind, leg.direction))
                .collect::<Vec<_>>(),
            vec![
                (GuideLegKind::Climb, TraversalDirection::Forward),
                (GuideLegKind::Climb, TraversalDirection::Reverse),
                (GuideLegKind::Deck, TraversalDirection::Forward),
                (GuideLegKind::Deck, TraversalDirection::Reverse),
            ]
        );
        assert!(report.passed(), "{:#?}", report.legs);
    }
}
