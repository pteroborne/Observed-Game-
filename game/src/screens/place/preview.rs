use crate::layout::PLACE_TILE;
use bevy::prelude::*;
use observed_core::RoomId;
use observed_match::hybrid::HybridMatch;
use std::f32::consts::PI;

use super::factory::room_floor_material;
use super::lighting::{spawn_place_lighting, spawn_surface_detail};
use super::mesh::{spawn_polygon_shell, spawn_polygon_walls};
use super::shell;
use crate::GameState;
use crate::camera;
use crate::hallway::{self, HallwayFlavor};
use crate::layout::WALL_HEIGHT;
use crate::rivals;
use crate::screens::match_runtime::{district_for_place, palette_for_game};
use crate::sim::nav::{connections_for, room_target};
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
    let base_y = y_offset + gap.floor_y;
    commands.spawn((
        PlaceGeometry,
        PassagePreview,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(floor_material),
        Transform::from_translation(foot(centre) + Vec3::Y * (base_y + 0.02))
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
        Transform::from_translation(foot(centre) + Vec3::Y * (base_y + WALL_HEIGHT))
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
            Transform::from_translation(foot(wc) + Vec3::Y * (base_y + WALL_HEIGHT * 0.5))
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
        sealed_slots: match dest {
            Place::Room(r) => crate::sim::nav::sealed_slots_for_room(game, r),
            Place::Hallway { .. } => Vec::new(),
        },
        hallway_entry_room_slot: match dest {
            Place::Hallway { .. } => Some(gap.threshold.room.slot),
            _ => None,
        },
        hallway_exit_room_slot: None,
        target,
        room_role: match dest {
            Place::Room(room) => game
                .competitive
                .map_spec
                .as_ref()
                .and_then(|spec| spec.room(room).map(|room| room.role)),
            Place::Hallway { .. } => None,
        },
        corridor_role: match dest {
            Place::Room(_) => None,
            Place::Hallway { from, to, .. } => game
                .competitive
                .map_spec
                .as_ref()
                .and_then(|spec| spec.corridor_role_between(from, to)),
        },
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
    klaxon_active: bool,
) {
    let y_offset = teleport::place_y_offset(place);
    match dest.place {
        Place::Hallway { .. } => spawn_hallway_preview(
            commands,
            assets,
            gap,
            dest.place,
            nav,
            palette_for_game(nav.seed, dest.place, game, klaxon_active),
            y_offset,
        ),
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
            &dest.sealed_slots,
            dest.target,
            nav,
            game,
            klaxon_active,
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
    palette: observed_style::DistrictPalette,
    y_offset: f32,
) {
    let Place::Hallway { variation, .. } = next else {
        return;
    };
    let dest = teleport::geom_for(next, nav);
    let (hx, hz) = (dest.half.x, dest.half.y);
    let light_color = palette.light_color;
    let Some(align) = teleport::hallway_alignment(gap, &dest) else {
        spawn_passage_stub(commands, assets, gap, y_offset);
        return;
    };
    let preview_floor_y = dest
        .gaps
        .iter()
        .find(|candidate| {
            candidate.threshold.hall == gap.threshold.hall
                && candidate.threshold.hall.side == gap.threshold.room.room
                && candidate.threshold.local_side == teleport::ThresholdLocalSide::Hall
        })
        .map(|candidate| candidate.floor_y)
        .unwrap_or(0.0);
    let mut parent = camera::alignment_transform(align);
    parent.translation.y = y_offset + gap.floor_y - preview_floor_y;
    let place_in = |local: Transform| parent.mul_transform(local);

    let floor_material = if hallway::template(variation).flavor == HallwayFlavor::PressureGate {
        assets.trap_idle_material.clone()
    } else {
        assets.floor_material.clone()
    };

    let plane_scale = Vec3::new(hx * 2.0 / PLACE_TILE, 1.0, hz * 2.0 / PLACE_TILE);
    let shell_height = teleport::structural_height(&dest, WALL_HEIGHT);
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
            Transform::from_xyz(0.0, shell_height, 0.0)
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
    sealed_slots: &[teleport::ThresholdSlotId],
    target: Option<RoomId>,
    nav: &teleport::Nav,
    game: &HybridMatch,
    klaxon_active: bool,
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
    let mut dest = teleport::room_preview_geom(
        dest_room,
        conns,
        connection_slots,
        sealed_slots,
        target,
        game.competitive
            .map_spec
            .as_ref()
            .and_then(|spec| spec.room(dest_room).map(|room| room.role)),
        nav.seed,
    );
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
    parent.translation.y = y_offset + gap.floor_y - src.floor_y;

    spawn_room_miniature(
        commands,
        assets,
        meshes,
        materials,
        &dest,
        &poly,
        dest_room,
        parent,
        nav.seed,
        game,
        klaxon_active,
        RoomMiniatureOverlays {
            open_edge_target: Some(back),
            rival_lane_origin: Vec3::new(0.0, 0.82, 0.0),
            anchor_torch_root: Vec3::new(src.width * 0.5 + 0.3, 0.55, 0.6),
        },
    );
}

/// What [`spawn_room_miniature`] draws on top of a room's bare shell, parameterized so a
/// full-scale threshold preview and a wall-mounted monitor panel can each place the same
/// overlays sensibly in their own local frame.
pub(crate) struct RoomMiniatureOverlays {
    /// A gap whose wall should stay split open (the doorway you're looking back through).
    /// `None` for a monitor panel, which has no "back" doorway — its shell renders fully
    /// closed except for the room's own passable thresholds.
    pub(crate) open_edge_target: Option<RoomId>,
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
    let floor_material = room_floor_material(dest_room, &assets.floor_material, materials);
    let palette = palette_for_game(seed, Place::Room(dest_room), game, klaxon_active);
    let light_color = palette.light_color;

    spawn_polygon_shell(commands, assets, meshes, poly, floor_material, parent, true);
    spawn_polygon_walls(commands, assets, poly, &dest.gaps, parent, true, |g| {
        overlays.open_edge_target == Some(g.target) || g.kind.is_passage()
    });
    spawn_place_lighting(commands, assets, dest, light_color, parent, true);
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
