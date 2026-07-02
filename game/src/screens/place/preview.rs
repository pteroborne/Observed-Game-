use crate::layout::PLACE_TILE;
use bevy::prelude::*;
use observed_core::RoomId;
use observed_match::hybrid::HybridMatch;
use observed_style as style;
use std::f32::consts::PI;

use super::factory::{room_floor_material, room_light_color};
use super::lighting::{spawn_place_lighting, spawn_surface_detail};
use super::mesh::{spawn_polygon_shell, spawn_polygon_walls};
use crate::GameState;
use crate::camera;
use crate::hallway::{self, HallwayFlavor};
use crate::layout::WALL_HEIGHT;
use crate::rivals;
use crate::screens::match_runtime::{connections_for, district_for_place, room_target};
use crate::sim::state::FrozenDest;
use crate::teleport::{self, DoorGap, Place};
use crate::view::assets::MatchAssets;
use crate::view::components::{PassagePreview, PlaceGeometry};

fn gap_yaw(normal: Vec2) -> f32 {
    (-normal.y).atan2(normal.x)
}

/// A short lit corridor stub behind a real passage doorway (floor, ceiling, two side
/// walls) so looking through the frame shows the passage continuing, not void. Crossing
/// the threshold teleports before the body reaches the stub's open end.
pub(crate) fn spawn_passage_stub(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    y_offset: f32,
) {
    const DEPTH: f32 = 3.0;
    let along = Vec2::new(-gap.normal.y, gap.normal.x);
    let centre = gap.center + gap.normal * (DEPTH * 0.5);
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal)); // local X = tangent, Z = normal
    let foot = |c: Vec2| Vec3::new(c.x, 0.0, c.y);
    let floor_material = assets.floor_material.clone();
    commands.spawn((
        PlaceGeometry,
        PassagePreview,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(floor_material),
        Transform::from_translation(foot(centre) + Vec3::Y * (y_offset + 0.02))
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.1), 0.04, DEPTH)),
        Name::new("Passage stub floor"),
    ));
    commands.spawn((
        PlaceGeometry,
        PassagePreview,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(assets.ceiling_material.clone()),
        Transform::from_translation(foot(centre) + Vec3::Y * (y_offset + WALL_HEIGHT))
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.1), 0.04, DEPTH)),
        Name::new("Passage stub ceiling"),
    ));
    for side in [gap.width * 0.5, -gap.width * 0.5] {
        let wc = centre + along * side;
        commands.spawn((
            PlaceGeometry,
            PassagePreview,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.wall_material.clone()),
            Transform::from_translation(foot(wc) + Vec3::Y * (y_offset + WALL_HEIGHT * 0.5))
                .with_rotation(rot)
                .with_scale(Vec3::new(0.2, WALL_HEIGHT, DEPTH)),
            Name::new("Passage stub wall"),
        ));
    }
}

/// Resolve a doorway's destination live (used only when no frozen snapshot exists yet —
/// e.g. the first render frame). Mirrors `compute_gap_dests` for one gap.
pub(crate) fn fallback_dest(
    place: Place,
    gap: &DoorGap,
    nav: &teleport::Nav,
    game: &HybridMatch,
) -> FrozenDest {
    let (dest, _) = teleport::apply_crossing(place, gap, nav);
    let (conns, target) = match dest {
        Place::Room(r) => {
            let c = connections_for(game, r);
            let t = room_target(game, r, &c);
            (c, t)
        }
        Place::Hallway { .. } => (Vec::new(), None),
    };
    FrozenDest {
        gap_center: gap.center,
        threshold: gap.threshold,
        place: dest,
        conns,
        connection_slots: Vec::new(),
        hallway_entry_room_slot: match dest {
            Place::Hallway { .. } => Some(gap.threshold.room.slot),
            _ => None,
        },
        hallway_exit_room_slot: None,
        target,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_passage_preview(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    gap: &DoorGap,
    place: Place,
    dest: &FrozenDest,
    nav: &teleport::Nav,
    game: &HybridMatch,
) {
    let y_offset = teleport::place_y_offset(place);
    match dest.place {
        Place::Hallway { .. } => {
            spawn_hallway_preview(commands, assets, gap, dest.place, nav, y_offset)
        }
        Place::Room(dest_room) => spawn_room_preview(
            commands,
            assets,
            meshes,
            materials,
            gap,
            place,
            dest_room,
            &dest.conns,
            &dest.connection_slots,
            dest.target,
            nav,
            game,
        ),
    }
}

/// The hallway preview (a box place): align its entry to the doorway and extend it away.
pub(crate) fn spawn_hallway_preview(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    next: Place,
    nav: &teleport::Nav,
    y_offset: f32,
) {
    let Place::Hallway { variation, .. } = next else {
        return;
    };
    let dest = teleport::geom_for(next, nav);
    let (hx, hz) = (dest.half.x, dest.half.y);
    let light_color = style::district(district_for_place(nav.seed, next)).light_color;
    let Some(align) = teleport::hallway_alignment(gap, &dest) else {
        spawn_passage_stub(commands, assets, gap, y_offset);
        return;
    };
    let mut parent = camera::alignment_transform(align);
    parent.translation.y = y_offset;
    let place_in = |local: Transform| parent.mul_transform(local);

    let floor_material = if hallway::template(variation).flavor == HallwayFlavor::PressureGate {
        assets.trap_idle_material.clone()
    } else {
        assets.floor_material.clone()
    };

    let plane_scale = Vec3::new(hx * 2.0 / PLACE_TILE, 1.0, hz * 2.0 / PLACE_TILE);
    commands.spawn((
        PlaceGeometry,
        PassagePreview,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.floor_mesh.clone()),
        MeshMaterial3d(floor_material),
        place_in(Transform::from_scale(plane_scale)),
        Name::new("Preview floor"),
    ));
    commands.spawn((
        PlaceGeometry,
        PassagePreview,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.ceiling_mesh.clone()),
        MeshMaterial3d(assets.ceiling_material.clone()),
        place_in(
            Transform::from_xyz(0.0, WALL_HEIGHT, 0.0)
                .with_rotation(Quat::from_rotation_x(PI))
                .with_scale(plane_scale),
        ),
        Name::new("Preview ceiling"),
    ));
    let arena = teleport::place_arena(&dest, 0.0, WALL_HEIGHT);
    for solid in &arena.solids {
        let c = (solid.min + solid.max) * 0.5;
        if c.z < -hz + 0.5 {
            continue;
        }
        let size = solid.max - solid.min;
        commands.spawn((
            PlaceGeometry,
            PassagePreview,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.wall_material.clone()),
            place_in(Transform::from_translation(c).with_scale(size)),
            Name::new("Preview wall"),
        ));
    }
    spawn_place_lighting(commands, assets, &dest, light_color, parent, true);
    let accent =
        assets.district_accent_materials[district_for_place(nav.seed, next).index()].clone();
    spawn_surface_detail(commands, assets, &dest, accent, parent, true);
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
    conns: &[RoomId],
    connection_slots: &[teleport::RoomConnectionSlot],
    target: Option<RoomId>,
    nav: &teleport::Nav,
    game: &HybridMatch,
) {
    let back = match place {
        Place::Hallway { from, to, .. } => {
            if dest_room == to {
                from
            } else {
                to
            }
        }
        Place::Room(src_room) => src_room,
    };
    let mut dest =
        teleport::room_preview_geom(dest_room, conns, connection_slots, target, nav.seed);
    teleport::open_entry(&mut dest, Some(back));
    let Some(poly) = dest.poly.clone() else {
        return;
    };
    let y_offset = teleport::place_y_offset(place);
    let Some(src) = dest.gaps.iter().find(|g| g.target == back).copied() else {
        spawn_passage_stub(commands, assets, gap, y_offset);
        return;
    };

    let mut parent = camera::alignment_transform(teleport::room_alignment(gap, &src));
    parent.translation.y = y_offset;

    let floor_material = room_floor_material(dest_room, &assets.floor_material, materials);
    let light_color = room_light_color(dest_room);

    spawn_polygon_shell(
        commands,
        assets,
        meshes,
        &poly,
        floor_material,
        parent,
        true,
    );
    spawn_polygon_walls(commands, assets, &poly, &dest.gaps, parent, true, |g| {
        g.target == back || g.kind.is_passage()
    });
    spawn_place_lighting(commands, assets, &dest, light_color, parent, true);
    let dest_district = district_for_place(nav.seed, Place::Room(dest_room));
    let accent = assets.district_accent_materials[dest_district.index()].clone();
    spawn_surface_detail(commands, assets, &dest, accent, parent, true);

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
            parent.mul_transform(Transform::from_xyz(lane, 0.82, 0.0)),
            Name::new("Preview rival"),
        ));
    }
}
