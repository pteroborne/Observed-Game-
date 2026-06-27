//! Phase 22: atomic **rewire-while-unobserved** rendering.
//!
//! Phase 21 tells us exactly which doorway apertures are visible. This model adds
//! the missing rendering contract:
//!
//! 1. Decoherence proposes one deterministic batch containing hidden portals.
//! 2. The batch is atomic: every affected rendered module changes together.
//! 3. Commit is legal only while every affected portal is still unseen and no
//!    traversal occupies its doorway.
//! 4. A traversal captures the currently rendered destination. A pending batch
//!    cannot change that destination until the traversal has completed.
//!
//! The presentation layer replaces actual 3D module entities only after a legal
//! commit. Therefore a swap cannot occur in the camera frustum, and a player can
//! never lose the doorway or destination they are currently crossing.

use bevy::prelude::*;
use fps_visibility_lab::field::{
    GAP_HALF, VisionField, door_pos, forward, room_center, side_dir, visible, walls,
};
use observation_lab::model::Side;
use observed_core::RoomId;
use player_input::PlayerIntent;

pub const GATEWAY_COUNT: usize = 4;
pub const TRANSIT_SECONDS: f32 = 1.4;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GatewayId(pub u8);

impl GatewayId {
    pub const NORTH: Self = Self(0);
    pub const EAST: Self = Self(1);
    pub const SOUTH: Self = Self(2);
    pub const WEST: Self = Self(3);
    pub const ALL: [Self; GATEWAY_COUNT] = [Self::NORTH, Self::EAST, Self::SOUTH, Self::WEST];

    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn side(self) -> Side {
        Side::ALL[self.index()]
    }

    pub fn label(self) -> &'static str {
        match self.side() {
            Side::North => "N",
            Side::East => "E",
            Side::South => "S",
            Side::West => "W",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleId(pub u8);

impl ModuleId {
    pub const AMBER_COLUMNS: Self = Self(0);
    pub const CYAN_CROSSBEAM: Self = Self(1);
    pub const MAGENTA_STEPS: Self = Self(2);
    pub const GREEN_ARCH: Self = Self(3);
    pub const ALL: [Self; GATEWAY_COUNT] = [
        Self::AMBER_COLUMNS,
        Self::CYAN_CROSSBEAM,
        Self::MAGENTA_STEPS,
        Self::GREEN_ARCH,
    ];

    pub fn label(self) -> &'static str {
        match self.0 {
            0 => "amber columns",
            1 => "cyan crossbeam",
            2 => "magenta steps",
            3 => "green arch",
            _ => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GatewayState {
    pub id: GatewayId,
    pub displayed: ModuleId,
    pub revision: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwapChange {
    pub gateway: GatewayId,
    pub from: ModuleId,
    pub to: ModuleId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapBatch {
    pub id: u32,
    pub changes: Vec<SwapChange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitRecord {
    pub batch: u32,
    pub changes: Vec<SwapChange>,
    pub all_hidden: bool,
    pub all_clear: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transit {
    pub gateway: GatewayId,
    pub destination: ModuleId,
    pub progress: f32,
}

#[derive(Resource, Clone, Debug)]
pub struct RewireStage {
    pub vision: VisionField,
    pub gateways: [GatewayState; GATEWAY_COUNT],
    pub pending: Option<SwapBatch>,
    pub transit: Option<Transit>,
    pub last_arrival: Option<ModuleId>,
    pub request_count: u32,
    pub commit_count: u32,
    pub seam_violations: u32,
    pub commit_log: Vec<CommitRecord>,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for RewireStage {
    fn default() -> Self {
        let mut vision = VisionField::authored();
        vision.eye = room_center(RoomId(4));
        vision.yaw = std::f32::consts::FRAC_PI_2;
        vision.recompute();
        Self {
            vision,
            gateways: std::array::from_fn(|index| GatewayState {
                id: GatewayId(index as u8),
                displayed: ModuleId::ALL[index],
                revision: 0,
            }),
            pending: None,
            transit: None,
            last_arrival: None,
            request_count: 0,
            commit_count: 0,
            seam_violations: 0,
            commit_log: Vec::new(),
            reset_count: 0,
            last_event: "Turn away, decohere, then turn back: hidden modules swap atomically."
                .to_string(),
        }
    }
}

impl RewireStage {
    pub fn gateway(&self, id: GatewayId) -> &GatewayState {
        &self.gateways[id.index()]
    }

    pub fn gateway_mut(&mut self, id: GatewayId) -> &mut GatewayState {
        &mut self.gateways[id.index()]
    }

    pub fn portal_position(id: GatewayId) -> Vec2 {
        door_pos(RoomId(4), id.side())
    }

    pub fn portal_visible(&self, id: GatewayId) -> bool {
        let centre = Self::portal_position(id);
        let normal = side_dir(id.side());
        let tangent = Vec2::new(-normal.y, normal.x);
        let facing = forward(self.vision.yaw);
        let occluders = walls();
        [-0.92, -0.46, 0.0, 0.46, 0.92].into_iter().any(|fraction| {
            visible(
                self.vision.eye,
                facing,
                centre + tangent * GAP_HALF * fraction,
                &occluders,
            )
        })
    }

    pub fn occupied(&self, id: GatewayId) -> bool {
        self.transit.is_some_and(|transit| transit.gateway == id)
    }

    pub fn faced_gateway(&self) -> GatewayId {
        let facing = forward(self.vision.yaw);
        GatewayId::ALL
            .into_iter()
            .max_by(|a, b| {
                let a_dot = side_dir(a.side()).dot(facing);
                let b_dot = side_dir(b.side()).dot(facing);
                a_dot.total_cmp(&b_dot)
            })
            .unwrap_or(GatewayId::NORTH)
    }

    pub fn set_facing(&mut self, gateway: GatewayId) {
        self.vision.yaw = match gateway.side() {
            Side::North => 0.0,
            Side::East => std::f32::consts::FRAC_PI_2,
            Side::South => std::f32::consts::PI,
            Side::West => std::f32::consts::PI + std::f32::consts::FRAC_PI_2,
        };
        self.vision.recompute();
    }

    pub fn advance_camera(&mut self, intent: PlayerIntent, dt: f32) {
        if self.transit.is_some() {
            self.advance_transit(dt);
            return;
        }
        // Phase 22 isolates the render transition. Rotation is live; position
        // remains in the hub so portal visibility is the only swap gate.
        self.vision.advance_camera(
            PlayerIntent {
                look: intent.look,
                ..Default::default()
            },
            dt,
        );
    }

    /// Propose a deterministic permutation of the modules behind all currently
    /// hidden portals. The rendered state does not change until `commit_pending`.
    pub fn request_rewire(&mut self) -> bool {
        if self.pending.is_some() {
            self.last_event = "A swap batch is already pending.".to_string();
            return false;
        }

        self.request_count += 1;
        let hidden: Vec<GatewayId> = GatewayId::ALL
            .into_iter()
            .filter(|gateway| !self.portal_visible(*gateway))
            .collect();
        if hidden.len() < 2 {
            self.last_event = "Fewer than two portals are hidden; no safe batch.".to_string();
            return false;
        }

        let offset = 1 + (self.request_count as usize % (hidden.len() - 1));
        let modules: Vec<ModuleId> = hidden
            .iter()
            .map(|gateway| self.gateway(*gateway).displayed)
            .collect();
        let changes = hidden
            .iter()
            .enumerate()
            .map(|(index, gateway)| SwapChange {
                gateway: *gateway,
                from: modules[index],
                to: modules[(index + offset) % modules.len()],
            })
            .collect();

        self.pending = Some(SwapBatch {
            id: self.request_count,
            changes,
        });
        self.last_event = format!(
            "Batch {} staged behind {} hidden portals.",
            self.request_count,
            hidden.len()
        );
        true
    }

    /// Atomically install a pending batch only when every affected aperture is
    /// outside the observed set and clear of an in-progress traversal.
    pub fn commit_pending(&mut self) -> bool {
        let Some(batch) = self.pending.as_ref() else {
            return false;
        };
        let all_hidden = batch
            .changes
            .iter()
            .all(|change| !self.portal_visible(change.gateway));
        let all_clear = batch
            .changes
            .iter()
            .all(|change| !self.occupied(change.gateway));
        if !all_hidden || !all_clear {
            self.last_event = if !all_hidden {
                format!("Batch {} held: an affected portal is visible.", batch.id)
            } else {
                format!("Batch {} held: a player occupies its doorway.", batch.id)
            };
            return false;
        }

        let batch = self.pending.take().expect("pending batch was checked");
        for change in &batch.changes {
            let gateway = self.gateway_mut(change.gateway);
            debug_assert_eq!(gateway.displayed, change.from);
            gateway.displayed = change.to;
            gateway.revision += 1;
        }
        if !all_hidden || !all_clear {
            self.seam_violations += 1;
        }
        self.commit_count += 1;
        self.commit_log.push(CommitRecord {
            batch: batch.id,
            changes: batch.changes.clone(),
            all_hidden,
            all_clear,
        });
        self.last_event = format!(
            "Batch {} committed atomically off-camera; {} modules replaced.",
            batch.id,
            batch.changes.len()
        );
        true
    }

    /// Begin a diagnostic traversal through a gateway. The destination is captured
    /// from the rendered module and remains stable for the whole crossing.
    pub fn begin_transit(&mut self, gateway: GatewayId) -> bool {
        if self.transit.is_some() {
            return false;
        }
        let destination = self.gateway(gateway).displayed;
        self.transit = Some(Transit {
            gateway,
            destination,
            progress: 0.0,
        });
        self.last_event = format!(
            "Crossing {} toward {}; its rendered route is pinned.",
            gateway.label(),
            destination.label()
        );
        true
    }

    pub fn begin_faced_transit(&mut self) -> bool {
        let gateway = self.faced_gateway();
        self.begin_transit(gateway)
    }

    pub fn advance_transit(&mut self, dt: f32) {
        let Some(mut transit) = self.transit else {
            return;
        };
        transit.progress = (transit.progress + dt / TRANSIT_SECONDS).min(1.0);
        let centre = room_center(RoomId(4));
        let direction = side_dir(transit.gateway.side());
        self.vision.eye = centre + direction * (5.5 * transit.progress);
        self.vision.recompute();

        if transit.progress >= 1.0 {
            self.last_arrival = Some(transit.destination);
            self.transit = None;
            self.vision.eye = centre;
            self.vision.recompute();
            self.last_event = format!(
                "Arrived at {} through the pinned {} doorway.",
                transit.destination.label(),
                transit.gateway.label()
            );
        } else {
            self.transit = Some(transit);
        }
    }

    pub fn reset(&mut self) {
        let resets = self.reset_count + 1;
        *self = Self::default();
        self.reset_count = resets;
    }

    pub fn route_signature(&self) -> [ModuleId; GATEWAY_COUNT] {
        self.gateways.map(|gateway| gateway.displayed)
    }

    pub fn no_pop_proven(&self) -> bool {
        self.commit_count > 0
            && self.seam_violations == 0
            && self
                .commit_log
                .iter()
                .all(|record| record.all_hidden && record.all_clear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_hub_has_four_distinct_modules_and_one_faced_portal() {
        let stage = RewireStage::default();
        let distinct = stage
            .route_signature()
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(distinct.len(), GATEWAY_COUNT);
        assert!(stage.portal_visible(GatewayId::EAST));
        assert!(!stage.portal_visible(GatewayId::WEST));
    }

    #[test]
    fn a_partly_visible_aperture_is_conservatively_frozen() {
        let mut stage = RewireStage::default();
        // The east doorway centre is 45 degrees left of this view (outside the
        // 35-degree half-FOV), but its southern edge is still inside the frustum.
        stage.vision.yaw = 3.0 * std::f32::consts::FRAC_PI_4;
        stage.vision.recompute();
        let centre_door = stage.vision.graph.door_id(RoomId(4), Side::East);
        assert!(!stage.vision.seen_doors[centre_door.0 as usize]);
        assert!(
            stage.portal_visible(GatewayId::EAST),
            "any visible aperture sample must freeze the whole rendered portal"
        );
    }

    #[test]
    fn turn_away_rewire_turn_back_changes_the_world_without_a_visible_commit() {
        let mut stage = RewireStage::default();
        let before = stage.gateway(GatewayId::EAST).displayed;
        assert!(stage.portal_visible(GatewayId::EAST));

        stage.set_facing(GatewayId::NORTH);
        assert!(!stage.portal_visible(GatewayId::EAST));
        assert!(stage.request_rewire());
        assert!(stage.commit_pending());

        stage.set_facing(GatewayId::EAST);
        assert_ne!(stage.gateway(GatewayId::EAST).displayed, before);
        assert!(stage.no_pop_proven());
    }

    #[test]
    fn the_visible_portal_is_excluded_and_never_changes() {
        let mut stage = RewireStage::default();
        let east = stage.gateway(GatewayId::EAST).displayed;
        assert!(stage.request_rewire());
        assert!(stage.commit_pending());
        assert_eq!(stage.gateway(GatewayId::EAST).displayed, east);
        assert!(
            stage.commit_log[0]
                .changes
                .iter()
                .all(|change| change.gateway != GatewayId::EAST)
        );
    }

    #[test]
    fn a_batch_is_atomic_and_preserves_the_module_permutation() {
        let mut stage = RewireStage::default();
        let before = stage.route_signature();
        assert!(stage.request_rewire());
        let affected = stage.pending.as_ref().unwrap().changes.len();
        assert!(stage.commit_pending());
        let after = stage.route_signature();
        assert_ne!(before, after);
        assert_eq!(
            before
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            after.into_iter().collect::<std::collections::BTreeSet<_>>()
        );
        assert_eq!(
            stage
                .gateways
                .iter()
                .filter(|gateway| gateway.revision == 1)
                .count(),
            affected
        );
    }

    #[test]
    fn a_visible_pending_portal_blocks_the_whole_batch() {
        let mut stage = RewireStage::default();
        stage.set_facing(GatewayId::NORTH);
        assert!(stage.request_rewire());
        let before = stage.route_signature();
        let affected = stage.pending.as_ref().unwrap().changes[0].gateway;
        stage.set_facing(affected);
        assert!(!stage.commit_pending());
        assert_eq!(stage.route_signature(), before);
        assert!(stage.pending.is_some());
        stage.set_facing(GatewayId::NORTH);
        if stage.portal_visible(affected) {
            stage.set_facing(GatewayId::EAST);
        }
        assert!(stage.commit_pending());
    }

    #[test]
    fn transit_holds_a_pending_batch_and_arrives_at_the_old_destination() {
        let mut stage = RewireStage::default();
        stage.set_facing(GatewayId::NORTH);
        let old = stage.gateway(GatewayId::EAST).displayed;
        assert!(stage.begin_transit(GatewayId::EAST));
        assert!(stage.request_rewire());
        assert!(
            stage
                .pending
                .as_ref()
                .unwrap()
                .changes
                .iter()
                .any(|change| change.gateway == GatewayId::EAST)
        );
        let before = stage.route_signature();
        assert!(
            !stage.commit_pending(),
            "occupied doorway must hold the batch"
        );
        assert_eq!(stage.route_signature(), before);

        while stage.transit.is_some() {
            stage.advance_transit(1.0 / 60.0);
            assert_eq!(
                stage.gateway(GatewayId::EAST).displayed,
                old,
                "the route cannot change under the crossing player"
            );
        }
        assert_eq!(stage.last_arrival, Some(old));
        assert!(stage.commit_pending());
        assert_ne!(stage.gateway(GatewayId::EAST).displayed, old);
    }

    #[test]
    fn the_same_sequence_produces_the_same_batches_and_routes() {
        let run = || {
            let mut stage = RewireStage::default();
            for gateway in [
                GatewayId::NORTH,
                GatewayId::WEST,
                GatewayId::SOUTH,
                GatewayId::EAST,
            ] {
                stage.set_facing(gateway);
                stage.request_rewire();
                stage.commit_pending();
            }
            (stage.route_signature(), stage.commit_log)
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn reset_restores_the_authored_routes_and_clears_transitions() {
        let mut stage = RewireStage::default();
        stage.set_facing(GatewayId::NORTH);
        stage.request_rewire();
        stage.commit_pending();
        stage.begin_transit(GatewayId::EAST);
        stage.reset();
        assert_eq!(stage.route_signature(), ModuleId::ALL);
        assert!(stage.pending.is_none());
        assert!(stage.transit.is_none());
        assert_eq!(stage.commit_count, 0);
        assert_eq!(stage.reset_count, 1);
    }
}
