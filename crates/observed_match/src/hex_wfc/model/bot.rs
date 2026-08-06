//! Deterministic objective bot. It emits the same `PlayerIntent` as a client
//! and physically walks halls, ramps, and grounded switchback stairs.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexArchetype, HexCoord, HexFace, HexPlacement, HexRoute, HexSpace, PortClass,
};
use observed_hex::hex_origin;
use observed_traversal::{
    DeckHandoff, FollowTarget, FollowerPose, TraversalDirection, follow_stateless,
};
use player_input::PlayerIntent;

use super::movement::face_plan_dir;
use super::{FLOOR_SLAB_TOP, HexPlayerCommand, HexWfcMatch};

mod driver;
mod leg;
pub use driver::{HexBotDriver, ModuleInstanceId, TraversalCursor, TraversalLease};

/// Most yaw a bot may turn in one 60 Hz tick, in radians.
///
/// `HexWfcMatch` overrides `FpsConfig::look_step` to `1.0`, so the look delta a
/// bot emits *is* the yaw delta the controller applies — this constant is
/// already in rad/tick, no scaling in between. It used to be 0.25, i.e. ~859
/// deg/s: a bot snapped to any new heading within a couple of ticks, which both
/// whipped the spectator camera and made reversing free, so an oscillating
/// waypoint cost the bot nothing. At ~275 deg/s a marginal flip is ridden out
/// instead of chased.
///
/// Bot-side on purpose: `step_character` is shared with human look input, so
/// clamping there would slow the player's mouse.
const MAX_TURN_PER_TICK: f32 = 0.08;
const STUCK_ENTER_TICKS: u16 = 45;
const STUCK_SWEEP_TICKS: u16 = 24;
const UNSTICK_STRAFE: f32 = 0.9;
const UNSTICK_FORWARD: f32 = 0.45;

/// What a bot is trying to do this tick.
///
/// Deliberately a plain enum chosen by one pure function, mirroring the
/// Guardian (`super::guardian::HexGuardianStatus`) rather than introducing a
/// behaviour-tree framework: there are four behaviours, none of them nest, and
/// `agents.md` asks for the smallest thing that works before any framework. If
/// behaviours ever need to compose or reorder, that is the moment to revisit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BotBehaviour {
    /// Escaped, absent, or already standing on the objective.
    Idle,
    /// A route to the objective exists: follow it.
    Seek,
    /// Following a route but making no headway; break contact.
    Recover,
    /// **No route to the objective exists.** Head for whichever open neighbour
    /// sits nearest the objective and try again next tick.
    Explore,
}

impl HexWfcMatch {
    /// Which behaviour `id` is in this tick. Pure: reads only existing state.
    #[must_use]
    pub fn bot_behaviour(&self, id: PlayerId) -> BotBehaviour {
        self.bot_behaviour_and_route(id).0
    }

    /// The behaviour *and* the route it was decided from.
    ///
    /// Deciding the behaviour requires routing to the objective, and acting on `Seek` or
    /// `Recover` requires that same route. Returning both means the A* runs once per bot
    /// per tick rather than twice — the callers were previously recomputing an identical
    /// search. Pure, and the behaviour is bit-for-bit what [`Self::bot_behaviour`]
    /// returned before.
    fn bot_behaviour_and_route(&self, id: PlayerId) -> (BotBehaviour, Option<HexRoute>) {
        let Some(player) = self.players.get(&id) else {
            return (BotBehaviour::Idle, None);
        };
        let Some(target) = self.bot_objective_cell(id) else {
            return (BotBehaviour::Idle, None);
        };
        if target == player.cell {
            return (BotBehaviour::Idle, None);
        }
        let route = self.facility.route_between_cells(player.cell, target);
        if route.as_ref().is_none_or(|route| route.cells.len() <= 1) {
            return (BotBehaviour::Explore, route);
        }
        let behaviour = if self.stuck_ticks.get(&id).copied().unwrap_or(0) >= STUCK_ENTER_TICKS {
            BotBehaviour::Recover
        } else {
            BotBehaviour::Seek
        };
        (behaviour, route)
    }

    /// The abstract command the objective bot issues for `id` this tick.
    #[must_use]
    pub fn bot_command(&self, id: PlayerId) -> PlayerIntent {
        HexBotDriver::new().command(self, id).intent
    }

    /// Fallback when no route to the objective exists.
    ///
    /// Previously this case returned `PlayerIntent::default()`, so a bot that
    /// lost its route — stranded in a pocket, or on a cell orphaned by a
    /// relayout — stood still forever. It was never exercised: the stall soak
    /// skips any layout whose spawn cannot reach the exit at all.
    ///
    /// Instead, walk to whichever open lateral neighbour lies nearest the
    /// objective in plan. Greedy and deliberately memoryless: the real route is
    /// retried every tick, so this only has to break the deadlock, not solve
    /// the maze. Deterministic — ties fall to `HexFace::LATERAL` order.
    fn explore_command(&self, player: &super::HexPlayerState) -> PlayerIntent {
        let Some(placement) = self.facility.placements.get(&player.cell) else {
            return PlayerIntent::default();
        };
        let goal = self.objective_target(player.id).map_or_else(
            || Vec3::from_array(hex_origin(self.facility.config.exit())),
            |target| target.position,
        );
        let grid = self.facility.config.grid();
        let best = HexFace::LATERAL
            .into_iter()
            .filter(|&face| placement.is_open(face))
            .filter_map(|face| grid.neighbor(player.cell, face))
            .filter(|cell| {
                self.facility
                    .placements
                    .get(cell)
                    .is_some_and(|neighbour| neighbour.space != HexSpace::Void)
            })
            .min_by(|a, b| {
                let plan = |cell: &HexCoord| {
                    let origin = Vec3::from_array(hex_origin(*cell));
                    Vec2::new(origin.x - goal.x, origin.z - goal.z).length()
                };
                plan(a).total_cmp(&plan(b))
            });
        match best {
            Some(cell) => {
                let target = self
                    .lateral_waypoint(player.cell, cell, player.position)
                    .unwrap_or_else(|| Vec3::from_array(hex_origin(cell)));
                steer_toward(player.yaw, player.position, target)
            }
            None => PlayerIntent::default(),
        }
    }

    fn apply_unstick(&self, id: PlayerId, mut intent: PlayerIntent) -> PlayerIntent {
        let stuck = self.stuck_ticks.get(&id).copied().unwrap_or(0);
        if stuck >= STUCK_ENTER_TICKS {
            let side = if (stuck / STUCK_SWEEP_TICKS).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            intent.movement = Vec2::new(side * UNSTICK_STRAFE, UNSTICK_FORWARD);
        }
        intent
    }

    #[must_use]
    pub(crate) fn bot_objective_cell(&self, id: PlayerId) -> Option<HexCoord> {
        self.objective_target(id).map(|target| target.cell)
    }

    /// Compatibility adapter for intent-only callers that do not retain a bot
    /// driver. Production hosts should own a [`HexBotDriver`] beside the match
    /// so route caches and traversal leases survive between ticks.
    #[must_use]
    pub fn bot_player_command(&self, id: PlayerId) -> HexPlayerCommand {
        HexBotDriver::new().command(self, id)
    }

    /// Select the stable projected guide for a vertical route transition.
    fn traversal_lease(
        &self,
        player: &super::HexPlayerState,
        next: HexCoord,
        objective: HexCoord,
    ) -> Option<TraversalLease> {
        let up = next.level > player.cell.level;
        let feet = player.position.y
            - self
                .content
                .traversal_profile()
                .requirements()
                .capsule_half_height;
        let pose = FollowerPose {
            feet: Vec3::new(player.position.x, feet, player.position.z),
            yaw: player.yaw,
        };
        let direction = if up {
            TraversalDirection::Forward
        } else {
            TraversalDirection::Reverse
        };
        let unfinished = |at: HexCoord, direction| {
            let guide = self.geometry.guides.get(&at)?;
            let spine = guide.climb.as_ref()?;
            follow_stateless(
                pose,
                FollowTarget::Climb {
                    spine,
                    approach: guide.deck.as_ref(),
                    direction,
                },
                self.content.traversal_profile(),
            )
            .is_on_unfinished_climb()
            .then_some(())
        };
        let source_cell = if up {
            let below = (player.cell.level > 0).then(|| HexCoord {
                level: player.cell.level - 1,
                ..player.cell
            });
            below
                .filter(|&below| unfinished(below, TraversalDirection::Forward).is_some())
                .unwrap_or(player.cell)
        } else if unfinished(player.cell, TraversalDirection::Reverse).is_some() {
            player.cell
        } else {
            next
        };
        self.geometry
            .guides
            .get(&source_cell)?
            .climb
            .as_ref()
            .filter(|spine| !spine.is_empty())?;
        Some(TraversalLease {
            instance: ModuleInstanceId { source_cell },
            route_from: player.cell,
            route_to: next,
            objective,
            direction,
        })
    }

    fn vertical_command(
        &self,
        cell: HexCoord,
        yaw: f32,
        position: Vec3,
        next: HexCoord,
    ) -> PlayerIntent {
        let up = next.level > cell.level;
        let placement = &self.facility.placements[&cell];
        let class = if up { placement.up } else { placement.down };
        // A spine describes a climb; where one exists it is authoritative,
        // whatever the port class.
        //
        // This used to key on `ShaftOpen` alone, so only towers were followed
        // and every `RampOpen` cell went to `ramp_walk_dir` instead. That
        // function assumes the shape of `hall_ramp` variant 0 - one straight
        // mass rising from the open door to the face opposite - and simply
        // steers at that face. The authored perimeter ramps share variant 0's
        // *signature* and not its shape: they climb around the wall and leave
        // the middle open. A body spawned on one was aimed at the far wall and
        // held there for all 36,000 steps of a match, never moving.
        //
        // Sharing a signature is what lets the solver treat two tiles as
        // alternatives. It is not a promise that they are the same shape, and
        // anything that reads geometry from an archetype rather than from the
        // tile will break the moment a second shape appears.
        let has_spine = self
            .geometry
            .climbs
            .get(&cell)
            .is_some_and(|spine| !spine.is_empty());
        if class == PortClass::ShaftOpen || has_spine {
            let feet = position.y
                - self
                    .content
                    .traversal_profile()
                    .requirements()
                    .capsule_half_height;
            let feet_point = Vec3::new(position.x, feet, position.z);
            // Height rounding changes the logical cell while the body is still
            // on the last stretch of a climb. Pick the spine by contact with
            // its authored line and its explicit terminal node, not by a
            // height threshold around the deck.
            //
            // The threshold oscillated on the perimeter tower's sloped top
            // tread: one tick targeted the lower spine's head, the next the
            // upper spine's foot, and the bot traced a 0.7 m circle forever.
            // `StairSpine::{has_arrived, has_descended}` exist precisely to
            // make this handoff shape-independent.
            let unfinished = |at: HexCoord, climbing_up: bool| {
                let Some(spine) = self.geometry.climbs.get(&at) else {
                    return false;
                };
                follow_stateless(
                    FollowerPose {
                        feet: feet_point,
                        yaw,
                    },
                    FollowTarget::Climb {
                        spine,
                        approach: self.geometry.decks.get(&at),
                        direction: if climbing_up {
                            TraversalDirection::Forward
                        } else {
                            TraversalDirection::Reverse
                        },
                    },
                    self.content.traversal_profile(),
                )
                .is_on_unfinished_climb()
            };
            let base = if up {
                let below = (cell.level > 0).then(|| HexCoord {
                    level: cell.level - 1,
                    ..cell
                });
                below
                    .filter(|&below| unfinished(below, true))
                    .unwrap_or(cell)
            } else if unfinished(cell, false) {
                cell
            } else {
                next
            };
            if let Some(command) = self.climb_command(base, yaw, feet_point, up) {
                return command;
            }
            // No climb under this cell at all. Nothing can be steered along a
            // line that does not exist, so head for the centre and let the
            // stuck detector recover rather than issuing a nonsense heading.
            return steer_toward(yaw, position, Vec3::from_array(hex_origin(base)));
        }

        let dir = ramp_walk_dir(placement, up);
        if dir == Vec2::ZERO {
            return steer_toward(yaw, position, Vec3::from_array(hex_origin(cell)));
        }
        let aim = position + Vec3::new(dir.x, 0.0, dir.y) * 12.0;
        steer_toward(yaw, position, aim)
    }

    /// Walk `cell`'s climb in the requested direction, first crossing the floor
    /// to reach it if the body is not on it yet.
    ///
    /// The approach is the half that is easy to forget. A body that enters a
    /// tower through a lateral door stands wherever that door is, which for the
    /// switchback can be the far side of the stairwell from the foot of the
    /// climb. Steering it straight at the flight walks it into the guard rail
    /// around the opening and holds it there — measured, all four soak bots
    /// pinned against the same rail. The floor path is how it gets round.
    fn climb_command(
        &self,
        cell: HexCoord,
        yaw: f32,
        feet_point: Vec3,
        up: bool,
    ) -> Option<PlayerIntent> {
        let spine = self.geometry.climbs.get(&cell)?;
        follow_stateless(
            FollowerPose {
                feet: feet_point,
                yaw,
            },
            FollowTarget::Climb {
                spine,
                approach: self.geometry.decks.get(&cell),
                direction: if up {
                    TraversalDirection::Forward
                } else {
                    TraversalDirection::Reverse
                },
            },
            self.content.traversal_profile(),
        )
        .intent
    }

    /// Continue a stair flight after height rounding has changed the logical
    /// cell but before the capsule's feet reach the destination deck.
    fn finish_stair_command(
        &self,
        cell: HexCoord,
        yaw: f32,
        position: Vec3,
    ) -> Option<PlayerIntent> {
        self.facility.placements.get(&cell)?;
        let requirements = self.content.traversal_profile().requirements();
        let feet = position.y - requirements.capsule_half_height;
        let floor = hex_origin(cell)[1] + FLOOR_SLAB_TOP;
        let feet_point = Vec3::new(position.x, feet, position.z);
        let below = (cell.level > 0).then(|| HexCoord {
            level: cell.level - 1,
            ..cell
        });
        // Any cell with a climb, not only a `Shaft`.
        //
        // This used to test the archetype, which meant a body part-way up a
        // *ramp* was never recognised as still climbing: height rounding moved
        // its logical cell to the level above, this returned `None`, and the
        // lateral steering for the next cell took over while its feet were
        // still more than a metre below the deck. It walked into the wall and
        // stayed there. Towers were fine because they are `Shaft`.
        //
        // A climb is a climb. What decides this is whether there is a spine to
        // still be on, which is the same thing `climb_command` needs anyway.
        let climbing = |at: HexCoord| {
            self.geometry
                .climbs
                .get(&at)
                .is_some_and(|spine| !spine.is_empty())
        };
        if !climbing(cell) && !below.is_some_and(climbing) {
            return None;
        }
        let unfinished = |at: HexCoord, direction: TraversalDirection| {
            let Some(spine) = self.geometry.climbs.get(&at) else {
                return false;
            };
            follow_stateless(
                FollowerPose {
                    feet: feet_point,
                    yaw,
                },
                FollowTarget::Climb {
                    spine,
                    approach: self.geometry.decks.get(&at),
                    direction,
                },
                self.content.traversal_profile(),
            )
            .is_on_unfinished_climb()
        };
        // A logical level transition happens before the body reaches the
        // authored terminal node. Finish whichever spine is still underfoot;
        // height cannot decide this on a sloped terminal tread, where ordinary
        // controller motion crosses the same threshold every few ticks.
        if let Some(below) = below
            && unfinished(below, TraversalDirection::Forward)
        {
            self.climb_command(below, yaw, feet_point, true)
        // Descending stays height-gated. A lateral walker on the deck can pass
        // near this tower's spine foot; proximity alone would pull it backward
        // down a climb it never intended to take.
        } else if feet > floor + requirements.step_height
            && unfinished(cell, TraversalDirection::Reverse)
        {
            self.climb_command(cell, yaw, feet_point, false)
        } else {
            None
        }
    }

    /// Waypoint for an ordinary lateral hop: steer at the shared doorway until
    /// the body is through its plane, then at the neighbour's centre.
    ///
    /// Aiming straight at the neighbour's centre works only from near a cell's
    /// middle. From anywhere off-centre the heading runs wide of the 4.5 m
    /// aperture and into the corner where three cells meet — and there the
    /// logical cell can resolve to a third, *off-route* cell whose own route
    /// points back the way it came, so the bot shuttles between the two. That
    /// was measured as a 2.2 m limit cycle, identical positions each pass,
    /// repeated for ~2,300 ticks with the bot never registering as stuck.
    ///
    /// Steering at the door and only then at the centre keeps the path inside
    /// the two cells that actually share the aperture.
    fn lateral_waypoint(&self, cell: HexCoord, next: HexCoord, position: Vec3) -> Option<Vec3> {
        let face = HexFace::LATERAL
            .into_iter()
            .find(|&face| self.facility.config.grid().neighbor(cell, face) == Some(next))?;
        let origin = Vec3::from_array(hex_origin(cell));
        let [a, b] = observed_hex::face_edge(face);
        let door = Vec3::new(
            origin.x + (a.0 + b.0) as f32 * 0.5,
            position.y,
            origin.z + (a.1 + b.1) as f32 * 0.5,
        );
        let outward = face_plan_dir(face);
        let past_aperture = Vec2::new(position.x - door.x, position.z - door.z).dot(outward) > 0.0;
        if past_aperture {
            // Through the doorway; head for the neighbour's middle so the body
            // clears the threshold instead of loitering in it.
            let next_origin = Vec3::from_array(hex_origin(next));
            Some(Vec3::new(next_origin.x, position.y, next_origin.z))
        } else {
            Some(door)
        }
    }

    /// Cross a stair tower's floor to a lateral door by following the walkable
    /// path the tile declares, rather than heading straight for the aperture.
    ///
    /// A centre-to-centre heading cuts across the stairwell. This used to be
    /// sixty lines of local coordinates measured off the generated switchback —
    /// corner cases for arriving from below, for each door face, and a
    /// rectangle-crossing test against the guarded opening — all of which were
    /// true of exactly one tower shape. The tile now says where its floor goes
    /// (`DeckPath`), so this walks that and nothing here knows the shape.
    fn stair_lateral_command(
        &self,
        cell: HexCoord,
        next: HexCoord,
        yaw: f32,
        position: Vec3,
    ) -> Option<PlayerIntent> {
        if self.facility.placements.get(&cell)?.archetype != HexArchetype::Shaft {
            return None;
        }
        let deck = self.geometry.decks.get(&cell)?;
        let face = HexFace::LATERAL
            .into_iter()
            .find(|&face| self.facility.config.grid().neighbor(cell, face) == Some(next))?;
        let origin = Vec3::from_array(hex_origin(cell));
        let [a, b] = observed_hex::face_edge(face);
        let door = origin
            + Vec3::new(
                (a.0 + b.0) as f32 * 0.5,
                FLOOR_SLAB_TOP,
                (a.1 + b.1) as f32 * 0.5,
            );
        // Once through the aperture the deck no longer applies: this cell's path
        // cannot describe the neighbour's floor.
        let outward = face_plan_dir(face);
        let feet = Vec3::new(
            position.x,
            position.y
                - self
                    .content
                    .traversal_profile()
                    .requirements()
                    .capsule_half_height,
            position.z,
        );
        follow_stateless(
            FollowerPose { feet, yaw },
            FollowTarget::Deck {
                path: deck,
                goal: door,
                handoff: Some(DeckHandoff {
                    threshold: door,
                    outward,
                    destination: Vec3::from_array(hex_origin(next)),
                }),
            },
            self.content.traversal_profile(),
        )
        .intent
    }
}

/// Plan heading that walks a two-cell ramp in the requested direction.
fn ramp_walk_dir(placement: &HexPlacement, up: bool) -> Vec2 {
    let Some(open) = HexFace::LATERAL
        .into_iter()
        .find(|&face| placement.is_open(face))
    else {
        return Vec2::ZERO;
    };
    let rise = match placement.archetype {
        HexArchetype::RampUp => open.opposite(),
        HexArchetype::RampHead => open,
        _ => open,
    };
    face_plan_dir(if up { rise } else { rise.opposite() })
}

fn steer_toward(yaw: f32, position: Vec3, target: Vec3) -> PlayerIntent {
    steer_toward_with_speed(yaw, position, target, true, 1.0)
}

fn steer_toward_with_speed(
    yaw: f32,
    position: Vec3,
    target: Vec3,
    sprint: bool,
    movement_scale: f32,
) -> PlayerIntent {
    let direction = target - position;
    let desired_yaw = direction.x.atan2(-direction.z);
    let look = wrap_angle(desired_yaw - yaw).clamp(-MAX_TURN_PER_TICK, MAX_TURN_PER_TICK);
    PlayerIntent {
        movement: Vec2::Y * movement_scale,
        look: Vec2::new(look, 0.0),
        sprint_held: sprint,
        ..PlayerIntent::default()
    }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(PI * 2.0) - PI
}
