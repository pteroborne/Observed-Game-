//! Physical observation-driven Guardian for the hex facility.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::PlayerId;
use observed_facility::{hex_wfc::HexWfcWorld, map_spec::RoomRole};
use observed_hex::{HexCoord, hex_origin, travel_distance};

use super::{HexLanternState, HexMatchEvent, HexMatchEventKind, HexPlayerState};

/// Route cost at which Guardian pressure reaches zero. Doubles as the search bound in
/// [`HexGuardianState::pressure_for`], since at or past this cost the answer is 0.0
/// whether or not a route exists.
const PRESSURE_FALLOFF_COST: u32 = 12_000;
const CATCH_DISTANCE: f32 = 1.1;
mod physical;

/// The four-tier Tumbler fits the lowest (3 m) doorway, including controller skin.
pub const MAJOR_MODEL_SCALE: f32 = 0.9;
/// World-aligned solid envelope contains the scaled tiers, including their rotating corners.
pub const MAJOR_RADIUS: f32 = 1.35;
pub const MAJOR_HALF_HEIGHT: f32 = 1.4;
const POSITION_ABOVE_FEET: f32 = 0.9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexGuardianStatus {
    Active,
    FrozenByPlayer,
    FrozenByAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HexGuardianState {
    pub cell: HexCoord,
    /// Legacy reference point 0.9 m above the physical feet; presentation uses `feet()`.
    pub position: Vec3,
    pub status: HexGuardianStatus,
    pub target: Option<PlayerId>,
    pub(super) motion: physical::MajorMotion,
}

impl HexGuardianState {
    #[must_use]
    pub fn new(world: &HexWfcWorld) -> Self {
        let cell = guardian_home(world);
        Self {
            cell,
            position: Vec3::from_array(hex_origin(cell))
                + Vec3::Y * (super::FLOOR_SLAB_TOP + POSITION_ABOVE_FEET),
            status: HexGuardianStatus::Active,
            target: None,
            motion: physical::MajorMotion::default(),
        }
    }

    /// A Guardian standing in `cell`, hunting: one released after the match began.
    #[must_use]
    pub fn at(cell: HexCoord) -> Self {
        Self {
            cell,
            position: Vec3::from_array(hex_origin(cell))
                + Vec3::Y * (super::FLOOR_SLAB_TOP + POSITION_ABOVE_FEET),
            status: HexGuardianStatus::Active,
            target: None,
            motion: physical::MajorMotion::default(),
        }
    }

    #[must_use]
    pub fn pressure_for(&self, world: &HexWfcWorld, player: &HexPlayerState) -> f32 {
        if player.cell == self.cell {
            return (1.0 - player.position.distance(self.position) / 12.0).clamp(0.55, 1.0);
        }
        // Bounded at exactly the cost where the formula saturates. `1.0 - cost/PRESSURE_
        // FALLOFF_COST` is <= 0 for any cost at or above the falloff, and the clamp floors
        // it at 0.0 — which is also what a missing route yields. So a route priced at or
        // past the bound is indistinguishable from no route, and searching for one is
        // wasted work. Presentation calls this every frame, per lantern, and the unbounded
        // form expanded the whole component whenever the Guardian was far away.
        world
            .route_within_cost(player.cell, self.cell, PRESSURE_FALLOFF_COST)
            .map_or(0.0, |route| {
                (1.0 - route.cost_millis as f32 / PRESSURE_FALLOFF_COST as f32).clamp(0.0, 0.8)
            })
    }

    #[must_use]
    pub fn physically_placed(&self) -> bool {
        self.motion.placed()
    }

    /// The sole visual/sight origin, on the physical walking surface.
    #[must_use]
    pub fn feet(&self) -> Vec3 {
        self.position - Vec3::Y * POSITION_ABOVE_FEET
    }

    /// Decide whether to freeze, catch, or pursue; physical movement is resolved by
    /// the match afterward, never by cell teleportation or a presentation glide.
    pub(super) fn step(
        &mut self,
        tick: u64,
        world: &HexWfcWorld,
        lanterns: &HexLanternState,
        players: &mut BTreeMap<PlayerId, HexPlayerState>,
        events: &mut Vec<HexMatchEvent>,
        bounds: HexGuardianBounds<'_>,
    ) -> Option<(HexCoord, Vec3)> {
        let HexGuardianBounds {
            prison,
            closed,
            directive,
            clear,
            eye_height,
        } = bounds;
        let observed = players
            .values()
            .any(|player| player_sees_guardian(player, self, eye_height, clear));
        self.status = if observed {
            HexGuardianStatus::FrozenByPlayer
        } else if lanterns.anchors_blueprint_cell(world, self.cell) {
            HexGuardianStatus::FrozenByAnchor
        } else {
            HexGuardianStatus::Active
        };
        if self.status != HexGuardianStatus::Active {
            self.motion.stop();
            return None;
        }
        let same_cell = players
            .values()
            .any(|player| player.in_facility() && player.cell == self.cell);
        if let Some(goal) = directive
            && goal != self.cell
            && !same_cell
        {
            self.target = None;
            return Some((goal, Vec3::from_array(hex_origin(goal))));
        }
        let target_id = leading_player(world, players, prison.is_some())?;
        self.target = Some(target_id);
        let player = &players[&target_id];
        // A catch needs actual proximity, deck height and unobstructed sight, not
        // merely equal cell labels. A partition cannot catch through a wall.
        if player.cell == self.cell
            && player
                .position
                .with_y(0.0)
                .distance(self.position.with_y(0.0))
                <= CATCH_DISTANCE
            && (player.position.y - self.position.y).abs() <= 1.5
            && !closed(player.cell, self.cell)
            && clear(self.position, player.position)
            && let Some(destination) = recovery_destination(world, self.cell)
        {
            let player = players.get_mut(&target_id).expect("target exists");
            let from = player.cell;
            player.cell = destination;
            player.position = Vec3::from_array(hex_origin(destination)) + Vec3::Y * 0.9;
            events.push(HexMatchEvent {
                tick,
                kind: HexMatchEventKind::GuardianCatch,
                player: Some(target_id),
                cell: Some(from),
            });
            self.cell = guardian_home(world);
            self.position = Vec3::from_array(hex_origin(self.cell))
                + Vec3::Y * (super::FLOOR_SLAB_TOP + POSITION_ABOVE_FEET);
            self.target = None;
            self.motion = physical::MajorMotion::default();
            return None;
        }
        Some((player.cell, player.position))
    }
}

fn guardian_home(world: &HexWfcWorld) -> HexCoord {
    world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.role == RoomRole::GuardianControl)
        .or_else(|| world.blueprints.first())
        .map_or_else(|| world.config.spawn(), |blueprint| blueprint.anchor)
}

fn leading_player(
    world: &HexWfcWorld,
    players: &BTreeMap<PlayerId, HexPlayerState>,
    hunt_alone: bool,
) -> Option<PlayerId> {
    let active = players
        .values()
        .filter(|player| player.in_facility())
        .count();
    // The Guardian is competitive pressure, not a single-player traversal
    // blocker. Once only one runner remains (including one-player labs), the
    // route itself is the remaining challenge.
    if active == 0 || (active == 1 && !hunt_alone) {
        return None;
    }
    players
        .values()
        .filter(|player| player.in_facility())
        .min_by_key(|player| {
            (
                // Target selection runs every fixed tick. Exact A* here made the
                // Guardian perform one full-facility search per runner even though it
                // only needs a stable estimate of who is ahead; the route itself is
                // still resolved when the Guardian takes its infrequent 120-tick step.
                travel_distance(player.cell, world.config.exit()),
                player.id,
            )
        })
        .map(|player| player.id)
}

fn recovery_destination(world: &HexWfcWorld, guardian_cell: HexCoord) -> Option<HexCoord> {
    world
        .blueprints
        .iter()
        .filter(|blueprint| blueprint.anchor != guardian_cell)
        .max_by_key(|blueprint| {
            (
                u8::from(blueprint.role == RoomRole::Recovery),
                world
                    .route_between_cells(blueprint.anchor, world.config.exit())
                    .map_or(0, |route| route.cost_millis),
                std::cmp::Reverse(blueprint.anchor),
            )
        })
        .map(|blueprint| blueprint.anchor)
}

/// Where a Guardian may not go - the prison's lobby, and across a closed door - and where
/// the Rogue has sent it.
#[derive(Clone, Copy)]
pub(super) struct HexGuardianBounds<'a> {
    pub prison: Option<&'a super::prison::HexPrison>,
    /// Whether a closed door stands between two neighbouring cells.
    pub closed: &'a dyn Fn(HexCoord, HexCoord) -> bool,
    /// The Rogue's directive, which a major Guardian walks to instead of hunting.
    pub directive: Option<HexCoord>,
    /// Whether nothing solid stands between two points: the match's colliders.
    pub clear: &'a dyn Fn(Vec3, Vec3) -> bool,
    /// How far above a body's centre its eye is.
    pub eye_height: f32,
}

/// Samples from the four-tier major's lower body through its crown and eye.
/// Seeing its head over cover must count even when its lower tiers are hidden.
const SEEN_AT: [f32; 5] = [0.3, 0.8, 1.4, 2.1, 2.65];

impl super::HexWfcMatch {
    /// Whether a supplied embodied viewpoint sees the major's actual solid pose.
    /// Uses the same range, samples and structural transparency as freezing.
    #[must_use]
    pub fn major_visible_from(&self, viewer: &HexPlayerState, major: &HexGuardianState) -> bool {
        player_sees_guardian(viewer, major, self.eye_height(), &|a, b| {
            super::sight::line_is_clear(&self.physics, &self.geometry, a, b)
        })
    }

    /// Send every major Guardian to `cell` instead of hunting, or back to the hunt with
    /// `None`. The Ascent rules own the directive and say when it is spent
    /// (`ascent::sim::ArchitectLab::directed`); this only walks the bodies.
    pub fn direct_guardians(&mut self, cell: Option<HexCoord>) {
        self.guardian_directive = cell;
    }

    /// Where the major Guardians have been sent, if anywhere.
    #[must_use]
    pub const fn guardian_directive(&self) -> Option<HexCoord> {
        self.guardian_directive
    }
}

impl HexGuardianBounds<'_> {
    /// No prison, no doors, and nothing solid in the way.
    #[cfg(test)]
    pub(super) const OPEN: HexGuardianBounds<'static> = HexGuardianBounds {
        prison: None,
        closed: &|_, _| false,
        directive: None,
        clear: &|_, _| true,
        eye_height: 0.7,
    };
}

fn player_sees_guardian(
    player: &HexPlayerState,
    guardian: &HexGuardianState,
    eye_height: f32,
    clear: &dyn Fn(Vec3, Vec3) -> bool,
) -> bool {
    if !player.in_facility() {
        return false;
    }
    let eye = player.position + Vec3::Y * eye_height;
    let forward = Vec3::new(player.yaw.sin(), 0.0, -player.yaw.cos());
    let right = Vec3::new(player.yaw.cos(), 0.0, player.yaw.sin());
    // Physical sight is authoritative. A visible body across an atrium or several
    // corridor cells must freeze even where no short walking route connects it.
    SEEN_AT.into_iter().any(|height| {
        [
            Vec3::ZERO,
            Vec3::X * 0.6,
            -Vec3::X * 0.6,
            Vec3::Z * 0.6,
            -Vec3::Z * 0.6,
        ]
        .into_iter()
        .any(|side| {
            let offset = guardian.feet() + Vec3::Y * height + side - eye;
            let distance = offset.length();
            let ahead = offset.dot(forward);
            let horizontal = offset.dot(right).atan2(ahead).abs();
            let pitch = offset.y.atan2(offset.with_y(0.0).length());
            distance <= super::SIGHT_REACH
                && horizontal <= 1.14
                && (pitch - player.pitch).abs() <= 0.85
                && clear(eye, eye + offset)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_core::PlayerId;
    use observed_facility::hex_wfc::HexWfcConfig;

    fn guardian_world() -> (HexWfcWorld, HexCoord) {
        let config = HexWfcConfig {
            levels: 3,
            ..HexWfcConfig::default()
        };
        for seed in 0..2_000 {
            let Ok(world) = HexWfcWorld::generate(seed, config) else {
                continue;
            };
            if let Some(anchor) = world
                .blueprints
                .iter()
                .find(|blueprint| blueprint.role == RoomRole::GuardianControl)
                .map(|room| room.anchor)
            {
                return (world, anchor);
            }
        }
        panic!("seed corpus did not stamp GuardianControl");
    }

    fn player(id: PlayerId, cell: HexCoord, position: Vec3, yaw: f32) -> HexPlayerState {
        HexPlayerState {
            id,
            team: observed_core::TeamId((id.0 / 2) as u8),
            cell,
            position,
            yaw,
            pitch: 0.0,
            escaped: false,
            place: crate::hex_wfc::model::HexBodyPlace::Facility,
        }
    }

    #[test]
    fn guardian_spawns_in_control_room_when_present() {
        let (world, anchor) = guardian_world();
        assert_eq!(HexGuardianState::new(&world).cell, anchor);
    }

    #[test]
    fn direct_observation_freezes_the_physical_guardian() {
        let (world, cell) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let before = guardian.position;
        let id = PlayerId(0);
        let mut players =
            BTreeMap::from([(id, player(id, cell, guardian.position + Vec3::Z * 5.0, 0.0))]);
        let lanterns = HexLanternState::new([id], &world);
        guardian.step(
            120,
            &world,
            &lanterns,
            &mut players,
            &mut Vec::new(),
            HexGuardianBounds::OPEN,
        );
        assert_eq!(guardian.status, HexGuardianStatus::FrozenByPlayer);
        assert_eq!(guardian.position, before);
    }

    #[test]
    fn direct_sight_across_multiple_cells_freezes_but_a_wall_does_not() {
        let (world, cell) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let before = guardian.position;
        let id = PlayerId(0);
        let distant = world.config.spawn();
        let mut players = BTreeMap::from([(id, player(id, distant, before + Vec3::Z * 35.0, 0.0))]);
        let lanterns = HexLanternState::new([id], &world);
        guardian.step(
            120,
            &world,
            &lanterns,
            &mut players,
            &mut Vec::new(),
            HexGuardianBounds::OPEN,
        );
        assert_eq!(guardian.status, HexGuardianStatus::FrozenByPlayer);
        assert_eq!(guardian.position, before);
        guardian.step(
            121,
            &world,
            &lanterns,
            &mut players,
            &mut Vec::new(),
            HexGuardianBounds {
                clear: &|_, _| false,
                ..HexGuardianBounds::OPEN
            },
        );
        assert_eq!(guardian.status, HexGuardianStatus::Active);
        players.get_mut(&id).unwrap().cell = cell;
        players.get_mut(&id).unwrap().pitch = 1.4;
        guardian.step(
            122,
            &world,
            &lanterns,
            &mut players,
            &mut Vec::new(),
            HexGuardianBounds::OPEN,
        );
        assert_eq!(
            guardian.status,
            HexGuardianStatus::Active,
            "looking up does not observe it"
        );
    }

    #[test]
    fn the_major_at_the_side_of_the_view_still_freezes() {
        let (world, cell) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let before = guardian.position;
        let id = PlayerId(0);
        let angle = 1.0_f32;
        let offset = Vec3::new(angle.sin(), 0.0, -angle.cos()) * 5.0;
        let mut players = BTreeMap::from([(id, player(id, cell, before - offset, 0.0))]);
        let lanterns = HexLanternState::new([id], &world);
        guardian.step(
            120,
            &world,
            &lanterns,
            &mut players,
            &mut Vec::new(),
            HexGuardianBounds::OPEN,
        );
        assert_eq!(guardian.status, HexGuardianStatus::FrozenByPlayer);
        assert_eq!(guardian.position, before);
    }

    #[test]
    fn the_major_freezes_when_only_its_crown_is_visible_over_cover() {
        let (world, cell) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let before = guardian.position;
        let id = PlayerId(0);
        let mut players = BTreeMap::from([(id, player(id, cell, before + Vec3::Z * 5.0, 0.0))]);
        let lanterns = HexLanternState::new([id], &world);
        guardian.step(
            120,
            &world,
            &lanterns,
            &mut players,
            &mut Vec::new(),
            HexGuardianBounds {
                clear: &|_, to| to.y > before.y + 1.5,
                ..HexGuardianBounds::OPEN
            },
        );
        assert_eq!(guardian.status, HexGuardianStatus::FrozenByPlayer);
        assert_eq!(guardian.position, before);
    }

    #[test]
    fn a_lantern_anchoring_the_control_room_freezes_the_guardian() {
        let (world, cell) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let id = PlayerId(0);
        let mut lanterns = HexLanternState::new([id], &world);
        let room = world
            .blueprints
            .iter()
            .find(|blueprint| blueprint.cells.contains(&cell))
            .expect("guardian room");
        lanterns
            .deploy(
                id,
                observed_facility::hex_wfc::HexThresholdKey {
                    room_generation_key: room.generation_key(),
                    port: "lower_port",
                },
                cell,
                guardian.position,
            )
            .expect("deploy anchor");
        let far = world.config.exit();
        let mut players = BTreeMap::from([(
            id,
            player(id, far, Vec3::from_array(hex_origin(far)) + Vec3::Y, 0.0),
        )]);
        guardian.step(
            120,
            &world,
            &lanterns,
            &mut players,
            &mut Vec::new(),
            HexGuardianBounds::OPEN,
        );
        assert_eq!(guardian.status, HexGuardianStatus::FrozenByAnchor);
    }

    #[test]
    fn an_unobserved_catch_sends_the_leader_to_recovery() {
        let (world, _) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let cell = world.config.exit();
        guardian.cell = cell;
        guardian.position = Vec3::from_array(hex_origin(cell)) + Vec3::Y * 0.9;
        let id = PlayerId(0);
        let rival_id = PlayerId(1);
        let start = guardian.position + Vec3::Z;
        let rival_cell = world.config.spawn();
        let mut players = BTreeMap::from([
            (
                id,
                // Facing +Z while the Guardian is behind them along -Z.
                player(id, cell, start, std::f32::consts::PI),
            ),
            (
                rival_id,
                player(
                    rival_id,
                    rival_cell,
                    Vec3::from_array(hex_origin(rival_cell)) + Vec3::Y,
                    0.0,
                ),
            ),
        ]);
        let lanterns = HexLanternState::new([id, rival_id], &world);
        let mut events = Vec::new();
        guardian.step(
            1,
            &world,
            &lanterns,
            &mut players,
            &mut events,
            HexGuardianBounds::OPEN,
        );
        assert_ne!(players[&id].cell, cell);
        assert_eq!(guardian.cell, guardian_home(&world));
        assert_eq!(guardian.target, None);
        assert!(events.iter().any(|event| {
            event.kind == HexMatchEventKind::GuardianCatch && event.player == Some(id)
        }));
    }
}
