//! First-person embodiment: a continuous body standing on a discrete board.
//!
//! The split is the whole idea. [`crate::model`] owns every *rule* on a hex
//! lattice of whole cells, and is untouched by this module. The body owns
//! *where the player physically is*, and each tick reports two derived facts
//! back into the model: which cell it stands on, and which face it is looking
//! down. Nothing else crosses the boundary.
//!
//! That keeps the shove exactly as canon requires — resolved in fixed-tick
//! simulation on the lattice, never by a physics query — while the player walks
//! and aims continuously. The controller itself
//! ([`observed_traversal::step_body`]) is already pure and already runs at
//! `FIXED_DT`, which is this model's tick, so embodiment adds no new source of
//! nondeterminism.
//!
//! ## Floor plates
//!
//! The lattice tiles *exactly* with `14 x 12` rectangles offset by 7 per row:
//! an East neighbour sits 14 along x, and a row sits 12 along z with a 7 shear,
//! so plates meet edge to edge with no gap and no overlap. Void cells therefore
//! leave exact rectangular holes to fall through.
//!
//! This makes the physical floor plate a rectangle while the *lattice* stays
//! hexagonal. That is deliberate rather than an approximation: the hex lattice
//! is the connectivity and targeting structure, and an authored tile's geometry
//! was never required to be a hexagonal prism. Rendering matches collision
//! exactly, which is what the Legibility Contract actually demands.

use bevy::prelude::*;
use observed_hex::{
    coords::{HexCoord, HexGridSize},
    faces::HexFace,
};
use observed_traversal::{Aabb3, FIXED_DT, FpsArena, FpsBody, FpsConfig, FpsStep, step_body};
use player_input::{PlayerId, PlayerIntent};

use crate::model::{CellKind, KineticIntent, KineticWorld};

/// Half-width of a floor plate: half of `observed_hex::metrics::ACROSS_FLATS`.
pub const PLATE_HALF_X: f32 = 7.0;
/// Half-depth of a floor plate: half the 12 metre row pitch.
pub const PLATE_HALF_Z: f32 = 6.0;
/// Row shear along x, matching `hex_origin_plan`.
pub const ROW_SHEAR_X: f32 = 7.0;
/// Column pitch along x.
pub const COL_PITCH_X: f32 = 14.0;
/// Row pitch along z.
pub const ROW_PITCH_Z: f32 = 12.0;
/// How thick a plate is. Only the top face is ever stood on.
pub const PLATE_THICKNESS: f32 = 2.0;
/// How tall a wall cell stands above the floor.
pub const WALL_HEIGHT: f32 = 7.0;
/// Top surface of every plate.
pub const FLOOR_TOP: f32 = 0.0;
/// How far below the floor an Observer must fall before the void claims them.
///
/// Roughly a second and a half of falling: long enough to understand what just
/// happened, short enough not to be a punishment.
pub const VOID_DEATH_DROP: f32 = 22.0;

/// Plan position of a cell's centre in world metres.
#[must_use]
pub fn plate_center(coord: HexCoord) -> Vec3 {
    Vec3::new(
        f32::from(coord.q) * COL_PITCH_X + f32::from(coord.r) * ROW_SHEAR_X,
        FLOOR_TOP,
        f32::from(coord.r) * ROW_PITCH_Z,
    )
}

/// Which cell a world position stands on, if any.
///
/// Exact inverse of [`plate_center`] for the tiling above: recover the row
/// first, undo its shear, then recover the column.
#[must_use]
pub fn cell_at(position: Vec3, grid: HexGridSize) -> Option<HexCoord> {
    let r = (position.z / ROW_PITCH_Z).round();
    let q = ((position.x - r * ROW_SHEAR_X) / COL_PITCH_X).round();
    if r < 0.0 || q < 0.0 {
        return None;
    }
    let coord = HexCoord {
        q: u16::try_from(q as i64).ok()?,
        r: u16::try_from(r as i64).ok()?,
        level: 0,
    };
    grid.contains(coord).then_some(coord)
}

/// The lateral face an eye is looking down.
///
/// Picks the face whose plan direction best matches the look direction, so the
/// targeting lane always has a definite answer and never flickers between two
/// faces at a boundary — ties resolve by `HexFace::LATERAL` order.
#[must_use]
pub fn face_for_yaw(yaw: f32) -> HexFace {
    // yaw 0 looks toward -Z, matching `FpsBody::forward`.
    let forward = Vec2::new(yaw.sin(), -yaw.cos());
    let mut best = (f32::NEG_INFINITY, HexFace::East);
    for face in HexFace::LATERAL {
        let (dq, dr, _) = face.delta();
        let direction = Vec2::new(
            dq as f32 * COL_PITCH_X + dr as f32 * ROW_SHEAR_X,
            dr as f32 * ROW_PITCH_Z,
        )
        .normalize_or_zero();
        let score = forward.dot(direction);
        if score > best.0 {
            best = (score, face);
        }
    }
    best.1
}

/// Build the physical arena from the board.
///
/// Every standable cell contributes one plate; void cells contribute nothing,
/// which is the hole. Wall cells contribute a full-height block.
#[must_use]
pub fn build_arena(world: &KineticWorld) -> FpsArena {
    let mut solids = Vec::with_capacity(world.grid.cell_count());
    for index in 0..world.grid.cell_count() {
        let coord = world.grid.coord(index);
        let center = plate_center(coord);
        match world.cell(coord) {
            // Nothing to stand on. This is what "shoved into void" means
            // physically as well as logically.
            CellKind::Void => {}
            CellKind::Wall => solids.push(Aabb3::from_center_half(
                Vec3::new(center.x, FLOOR_TOP + WALL_HEIGHT / 2.0, center.z),
                Vec3::new(PLATE_HALF_X, WALL_HEIGHT / 2.0, PLATE_HALF_Z),
            )),
            _ => solids.push(Aabb3::from_center_half(
                Vec3::new(center.x, FLOOR_TOP - PLATE_THICKNESS / 2.0, center.z),
                Vec3::new(PLATE_HALF_X, PLATE_THICKNESS / 2.0, PLATE_HALF_Z),
            )),
        }
    }

    let far_x = f32::from(world.grid.cols) * COL_PITCH_X + f32::from(world.grid.rows) * ROW_SHEAR_X;
    let far_z = f32::from(world.grid.rows) * ROW_PITCH_Z;
    FpsArena {
        solids,
        // `FpsArena::floor_y` is an UNBOUNDED ground plane, not a bounded floor:
        // `step_body` grants support whenever the feet reach it, with no x/z
        // test at all. Setting it to `FLOOR_TOP` — the exact height of every
        // plate's top — laid an invisible floor across the whole world, so the
        // void was not a hole, nothing could fall, and an Observer standing on
        // a void cell became unreachable by any Guardian. Park the plane far
        // below everything and let `VOID_DEATH_DROP` own the fall instead.
        floor_y: FLOOR_TOP - 10_000.0,
        // Generous, because the respawn bound is a square centred on the origin
        // while the board is a sheared parallelogram in the positive quadrant.
        floor_half: far_x.max(far_z) + COL_PITCH_X,
    }
}

/// What the player asked the tool to do this tick, separate from movement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToolRequest {
    #[default]
    None,
    Push,
    Pull,
    ToggleGenerator,
}

impl ToolRequest {
    fn as_intent(self) -> KineticIntent {
        match self {
            ToolRequest::None => KineticIntent::Idle,
            ToolRequest::Push => KineticIntent::Push,
            ToolRequest::Pull => KineticIntent::Pull,
            ToolRequest::ToggleGenerator => KineticIntent::ToggleGenerator,
        }
    }
}

/// The player's body, and the arena it collides against.
#[derive(Resource, Clone, Debug)]
pub struct Embodiment {
    pub id: PlayerId,
    pub body: FpsBody,
    pub config: FpsConfig,
    pub arena: FpsArena,
    /// Set for one tick when the body left the floor entirely and the
    /// controller recovered it. This is a fall into true void.
    pub fell_into_void: bool,
    /// Last face the body resolved to, kept so presentation can show the lane
    /// without recomputing it.
    pub facing: HexFace,
}

impl Embodiment {
    #[must_use]
    pub fn new(world: &KineticWorld) -> Self {
        let spawn = world
            .observers
            .first()
            .map_or(HexCoord::default(), |observer| observer.cell);
        let center = plate_center(spawn);
        let config = FpsConfig::deliberate_rapier();
        Self {
            id: world
                .observers
                .first()
                .map_or(PlayerId(0), |observer| observer.id),
            body: FpsBody::spawned(
                Vec3::new(center.x, FLOOR_TOP + config.half_height, center.z),
                0.0,
            ),
            config,
            arena: build_arena(world),
            fell_into_void: false,
            facing: HexFace::East,
        }
    }

    /// Rebuild the arena after the board's geometry changes.
    ///
    /// A retracting tile becoming void must remove its plate, or the player
    /// would keep standing on a floor the simulation says is gone.
    pub fn refresh_arena(&mut self, world: &KineticWorld) {
        self.arena = build_arena(world);
    }

    /// Move the body without advancing the board.
    ///
    /// What "pause" means here: the facility holds still and you do not. Used by
    /// the pause key and the screenshot path, so a stance can be lined up
    /// without a Guardian walking onto you mid-look.
    pub fn step_body_only(&mut self, intent: PlayerIntent) -> FpsStep {
        let report = step_body(&mut self.body, intent, &self.arena, &self.config, FIXED_DT);
        self.facing = face_for_yaw(self.body.yaw);
        report
    }

    /// Advance the body and the board by exactly one fixed tick.
    ///
    /// Order matters and is part of the contract: move the body, derive cell
    /// and facing from it, then let the model resolve rules against those
    /// derived facts. The model never reads a float position, and the body
    /// never decides a rule.
    pub fn step(
        &mut self,
        world: &mut KineticWorld,
        intent: PlayerIntent,
        request: ToolRequest,
    ) -> FpsStep {
        // Pass the intent through untouched. `PlayerIntent::sanitized` clamps
        // `look` to unit length as well as `movement`, and `step_body`'s own
        // comment warns against exactly that: it caps the turn at
        // `look_step` radians per tick no matter how far the mouse moved, so a
        // two-pixel nudge and a full flick turn at the identical ~120 deg/s and
        // all sensitivity is lost. `step_body` already clamps `movement`
        // itself, which is the only clamp that was ever wanted.
        let report = step_body(&mut self.body, intent, &self.arena, &self.config, FIXED_DT);

        // The arena's ground plane is parked far below the board, so a body over
        // a hole really does fall. This is what ends that fall.
        self.fell_into_void = report.recovered;
        if self.body.position.y < FLOOR_TOP - VOID_DEATH_DROP {
            self.body.reset();
            self.fell_into_void = true;
        }

        self.facing = face_for_yaw(self.body.yaw);
        if let Some(index) = world
            .observers
            .iter()
            .position(|observer| observer.id == self.id)
        {
            if let Some(cell) = cell_at(self.body.position, world.grid) {
                world.observers[index].cell = cell;
            }
            world.observers[index].facing = self.facing;
        }

        let intents = [(self.id, request.as_intent())];
        world.step(&intents);
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CellKind;

    fn coord(q: u16, r: u16) -> HexCoord {
        HexCoord { q, r, level: 0 }
    }

    #[test]
    fn plate_centres_and_cell_lookup_are_exact_inverses() {
        let world = KineticWorld::authored();
        for index in 0..world.grid.cell_count() {
            let expected = world.grid.coord(index);
            let center = plate_center(expected);
            assert_eq!(cell_at(center, world.grid), Some(expected));
        }
    }

    #[test]
    fn plates_tile_without_gap_or_overlap() {
        // Anywhere inside a plate resolves to that plate, right up to its edge.
        let world = KineticWorld::authored();
        let target = coord(4, 3);
        let center = plate_center(target);
        for (dx, dz) in [
            (0.0, 0.0),
            (PLATE_HALF_X - 0.01, 0.0),
            (-(PLATE_HALF_X - 0.01), 0.0),
            (0.0, PLATE_HALF_Z - 0.01),
            (0.0, -(PLATE_HALF_Z - 0.01)),
            (PLATE_HALF_X - 0.01, PLATE_HALF_Z - 0.01),
        ] {
            let probe = Vec3::new(center.x + dx, 0.0, center.z + dz);
            assert_eq!(
                cell_at(probe, world.grid),
                Some(target),
                "offset {dx},{dz} left its own plate"
            );
        }
        // One step past the edge belongs to the neighbour, not to nothing.
        let probe = Vec3::new(center.x + PLATE_HALF_X + 0.01, 0.0, center.z);
        assert_eq!(cell_at(probe, world.grid), Some(coord(5, 3)));
    }

    #[test]
    fn every_lateral_face_is_reachable_by_looking_at_it() {
        // Face the exact plan direction of each face and confirm it resolves to
        // that face, so no face is unreachable and none is double-claimed.
        for face in HexFace::LATERAL {
            let (dq, dr, _) = face.delta();
            let direction = Vec2::new(
                dq as f32 * COL_PITCH_X + dr as f32 * ROW_SHEAR_X,
                dr as f32 * ROW_PITCH_Z,
            )
            .normalize();
            // Invert `forward = (sin yaw, -cos yaw)`.
            let yaw = direction.x.atan2(-direction.y);
            assert_eq!(face_for_yaw(yaw), face, "looking straight down {face:?}");
        }
    }

    #[test]
    fn the_camera_looks_down_the_very_lane_the_tool_targets() {
        // Regression: `FpsBody::forward` is (sin yaw, -cos yaw), which is the
        // opposite handedness to a Y-rotation of Bevy's -Z camera default.
        // Rebuilding the camera from Euler angles aimed it 180 degrees from the
        // lane the tool was actually resolving against.
        for face in HexFace::LATERAL {
            let (dq, dr, _) = face.delta();
            let lane = Vec2::new(
                dq as f32 * COL_PITCH_X + dr as f32 * ROW_SHEAR_X,
                dr as f32 * ROW_PITCH_Z,
            )
            .normalize();
            let yaw = lane.x.atan2(-lane.y);
            let body = FpsBody::spawned(Vec3::ZERO, yaw);
            assert_eq!(face_for_yaw(body.yaw), face, "the tool targets {face:?}");

            let camera =
                Transform::from_translation(Vec3::ZERO).looking_to(body.look_dir(), Vec3::Y);
            let forward = camera.forward();
            let agreement = Vec2::new(forward.x, forward.z).normalize().dot(lane);
            assert!(
                agreement > 0.99,
                "camera points {agreement} along the {face:?} lane it should face"
            );
        }
    }

    /// A body standing over void must fall and be claimed.
    ///
    /// Regression for the worst bug this lab had: `FpsArena::floor_y` is an
    /// *unbounded* ground plane, and it was set to `FLOOR_TOP`, the exact height
    /// of every plate's top. That laid an invisible floor across the entire
    /// world. The void was not a hole, nothing could ever fall, and an Observer
    /// standing on a void cell was unreachable by any Guardian — free
    /// invulnerability in a lab whose whole premise is that the architecture
    /// kills. The old test for this asserted a *count of colliders* and passed
    /// happily throughout.
    #[test]
    fn a_body_over_the_void_falls_and_is_claimed() {
        let mut world = KineticWorld::authored();
        world.minors.clear();
        world.major.cell = coord(1, 1);
        let rim = coord(8, 3);
        assert_eq!(world.cell(rim), CellKind::Void, "the rim is void");

        let mut body = Embodiment::new(&world);
        let center = plate_center(rim);
        body.body.position = Vec3::new(center.x, FLOOR_TOP + body.config.half_height, center.z);
        body.body.velocity = Vec3::ZERO;
        body.body.grounded = false;

        let mut claimed = false;
        for _ in 0..600 {
            body.step(&mut world, PlayerIntent::default(), ToolRequest::None);
            if body.fell_into_void {
                claimed = true;
                break;
            }
        }
        assert!(claimed, "a body left standing in the void never fell");
    }

    /// The companion half: solid floor must still hold you up. Without this, the
    /// test above passes trivially if everything falls forever.
    #[test]
    fn a_body_on_a_plate_stays_on_it() {
        let mut world = KineticWorld::authored();
        world.minors.clear();
        world.major.cell = coord(1, 1);
        let mut body = Embodiment::new(&world);

        for _ in 0..600 {
            body.step(&mut world, PlayerIntent::default(), ToolRequest::None);
            assert!(
                !body.fell_into_void,
                "a body standing on a plate fell through it"
            );
        }
        assert!(
            (body.body.position.y - (FLOOR_TOP + body.config.half_height)).abs() < 0.2,
            "a resting body drifted off its plate: y = {}",
            body.body.position.y
        );
    }

    #[test]
    fn void_cells_leave_a_hole_and_walls_stand_full_height() {
        let world = KineticWorld::authored();
        let arena = build_arena(&world);
        let standable = (0..world.grid.cell_count())
            .filter(|index| world.cell(world.grid.coord(*index)) != CellKind::Void)
            .count();
        assert_eq!(arena.solids.len(), standable, "void contributes no solid");

        let wall_center = plate_center(coord(2, 2));
        let wall = arena
            .solids
            .iter()
            .find(|solid| {
                (solid.min.x..solid.max.x).contains(&wall_center.x)
                    && (solid.min.z..solid.max.z).contains(&wall_center.z)
            })
            .expect("the wall cell has a solid");
        assert!(wall.max.y >= FLOOR_TOP + WALL_HEIGHT - 0.01);
    }

    #[test]
    fn a_retracted_tile_stops_being_something_to_stand_on() {
        let mut world = KineticWorld::authored();
        let before = build_arena(&world).solids.len();
        // Run the authored retracting tile out.
        for _ in 0..200 {
            world.step(&[]);
        }
        assert_eq!(world.cell(coord(3, 5)), CellKind::Void);
        assert_eq!(build_arena(&world).solids.len(), before - 1);
    }

    #[test]
    fn walking_moves_the_derived_cell_without_any_step_intent() {
        let mut world = KineticWorld::authored();
        world.minors.clear();
        world.major.cell = coord(1, 1);
        let mut body = Embodiment::new(&world);
        let start = world.observers[0].cell;

        // Look down +x (East) and walk forward.
        body.body.yaw = std::f32::consts::FRAC_PI_2;
        let intent = PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..Default::default()
        };
        for _ in 0..240 {
            body.step(&mut world, intent, ToolRequest::None);
        }

        assert_eq!(body.facing, HexFace::East);
        assert_ne!(
            world.observers[0].cell, start,
            "continuous movement moved the authoritative cell"
        );
        assert_eq!(
            world.observers[0].cell,
            cell_at(body.body.position, world.grid).unwrap()
        );
    }

    #[test]
    fn identical_inputs_reproduce_identical_bodies_and_boards() {
        let script = |tick: u32| -> (PlayerIntent, ToolRequest) {
            let intent = PlayerIntent {
                movement: Vec2::new(((tick % 7) as f32 - 3.0) / 3.0, 1.0),
                look: Vec2::new(((tick % 11) as f32 - 5.0) / 50.0, 0.0),
                jump_pressed: tick.is_multiple_of(53),
                ..Default::default()
            };
            let request = match tick % 31 {
                0 => ToolRequest::Push,
                17 => ToolRequest::Pull,
                _ => ToolRequest::None,
            };
            (intent, request)
        };

        let mut left_world = KineticWorld::authored();
        let mut right_world = KineticWorld::authored();
        let mut left = Embodiment::new(&left_world);
        let mut right = Embodiment::new(&right_world);

        for tick in 0..600 {
            let (intent, request) = script(tick);
            left.step(&mut left_world, intent, request);
            right.step(&mut right_world, intent, request);
            assert_eq!(
                left_world.digest(),
                right_world.digest(),
                "board diverged at tick {tick}"
            );
            assert_eq!(left.body, right.body, "body diverged at tick {tick}");
        }
    }
}
