//! Replay-owned render scene. Reads only the recorded tape and viewer controls.
use super::{ReplayMapPanel, ReplayPlayback};
use crate::{
    GameState,
    sim::replay::{
        ReplayTape,
        scene::{ReplaySceneFrame, ReplayStructure, body_position},
    },
};
use bevy::{
    camera::{RenderTarget, ScalingMode, visibility::RenderLayers},
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::TextureFormat,
};
use observed_match::hex_wfc::HexBodyPlace;
#[path = "figures.rs"]
mod figures;

use std::{collections::BTreeMap, sync::Arc};

const LAYER: usize = 20;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CameraView {
    #[default]
    Follow,
    Team,
    Floor,
    Eyes,
}
impl CameraView {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Follow => Self::Team,
            Self::Team => Self::Floor,
            Self::Floor => Self::Eyes,
            Self::Eyes => Self::Follow,
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Follow => "Follow",
            Self::Team => "Team",
            Self::Floor => "Floor",
            Self::Eyes => "Eyes",
        }
    }
}
#[derive(Component)]
pub(crate) struct ReplayCamera;
#[derive(Component)]
pub(crate) struct ReplayGeometry;
#[derive(Component)]
pub(crate) struct ReplayFigure;
#[derive(Resource)]
pub(crate) struct ReplayScene {
    image: Handle<Image>,
    built: Option<(usize, u8, CameraView, u8, bool)>,
    pub(super) meshes: BTreeMap<usize, Handle<Mesh>>,
    pub(super) materials: BTreeMap<(u8, bool), Handle<StandardMaterial>>,
    pub(super) figure_meshes: BTreeMap<(u8, usize), Handle<Mesh>>,
}

pub(crate) fn setup(
    mut commands: Commands,
    tape: Option<Res<ReplayTape>>,
    mut images: Option<ResMut<Assets<Image>>>,
    panel: Query<Entity, With<ReplayMapPanel>>,
) {
    if tape.is_none_or(|t| t.scene_frames.is_empty()) {
        return;
    }
    let (Some(images), Ok(panel)) = (images.as_deref_mut(), panel.single()) else {
        return;
    };
    let image = images.add(Image::new_target_texture(
        1240,
        1120,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    commands.entity(panel).insert(ImageNode::new(image.clone()));
    commands.spawn((
        ReplayCamera,
        DespawnOnExit(GameState::Replay),
        Camera3d::default(),
        Msaa::Off,
        Camera {
            order: -5,
            clear_color: crate::view::theme::PANEL.into(),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        RenderLayers::layer(LAYER),
        AmbientLight {
            brightness: 600.0,
            ..default()
        },
        Transform::default(),
    ));
    commands.spawn((
        DespawnOnExit(GameState::Replay),
        DirectionalLight {
            illuminance: 6000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        RenderLayers::layer(LAYER),
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.8, -1.0, 0.0)),
    ));
    commands.insert_resource(ReplayScene {
        image,
        built: None,
        meshes: BTreeMap::new(),
        materials: BTreeMap::new(),
        figure_meshes: BTreeMap::new(),
    });
}

#[derive(SystemParam)]
pub(crate) struct SceneDraw<'w, 's> {
    commands: Commands<'w, 's>,
    cache: Option<ResMut<'w, ReplayScene>>,
    meshes: Option<ResMut<'w, Assets<Mesh>>>,
    materials: Option<ResMut<'w, Assets<StandardMaterial>>>,
    geometry: Query<'w, 's, Entity, With<ReplayGeometry>>,
    figures: Query<'w, 's, Entity, With<ReplayFigure>>,
    camera: Query<'w, 's, (&'static mut Transform, &'static mut Projection), With<ReplayCamera>>,
}

pub(crate) fn sync(
    tape: Option<Res<ReplayTape>>,
    playback: Res<ReplayPlayback>,
    mut draw: SceneDraw,
) {
    let Some(tape) = tape else {
        return;
    };
    let Some((frame, following, fraction)) = frames(&tape, playback.cursor) else {
        return;
    };
    let (Some(cache), Some(meshes), Some(materials)) = (
        draw.cache.as_deref_mut(),
        draw.meshes.as_deref_mut(),
        draw.materials.as_deref_mut(),
    ) else {
        return;
    };
    let Some(body) = frame
        .bodies
        .iter()
        .find(|b| b.actor == playback.focus)
        .or_else(|| frame.bodies.first())
    else {
        return;
    };
    let prison = body.place == HexBodyPlace::Prison;
    let structure = if prison {
        frame.prisons.get(&body.team).unwrap_or(&frame.facility)
    } else {
        &frame.facility
    };
    let level = if prison {
        0
    } else {
        playback.event_focus.map_or(
            body.cell.level.saturating_add_signed(playback.floor_offset),
            |c| c.level,
        )
    };
    let eye_view = playback.view == CameraView::Eyes && body.place != HexBodyPlace::Void;
    let actor_position = body_position(
        body,
        following.and_then(|f| f.bodies.iter().find(|b| b.player == body.player)),
        fraction,
    );
    let position = if prison || eye_view {
        actor_position
    } else {
        playback.event_focus.map_or(actor_position, |c| {
            Vec3::from_array(observed_hex::hex_origin(c))
        })
    };
    let iso = observed_style::iso::detent_bearing(usize::from(playback.detent));
    let key = (
        Arc::as_ptr(structure) as usize,
        level,
        playback.view,
        playback.detent,
        prison,
    );
    if cache.built != Some(key) {
        for entity in &draw.geometry {
            draw.commands.entity(entity).despawn();
        }
        build_structure(
            &mut draw.commands,
            cache,
            meshes,
            materials,
            structure,
            level,
            (iso, eye_view),
        );
        cache.built = Some(key);
    }
    let clock = replay_tick(frame, following, fraction) as f32 / 60.0;
    for entity in &draw.figures {
        draw.commands.entity(entity).despawn();
    }
    figures::draw(
        &mut draw.commands,
        cache,
        meshes,
        materials,
        figures::FigureView {
            tape: &tape,
            frame,
            following,
            fraction,
            clock,
            focus: body,
            level,
            prison,
            eye_view,
        },
    );
    if let Ok((mut transform, mut projection)) = draw.camera.single_mut() {
        if eye_view {
            *projection = Projection::Perspective(PerspectiveProjection {
                near: 0.05,
                far: 1800.0,
                ..default()
            });
            *transform = Transform::from_translation(position + Vec3::Y * frame.eye_height)
                .with_rotation(
                    Quat::from_rotation_y(-body.yaw) * Quat::from_rotation_x(body.pitch),
                );
        } else {
            let (target, span) =
                framing(frame, structure, body.team, position, level, playback.view);
            *projection = Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: span,
                },
                far: 2400.0,
                ..OrthographicProjection::default_3d()
            });
            *transform = Transform::from_translation(
                target + Vec3::new(iso.x * 180.0, 160.0, iso.y * 180.0),
            )
            .looking_at(target, Vec3::Y);
        }
    }
}

fn build_structure(
    commands: &mut Commands,
    cache: &mut ReplayScene,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    structure: &ReplayStructure,
    level: u8,
    view: (Vec2, bool),
) {
    let (bearing, eyes) = view;
    for piece in &structure.pieces {
        let Some(floor) = crate::view::cutaway::surface(piece, level, bearing, eyes) else {
            continue;
        };
        let key = Arc::as_ptr(piece) as usize;
        let mesh = if let Some(mesh) = cache.meshes.get(&key) {
            mesh.clone()
        } else {
            let Some(mesh) = crate::view::cutaway::mesh(&piece.shape) else {
                continue;
            };
            let mesh = meshes.add(mesh);
            cache.meshes.insert(key, mesh.clone());
            mesh
        };
        let register = structure
            .registers
            .get(&piece.source_cell)
            .copied()
            .unwrap_or(observed_content::ArchitectureRegister::Institutional);
        let material = cache
            .materials
            .entry((register as u8 * 2 + u8::from(floor), false))
            .or_insert_with(|| {
                let look = observed_style::hex_shell_surface(
                    register,
                    if floor {
                        observed_style::ArchitectureSurfaceRole::Floor
                    } else {
                        observed_style::ArchitectureSurfaceRole::Wall
                    },
                );
                materials.add(StandardMaterial {
                    base_color: look.base_color,
                    emissive: look.emissive,
                    perceptual_roughness: 0.9,
                    ..default()
                })
            })
            .clone();
        commands.spawn((
            ReplayGeometry,
            DespawnOnExit(GameState::Replay),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            RenderLayers::layer(LAYER),
            Transform::from_translation(piece.center)
                .with_rotation(Quat::from_array(piece.rotation)),
        ));
    }
}

pub(crate) fn frames(
    tape: &ReplayTape,
    cursor: f32,
) -> Option<(&ReplaySceneFrame, Option<&ReplaySceneFrame>, f32)> {
    let index = cursor.floor() as usize;
    let i = tape
        .scene_frames
        .partition_point(|f| f.sample <= index)
        .checked_sub(1)?;
    let frame = &tape.scene_frames[i];
    let next = tape.scene_frames.get(i + 1);
    let fraction = next.map_or(0.0, |next| {
        ((cursor - frame.sample as f32) / (next.sample - frame.sample) as f32).clamp(0.0, 1.0)
    });
    Some((frame, next, fraction))
}
pub(super) fn replay_tick(
    frame: &ReplaySceneFrame,
    next: Option<&ReplaySceneFrame>,
    fraction: f32,
) -> f64 {
    frame.tick as f64
        + next.map_or(0.0, |next| {
            (next.tick - frame.tick) as f64 * f64::from(fraction)
        })
}
fn framing(
    frame: &ReplaySceneFrame,
    structure: &ReplayStructure,
    team: observed_core::TeamId,
    position: Vec3,
    level: u8,
    view: CameraView,
) -> (Vec3, f32) {
    let mut min = position;
    let mut max = position;
    match view {
        CameraView::Floor => {
            for piece in &structure.pieces {
                if piece.source_cell.level == level {
                    min = min.min(piece.center);
                    max = max.max(piece.center);
                }
            }
        }
        CameraView::Team => {
            for b in &frame.bodies {
                if b.team == team && b.cell.level == level && b.place != HexBodyPlace::Void {
                    min = min.min(b.position);
                    max = max.max(b.position);
                }
            }
        }
        _ => {}
    }
    let target = if view == CameraView::Follow {
        position
    } else {
        (min + max) * 0.5
    };
    let span = if view == CameraView::Follow {
        55.0
    } else {
        ((max - min).length() * 0.95 + 30.0).max(55.0)
    };
    (
        Vec3::new(
            target.x,
            f32::from(level) * observed_hex::TILE_LEVEL_HEIGHT + 1.0,
            target.z,
        ),
        span,
    )
}

pub(crate) fn cleanup(
    mut commands: Commands,
    scene: Option<Res<ReplayScene>>,
    mut images: Option<ResMut<Assets<Image>>>,
) {
    if let (Some(scene), Some(images)) = (scene, images.as_deref_mut()) {
        images.remove(scene.image.id());
    }
    commands.remove_resource::<ReplayScene>();
}
