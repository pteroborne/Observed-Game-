//! Deterministic objective bot. It emits the same `PlayerIntent` as a client
//! and physically walks halls, ramps, and grounded switchback stairs.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexCoord, HexFace, HexRoute, HexSpace};
use observed_hex::hex_origin;
use observed_traversal::{DeckHandoff, FollowTarget, FollowerPose, follow_stateless};
use player_input::PlayerIntent;

use super::movement::face_plan_dir;
use super::{FLOOR_SLAB_TOP, HexPlayerCommand, HexWfcMatch};

mod driver;
mod leg;
pub use driver::HexBotDriver;

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
        // No archetype test. What decides whether this crossing needs a
        // declared route is whether the tile **ships one**, and the next line
        // asks exactly that.
        //
        // The gate here gated on `Shaft`, which is the same inference
        // `finish_stair_command` already had removed from it next door: it used
        // to test the archetype, and a body part-way up a *ramp* was therefore
        // never recognised as still climbing. A deck is a deck. Only towers
        // author one today and every tower is a `Shaft`, so this changes no
        // behaviour on the committed corpus - it stops the next decked tile
        // that is not called `Shaft` from being silently ignored.
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
