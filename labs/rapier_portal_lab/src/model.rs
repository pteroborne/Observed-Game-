//! Pure portal/WFC/lock state. Rendering and Rapier only consume this model.

use bevy::math::{Vec2, Vec3};

use crate::authoring::{ArchitectureKind, AuthoredModule};

pub const SOURCE_THRESHOLD_Z: f32 = -8.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Place {
    SourceRoom,
    Hallway,
    DestinationRoom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModulePlacement {
    pub kind: ArchitectureKind,
    pub z_offset: f32,
    pub length: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Composition {
    pub generation: u32,
    pub placements: [ModulePlacement; 2],
}

impl Composition {
    pub fn total_length(&self) -> f32 {
        self.placements.iter().map(|module| module.length).sum()
    }

    pub fn labels(&self) -> String {
        format!(
            "{} -> {}",
            self.placements[0].kind.label(),
            self.placements[1].kind.label()
        )
    }
}

fn module(modules: &[AuthoredModule], kind: ArchitectureKind) -> &AuthoredModule {
    modules
        .iter()
        .find(|module| module.kind == kind)
        .expect("the authored kit contains both WFC tiles")
}

/// Collapse a two-cell WFC strip. Both cells accept both ground-port modules; the
/// uniqueness constraint removes the first tile from the second cell's domain.
pub fn solve_composition(seed: u64, generation: u32, modules: &[AuthoredModule]) -> Composition {
    let first = if (seed.wrapping_add(u64::from(generation)) & 1) == 0 {
        ArchitectureKind::Gantry
    } else {
        ArchitectureKind::Colonnade
    };
    let second = ArchitectureKind::ALL
        .into_iter()
        .find(|kind| *kind != first)
        .expect("two-tile domain leaves one tile after uniqueness propagation");
    let first_length = module(modules, first).length;
    let second_length = module(modules, second).length;
    Composition {
        generation,
        placements: [
            ModulePlacement {
                kind: first,
                z_offset: 0.0,
                length: first_length,
            },
            ModulePlacement {
                kind: second,
                z_offset: -first_length,
                length: second_length,
            },
        ],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PortalModel {
    pub seed: u64,
    pub composition: Composition,
    pub place: Place,
    pub player_observed: bool,
    pub anchored: bool,
    pub blame_attempts: u32,
    pub blame_commits: u32,
}

impl PortalModel {
    pub fn new(seed: u64, modules: &[AuthoredModule]) -> Self {
        Self {
            seed,
            composition: solve_composition(seed, 0, modules),
            place: Place::SourceRoom,
            player_observed: true,
            anchored: false,
            blame_attempts: 0,
            blame_commits: 0,
        }
    }

    pub fn connection_locked(&self) -> bool {
        self.player_observed || self.anchored
    }

    /// The diegetic frame indicator reports durable anchor state only.
    pub fn anchor_indicator_lit(&self) -> bool {
        self.anchored
    }

    pub fn update_player_observation(&mut self, position: Vec3, yaw: f32) {
        if self.place == Place::Hallway {
            // Occupying the connection is observation even when neither threshold is in view.
            self.player_observed = true;
            return;
        }
        let threshold = match self.place {
            Place::SourceRoom => Vec2::new(0.0, SOURCE_THRESHOLD_Z),
            Place::DestinationRoom => {
                self.player_observed = false;
                return;
            }
            Place::Hallway => unreachable!(),
        };
        let origin = Vec2::new(position.x, position.z);
        let to_threshold = threshold - origin;
        let distance = to_threshold.length();
        let forward = Vec2::new(-yaw.sin(), -yaw.cos());
        self.player_observed = distance < 14.0
            && to_threshold
                .try_normalize()
                .is_some_and(|direction| forward.dot(direction) > 0.72);
    }

    pub fn toggle_anchor(&mut self) {
        self.anchored = !self.anchored;
    }

    /// Ask BLAME to refactor the remote hallway. Returns whether a new WFC result committed.
    pub fn request_blame(&mut self, modules: &[AuthoredModule]) -> bool {
        self.blame_attempts += 1;
        if self.connection_locked() {
            return false;
        }
        let generation = self.composition.generation + 1;
        self.composition = solve_composition(self.seed, generation, modules);
        self.blame_commits += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authoring::authored_modules;

    #[test]
    fn wfc_uses_both_authored_types_and_alternates_deterministically() {
        let modules = authored_modules();
        let a = solve_composition(42, 0, &modules);
        let b = solve_composition(42, 1, &modules);
        assert_ne!(a.placements[0].kind, a.placements[1].kind);
        assert_ne!(a.placements[0].kind, b.placements[0].kind);
        assert_eq!(a, solve_composition(42, 0, &modules));
    }

    #[test]
    fn player_observation_locks_without_lighting_anchor_indicator() {
        let modules = authored_modules();
        let mut model = PortalModel::new(7, &modules);
        model.update_player_observation(Vec3::new(0.0, 1.0, 0.0), 0.0);
        let before = model.composition.clone();
        assert!(model.player_observed);
        assert!(model.connection_locked());
        assert!(!model.anchor_indicator_lit());
        assert!(!model.request_blame(&modules));
        assert_eq!(model.composition, before);
    }

    #[test]
    fn looking_away_releases_temporary_lock_and_blame_recomposes() {
        let modules = authored_modules();
        let mut model = PortalModel::new(7, &modules);
        model.update_player_observation(Vec3::new(0.0, 1.0, 0.0), std::f32::consts::PI);
        let before = model.composition.clone();
        assert!(!model.player_observed);
        assert!(model.request_blame(&modules));
        assert_ne!(model.composition, before);
        assert!(!model.anchor_indicator_lit());
    }

    #[test]
    fn anchor_locks_while_unobserved_and_lights_indicator() {
        let modules = authored_modules();
        let mut model = PortalModel::new(7, &modules);
        model.update_player_observation(Vec3::new(0.0, 1.0, 0.0), std::f32::consts::PI);
        model.toggle_anchor();
        let before = model.composition.clone();
        assert!(model.connection_locked());
        assert!(model.anchor_indicator_lit());
        assert!(!model.request_blame(&modules));
        assert_eq!(model.composition, before);
    }
}
