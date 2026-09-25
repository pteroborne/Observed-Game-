//! The objectives' bodies, in the equipment's hexagonal language: a keystone crystal
//! over its plinth, console plinths with lit screens, a station's sync column, and the
//! exit's column of light.
//!
//! Hardware comes from `observed_style::equipment` and never glows; every lit part is
//! the objective's own semantic colour (`OutlineRole::Pickup`, `Interactable`,
//! `MarkerRole::Exit`), so an objective reads by the same colour it always has.
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use observed_authoring::RoomSocketKind;
use observed_style::equipment::{Hardware, finish};

use super::equipment::{Spin, hex_prism, hex_ring, light_tube};

/// A station's column of light, which fills as the local team synchronizes it.
#[derive(Component)]
pub(super) struct SyncColumn {
    pub(super) room_generation_key: u64,
}

/// How tall a station's sync column stands when full, metres.
pub(super) const SYNC_COLUMN: f32 = 2.4;

#[derive(Resource)]
pub(super) struct ObjectiveModels {
    plinth: Handle<Mesh>,
    plinth_rim: Handle<Mesh>,
    crystal_upper: Handle<Mesh>,
    crystal_lower: Handle<Mesh>,
    halo: Handle<Mesh>,
    console: Handle<Mesh>,
    console_edge: Handle<Mesh>,
    screen: Handle<Mesh>,
    column: Handle<Mesh>,
    exit_column: Handle<Mesh>,
    exit_ring: Handle<Mesh>,
    body: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    pickup: Handle<StandardMaterial>,
    interactable: Handle<StandardMaterial>,
    sync_haze: Handle<StandardMaterial>,
    exit: Handle<StandardMaterial>,
    exit_haze: Handle<StandardMaterial>,
}

fn hardware(materials: &mut Assets<StandardMaterial>, part: Hardware) -> Handle<StandardMaterial> {
    let f = finish(part);
    materials.add(StandardMaterial {
        base_color: f.base_color,
        metallic: f.metallic,
        perceptual_roughness: f.roughness,
        ..default()
    })
}

fn haze(materials: &mut Assets<StandardMaterial>, color: LinearRgba) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::LinearRgba(color),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    })
}

impl ObjectiveModels {
    /// `pickup`, `interactable` and `exit` are the objectives' existing lit materials.
    pub(super) fn new(
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        pickup: Handle<StandardMaterial>,
        interactable: Handle<StandardMaterial>,
        exit: Handle<StandardMaterial>,
    ) -> Self {
        let interactable_color = observed_style::outline(observed_style::OutlineRole::Interactable)
            .color
            .to_linear();
        let exit_color = observed_style::marker(observed_style::MarkerRole::Exit).emissive;
        Self {
            plinth: meshes.add(hex_prism(0.34, 0.28, 0.0, 0.16)),
            plinth_rim: meshes.add(hex_ring(0.30, 0.22, 0.16, 0.18)),
            crystal_upper: meshes.add(hex_prism(0.22, 0.004, 0.0, 0.42)),
            crystal_lower: meshes.add(hex_prism(0.004, 0.22, -0.3, 0.0)),
            halo: meshes.add(Torus::new(0.36, 0.385).mesh().build()),
            console: meshes.add(hex_prism(0.46, 0.38, 0.0, 0.95)),
            console_edge: meshes.add(hex_ring(0.39, 0.30, 0.95, 0.98)),
            screen: meshes.add(Cuboid::new(0.52, 0.34, 0.03)),
            column: meshes.add(light_tube(0.3, 1.0)),
            exit_column: meshes.add(light_tube(0.95, 6.5)),
            exit_ring: meshes.add(hex_ring(1.1, 0.95, 0.0, 0.05)),
            body: hardware(materials, Hardware::Body),
            trim: hardware(materials, Hardware::Trim),
            pickup,
            interactable,
            sync_haze: haze(materials, interactable_color * 0.35),
            exit,
            exit_haze: haze(materials, exit_color * 0.08),
        }
    }

    /// The parts of an objective of `kind`, relative to its socket on the floor, facing
    /// the socket's own forward.
    pub(super) fn spawn_parts(
        &self,
        root: &mut ChildSpawnerCommands,
        kind: RoomSocketKind,
        room_generation_key: u64,
    ) {
        let part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, at: Transform| {
            (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), at)
        };
        match kind {
            RoomSocketKind::Keystone => {
                root.spawn(part(&self.plinth, &self.body, Transform::IDENTITY));
                root.spawn(part(&self.plinth_rim, &self.pickup, Transform::IDENTITY));
                // The crystal hovers over its plinth and turns, its halo tilted.
                root.spawn((
                    Transform::from_xyz(0.0, 0.95, 0.0),
                    Visibility::Inherited,
                    Spin {
                        axis: Vec3::Y,
                        rate: 0.7,
                        rest: Quat::IDENTITY,
                        in_hand: false,
                    },
                ))
                .with_children(|crystal| {
                    crystal.spawn(part(&self.crystal_upper, &self.pickup, Transform::IDENTITY));
                    crystal.spawn(part(&self.crystal_lower, &self.pickup, Transform::IDENTITY));
                    crystal.spawn(part(
                        &self.halo,
                        &self.trim,
                        Transform::from_rotation(Quat::from_rotation_x(0.35)),
                    ));
                });
            }
            RoomSocketKind::Exit => {
                root.spawn(part(&self.exit_ring, &self.exit, Transform::IDENTITY));
                root.spawn((
                    part(
                        &self.exit_column,
                        &self.exit_haze,
                        Transform::from_xyz(0.0, 0.05, 0.0),
                    ),
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            }
            _ => {
                root.spawn(part(&self.console, &self.body, Transform::IDENTITY));
                root.spawn(part(&self.console_edge, &self.trim, Transform::IDENTITY));
                // The screen leans back toward whoever uses it, on the console's front.
                root.spawn(part(
                    &self.screen,
                    &self.interactable,
                    Transform::from_xyz(0.0, 1.12, 0.12).with_rotation(Quat::from_rotation_x(-0.6)),
                ));
                if matches!(kind, RoomSocketKind::StationA | RoomSocketKind::StationB) {
                    root.spawn((
                        SyncColumn {
                            room_generation_key,
                        },
                        part(
                            &self.column,
                            &self.sync_haze,
                            Transform::from_xyz(0.0, 1.0, 0.0),
                        ),
                        NotShadowCaster,
                        NotShadowReceiver,
                        Visibility::Hidden,
                    ));
                }
            }
        }
    }
}

/// How full a station's sync column is: the local team's progress on this station, and
/// all of it once any station is synchronized.
#[must_use]
pub(super) fn sync_fill(
    room_generation_key: u64,
    active_room: Option<u64>,
    ticks: u16,
    complete: bool,
) -> f32 {
    if complete {
        1.0
    } else if active_room == Some(room_generation_key) {
        (f32::from(ticks) / f32::from(observed_match::hex_wfc::DUAL_STATION_HOLD_TICKS))
            .clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::sync_fill;

    #[test]
    fn a_column_fills_with_its_own_station_and_holds_full_when_done() {
        let half = observed_match::hex_wfc::DUAL_STATION_HOLD_TICKS / 2;
        assert!((sync_fill(7, Some(7), half, false) - 0.5).abs() < 1e-3);
        assert_eq!(sync_fill(8, Some(7), half, false), 0.0, "another station");
        assert_eq!(sync_fill(8, None, 0, true), 1.0, "synchronized");
        assert_eq!(sync_fill(7, Some(7), 0, false), 0.0);
    }
}
