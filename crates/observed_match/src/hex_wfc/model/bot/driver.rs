//! Stateful, non-authoritative ownership for objective-bot routing and local traversal.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexCoord, HexRoute};
use observed_hex::hex_origin;
use observed_traversal::GraphFollowState;
use player_input::PlayerIntent;

use crate::hex_wfc::HexTraversalCursor;

use super::super::objectives::HexObjectiveTarget;
use super::super::{HexMatchEventKind, HexPlayerCommand, HexWfcMatch};
use super::leg::{self, ExternalTransition};
use super::{BotBehaviour, STUCK_ENTER_TICKS, steer_toward};

/// One held graph leg and the objective it was committed to.
///
/// The objective lives here rather than in [`HexTraversalLease`] because a lease
/// is a contract about a module, not about why a bot wanted to cross it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct GraphLeg {
    objective: HexCoord,
    cursor: HexTraversalCursor,
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
/// while a graph leg is reacquired from stable logical geometry.
#[derive(Clone, Debug, Default)]
pub struct HexBotDriver {
    routes: BTreeMap<PlayerId, BotRouteCache>,
    /// Graph legs held over the module a bot is currently crossing. One map,
    /// so there is one answer to "may this bot still be mid-leg?".
    legs: BTreeMap<PlayerId, GraphLeg>,
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
        self.legs.clear();
    }

    /// Forget one bot's local state after a seat/driver ownership change.
    pub fn clear_player(&mut self, id: PlayerId) {
        self.routes.remove(&id);
        self.legs.remove(&id);
    }

    /// The graph leg this bot is executing across its current module.
    #[must_use]
    pub fn leg(&self, id: PlayerId) -> Option<&HexTraversalCursor> {
        self.legs.get(&id).map(|leg| &leg.cursor)
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
            .legs
            .get(&id)
            .is_some_and(|leg| Some(leg.objective) != objective)
        {
            self.legs.remove(&id);
        }
        if self
            .routes
            .get(&id)
            .is_some_and(|cache| Some(cache.target) != objective)
        {
            self.routes.remove(&id);
        }

        if let Some(delta) = &game.last_relayout_delta {
            // A graph leg names the exact module revision it was taken against,
            // so `leg::follow` rejects a replaced module on its own. Dropping
            // the obviously dead ones here keeps invalidation scoped to the
            // module that actually changed rather than to the whole facility.
            self.legs.retain(|_, leg| {
                !delta
                    .changed_cells
                    .contains(&leg.cursor.lease.instance.source_cell)
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

        // A held leg precedes every logical-cell shortcut below. Height rounding
        // can report the upper cell while the body is still on the final sloped
        // tread; a leg may not be reinterpreted from that.
        if let Some(intent) = self.follow_leg(game, id) {
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

        // The only question the facility planner is asked: which external port
        // comes next. A module that ships its own traversal graph answers the
        // rest by acquiring the matching local leg — no branch on archetype,
        // port class, or authored shape, which is the property a new
        // graph-shaped fixture relies on.
        if let Some(transition) = ExternalTransition::between(game, player.cell, next)
            && leg::ships_a_graph(game, transition.from)
            && let Some(intent) = self.acquire_leg(game, id, objective, transition)
        {
            return game.apply_unstick(id, intent);
        }

        // Everything a module declares is executed above. What is left is the
        // one shape the corpus still describes only by its name: an
        // unannotated ramp, which no leg can cross because it ships nothing to
        // cross it by.
        let base = if next.level != player.cell.level {
            game.unannotated_ramp_command(player.cell, player.yaw, player.position, next)
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

    /// Take the local leg that crosses this module toward the requested port,
    /// then execute its first tick.
    ///
    /// Re-acquisition is idempotent while a leg is already held: the held
    /// cursor answers first, so asking again on the same tick — or after the
    /// body's logical cell has moved on — cannot rewind the edge under it.
    fn acquire_leg(
        &mut self,
        game: &HexWfcMatch,
        id: PlayerId,
        objective: HexCoord,
        transition: ExternalTransition,
    ) -> Option<PlayerIntent> {
        if self.legs.contains_key(&id) {
            return self.follow_leg(game, id);
        }
        let player = game.players.get(&id)?;
        let pose = leg::pose(game, player.position, player.yaw);
        let cursor = leg::acquire(game, pose.feet, transition)?;
        self.legs.insert(id, GraphLeg { objective, cursor });
        self.follow_leg(game, id)
    }

    /// Advance a held graph leg. Anything other than continued following —
    /// arrival, leaving the graph, or a replaced module — releases the leg and
    /// hands the tick back to the caller.
    fn follow_leg(&mut self, game: &HexWfcMatch, id: PlayerId) -> Option<PlayerIntent> {
        let mut held = self.legs.get(&id)?.clone();
        let player = game.players.get(&id)?;
        let pose = leg::pose(game, player.position, player.yaw);
        let decision = leg::follow(game, &mut held.cursor, pose);
        if decision.state == GraphFollowState::Following {
            self.legs.insert(id, held);
            decision.intent
        } else {
            self.legs.remove(&id);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use observed_facility::hex_wfc::{HexMutationRegion, HexRelayoutDelta, HexWfcConfig};
    use observed_hex::HexFace;

    use super::super::leg::ResolvedModuleGraph;
    use super::*;
    use crate::hex_wfc::{
        HEX_INPUT_VERSION, HexInputFrame, HexMatchConfig, HexMatchEvent, HexWfcMatch, ProjectedPort,
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
        let stateless = game.bot_player_command(id);
        let mut driver = HexBotDriver::new();
        let first = driver.command(&game, id);
        assert_eq!(first, stateless, "first-tick extraction must be exact");
        // "Ascending" is the fact these tests retain across every perturbation
        // below, so say it in the terms the leg is actually expressed in: the
        // leased exit is the module's `Up` port, not its `Down` one.
        assert_eq!(
            Some(
                driver
                    .leg(id)
                    .expect("the fixture must acquire a leg")
                    .lease
                    .exit
            ),
            ResolvedModuleGraph::resolve(&game, from)
                .expect("the leased module resolves")
                .graph
                .port_bindings
                .get(&ProjectedPort {
                    cell: from,
                    face: HexFace::Up,
                })
                .copied(),
            "the fixture must lease the ascending leg"
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
        let initial = driver.leg(id).expect("leg").clone();

        assert_eq!(driver.command(&game, id), first);
        assert_eq!(driver.leg(id), Some(&initial));

        // The body has not moved, but height rounding reports the destination
        // logical level early. The retained leg must emit the same local
        // command instead of reinterpreting the route in reverse.
        game.players.get_mut(&id).expect("player").cell = to;
        assert_eq!(driver.command(&game, id), first);
        assert_eq!(
            driver.leg(id).expect("leg retained").lease,
            initial.lease,
            "the committed module and its ascending exit must survive the rounding"
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
        assert!(driver.leg(id).is_some(), "unrelated relayout keeps lease");

        game.last_relayout_delta = Some(delta(BTreeSet::from([source]), game.facility.generation));
        driver.invalidate_from_match(&game, id, game.objective_target(id));
        assert!(driver.leg(id).is_none(), "source relayout revokes lease");
    }

    #[test]
    fn controller_recovery_emits_once_and_revokes_the_stale_lease() {
        let (mut game, mut driver, id, _source, _to, _first) = leased_fixture();
        assert!(driver.leg(id).is_some(), "fixture begins with a lease");
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
        assert!(driver.leg(id).is_none(), "recovery revokes the old lease");
        assert!(
            !driver.has_cached_route(id),
            "recovery revokes the old route cache"
        );
    }

    #[test]
    fn displacement_escape_and_target_changes_revoke_only_the_affected_leg() {
        for kind in [
            HexMatchEventKind::PlayerRecovered,
            HexMatchEventKind::GuardianCatch,
            HexMatchEventKind::PlayerEscaped,
        ] {
            let (mut game, mut driver, id, _source, _to, _first) = leased_fixture();
            let other = PlayerId(1);
            let held = driver.legs[&id].clone();
            driver.legs.insert(other, held);
            game.recent_events = vec![HexMatchEvent {
                tick: game.tick,
                kind,
                player: Some(id),
                cell: Some(game.players[&id].cell),
            }];
            driver.invalidate_from_match(&game, id, game.objective_target(id));
            assert!(driver.leg(id).is_none(), "{kind:?} revokes affected bot");
            assert!(driver.leg(other).is_some(), "{kind:?} preserves other bot");
        }

        let (game, mut driver, id, _source, _to, _first) = leased_fixture();
        driver.legs.get_mut(&id).expect("leg").objective.q += 1;
        driver.invalidate_from_match(&game, id, game.objective_target(id));
        assert!(driver.leg(id).is_none(), "target change revokes lease");
    }

    #[test]
    fn disappearing_projected_guide_clears_a_stale_leg() {
        let (mut game, mut driver, id, source, _to, _first) = leased_fixture();
        game.geometry.guides.remove(&source);
        assert_eq!(driver.follow_leg(&game, id), None);
        assert!(driver.leg(id).is_none());
    }

    /// Stand a bot in the first cell of a lateral route step and give that cell
    /// a module graph it did not have before.
    ///
    /// The cell is an ordinary hall: no spine, no deck, no `Shaft` archetype.
    /// If crossing it still works, the runtime read the graph and nothing else.
    fn graph_module_fixture() -> (HexWfcMatch, HexBotDriver, PlayerId, HexCoord, HexCoord) {
        use observed_hex::HexFace;
        use observed_traversal::{TraversalGuideBuilder, TraversalMode};

        use crate::hex_wfc::{
            HexModuleInstanceId, HexModuleRevision, ProjectedPort, ProjectedTraversalGraph,
            ProjectedTraversalGuide,
        };

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
        let id = PlayerId(0);
        let objective = game
            .objective_target(id)
            .expect("the gate fixture has an objective")
            .cell;
        let route = game
            .facility
            .route_between_cells(game.players[&id].cell, objective)
            .expect("the gate spawn reaches its objective");
        let (from, to, face) = route
            .cells
            .windows(2)
            .find_map(|pair| {
                let face = HexFace::LATERAL.into_iter().find(|&face| {
                    game.facility.config.grid().neighbor(pair[0], face) == Some(pair[1])
                })?;
                (!game.geometry.guides.contains_key(&pair[0])).then_some((pair[0], pair[1], face))
            })
            .expect("the gate route crosses an unannotated cell laterally");

        let origin = Vec3::from_array(hex_origin(from));
        let half_height = game
            .content()
            .traversal_profile()
            .requirements()
            .capsule_half_height;
        let feet = Vec3::new(origin.x, origin.y + 0.5, origin.z);
        let toward = (Vec3::from_array(hex_origin(to)) - origin).normalize_or_zero();

        let mut builder = TraversalGuideBuilder::new();
        let entry = builder.node(feet);
        let middle = builder.node(feet + toward * 2.0);
        let exit = builder.node(feet + toward * 4.0);
        builder.connect(entry, middle, TraversalMode::Walk);
        // Deliberately a climb edge across a flat hall. A graph declares its own
        // modes; the runtime must not sanity-check them against an archetype.
        builder.connect(middle, exit, TraversalMode::Climb);
        let guide = builder.build().expect("fixture graph is valid");

        game.geometry.guides.insert(
            from,
            ProjectedTraversalGuide {
                instance: HexModuleInstanceId { source_cell: from },
                revision: HexModuleRevision::single(
                    from,
                    game.facility.cell_revision(from).expect("resolved cell"),
                ),
                source_cells: vec![from],
                graph: Some(ProjectedTraversalGraph {
                    guide,
                    port_bindings: BTreeMap::from([(ProjectedPort { cell: from, face }, exit)]),
                }),
                source_cell: from,
                climb: None,
                deck: None,
            },
        );

        let player = game.players.get_mut(&id).expect("solo player");
        player.cell = from;
        player.position = feet + Vec3::Y * half_height;
        player.yaw = 0.0;

        (game, HexBotDriver::new(), id, from, to)
    }

    #[test]
    fn a_graph_shaped_module_is_crossed_without_any_bot_branch() {
        use observed_traversal::{TraversalEdgeId, TraversalMode, follow_graph};

        let (game, mut driver, id, from, _to) = graph_module_fixture();
        let command = driver.command(&game, id);

        let held = driver.leg(id).expect("the graph module is leased").clone();
        assert_eq!(held.lease.instance.source_cell, from);
        assert_eq!(held.local.edge, TraversalEdgeId(0));
        // This used to also assert `driver.cursor(id).is_none()` — that the
        // graph path was taken *instead of* the compatibility one. There is no
        // longer a compatibility cursor to take, so the claim is now carried by
        // the driver's shape rather than by an assertion.

        // The command is exactly what the shared follower produces for this
        // graph. No archetype, port class, or shape rule contributed anything.
        let projected = game.geometry.guides[&from]
            .graph
            .as_ref()
            .expect("fixture graph");
        assert_eq!(
            projected.guide.edges()[1].mode,
            TraversalMode::Climb,
            "the fixture must exercise a mode the cell's archetype cannot imply"
        );
        let mut expected_cursor = held.local;
        let player = &game.players[&id];
        let expected = follow_graph(
            leg::pose(&game, player.position, player.yaw),
            &projected.guide,
            &mut expected_cursor,
            held.lease.exit,
            game.content().traversal_profile(),
        );
        assert_eq!(
            Some(command.intent),
            expected.intent,
            "the runtime must emit the production follower's intent verbatim"
        );
    }

    #[test]
    fn changing_the_logical_cell_mid_leg_cannot_change_the_edge_being_followed() {
        let (mut game, mut driver, id, from, to) = graph_module_fixture();
        let first = driver.command(&game, id);
        let committed = driver.leg(id).expect("leased").clone();

        // Same tick, asked again: idempotent.
        assert_eq!(driver.command(&game, id), first);
        assert_eq!(driver.leg(id), Some(&committed));

        // Height rounding reports the destination cell while the body has not
        // moved. A route re-read here would pick a different module entirely;
        // the held cursor must answer first and keep the same edge.
        game.players.get_mut(&id).expect("player").cell = to;
        assert_eq!(driver.command(&game, id), first);
        assert_eq!(driver.leg(id), Some(&committed));

        // Even a logical cell that is nowhere near the route cannot rewind it.
        game.players.get_mut(&id).expect("player").cell = HexCoord {
            q: from.q.saturating_add(50),
            ..from
        };
        assert_eq!(driver.command(&game, id), first);
        assert_eq!(
            driver.leg(id).expect("leased").local,
            committed.local,
            "only reaching a node may advance the cursor"
        );
    }

    #[test]
    fn replacing_the_leased_module_releases_the_graph_leg() {
        use crate::hex_wfc::HexModuleRevision;

        let (mut game, mut driver, id, from, _to) = graph_module_fixture();
        let _ = driver.command(&game, id);
        let stale = driver.leg(id).expect("leased").lease.revision.clone();

        // The projection replaces this exact module: same instance, new
        // revision. The lease names the revision it was taken against, so the
        // held leg is released rather than followed over stale geometry.
        let replaced = HexModuleRevision::single(from, 0xDEAD_BEEF);
        game.geometry
            .guides
            .get_mut(&from)
            .expect("fixture guide")
            .revision = replaced.clone();
        assert_ne!(stale, replaced);
        assert_eq!(driver.follow_leg(&game, id), None);
        assert!(
            driver.leg(id).is_none(),
            "a replaced module revokes the leg"
        );

        // The next full command may take a fresh leg, but only against the
        // revision that is actually projected now.
        let _ = driver.command(&game, id);
        assert_eq!(
            driver.leg(id).expect("re-leased").lease.revision,
            replaced,
            "re-acquisition must name the live revision"
        );
    }

    #[test]
    fn displacement_and_escape_clear_a_held_graph_leg_too() {
        for kind in [
            HexMatchEventKind::PlayerRecovered,
            HexMatchEventKind::GuardianCatch,
            HexMatchEventKind::PlayerEscaped,
        ] {
            let (mut game, mut driver, id, _from, _to) = graph_module_fixture();
            let _ = driver.command(&game, id);
            assert!(driver.leg(id).is_some(), "{kind:?} fixture begins leased");

            game.recent_events = vec![HexMatchEvent {
                tick: game.tick,
                kind,
                player: Some(id),
                cell: Some(game.players[&id].cell),
            }];
            driver.invalidate_from_match(&game, id, game.objective_target(id));
            assert!(
                driver.leg(id).is_none(),
                "{kind:?} must revoke the graph leg"
            );
        }

        // A changed objective revokes the leg even though no event fired: the
        // module is still fine, but the reason for crossing it is gone.
        let (game, mut driver, id, _from, _to) = graph_module_fixture();
        let _ = driver.command(&game, id);
        driver.legs.get_mut(&id).expect("leased").objective.q += 1;
        driver.invalidate_from_match(&game, id, game.objective_target(id));
        assert!(driver.leg(id).is_none(), "a new objective revokes the leg");
    }

    #[test]
    fn legacy_cells_present_an_adapter_graph_so_migration_can_be_partial() {
        use observed_hex::HexFace;
        use observed_traversal::TraversalMode;

        use super::super::super::bot::leg::{ExternalTransition, ResolvedModuleGraph};

        let (game, _driver, id, from, to) = graph_module_fixture();
        let transition = ExternalTransition::between(&game, from, to).expect("lateral transition");

        // A neighbouring cell that ships nothing at all still resolves to a
        // graph, so a half-migrated facility has one runtime path rather than
        // two. Ports bind to the cell's own open doorways.
        let unannotated = game
            .facility
            .placements
            .keys()
            .copied()
            .find(|cell| {
                *cell != from
                    && !game.geometry.guides.contains_key(cell)
                    && HexFace::LATERAL
                        .into_iter()
                        .filter(|&face| game.facility.placements[cell].is_open(face))
                        .count()
                        >= 2
            })
            .expect("the gate corpus contains a plain two-door cell");
        let adapter = ResolvedModuleGraph::resolve(&game, unannotated).expect("adapter graph");
        assert_eq!(adapter.instance.source_cell, unannotated);
        assert!(
            adapter.graph.port_bindings.len() >= 2,
            "every open doorway binds to a node"
        );
        assert!(
            adapter
                .graph
                .guide
                .edges()
                .iter()
                .all(|edge| edge.mode == TraversalMode::Walk),
            "a flat cell declares only walk edges"
        );
        for (port, node) in &adapter.graph.port_bindings {
            assert_eq!(port.cell, unannotated);
            assert!(adapter.graph.guide.node(*node).is_some());
        }

        // And the transition the planner produced is the one the module binds.
        assert_eq!(transition.from, from);
        assert_eq!(transition.to, to);
        assert!(transition.face.is_lateral());
        let _ = id;
    }

    #[test]
    fn recorded_driver_frames_replay_without_driver_state() {
        let (mut live, mut driver, id, _source, _to, _first) = leased_fixture();
        let mut replay = live.clone();
        let mut frames = Vec::new();
        let mut expected = Vec::new();
        assert!(driver.leg(id).is_some(), "test must exercise a real leg");

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
