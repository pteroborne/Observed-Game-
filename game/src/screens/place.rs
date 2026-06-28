//! The first-person place renderer: turn the current place (room or hallway) into
//! neon-noir geometry in its own local frame — floor/ceiling, walls straight from the
//! collision arena, neon doorway frames + animated leaves, a preview of the place beyond
//! each open doorway, and keystone items — plus the shared camera and the walking rival
//! avatars. Rebuilds only when the player teleports.

use std::f32::consts::PI;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_core::RoomId;
use observed_match::hybrid::HybridMatch;
use observed_match::maze::TILE_SIZE;
use observed_style::{self as style, MarkerRole};

use super::*;
use crate::GameState;
use crate::hallway::{self, HallwayFlavor};
use crate::items::{ItemKind, ItemsState, PlacedItem};
use crate::keystones::KeystoneState;
use crate::rivals;
use crate::teleport::{self, GapKind, Place};

/// Place the shared camera at the player's first-person eye in the current place's
/// local frame (each place is centred at the origin). No shake — a reroute is felt
/// through the world's flickering lights ([`super::flicker_lights`]), not the camera.
pub(crate) fn present_match_camera(
    tp: Res<TeleportState>,
    mut camera: Query<&mut Transform, With<GameCam>>,
) {
    if let Ok(mut transform) = camera.single_mut() {
        let eye = tp.body.eye(&tp.config);
        *transform = Transform::from_translation(eye).looking_to(tp.body.look_dir(), Vec3::Y);
    }
}

/// Render the current place (room or hallway) in its own local frame: floor + ceiling,
/// walls taken straight from the collision arena (so the doorway gaps line up exactly),
/// a neon frame at each gap, an animated leaf, and a preview of the place beyond each
/// passage doorway. Rebuilds only when the player teleports.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_place(
    assets: Res<MatchAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    runtime: Res<MatchRuntime>,
    existing: Query<Entity, With<PlaceGeometry>>,
    mut commands: Commands,
) {
    let tp = tp.into_inner();
    if tp.rendered == Some(tp.place) {
        return;
    }
    tp.rendered = Some(tp.place);
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    // The navigation snapshot the crossing will use — so a doorway can preview the exact
    // place you'll teleport into.
    let game = runtime.live.host_match();
    let nav = nav_for_place(game, &keys, &items, tp.place);

    // Render the cached geometry (built once on teleport), including any maze walls.
    let geom = tp.geom.clone();
    let (hx, hz) = (geom.half.x, geom.half.y);

    // No spine guidance: every floor is the neutral surface, except a pressure-gate
    // hazard hall, which keeps its hazard read.
    let floor_material = match tp.place {
        Place::Hallway { variation, .. }
            if hallway::template(variation).flavor == HallwayFlavor::PressureGate =>
        {
            assets.trap_idle_material.clone()
        }
        _ => assets.floor_material.clone(),
    };

    if let Some(poly) = geom.poly.clone() {
        // A polygon room: a custom floor/ceiling fan matching the footprint, and one
        // angled wall panel per edge (split around an open doorway).
        spawn_polygon_shell(
            &mut commands,
            &assets,
            &mut meshes,
            &poly,
            floor_material,
            Transform::IDENTITY,
            false,
        );
        spawn_polygon_walls(
            &mut commands,
            &assets,
            &poly,
            &geom.gaps,
            Transform::IDENTITY,
            false,
            |g| g.kind.is_passage(),
        );
    } else {
        // A box place (hallway / maze): floor + ceiling planes and walls straight from
        // the collision arena, so the doorway gaps line up exactly.
        let plane_scale = Vec3::new(hx * 2.0 / TILE_SIZE, 1.0, hz * 2.0 / TILE_SIZE);
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.floor_mesh.clone()),
            MeshMaterial3d(floor_material),
            Transform::from_xyz(0.0, 0.0, 0.0).with_scale(plane_scale),
            Name::new("Place floor"),
        ));
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.ceiling_mesh.clone()),
            MeshMaterial3d(assets.ceiling_material.clone()),
            Transform::from_xyz(0.0, WALL_HEIGHT, 0.0)
                .with_rotation(Quat::from_rotation_x(PI))
                .with_scale(plane_scale),
            Name::new("Place ceiling"),
        ));
        for solid in &tp.arena.solids {
            let center = (solid.min + solid.max) * 0.5;
            let size = solid.max - solid.min;
            commands.spawn((
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(assets.wall_material.clone()),
                Transform::from_translation(center).with_scale(size),
                Name::new("Place wall"),
            ));
        }
    }

    // A neon frame at each doorway. A passage threshold is **always open** (transparent):
    // behind it we render the frozen neighbour you'll teleport into — the actual hallway from
    // a room, the actual room from a hallway — so what you see is what you walk into, and
    // there's no hiding leaf. A side door is a sealed leaf on a solid wall; the keystone gate
    // is a red locked leaf.
    for gap in &geom.gaps {
        // Whether this doorway's edge is anchored by a torch — drives the frame light.
        let (ea, eb) = match tp.place {
            Place::Room(r) => (r, gap.target),
            Place::Hallway { from, to, .. } => (from, to),
        };
        let tethered = nav.is_tethered(ea, eb);
        spawn_place_frame(&mut commands, &assets, gap, tethered);
        if gap.kind.is_passage() {
            let dest = tp
                .gap_dests
                .iter()
                .find(|d| (d.gap_center - gap.center).length() < 0.05)
                .cloned()
                .unwrap_or_else(|| fallback_dest(tp.place, gap, &nav, game));
            spawn_passage_preview(
                &mut commands,
                &assets,
                &mut meshes,
                gap,
                tp.place,
                &dest,
                &nav,
                game,
            );
        } else if gap.kind == GapKind::LockedExit {
            // The keystone gate: a red sealed door over the way out.
            spawn_locked_leaf(&mut commands, &assets, gap, tethered);
        } else {
            // A side door on a solid wall: a sealed leaf (never opens until the
            // room-discovery system turns it into a real connection).
            spawn_leaf(
                &mut commands,
                &assets,
                gap,
                false,
                assets.door_leaf_material.clone(),
            );
        }
    }

    // A keystone item glows in this room until it is picked up.
    if let Place::Room(room) = tp.place
        && keys.has_uncollected(room)
    {
        spawn_keystone_item(&mut commands, &assets, room);
    }
    for item in items.placed_in(tp.place) {
        spawn_dropped_item(&mut commands, &assets, item);
    }

    // The place's lighting: an overhead fill plus a few flickering ceiling fixtures, in the
    // district's light temperature. Built once here and *identically* behind every doorway
    // preview, so what you see through a door is lit exactly like what you walk into.
    let district = district_for_place(tp.place);
    let light_color = style::district(district).light_color;
    spawn_place_lighting(
        &mut commands,
        &assets,
        &geom,
        light_color,
        Transform::IDENTITY,
        false,
    );
    let accent = assets.district_accent_materials[district.index()].clone();
    spawn_surface_detail(
        &mut commands,
        &assets,
        &geom,
        accent,
        Transform::IDENTITY,
        false,
    );
}

/// The Y rotation that aligns a unit cube's local +X with a doorway's tangent (so a
/// wall/frame/leaf sits flush on the edge, at any angle). Local +Z then runs along the
/// outward normal.
fn gap_yaw(normal: Vec2) -> f32 {
    let along = Vec2::new(-normal.y, normal.x);
    (-along.y).atan2(along.x)
}

/// A neon doorway frame (two posts + a lintel) at a gap, built from primitives and
/// rotated to the gap's wall so it matches the opening at any angle.
fn spawn_place_frame(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &teleport::DoorGap,
    tethered: bool,
) {
    let material = assets.doorframe_material.clone();
    let along = Vec2::new(-gap.normal.y, gap.normal.x); // span (tangent) axis
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let half = gap.width * 0.5;
    for offset in [half, -half] {
        let p = gap.center + along * offset;
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(p.x, WALL_HEIGHT * 0.5, p.y).with_scale(Vec3::new(
                0.24,
                WALL_HEIGHT,
                0.24,
            )),
            Name::new("Doorframe post"),
        ));
    }
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_xyz(gap.center.x, WALL_HEIGHT - 0.2, gap.center.y)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.3), 0.34, 0.24)),
        Name::new("Doorframe lintel"),
    ));
    // A small lamp on the lintel whose colour shows whether this edge is **tethered** by an
    // anchor torch (the Control/anchor purple) or free (a neutral cool tone).
    let tether_color = if tethered {
        style::marker(MarkerRole::Control).base_color
    } else {
        Color::srgb(0.45, 0.62, 0.78)
    };
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        PointLight {
            color: tether_color,
            intensity: 1_400.0,
            range: 5.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(gap.center.x, WALL_HEIGHT - 0.35, gap.center.y),
        Name::new("Doorframe tether light"),
    ));
}

/// A short lit corridor stub behind a real passage doorway (floor, ceiling, two side
/// walls) so looking through the frame shows the passage continuing, not void. Crossing
/// the threshold teleports before the body reaches the stub's open end.
fn spawn_passage_stub(commands: &mut Commands, assets: &MatchAssets, gap: &teleport::DoorGap) {
    const DEPTH: f32 = 3.0;
    let along = Vec2::new(-gap.normal.y, gap.normal.x);
    let centre = gap.center + gap.normal * (DEPTH * 0.5);
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal)); // local X = tangent, Z = normal
    let foot = |c: Vec2| Vec3::new(c.x, 0.0, c.y);
    let floor_material = assets.floor_material.clone();
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(floor_material),
        Transform::from_translation(foot(centre) + Vec3::Y * 0.02)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.1), 0.04, DEPTH)),
        Name::new("Passage stub floor"),
    ));
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(assets.ceiling_material.clone()),
        Transform::from_translation(foot(centre) + Vec3::Y * WALL_HEIGHT)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.1), 0.04, DEPTH)),
        Name::new("Passage stub ceiling"),
    ));
    for side in [gap.width * 0.5, -gap.width * 0.5] {
        let wc = centre + along * side;
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.wall_material.clone()),
            Transform::from_translation(foot(wc) + Vec3::Y * (WALL_HEIGHT * 0.5))
                .with_rotation(rot)
                .with_scale(Vec3::new(0.2, WALL_HEIGHT, DEPTH)),
            Name::new("Passage stub wall"),
        ));
    }
}

/// Build the renderer `Transform` that places a previewed child place per a
/// [`teleport::Align2d`] (translate by its XZ offset, rotate by its yaw about +Y). The
/// same alignment is used by the seamless crossing remap, so the preview and the place you
/// teleport into coincide.
fn align_transform(a: teleport::Align2d) -> Transform {
    Transform::from_xyz(a.offset.x, 0.0, a.offset.y).with_rotation(Quat::from_rotation_y(a.yaw))
}

/// Render, behind an open passage doorway, a preview of the **actual place** you'll
/// teleport into when you cross it: the real hallway from a room, or the real room from
/// a hallway, aligned so its matching doorway sits in the opening and it extends away
/// through the door. So an opened door reveals where you're actually going, not a
/// featureless stub. Presentation-only: it reads the same `nav`/graph the crossing uses,
/// so (absent a mid-place decohere) it matches what you enter.
/// Resolve a doorway's destination live (used only when no frozen snapshot exists yet —
/// e.g. the first render frame). Mirrors `compute_gap_dests` for one gap.
fn fallback_dest(
    place: Place,
    gap: &teleport::DoorGap,
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
        place: dest,
        conns,
        target,
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_passage_preview(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    gap: &teleport::DoorGap,
    place: Place,
    dest: &FrozenDest,
    nav: &teleport::Nav,
    game: &HybridMatch,
) {
    // Render the **frozen** destination snapshotted at place-entry, so the preview is exactly
    // the place you'll teleport into (a hallway carries its variation; a room its frozen
    // connection shape) — observed → frozen.
    match dest.place {
        Place::Hallway { .. } => spawn_hallway_preview(commands, assets, gap, dest.place, nav),
        Place::Room(dest_room) => spawn_room_preview(
            commands,
            assets,
            meshes,
            gap,
            place,
            dest_room,
            &dest.conns,
            dest.target,
            nav,
            game,
        ),
    }
}

/// The hallway preview (a box place): align its entry to the doorway and extend it away.
fn spawn_hallway_preview(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &teleport::DoorGap,
    next: Place,
    nav: &teleport::Nav,
) {
    let Place::Hallway { variation, .. } = next else {
        return;
    };
    let dest = teleport::geom_for(next, nav);
    let (hx, hz) = (dest.half.x, dest.half.y);
    // The corridor reads in its own (current) district's light temperature.
    let light_color = style::district(district_for_place(next)).light_color;
    // The hallway's local +Z (entry→exit) maps to the doorway's outward normal, and its
    // entry sits just beyond the opening — the same alignment the crossing remap uses.
    let parent = align_transform(teleport::hallway_alignment(gap, hz));
    let place_in = |local: Transform| parent.mul_transform(local);

    // A pressure-gate hall keeps its hazard floor; everything else is the neutral surface.
    let floor_material = if hallway::template(variation).flavor == HallwayFlavor::PressureGate {
        assets.trap_idle_material.clone()
    } else {
        assets.floor_material.clone()
    };

    let plane_scale = Vec3::new(hx * 2.0 / TILE_SIZE, 1.0, hz * 2.0 / TILE_SIZE);
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
    // Perimeter + interior walls straight from the collision arena, minus the entry-side
    // wall (which would double the room wall at the threshold).
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
    // Light the preview with the *same* lighting the hallway will have when you walk in
    // (identical overhead fill + flickering fixtures), so the doorway is never brighter or
    // a different colour than the place beyond it. Same for the wall trim linework.
    spawn_place_lighting(commands, assets, &dest, light_color, parent, true);
    let accent = assets.district_accent_materials[district_for_place(next).index()].clone();
    spawn_surface_detail(commands, assets, &dest, accent, parent, true);
}

/// The room preview (a polygon place): build the destination room's footprint, align the
/// doorway that connects back to where you stand into the opening, and render it beyond
/// the door — its near doorway split open so you can see in. Needs the destination room's
/// own connection set (from the brain) to shape its polygon.
#[allow(clippy::too_many_arguments)]
fn spawn_room_preview(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    gap: &teleport::DoorGap,
    place: Place,
    dest_room: RoomId,
    conns: &[RoomId],
    target: Option<RoomId>,
    nav: &teleport::Nav,
    game: &HybridMatch,
) {
    // You're in a hallway looking at a room; the room's doorway back toward the *other*
    // end of this hallway is the one you'll enter through — align that to the opening.
    let Place::Hallway { from, to, .. } = place else {
        return;
    };
    let back = if dest_room == to { from } else { to };
    // `conns` is the room's frozen connection set (snapshotted at place-entry), so the
    // previewed shape matches the room you'll arrive in even if the brain rerolls it.
    let dest = teleport::room_preview_geom(dest_room, conns, target, nav.seed);
    let Some(poly) = dest.poly.clone() else {
        return;
    };
    let Some(src) = dest.gaps.iter().find(|g| g.target == back).copied() else {
        // The graph no longer connects them (decohered) — fall back to a plain stub.
        spawn_passage_stub(commands, assets, gap);
        return;
    };

    // Rigid alignment (shared with the crossing remap): rotate so the room doorway's
    // outward normal faces back toward the player (−gap.normal) and translate so the
    // doorway centre sits in the opening; the room then extends away through the door.
    let parent = align_transform(teleport::room_alignment(gap, &src));

    spawn_polygon_shell(
        commands,
        assets,
        meshes,
        &poly,
        assets.floor_material.clone(),
        parent,
        true,
    );
    // Open the doorway you'd enter through and any frozen passage already open in the room,
    // so the preview has the same wall/ceiling cuts as the arrival geometry.
    spawn_polygon_walls(commands, assets, &poly, &dest.gaps, parent, true, |g| {
        g.target == back || g.kind.is_passage()
    });
    // Light the previewed room with the *same* lighting it will have when you arrive
    // (district fill + fixtures), foreshadowing its neighbourhood, so the doorway and the
    // room beyond read identically before and after you cross. Same for the wall trim.
    let dest_district = district_for_place(Place::Room(dest_room));
    spawn_place_lighting(
        commands,
        assets,
        &dest,
        style::district(dest_district).light_color,
        parent,
        true,
    );
    let accent = assets.district_accent_materials[dest_district.index()].clone();
    spawn_surface_detail(commands, assets, &dest, accent, parent, true);

    // Render the neighbour room's current occupants (rival bots) through the open threshold,
    // so they're already visible and don't pop into existence when you cross in.
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

/// Spawn one place point light, tagged for teardown and the decoherence/idle flicker.
fn spawn_place_light(
    commands: &mut Commands,
    transform: Transform,
    color: Color,
    range: f32,
    flicker: FlickerLight,
    preview: bool,
) {
    let intensity = flicker.base;
    let mut light = commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        flicker,
        PointLight {
            color,
            intensity,
            range,
            shadows_enabled: false,
            ..default()
        },
        transform,
        Name::new("Place light"),
    ));
    if preview {
        light.insert(PassagePreview);
    }
}

/// Local XZ positions for a place's ceiling fixtures: a couple of wall sconces in a polygon
/// room, or overhead lights spaced down a hallway. Deterministic from the geometry so a
/// place and its doorway preview place their fixtures identically.
fn fixture_positions(geom: &teleport::PlaceGeom) -> Vec<Vec2> {
    if let Some(poly) = geom.poly.as_ref() {
        let n = poly.len();
        if n < 3 {
            return Vec::new();
        }
        [0usize, n / 2]
            .into_iter()
            .map(|i| (poly[i] + poly[(i + 1) % n]) * 0.5 * 0.78)
            .collect()
    } else {
        let hz = geom.half.y;
        let count = ((hz / 6.0).floor() as usize).clamp(1, 3);
        (0..count)
            .map(|k| Vec2::new(0.0, -hz + (k as f32 + 0.5) * (2.0 * hz / count as f32)))
            .collect()
    }
}

/// Spawn a place's full lighting — an overhead fill plus a few flickering ceiling fixtures
/// — under `xform` (identity for the live place, the doorway-alignment transform for a
/// preview, so a preview is lit identically to the place you cross into). `light_color` is
/// the place's district temperature; the warm lamp bodies stay neutral.
fn spawn_place_lighting(
    commands: &mut Commands,
    assets: &MatchAssets,
    geom: &teleport::PlaceGeom,
    light_color: Color,
    xform: Transform,
    preview: bool,
) {
    let (hx, hz) = (geom.half.x, geom.half.y);
    let place_in = |local: Transform| xform.mul_transform(local);
    // Overhead fill (steady; only the decoherence flash stutters it).
    spawn_place_light(
        commands,
        place_in(Transform::from_xyz(0.0, WALL_HEIGHT - 0.4, 0.0)),
        light_color,
        (hx.max(hz) + WALL_HEIGHT) * 1.6,
        FlickerLight {
            base: FIXTURE_LIGHT_INTENSITY,
            idle: 0.0,
            phase: 0.0,
        },
        preview,
    );
    // Flickering ceiling fixtures: the "failing office light" look + per-place interest.
    for (i, pos) in fixture_positions(geom).into_iter().enumerate() {
        let phase = i as f32 * 1.7 + 0.4;
        let mut lamp = commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.lamp_material.clone()),
            place_in(
                Transform::from_xyz(pos.x, WALL_HEIGHT - 0.16, pos.y)
                    .with_scale(Vec3::new(0.8, 0.12, 0.8)),
            ),
            Name::new("Ceiling fixture"),
        ));
        if preview {
            lamp.insert(PassagePreview);
        }
        spawn_place_light(
            commands,
            place_in(Transform::from_xyz(pos.x, WALL_HEIGHT - 0.6, pos.y)),
            light_color,
            11.0,
            FlickerLight {
                base: FIXTURE_LIGHT_INTENSITY * 0.55,
                idle: 0.7,
                phase,
            },
            preview,
        );
    }
}

/// The inward unit normal of edge `a`→`b` of a polygon centred at the origin (points
/// toward the interior), for tucking wall trim proud of the face.
fn inward_normal(a: Vec2, b: Vec2) -> Vec2 {
    let d = (b - a).normalize_or_zero();
    let n = Vec2::new(-d.y, d.x);
    if n.dot((a + b) * 0.5) > 0.0 { -n } else { n }
}

/// Draw the structural neon linework that gives a place's walls surface interest without
/// any textures (code-as-art): a baseboard seam at the floor and a cornice seam under the
/// ceiling, in the district's accent. Built under `xform` and tagged for the same
/// teardown/preview path as the rest of the place, so previews match what you walk into.
fn spawn_surface_detail(
    commands: &mut Commands,
    assets: &MatchAssets,
    geom: &teleport::PlaceGeom,
    accent: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
) {
    const LOW: f32 = 0.12;
    let high = WALL_HEIGHT - 0.18;
    let mut strip = |center: Vec3, scale: Vec3, yaw: f32| {
        let local = Transform::from_translation(center)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(scale);
        let mut e = commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(accent.clone()),
            xform.mul_transform(local),
            Name::new("Wall trim"),
        ));
        if preview {
            e.insert(PassagePreview);
        }
    };
    if let Some(poly) = geom.poly.as_ref() {
        let n = poly.len();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let d = b - a;
            let len = d.length();
            if len < 0.05 {
                continue;
            }
            let p = (a + b) * 0.5 + inward_normal(a, b) * 0.05;
            let yaw = (-d.y).atan2(d.x);
            for y in [LOW, high] {
                strip(Vec3::new(p.x, y, p.y), Vec3::new(len, 0.08, 0.06), yaw);
            }
        }
    } else {
        let (hx, hz) = (geom.half.x, geom.half.y);
        for y in [LOW, high] {
            for s in [-1.0_f32, 1.0] {
                // North/South walls run along X; West/East walls run along Z.
                strip(
                    Vec3::new(0.0, y, s * (hz - 0.05)),
                    Vec3::new(2.0 * hx, 0.08, 0.06),
                    0.0,
                );
                strip(
                    Vec3::new(s * (hx - 0.05), y, 0.0),
                    Vec3::new(0.06, 0.08, 2.0 * hz),
                    0.0,
                );
            }
        }
    }
}

/// A door leaf filling a sealed doorway gap, flush with the wall, so the opening reads as
/// shut rather than opening onto void. Transparent passage thresholds do not spawn leaves.
/// `closed_y`/`open_y` bracket the slide: closed sits on the floor up to the lintel, open
/// is lifted a full leaf-height so it tucks above the opening.
fn spawn_leaf(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &teleport::DoorGap,
    openable: bool,
    material: Handle<StandardMaterial>,
) {
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let leaf_h = WALL_HEIGHT - DOOR_LINTEL_H;
    let closed_y = leaf_h * 0.5;
    let open_y = closed_y + leaf_h;
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        DoorLeaf {
            center: gap.center,
            closed_y,
            open_y,
            openable,
        },
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_xyz(gap.center.x, closed_y, gap.center.y)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.1), leaf_h, DOOR_LEAF_D)),
        Name::new(if openable { "Door leaf" } else { "Closed door" }),
    ));
}

/// A red **locked** door leaf over the facility exit while the keystone gate is shut: a
/// sealed leaf (it never opens) plus its own neon frame.
fn spawn_locked_leaf(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &teleport::DoorGap,
    tethered: bool,
) {
    spawn_place_frame(commands, assets, gap, tethered);
    spawn_leaf(
        commands,
        assets,
        gap,
        false,
        assets.trap_active_material.clone(),
    );
}

/// A glowing gold keystone pickup at the centre of a room, tagged for proximity pickup.
fn spawn_keystone_item(commands: &mut Commands, assets: &MatchAssets, room: RoomId) {
    commands
        .spawn((
            KeystoneItem(room),
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.objective_material.clone()),
            Transform::from_xyz(0.0, 1.1, 0.0)
                .with_rotation(Quat::from_rotation_y(PI * 0.25))
                .with_scale(Vec3::splat(0.5)),
            Name::new("Keystone"),
        ))
        .with_children(|item| {
            item.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.82, 0.3),
                    intensity: 2_400.0,
                    range: 7.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::default(),
            ));
        });
}

fn spawn_dropped_item(commands: &mut Commands, assets: &MatchAssets, item: PlacedItem) {
    match item.kind {
        ItemKind::AnchorTorch => spawn_anchor_torch(commands, assets, item.pos),
        ItemKind::TeleportPad => spawn_teleport_pad(commands, assets, item.pos),
    }
}

fn spawn_anchor_torch(commands: &mut Commands, assets: &MatchAssets, pos: Vec2) {
    commands
        .spawn((
            DroppedItemVisual,
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.anchor_torch_material.clone()),
            Transform::from_xyz(pos.x, 0.55, pos.y).with_scale(Vec3::new(0.18, 1.1, 0.18)),
            Name::new("Anchor torch"),
        ))
        .with_children(|torch| {
            torch.spawn((
                Mesh3d(assets.halo_mesh.clone()),
                MeshMaterial3d(assets.anchor_torch_material.clone()),
                Transform::from_xyz(0.0, -0.52, 0.0).with_scale(Vec3::new(1.3, 1.0, 1.3)),
                Name::new("Anchor torch floor halo"),
            ));
            torch.spawn((
                PointLight {
                    color: style::marker(MarkerRole::Control).base_color,
                    intensity: 1_900.0,
                    range: 6.5,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.45, 0.0),
            ));
        });
}

fn spawn_teleport_pad(commands: &mut Commands, assets: &MatchAssets, pos: Vec2) {
    commands
        .spawn((
            DroppedItemVisual,
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.halo_mesh.clone()),
            MeshMaterial3d(assets.teleport_pad_material.clone()),
            Transform::from_xyz(pos.x, 0.05, pos.y).with_scale(Vec3::new(1.75, 1.0, 1.75)),
            Name::new("Teleport pad"),
        ))
        .with_children(|pad| {
            pad.spawn((
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(assets.teleport_pad_material.clone()),
                Transform::from_xyz(0.0, 0.10, 0.0).with_scale(Vec3::new(0.32, 0.08, 0.32)),
                Name::new("Teleport pad core"),
            ));
            pad.spawn((
                PointLight {
                    color: style::marker(MarkerRole::You).base_color,
                    intensity: 1_700.0,
                    range: 6.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.35, 0.0),
            ));
        });
}

/// Build the floor (or ceiling) mesh for a polygon room: a triangle fan from the centre
/// to each vertex, emitted with both windings so it's visible regardless of facing.
fn polygon_mesh(verts: &[Vec2], y: f32, normal_up: bool) -> Mesh {
    let ny = if normal_up { 1.0 } else { -1.0 };
    let mut positions: Vec<[f32; 3]> = vec![[0.0, y, 0.0]];
    let mut normals: Vec<[f32; 3]> = vec![[0.0, ny, 0.0]];
    let mut uvs: Vec<[f32; 2]> = vec![[0.5, 0.5]];
    for v in verts {
        positions.push([v.x, y, v.y]);
        normals.push([0.0, ny, 0.0]);
        uvs.push([0.5 + v.x * 0.04, 0.5 + v.y * 0.04]);
    }
    let n = verts.len() as u32;
    let mut indices: Vec<u32> = Vec::new();
    for i in 0..n {
        let a = 1 + i;
        let b = 1 + (i + 1) % n;
        indices.extend_from_slice(&[0, a, b, 0, b, a]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Spawn one piece of place geometry at `transform`, marking it a [`PassagePreview`]
/// when `preview` (so previews can be queried/tested); either way it is despawned with
/// the rest of the place geometry on the next teleport.
fn spawn_geo(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
    preview: bool,
    name: &'static str,
) {
    let mut entity = commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        transform,
        Name::new(name),
    ));
    if preview {
        entity.insert(PassagePreview);
    }
}

/// The floor + ceiling of a polygon room (custom fan meshes matching the footprint),
/// placed under `xform` (identity for the current place, the alignment transform for a
/// preview).
fn spawn_polygon_shell(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    poly: &[Vec2],
    floor_material: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
) {
    let floor = meshes.add(polygon_mesh(poly, 0.0, true));
    let ceiling = meshes.add(polygon_mesh(poly, WALL_HEIGHT, false));
    spawn_geo(
        commands,
        floor,
        floor_material,
        xform,
        preview,
        "Place floor",
    );
    spawn_geo(
        commands,
        ceiling,
        assets.ceiling_material.clone(),
        xform,
        preview,
        "Place ceiling",
    );
}

/// One angled wall panel per polygon edge, split around any doorway `open` returns true
/// for (so the body can walk through it / you can see in), placed under `xform`. Edges
/// with no open doorway are a solid wall.
fn spawn_polygon_walls(
    commands: &mut Commands,
    assets: &MatchAssets,
    poly: &[Vec2],
    gaps: &[teleport::DoorGap],
    xform: Transform,
    preview: bool,
    open: impl Fn(&teleport::DoorGap) -> bool,
) {
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let mid = (a + b) * 0.5;
        let gap = gaps.iter().find(|g| (g.center - mid).length() < 0.05);
        match gap {
            Some(g) if open(g) => {
                let dir = (b - a).normalize_or_zero();
                spawn_wall_segment(
                    commands,
                    assets,
                    a,
                    g.center - dir * (g.width * 0.5),
                    xform,
                    preview,
                );
                spawn_wall_segment(
                    commands,
                    assets,
                    g.center + dir * (g.width * 0.5),
                    b,
                    xform,
                    preview,
                );
            }
            _ => spawn_wall_segment(commands, assets, a, b, xform, preview),
        }
    }
}

/// A single rotated wall panel spanning `p1`→`p2` (extended slightly so corners seal),
/// placed under `xform`.
fn spawn_wall_segment(
    commands: &mut Commands,
    assets: &MatchAssets,
    p1: Vec2,
    p2: Vec2,
    xform: Transform,
    preview: bool,
) {
    const T: f32 = 0.4; // full thickness
    let d = p2 - p1;
    let len = d.length();
    if len < 0.05 {
        return;
    }
    let mid = (p1 + p2) * 0.5;
    let yaw = (-d.y).atan2(d.x); // align local +X with the edge direction
    let local = Transform::from_xyz(mid.x, WALL_HEIGHT * 0.5, mid.y)
        .with_rotation(Quat::from_rotation_y(yaw))
        .with_scale(Vec3::new(len + T, WALL_HEIGHT, T));
    spawn_geo(
        commands,
        assets.placeholder_mesh.clone(),
        assets.wall_material.clone(),
        xform.mul_transform(local),
        preview,
        "Room wall",
    );
}

/// How close (XZ, world units) the player must be for a passage door to open. Set wider
/// than a stride so the leaf has visibly slid up before you reach the threshold (the
/// open telegraphs the way on), rather than snapping open as you cross.
const DOOR_OPEN_RADIUS: f32 = 4.6;
/// Door-leaf slide speed (world units of vertical travel per second).
const DOOR_SLIDE_SPEED: f32 = 6.0;

/// Slide any future openable sealed leaf between shut and tucked-into-the-lintel by the
/// player's proximity. Current transparent passage thresholds do not spawn leaves, so most
/// leaves are sealed side doors / locked exits. Presentation-only: it reads the body
/// position and never writes match state.
pub(crate) fn animate_doors(
    time: Res<Time>,
    tp: Res<TeleportState>,
    paused: Res<MatchPaused>,
    mut leaves: Query<(&DoorLeaf, &mut Transform)>,
) {
    if paused.0 {
        return;
    }
    let body = Vec2::new(tp.body.position.x, tp.body.position.z);
    let max_step = DOOR_SLIDE_SPEED * time.delta_secs();
    for (leaf, mut transform) in &mut leaves {
        let target = if leaf.openable && body.distance(leaf.center) <= DOOR_OPEN_RADIUS {
            leaf.open_y
        } else {
            leaf.closed_y
        };
        let y = transform.translation.y;
        transform.translation.y = y + (target - y).clamp(-max_step, max_step);
    }
}

/// Reconcile + animate the **rival avatars** for the current place: spawn a walking
/// figure for each rival team whose clump shares the player's room, move the existing
/// ones along the room's exit axis (paced, so they never pop), and despawn any whose
/// team has moved on. Presentation-only — it reads the brain (`team_room`/`active_runner`)
/// and never writes match state, so determinism/replay/lockstep are untouched.
pub(crate) fn sync_rival_avatars(
    time: Res<Time>,
    runtime: Res<MatchRuntime>,
    tp: Res<TeleportState>,
    assets: Res<MatchAssets>,
    paused: Res<MatchPaused>,
    mut avatars: Query<(Entity, &RivalAvatar, &mut Transform)>,
    mut commands: Commands,
) {
    let game = runtime.live.host_match();
    // Only rooms host visible rivals — in this model a hallway is the player's alone.
    let present: Vec<usize> = match tp.place {
        Place::Room(room) => rivals::rivals_in_room(&game.competitive, room),
        Place::Hallway { .. } => Vec::new(),
    };

    let (a, b) = rivals::pace_segment(&tp.geom);
    let along = b - a;
    let tangent = Vec2::new(-along.y, along.x).normalize_or_zero();
    let n = present.len();

    // One mutable pass: despawn rivals that have left, walk the ones still here.
    let mut have: Vec<usize> = Vec::new();
    for (entity, avatar, mut transform) in &mut avatars {
        let Some(slot) = present.iter().position(|&t| t == avatar.team) else {
            commands.entity(entity).despawn();
            continue;
        };
        have.push(avatar.team);
        if paused.0 {
            continue; // hold position while paused
        }
        // Per-team phase + lateral lane so the rivals don't pace in lockstep or overlap.
        let phase = avatar.team as f32 * 0.7;
        let theta = time.elapsed_secs() * rivals::RIVAL_PACE_SPEED + phase;
        let u = rivals::triangle_wave(theta);
        let lane = (slot as f32 - (n as f32 - 1.0) * 0.5) * 1.3;
        let foot = a + along * u + tangent * lane;
        let bob = (theta * 6.0).sin() * 0.06; // a small walking bob
        transform.translation = Vec3::new(foot.x, 0.82 + bob, foot.y);
    }

    // Spawn newly co-present rivals at the back of the room (they walk in from there).
    for &team in &present {
        if !have.contains(&team) {
            commands.spawn((
                RivalAvatar { team },
                DespawnOnExit(GameState::Match),
                Mesh3d(assets.rival_body_mesh.clone()),
                MeshMaterial3d(assets.rival_material.clone()),
                Transform::from_xyz(a.x, 0.82, a.y),
                Name::new(format!("Rival team {team}")),
            ));
        }
    }
}
