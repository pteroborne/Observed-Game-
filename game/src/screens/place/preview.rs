use bevy::prelude::*;
use bevy::{
    camera::RenderTarget, camera::visibility::RenderLayers, render::render_resource::TextureFormat,
};
use observed_core::RoomId;
use observed_match::hybrid::HybridMatch;
use observed_style::{ObservationPanelRole, SurfaceRole};

use super::factory::place_surface_material;
use super::lighting::{spawn_place_lighting, spawn_surface_detail};
use super::mesh::{spawn_polygon_shell, spawn_polygon_walls};
use super::shell;
use crate::GameState;
use crate::camera;
use crate::hallway::{self, HallwayFlavor};
use crate::layout::WALL_HEIGHT;
use crate::rivals;
use crate::screens::match_runtime::{district_for_place, palette_for_game};
use crate::sim::state::ThresholdTransit;
use crate::teleport::{self, DoorGap, Place};
use crate::view::assets::MatchAssets;
use crate::view::components::{
    GameCam, PassagePreview, PlaceGeometry, PortalPreviewCamera, PortalSurface,
};

const PORTAL_RENDER_LAYER: usize = 1;
const MAX_PORTAL_PREVIEWS: usize = 8;
const PORTAL_TARGET_LONG_EDGE: u32 = 512;
const PORTAL_SCENE_STRIDE: f32 = 1024.0;
/// Portal-only fill in lumens. It is deliberately below the district key lights but
/// strong enough that a long dark hall retains readable wall/floor normals in the feed.
const PORTAL_SCANNER_INTENSITY: f32 = 1_500_000.0;

/// Move every remote preview entity off the main camera's layer as soon as it spawns.
/// The doorway surface itself is not a `PassagePreview`, so it remains in the playable
/// scene while destination geometry can only be seen by portal cameras.
pub(crate) fn isolate_passage_previews(
    mut commands: Commands,
    previews: Query<Entity, Added<PassagePreview>>,
    surfaces: Query<&PortalSurface, Added<PortalSurface>>,
) {
    for entity in &previews {
        commands
            .entity(entity)
            .insert(RenderLayers::layer(PORTAL_RENDER_LAYER));
    }
    for surface in &surfaces {
        if let Some(snapshot_id) = surface.snapshot_id {
            trace!("portal surface displays snapshot {:016X}", snapshot_id.0);
        } else {
            warn!("portal surface displays a LINK FAULT schematic");
        }
    }
}

fn portal_target_size(aperture: Vec2) -> UVec2 {
    let aspect = (aperture.x / aperture.y).clamp(0.25, 4.0);
    if aspect >= 1.0 {
        UVec2::new(
            PORTAL_TARGET_LONG_EDGE,
            (PORTAL_TARGET_LONG_EDGE as f32 / aspect).round() as u32,
        )
    } else {
        UVec2::new(
            (PORTAL_TARGET_LONG_EDGE as f32 * aspect).round() as u32,
            PORTAL_TARGET_LONG_EDGE,
        )
    }
}

fn portal_vertical_fov(aperture: Vec2, distance: f32) -> f32 {
    // Fit the physical opening, not the main window. The previous full-screen FOV was
    // much too narrow at arm's length, reducing valid previews to flat floor/ceiling
    // bands. A small guard band retains the frame edge under raster precision.
    let half_height = aperture.y * 0.5 * 1.025;
    (2.0 * half_height.atan2(distance.max(0.08))).clamp(0.18, 2.88)
}

/// Camera-relative spatial windows: every active portal camera occupies the same point
/// as the player in the isolated scene, but aims at and fits the physical aperture. This
/// is a position-dependent window (parallax comes from player position), not a copy of
/// the unrelated full-screen crop. Cameras warm for two frames, then render only while
/// their source threshold is plausibly visible; disabling one retains its last valid
/// target image.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_portal_cameras(
    main: Query<&Transform, (With<GameCam>, Without<PortalPreviewCamera>)>,
    mut portals: Query<(
        &mut Transform,
        &mut Projection,
        &mut Camera,
        &mut PortalPreviewCamera,
    )>,
) {
    let Ok(main_transform) = main.single() else {
        return;
    };
    let forward = main_transform.rotation * Vec3::NEG_Z;
    for (mut transform, mut projection, mut camera, mut portal) in &mut portals {
        debug_assert_ne!(portal.snapshot_id.0, 0);
        let toward = portal.source_center - main_transform.translation;
        let distance = toward.length();
        let direction = toward.normalize_or_zero();
        transform.translation = main_transform.translation + portal.scene_offset;
        transform.look_to(direction, Vec3::Y);
        let perspective = PerspectiveProjection {
            aspect_ratio: portal.aperture_size.x / portal.aperture_size.y,
            fov: portal_vertical_fov(portal.aperture_size, distance),
            near: 0.03,
            far: 256.0,
            ..default()
        };
        *projection = Projection::Perspective(perspective);
        let on_source_side = toward.dot(portal.source_normal) > -0.1;
        let visible = distance < 48.0 && forward.dot(direction) > -0.12 && on_source_side;
        camera.is_active = portal.warm_frames > 0 || visible;
        portal.warm_frames = portal.warm_frames.saturating_sub(1);
    }
}

fn gap_yaw(normal: Vec2) -> f32 {
    let tangent = Vec2::new(-normal.y, normal.x);
    (-tangent.y).atan2(tangent.x)
}

/// Spawn one always-present portal surface backed by an honest initialized target. A
/// valid transit adds an isolated camera and the exact destination snapshot scene;
/// invalid links retain the labeled high-contrast fault schematic.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_passage_preview(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    gap: &DoorGap,
    place: Place,
    transit: Option<&ThresholdTransit>,
    portal_index: usize,
    seed: u64,
    game: &HybridMatch,
    klaxon_active: bool,
) {
    assert!(
        portal_index < MAX_PORTAL_PREVIEWS,
        "a production place exceeded the eight-camera portal budget"
    );
    let y_offset = teleport::place_y_offset(place);
    let valid = transit.filter(|transit| transit.is_valid());
    let snapshot_id = valid.and_then(|transit| transit.destination.as_ref().map(|dest| dest.id));
    let fault = transit
        .and_then(|transit| transit.fault.as_deref())
        .unwrap_or("LINK FAULT: threshold transaction missing");
    let label = valid
        .and_then(|transit| transit.destination.as_ref())
        .map(|destination| {
            format!(
                "{}  SNAP {:016X}",
                match destination.place {
                    Place::Room(room) => format!("ROOM {}", room.0),
                    Place::Hallway {
                        corridor,
                        variation,
                        ..
                    } => {
                        format!("HALL {} / TEMPLATE {}", corridor.0, variation)
                    }
                },
                destination.id.0
            )
        })
        .unwrap_or_else(|| fault.to_string());
    let aperture_size = Vec2::new((gap.width - 0.18).max(0.1), WALL_HEIGHT - 0.18);
    let target_size = portal_target_size(aperture_size);
    let mut target = Image::new_target_texture(
        target_size.x,
        target_size.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    let mut pixels = vec![0_u8; (target_size.x * target_size.y * 4) as usize];
    for y in 0..target_size.y {
        for x in 0..target_size.x {
            let border = x < 12 || y < 12 || x >= target_size.x - 12 || y >= target_size.y - 12;
            let stripe = ((x + y) / 28).is_multiple_of(2);
            let color = if border {
                [255, 154, 45, 255]
            } else if stripe {
                [8, 42, 54, 255]
            } else {
                [4, 16, 24, 255]
            };
            let index = ((y * target_size.x + x) * 4) as usize;
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
    target.data = Some(pixels);
    let target = images.add(target);
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(target.clone()),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let base_y = y_offset + gap.floor_y;
    commands.spawn((
        PlaceGeometry,
        PortalSurface { snapshot_id },
        DespawnOnExit(GameState::Match),
        Mesh3d(meshes.add(super::mesh::portal_quad_mesh(Vec2::new(
            (gap.width - 0.18).max(0.1),
            WALL_HEIGHT - 0.18,
        )))),
        MeshMaterial3d(material),
        Transform::from_xyz(
            gap.center.x + gap.normal.x * 0.055,
            base_y + WALL_HEIGHT * 0.5,
            gap.center.y + gap.normal.y * 0.055,
        )
        .with_rotation(rot),
        Name::new(format!("Portal surface {label}")),
    ));
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Text2d::new(label),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.75, 0.35)),
        Transform::from_xyz(
            gap.center.x - gap.normal.x * 0.02,
            base_y + 0.24,
            gap.center.y - gap.normal.y * 0.02,
        )
        .with_rotation(rot)
        .with_scale(Vec3::splat(0.022)),
        Name::new("Portal destination label"),
    ));

    let Some(transit) = valid else {
        return;
    };
    let destination = transit.destination.as_ref().unwrap();
    let transform = transit.transform.unwrap();
    let scene_offset = Vec3::X * (PORTAL_SCENE_STRIDE * (portal_index as f32 + 1.0));
    commands.spawn((
        PlaceGeometry,
        PortalPreviewCamera {
            scene_offset,
            source_center: Vec3::new(gap.center.x, base_y + WALL_HEIGHT * 0.5, gap.center.y),
            source_normal: Vec3::new(gap.normal.x, 0.0, gap.normal.y),
            aperture_size,
            warm_frames: 2,
            snapshot_id: destination.id,
        },
        DespawnOnExit(GameState::Match),
        Camera3d::default(),
        Camera {
            order: -1 - portal_index as isize,
            clear_color: Color::srgb(0.015, 0.025, 0.04).into(),
            ..default()
        },
        // A neutral diegetic scanner fill travels with the portal eye. Destination
        // structure remains the exact snapshot, but its wall/floor normals cannot fall
        // into an unreadable flat silhouette when that destination's noir practicals
        // happen to face away from the aperture. This light communicates no lock state;
        // the frame indicator remains anchor-only.
        PointLight {
            color: observed_style::observation_panel(ObservationPanelRole::Footprint).base_color,
            intensity: PORTAL_SCANNER_INTENSITY,
            range: 48.0,
            shadows_enabled: false,
            ..default()
        },
        RenderTarget::Image(target.into()),
        Transform::default(),
        RenderLayers::layer(PORTAL_RENDER_LAYER),
        Name::new(format!("Portal camera {:016X}", destination.id.0)),
    ));

    match destination.place {
        Place::Hallway { .. } => spawn_hallway_preview(
            commands,
            assets,
            meshes,
            materials,
            destination.place,
            &destination.geom,
            transform,
            scene_offset,
            seed,
            palette_for_game(seed, destination.place, game, klaxon_active),
            &destination.arena,
        ),
        Place::Room(dest_room) => spawn_room_preview(
            commands,
            assets,
            meshes,
            materials,
            gap,
            place,
            dest_room,
            &destination.geom,
            transit.destination_gap.as_ref().unwrap(),
            transform,
            scene_offset,
            seed,
            game,
            klaxon_active,
        ),
    }
}

/// The hallway preview (a box place): align its entry to the doorway and extend it away.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_hallway_preview(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    next: Place,
    dest: &teleport::PlaceGeom,
    threshold_transform: crate::sim::state::ThresholdTransform,
    scene_offset: Vec3,
    seed: u64,
    palette: observed_style::DistrictPalette,
    arena: &observed_traversal::ArenaSpec,
) {
    let Place::Hallway { variation, .. } = next else {
        return;
    };
    let (hx, hz) = (dest.half.x, dest.half.y);
    // Arena coordinates already include the destination Place's absolute Y island.
    // Map that exact frozen arena to the source threshold without rebuilding a second
    // collider description. Purely local decorations use `local_parent`, which restores
    // the destination island offset expected by their zero-based geometry.
    let mut arena_parent = camera::alignment_transform(threshold_transform.alignment);
    arena_parent.translation.y =
        threshold_transform.source_floor_y - threshold_transform.destination_floor_y;
    arena_parent.translation += scene_offset;
    let mut local_parent = arena_parent;
    local_parent.translation.y += teleport::place_y_offset(next);
    let place_in = |local: Transform| local_parent.mul_transform(local);

    let ceiling_material = place_surface_material(
        SurfaceRole::Ceiling,
        &palette,
        &assets.ceiling_material,
        materials,
    );

    let shell_height = teleport::structural_height(dest, WALL_HEIGHT);
    commands.spawn((
        PlaceGeometry,
        PassagePreview,
        DespawnOnExit(GameState::Match),
        Mesh3d(meshes.add(super::mesh::rect_mesh(Vec2::new(hx, hz), 0.0, false))),
        MeshMaterial3d(ceiling_material),
        place_in(Transform::from_xyz(0.0, shell_height, 0.0)),
        Name::new("Preview ceiling"),
    ));
    super::authored::spawn_preview_collision_shell(
        commands,
        meshes,
        materials,
        arena,
        arena_parent,
        &palette,
        super::authored::ShellMaterials {
            floor: if hallway::template(variation).flavor == HallwayFlavor::PressureGate {
                &assets.trap_idle_material
            } else {
                &assets.floor_material
            },
            wall: &assets.wall_material,
            interior: (hallway::template(variation).flavor == HallwayFlavor::Gantry)
                .then_some((SurfaceRole::GantryDeck, &assets.gantry_deck_material)),
        },
    );
    spawn_place_lighting(commands, assets, dest, &palette, local_parent, true);
    // Module parity: the hallway you see through the doorway carries the same
    // WFC-composed light modules you will teleport into (same pure inputs).
    if let Place::Hallway { from, to, .. } = next {
        let district = district_for_place(seed, next);
        let placements = super::modules::solve_hallway_modules(
            super::modules::hall_module_seed(seed, from.0, to.0),
            dest,
            teleport::MAZE_CELL,
            district,
        );
        super::modules::spawn_hallway_modules(
            commands,
            assets,
            materials,
            super::modules::ModuleSpawn {
                palette: &palette,
                placements: &placements,
                cell: teleport::MAZE_CELL,
                xform: local_parent,
                preview: true,
            },
        );
    }
    let accent = assets.district_accent_materials[district_for_place(seed, next).index()].clone();
    spawn_surface_detail(commands, assets, dest, accent, local_parent, true);
}

/// The room preview (a polygon place): build the destination room's footprint, align the
/// doorway that connects back to where you stand into the opening, and render it beyond
/// the door — its near doorway split open so you can see in. Needs the destination room's
/// own connection set (from the brain) to shape its polygon.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_room_preview(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    gap: &DoorGap,
    place: Place,
    dest_room: RoomId,
    dest: &teleport::PlaceGeom,
    destination_gap: &DoorGap,
    threshold_transform: crate::sim::state::ThresholdTransform,
    scene_offset: Vec3,
    seed: u64,
    game: &HybridMatch,
    klaxon_active: bool,
) {
    let Some(poly) = dest.poly.clone() else {
        return;
    };
    let y_offset = teleport::place_y_offset(place);
    let mut parent = camera::alignment_transform(threshold_transform.alignment);
    parent.translation.y = y_offset + gap.floor_y - destination_gap.floor_y;
    parent.translation += scene_offset;

    spawn_room_miniature(
        commands,
        assets,
        meshes,
        materials,
        dest,
        &poly,
        dest_room,
        parent,
        seed,
        game,
        klaxon_active,
        RoomMiniatureOverlays {
            rival_lane_origin: Vec3::new(0.0, 0.82, 0.0),
            anchor_torch_root: Vec3::new(destination_gap.width * 0.5 + 0.3, 0.55, 0.6),
        },
    );
}

/// What [`spawn_room_miniature`] draws on top of a room's bare shell, parameterized so a
/// full-scale threshold preview and a wall-mounted monitor panel can each place the same
/// overlays sensibly in their own local frame.
pub(crate) struct RoomMiniatureOverlays {
    /// Local-frame origin a present rival's walking lane is centred on.
    pub(crate) rival_lane_origin: Vec3,
    /// Local-frame root for a rival anchor torch prop, if the room carries one.
    pub(crate) anchor_torch_root: Vec3,
}

/// The reusable core of a room preview: the destination room's shell (floor/ceiling/walls
/// honoring its live door/threshold states), lighting, surface detail, and the rival
/// presence/anchor overlays — all spawned under `parent`, at whatever scale `parent`
/// carries. Shared by the full-scale doorway preview ([`spawn_room_preview`], scale 1,
/// doorway-aligned) and the miniature wall-monitor panels
/// (`super::monitors`, scale « 1, wall-mounted) so "the room preview technique the
/// thresholds use" is the same technique everywhere, not reinvented per caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_room_miniature(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    dest: &teleport::PlaceGeom,
    poly: &[Vec2],
    dest_room: RoomId,
    parent: Transform,
    seed: u64,
    game: &HybridMatch,
    klaxon_active: bool,
    overlays: RoomMiniatureOverlays,
) {
    let palette = palette_for_game(seed, Place::Room(dest_room), game, klaxon_active);
    let floor_material = place_surface_material(
        SurfaceRole::Plain,
        &palette,
        &assets.floor_material,
        materials,
    );
    let wall_material = place_surface_material(
        SurfaceRole::Wall,
        &palette,
        &assets.wall_material,
        materials,
    );
    let ceiling_material = place_surface_material(
        SurfaceRole::Ceiling,
        &palette,
        &assets.ceiling_material,
        materials,
    );
    spawn_polygon_shell(
        commands,
        assets,
        meshes,
        poly,
        floor_material,
        ceiling_material,
        parent,
        true,
        WALL_HEIGHT,
    );
    spawn_polygon_walls(
        commands,
        assets,
        meshes,
        poly,
        &dest.gaps,
        wall_material,
        parent,
        true,
    );
    spawn_place_lighting(commands, assets, dest, &palette, parent, true);
    let accent = materials.add(StandardMaterial {
        base_color: Color::srgb(0.02, 0.03, 0.05),
        emissive: LinearRgba::rgb(
            palette.accent.red * 10.0,
            palette.accent.green * 10.0,
            palette.accent.blue * 10.0,
        ),
        unlit: true,
        ..default()
    });
    spawn_surface_detail(commands, assets, dest, accent, parent, true);

    let present = rivals::rivals_in_room(&game.competitive, dest_room);
    let n = present.len();
    for (slot, _team) in present.iter().enumerate() {
        let lane = (slot as f32 - (n as f32 - 1.0) * 0.5) * 1.3;
        commands.spawn((
            PlaceGeometry,
            PassagePreview,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.rival_body_mesh.clone()),
            MeshMaterial3d(assets.rival_material.clone()),
            parent.mul_transform(Transform::from_xyz(
                overlays.rival_lane_origin.x + lane,
                overlays.rival_lane_origin.y,
                overlays.rival_lane_origin.z,
            )),
            Name::new("Preview rival"),
        ));
    }

    // Preview == arrival honesty (Phase 42c ruling): if the previewed room carries a
    // rival anchor, its torch prop renders here too, not just once you actually cross
    // into the room. Presence avatars stay same-room-only (above); the anchor is a
    // durable claim so it is honest to show ahead of arrival. The lowest team index
    // wins ties, mirroring `sim::nav::rival_signals`'s anchor resolution.
    let local_team = crate::flow::LOCAL_TEAM.0 as usize;
    if let Some(anchor_team) = game
        .competitive
        .structure
        .anchors
        .iter()
        .filter(|anchor| anchor.room == dest_room && anchor.team.0 as usize != local_team)
        .map(|anchor| anchor.team)
        .min_by_key(|team| team.0)
    {
        let root = parent.mul_transform(
            Transform::from_translation(overlays.anchor_torch_root)
                .with_scale(Vec3::new(0.18, 1.1, 0.18)),
        );
        let torch =
            shell::spawn_rival_anchor_torch_at(commands, assets, root, anchor_team.0 as usize);
        commands.entity(torch).insert(PassagePreview);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_target_preserves_the_physical_aperture_aspect() {
        for aperture in [Vec2::new(4.2, 4.8), Vec2::new(8.0, 4.0)] {
            let target = portal_target_size(aperture);
            let target_aspect = target.x as f32 / target.y as f32;
            assert!((target_aspect - aperture.x / aperture.y).abs() < 0.01);
            assert_eq!(target.x.max(target.y), PORTAL_TARGET_LONG_EDGE);
        }
    }

    #[test]
    fn portal_projection_expands_when_the_viewer_approaches() {
        let aperture = Vec2::new(4.2, 4.8);
        let far = portal_vertical_fov(aperture, 10.0);
        let near = portal_vertical_fov(aperture, 1.6);

        assert!(near > far * 2.0);
        assert!(near > 1.8, "an arm's-length doorway needs a wide view");
    }
}
