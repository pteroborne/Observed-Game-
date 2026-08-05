//! Stateful, non-authoritative ownership for objective-bot routing and local traversal.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexCoord, HexRoute};
use observed_hex::hex_origin;
use observed_traversal::{
    FollowState, FollowTarget, FollowerPose, TraversalDirection, follow_stateless,
};
use player_input::PlayerIntent;

use super::super::objectives::HexObjectiveTarget;
use super::super::{HexMatchEventKind, HexPlayerCommand, HexWfcMatch};
use super::{BotBehaviour, STUCK_ENTER_TICKS, steer_toward};

/// Stable identity for one projected traversal module.
///
/// A facility-wide generation is deliberately not part of the identity: a
/// bounded relayout elsewhere must not revoke a bot's local traversal lease.
/// The projection replaces the guide at `source_cell` atomically when that
/// module itself changes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModuleInstanceId {
    pub source_cell: HexCoord,
}

/// The route transition a bot committed to when it entered an authored climb.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraversalLease {
    pub instance: ModuleInstanceId,
    pub route_from: HexCoord,
    pub route_to: HexCoord,
    pub objective: HexCoord,
    pub direction: TraversalDirection,
}

/// Stateful local progress through a leased traversal guide.
///
/// `state` is diagnostic; the lease direction is the important retained fact.
/// Logical-cell height rounding may change the route cell before the capsule
/// reaches the authored terminal, but it cannot reverse this direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraversalCursor {
    pub lease: TraversalLease,
    pub state: FollowState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BotRouteCache {
    generation: u32,
    target: HexCoord,
    from: HexCoord,
    route: Option<HexRoute>,
}

/// Non-authoritative, locally owned bot driving state.
///
/// The match remains replay/snapshot authoritative. Hosts own one driver next
/// to a match and may discard it on reconstruction: cached routes are derived,
/// while a traversal cursor is reacquired from stable logical geometry.
#[derive(Clone, Debug, Default)]
pub struct HexBotDriver {
    routes: BTreeMap<PlayerId, BotRouteCache>,
    cursors: BTreeMap<PlayerId, TraversalCursor>,
}

impl HexBotDriver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget all derived state, for example when the host replaces a match
    /// during resynchronization or starts a rematch.
    pub fn reset(&mut self) {
        self.routes.clear();
        self.cursors.clear();
    }

    /// Forget one bot's local state after a seat/driver ownership change.
    pub fn clear_player(&mut self, id: PlayerId) {
        self.routes.remove(&id);
        self.cursors.remove(&id);
    }

    #[must_use]
    pub fn cursor(&self, id: PlayerId) -> Option<&TraversalCursor> {
        self.cursors.get(&id)
    }

    #[cfg(test)]
    #[must_use]
    pub(in crate::hex_wfc::model) fn has_cached_route(&self, id: PlayerId) -> bool {
        self.routes.contains_key(&id)
    }

    #[cfg(test)]
    #[must_use]
    pub(in crate::hex_wfc::model) fn cached_route_generation(&self, id: PlayerId) -> Option<u32> {
        self.routes.get(&id).map(|cache| cache.generation)
    }

    /// Complete abstract command for one bot this tick.
    ///
    /// Calling this more than once against the same match tick is idempotent:
    /// event/delta invalidation and lease acquisition depend only on stable
    /// domain data, so same-tick consumers receive the same command.
    #[must_use]
    pub fn command(&mut self, game: &HexWfcMatch, id: PlayerId) -> HexPlayerCommand {
        let target = game.objective_target(id);
        self.invalidate_from_match(game, id, target);
        let actions = game.bot_action_buttons_for_target(id, target);
        let intent = target.map_or_else(PlayerIntent::default, |target| {
            self.cached_bot_command(game, id, target)
        });
        HexPlayerCommand { intent, actions }
    }

    fn invalidate_from_match(
        &mut self,
        game: &HexWfcMatch,
        id: PlayerId,
        target: Option<HexObjectiveTarget>,
    ) {
        let was_displaced = game.recent_events.iter().any(|event| {
            event.player == Some(id)
                && matches!(
                    event.kind,
                    HexMatchEventKind::PlayerRecovered
                        | HexMatchEventKind::GuardianCatch
                        | HexMatchEventKind::PlayerEscaped
                )
        });
        let absent_or_escaped = game.players.get(&id).is_none_or(|player| player.escaped);
        if was_displaced || absent_or_escaped {
            self.clear_player(id);
            return;
        }

        let objective = target.map(|target| target.cell);
        if self
            .cursors
            .get(&id)
            .is_some_and(|cursor| Some(cursor.lease.objective) != objective)
        {
            self.cursors.remove(&id);
        }
        if self
            .routes
            .get(&id)
            .is_some_and(|cache| Some(cache.target) != objective)
        {
            self.routes.remove(&id);
        }

        if let Some(delta) = &game.last_relayout_delta {
            self.cursors.retain(|_, cursor| {
                !delta
                    .changed_cells
                    .contains(&cursor.lease.instance.source_cell)
            });
        }
    }

    fn cached_bot_command(
        &mut self,
        game: &HexWfcMatch,
        id: PlayerId,
        target: HexObjectiveTarget,
    ) -> PlayerIntent {
        let Some(player) = game.players.get(&id) else {
            return PlayerIntent::default();
        };

        // The lease precedes logical-cell shortcuts. Height rounding can report
        // the upper cell while the body is still on the final sloped tread.
        if let Some(intent) = self.follow_cursor(game, id) {
            return game.apply_unstick(id, intent);
        }

        if target.cell == player.cell {
            return if player.position.distance(target.position) > 0.75 {
                steer_toward(player.yaw, player.position, target.position)
            } else {
                PlayerIntent::default()
            };
        }
        let current = player.cell;
        let generation = game.facility.generation;
        let cache_is_usable = self.routes.get(&id).is_some_and(|cache| {
            cache.generation == generation
                && cache.target == target.cell
                && match &cache.route {
                    Some(route) => route
                        .cells
                        .iter()
                        .position(|cell| *cell == current)
                        .is_some_and(|index| index + 1 < route.cells.len()),
                    None => cache.from == current,
                }
        });
        if !cache_is_usable {
            let route = game.facility.route_between_cells(current, target.cell);
            self.routes.insert(
                id,
                BotRouteCache {
                    generation,
                    target: target.cell,
                    from: current,
                    route,
                },
            );
        }
        let route = self.routes.get(&id).and_then(|cache| cache.route.clone());
        let behaviour = if route.as_ref().is_none_or(|route| route.cells.len() <= 1) {
            BotBehaviour::Explore
        } else if game.stuck_ticks.get(&id).copied().unwrap_or(0) >= STUCK_ENTER_TICKS {
            BotBehaviour::Recover
        } else {
            BotBehaviour::Seek
        };
        self.command_for_behaviour(game, id, target.cell, behaviour, route.as_ref())
    }

    fn command_for_behaviour(
        &mut self,
        game: &HexWfcMatch,
        id: PlayerId,
        objective: HexCoord,
        behaviour: BotBehaviour,
        route: Option<&HexRoute>,
    ) -> PlayerIntent {
        let Some(player) = game.players.get(&id) else {
            return PlayerIntent::default();
        };
        match behaviour {
            BotBehaviour::Idle => return PlayerIntent::default(),
            BotBehaviour::Explore => return game.explore_command(player),
            BotBehaviour::Seek | BotBehaviour::Recover => {}
        }
        let route = route.expect("Seek/Recover imply a route");
        let Some(index) = route.cells.iter().position(|cell| *cell == player.cell) else {
            return PlayerIntent::default();
        };
        let Some(&next) = route.cells.get(index + 1) else {
            return PlayerIntent::default();
        };
        let base = if next.level != player.cell.level {
            if let Some(lease) = game.traversal_lease(player, next, objective) {
                self.cursors.insert(
                    id,
                    TraversalCursor {
                        lease,
                        state: FollowState::Unavailable,
                    },
                );
                self.follow_cursor(game, id).unwrap_or_else(|| {
                    game.vertical_command(player.cell, player.yaw, player.position, next)
                })
            } else {
                game.vertical_command(player.cell, player.yaw, player.position, next)
            }
        } else if let Some(command) =
            game.finish_stair_command(player.cell, player.yaw, player.position)
        {
            command
        } else if let Some(command) =
            game.stair_lateral_command(player.cell, next, player.yaw, player.position)
        {
            command
        } else {
            let target = game
                .lateral_waypoint(player.cell, next, player.position)
                .unwrap_or_else(|| Vec3::from_array(hex_origin(next)));
            steer_toward(player.yaw, player.position, target)
        };
        game.apply_unstick(id, base)
    }

    fn follow_cursor(&mut self, game: &HexWfcMatch, id: PlayerId) -> Option<PlayerIntent> {
        let lease = self.cursors.get(&id)?.lease;
        let Some(guide) = game.geometry.guides.get(&lease.instance.source_cell) else {
            self.cursors.remove(&id);
            return None;
        };
        let Some(spine) = guide.climb.as_ref() else {
            self.cursors.remove(&id);
            return None;
        };
        let player = game.players.get(&id)?;
        let feet = player.position.y
            - game
                .content
                .traversal_profile()
                .requirements()
                .capsule_half_height;
        let decision = follow_stateless(
            FollowerPose {
                feet: Vec3::new(player.position.x, feet, player.position.z),
                yaw: player.yaw,
            },
            FollowTarget::Climb {
                spine,
                approach: guide.deck.as_ref(),
                direction: lease.direction,
            },
            game.content.traversal_profile(),
        );
        match decision.state {
            FollowState::PassedClimbTerminal | FollowState::Unavailable => {
                self.cursors.remove(&id);
                None
            }
            state => {
                self.cursors
                    .get_mut(&id)
                    .expect("cursor remains while following")
                    .state = state;
                decision.intent
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use observed_facility::hex_wfc::{HexMutationRegion, HexRelayoutDelta, HexWfcConfig};

    use super::*;
    use crate::hex_wfc::{
        HEX_INPUT_VERSION, HexInputFrame, HexMatchConfig, HexMatchEvent, HexWfcMatch,
    };

    const GATE_SEED: u64 = 0xa11c_0000_0000_0000;

    fn gate_config() -> HexWfcConfig {
        HexWfcConfig {
            cols: 12,
            rows: 9,
            levels: 5,
            min_rooms: 4,
            max_rooms: 8,
            retry_budget: 100,
            min_room_distance: 2,
        }
    }

    fn leased_fixture() -> (
        HexWfcMatch,
        HexBotDriver,
        PlayerId,
        HexCoord,
        HexCoord,
        HexPlayerCommand,
    ) {
        let mut game = HexWfcMatch::new_with_content(
            GATE_SEED,
            HexMatchConfig {
                guardian: false,
                teams: 1,
                members_per_team: 1,
                wfc: gate_config(),
            },
            Arc::clone(crate::hex_wfc::compatibility_test_content()),
        )
        .expect("gate fixture builds");
        game.objectives.enabled = false;
        let exit = game.facility.config.exit();
        let (from, to, feet) = game
            .geometry
            .guides
            .iter()
            .filter(|(_, guide)| guide.climb.as_ref().is_some_and(|spine| !spine.is_empty()))
            .find_map(|(&source, guide)| {
                let route = game.facility.route_between_cells(source, exit)?;
                let &next = route.cells.get(1)?;
                let feet = guide.climb.as_ref()?.nodes.first().copied()?;
                (next.level > source.level).then_some((source, next, feet))
            })
            .expect("gate route exposes an ascending projected climb");
        let half_height = game
            .content()
            .traversal_profile()
            .requirements()
            .capsule_half_height;
        let player = game.players.get_mut(&PlayerId(0)).expect("solo player");
        player.cell = from;
        player.position = feet + Vec3::Y * half_height;

        let id = PlayerId(0);
        let compatibility = game.bot_player_command(id);
        let mut driver = HexBotDriver::new();
        let first = driver.command(&game, id);
        assert_eq!(first, compatibility, "first-tick extraction must be exact");
        assert!(
            driver.cursor(id).is_some(),
            "the fixture must acquire a lease"
        );
        (game, driver, id, from, to, first)
    }

    fn delta(changed_cells: BTreeSet<HexCoord>, generation: u32) -> HexRelayoutDelta {
        HexRelayoutDelta {
            previous_generation: generation.saturating_sub(1),
            generation,
            previous_attempts: 0,
            region: HexMutationRegion {
                cells: changed_cells.clone(),
                boundary_cells: BTreeSet::new(),
                protected_cells: BTreeSet::new(),
            },
            changed_cells,
            placements: BTreeMap::new(),
            architecture: BTreeMap::new(),
            cell_revisions: BTreeMap::new(),
            previous_placements: BTreeMap::new(),
            previous_architecture: BTreeMap::new(),
            previous_cell_revisions: BTreeMap::new(),
            previous_blueprints: Vec::new(),
            removed_blueprints: Vec::new(),
            upserted_blueprints: Vec::new(),
        }
    }

    #[test]
    fn same_tick_and_logical_level_perturbation_retain_the_committed_direction() {
        let (mut game, mut driver, id, _from, to, first) = leased_fixture();
        let initial = *driver.cursor(id).expect("cursor");

        assert_eq!(driver.command(&game, id), first);
        assert_eq!(driver.cursor(id), Some(&initial));

        // The body has not moved, but height rounding reports the destination
        // logical level early. The retained lease must emit the same local
        // command instead of reinterpreting the route in reverse.
        game.players.get_mut(&id).expect("player").cell = to;
        assert_eq!(driver.command(&game, id), first);
        assert_eq!(
            driver.cursor(id).expect("cursor retained").lease.direction,
            TraversalDirection::Forward
        );
    }

    #[test]
    fn relayout_invalidation_is_scoped_to_the_leased_module() {
        let (mut game, mut driver, id, source, _to, _first) = leased_fixture();
        let unrelated = HexCoord {
            q: source.q.saturating_add(100),
            ..source
        };
        game.facility.generation = game.facility.generation.wrapping_add(1);
        game.last_relayout_delta =
            Some(delta(BTreeSet::from([unrelated]), game.facility.generation));
        let _ = driver.command(&game, id);
        assert!(
            driver.cursor(id).is_some(),
            "unrelated relayout keeps lease"
        );

        game.last_relayout_delta = Some(delta(BTreeSet::from([source]), game.facility.generation));
        driver.invalidate_from_match(&game, id, game.objective_target(id));
        assert!(driver.cursor(id).is_none(), "source relayout revokes lease");
    }

    #[test]
    fn controller_recovery_emits_once_and_revokes_the_stale_lease() {
        let (mut game, mut driver, id, _source, _to, _first) = leased_fixture();
        assert!(driver.cursor(id).is_some(), "fixture begins with a lease");
        assert!(driver.has_cached_route(id), "fixture begins with a route");

        // Keep the body's valid spawn but put both authoritative views beyond
        // the collision scene's safety volume. Matching positions prevent the
        // teleport reconciler from replacing the body before the KCC sees it.
        let outside = Vec3::splat(1_000_000.0);
        game.players.get_mut(&id).expect("player").position = outside;
        game.bodies.get_mut(&id).expect("body").position = outside;
        let events = game
            .step(&HexInputFrame {
                version: HEX_INPUT_VERSION,
                tick: game.tick + 1,
                commands: BTreeMap::new(),
            })
            .to_vec();

        assert_eq!(
            events
                .iter()
                .filter(|event| {
                    event.kind == HexMatchEventKind::PlayerRecovered && event.player == Some(id)
                })
                .count(),
            1,
            "the controller reset must be reported exactly once"
        );
        driver.invalidate_from_match(&game, id, game.objective_target(id));
        assert!(
            driver.cursor(id).is_none(),
            "recovery revokes the old lease"
        );
        assert!(
            !driver.has_cached_route(id),
            "recovery revokes the old route cache"
        );
    }

    #[test]
    fn displacement_escape_and_target_changes_revoke_only_the_affected_cursor() {
        for kind in [
            HexMatchEventKind::PlayerRecovered,
            HexMatchEventKind::GuardianCatch,
            HexMatchEventKind::PlayerEscaped,
        ] {
            let (mut game, mut driver, id, _source, _to, _first) = leased_fixture();
            let other = PlayerId(1);
            driver
                .cursors
                .insert(other, *driver.cursor(id).expect("cursor"));
            game.recent_events = vec![HexMatchEvent {
                tick: game.tick,
                kind,
                player: Some(id),
                cell: Some(game.players[&id].cell),
            }];
            driver.invalidate_from_match(&game, id, game.objective_target(id));
            assert!(driver.cursor(id).is_none(), "{kind:?} revokes affected bot");
            assert!(
                driver.cursor(other).is_some(),
                "{kind:?} preserves other bot"
            );
        }

        let (game, mut driver, id, _source, _to, _first) = leased_fixture();
        driver
            .cursors
            .get_mut(&id)
            .expect("cursor")
            .lease
            .objective
            .q += 1;
        driver.invalidate_from_match(&game, id, game.objective_target(id));
        assert!(driver.cursor(id).is_none(), "target change revokes lease");
    }

    #[test]
    fn disappearing_projected_guide_clears_a_stale_cursor() {
        let (mut game, mut driver, id, source, _to, _first) = leased_fixture();
        game.geometry.guides.remove(&source);
        assert_eq!(driver.follow_cursor(&game, id), None);
        assert!(driver.cursor(id).is_none());
    }

    #[test]
    fn recorded_driver_frames_replay_without_driver_state() {
        let (mut live, mut driver, id, _source, _to, _first) = leased_fixture();
        let mut replay = live.clone();
        let mut frames = Vec::new();
        let mut expected = Vec::new();
        assert!(
            driver.cursor(id).is_some(),
            "test must exercise a real cursor"
        );

        for _ in 0..4 {
            let frame = HexInputFrame {
                version: HEX_INPUT_VERSION,
                tick: live.tick + 1,
                commands: BTreeMap::from([(id, driver.command(&live, id))]),
            };
            live.step(&frame);
            expected.push(live.snapshot());
            frames.push(frame);
        }

        for (frame, expected) in frames.iter().zip(expected) {
            replay.step(frame);
            assert_eq!(replay.snapshot(), expected);
        }
    }
}
