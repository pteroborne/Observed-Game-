//! A place's structural shell: floor, ceiling, and walls for the two place shapes
//! (polygon room / box hallway), and the threshold gateways (doorframe + optional
//! sealed leaf) cut into them.

use bevy::prelude::*;
use observed_style::{self as style, MarkerRole};

use crate::GameState;
use crate::layout::{PLACE_TILE, WALL_HEIGHT};
use crate::view::assets::{DOOR_LEAF_D, DOOR_LINTEL_H, MatchAssets};
use crate::view::components::{DoorLeaf, PlaceGeometry};
use crate::view::theme::team_color;

use super::mesh;
use crate::teleport::{DeckSeg, DoorGap, PlaceGeom};

/// A polygon room's shell: the extruded floor/ceiling shell plus per-edge walls with
/// passage openings left open.
pub(crate) fn spawn_room_shell(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    geom: &PlaceGeom,
    floor_material: Handle<StandardMaterial>,
    y_offset: f32,
) {
    if let Some(poly) = geom.poly.as_ref() {
        let root_xform = Transform::from_xyz(0.0, y_offset, 0.0);
        mesh::spawn_polygon_shell(
            commands,
            assets,
            meshes,
            poly,
            floor_material,
            root_xform,
            false,
        );
        mesh::spawn_polygon_walls(commands, assets, poly, &geom.gaps, root_xform, false, |g| {
            g.kind.is_passage()
        });
    }
}

/// Whether an arena solid is the collision box `place_arena` generated for `deck`
/// (see `teleport::transition::place_arena`'s deck loop: a box centred at the deck's
/// XZ centre, spanning the deck's XZ half-extent, from `bottom_y` to `top_y`).
/// Matched by footprint + top height rather than object identity, since the arena
/// only stores `Aabb3`s — this is the explicit, named discrimination that keeps deck
/// solids out of the generic "Place wall" render path.
fn solid_is_deck(solid: &observed_traversal::Aabb3, deck: &DeckSeg, floor_y: f32) -> bool {
    const EPS: f32 = 0.02;
    let center = (solid.min + solid.max) * 0.5;
    let half = (solid.max - solid.min) * 0.5;
    (center.x - deck.center.x).abs() < EPS
        && (center.z - deck.center.y).abs() < EPS
        && (half.x - deck.half.x).abs() < EPS
        && (half.z - deck.half.y).abs() < EPS
        && (solid.min.y - (floor_y + deck.bottom_y)).abs() < EPS
        && (solid.max.y - (floor_y + deck.top_y)).abs() < EPS
}

/// A box hallway's shell: scaled floor/ceiling planes plus the collision arena's
/// interior maze walls rendered as solid blocks. `decks` (the gantry's platforms +
/// mount stair, empty everywhere else) are rendered separately by
/// [`spawn_gantry_decks`] and excluded here so they never double up as generic walls.
pub(crate) fn spawn_hallway_shell(
    commands: &mut Commands,
    assets: &MatchAssets,
    geom: &PlaceGeom,
    floor_material: Handle<StandardMaterial>,
    solids: &[observed_traversal::Aabb3],
    y_offset: f32,
) {
    let (hx, hz) = (geom.half.x, geom.half.y);
    let shell_height = crate::teleport::structural_height(geom, WALL_HEIGHT);
    let plane_scale = Vec3::new(hx * 2.0 / PLACE_TILE, 1.0, hz * 2.0 / PLACE_TILE);
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.floor_mesh.clone()),
        MeshMaterial3d(floor_material),
        Transform::from_xyz(0.0, y_offset, 0.0).with_scale(plane_scale),
        Name::new("Place floor"),
    ));
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.ceiling_mesh.clone()),
        MeshMaterial3d(assets.ceiling_material.clone()),
        Transform::from_xyz(0.0, y_offset + shell_height, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::PI))
            .with_scale(plane_scale),
        Name::new("Place ceiling"),
    ));
    for solid in solids {
        if geom
            .decks
            .iter()
            .any(|deck| solid_is_deck(solid, deck, y_offset))
        {
            continue;
        }
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
    if !geom.decks.is_empty() {
        spawn_gantry_decks(commands, assets, &geom.decks, y_offset);
    }
}

/// Half-thickness of a deck's lit rim strip (world units); slightly proud of the deck
/// top so it reads as a distinct edge line rather than blending into the deck surface.
const DECK_EDGE_THICKNESS: f32 = 0.08;
const DECK_EDGE_PROUD: f32 = 0.03;

/// Render the gantry's walkable decks (six jump-map platforms + the mount stair) as
/// standable, readable geometry: a deck box in the `GantryDeck` treatment, plus four
/// thin emissive rim strips along its top edges in the `GantryEdge` treatment — "chosen,
/// readable irreversibility" (docs/contention_arc_plan.md, Phase 40): the platform edges
/// must be lit and the commitment legible before the jump. Also drops one understory
/// landing marker spanning the platform lane's footprint at ground level, the visible
/// "where a fall puts you" read.
pub(crate) fn spawn_gantry_decks(
    commands: &mut Commands,
    assets: &MatchAssets,
    decks: &[DeckSeg],
    y_offset: f32,
) {
    for deck in decks {
        let deck_height = (deck.top_y - deck.bottom_y).max(0.02);
        let top_world_y = y_offset + deck.top_y;
        let center_world_y = y_offset + deck.bottom_y + deck_height * 0.5;
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.gantry_deck_material.clone()),
            Transform::from_xyz(deck.center.x, center_world_y, deck.center.y)
                .with_scale(Vec3::new(deck.half.x * 2.0, deck_height, deck.half.y * 2.0)),
            Name::new("Gantry deck"),
        ));

        // Four rim strips along the deck's top edges (±X and ±Z sides), proud of the
        // deck surface so the platform boundary reads as a lit commitment line.
        let edge_y = top_world_y + DECK_EDGE_PROUD;
        let edges = [
            // -X / +X edges run along Z.
            (
                Vec3::new(deck.center.x - deck.half.x, edge_y, deck.center.y),
                Vec3::new(DECK_EDGE_THICKNESS, DECK_EDGE_THICKNESS, deck.half.y * 2.0),
            ),
            (
                Vec3::new(deck.center.x + deck.half.x, edge_y, deck.center.y),
                Vec3::new(DECK_EDGE_THICKNESS, DECK_EDGE_THICKNESS, deck.half.y * 2.0),
            ),
            // -Z / +Z edges run along X.
            (
                Vec3::new(deck.center.x, edge_y, deck.center.y - deck.half.y),
                Vec3::new(deck.half.x * 2.0, DECK_EDGE_THICKNESS, DECK_EDGE_THICKNESS),
            ),
            (
                Vec3::new(deck.center.x, edge_y, deck.center.y + deck.half.y),
                Vec3::new(deck.half.x * 2.0, DECK_EDGE_THICKNESS, DECK_EDGE_THICKNESS),
            ),
        ];
        for (center, scale) in edges {
            commands.spawn((
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(assets.gantry_edge_material.clone()),
                Transform::from_translation(center).with_scale(scale),
                Name::new("Gantry deck edge"),
            ));
        }
    }

    // The understory floor beneath the platform lane: one flat marker spanning every
    // deck's XZ footprint at ground level — the visible "where failure puts you" read.
    let lane_min = decks.iter().fold(Vec2::splat(f32::INFINITY), |acc, d| {
        acc.min(d.center - d.half)
    });
    let lane_max = decks.iter().fold(Vec2::splat(f32::NEG_INFINITY), |acc, d| {
        acc.max(d.center + d.half)
    });
    let lane_center = (lane_min + lane_max) * 0.5;
    let lane_half = (lane_max - lane_min) * 0.5;
    const UNDERSTORY_THICKNESS: f32 = 0.06;
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(assets.understory_material.clone()),
        Transform::from_xyz(
            lane_center.x,
            y_offset + UNDERSTORY_THICKNESS * 0.5,
            lane_center.y,
        )
        .with_scale(Vec3::new(
            (lane_half.x * 2.0).max(0.1),
            UNDERSTORY_THICKNESS,
            (lane_half.y * 2.0).max(0.1),
        )),
        Name::new("Understory landing"),
    ));
}

/// How a threshold gateway is dressed: an optional sealed leaf (and whether it can
/// slide open), plus the tether light on the frame. Three kinds of threshold exist —
/// open passages, the locked exit, and sealed side doors — each a constructor here.
pub(crate) struct ThresholdStyle {
    leaf_material: Option<Handle<StandardMaterial>>,
    leaf_name: &'static str,
    openable: bool,
    tethered: bool,
    /// A rival team index whose presence beyond this threshold pins it (Phase 38
    /// contested observation): the frame light takes that team's colour so the
    /// player reads "a rival is holding this door open".
    rival_presence: Option<usize>,
    /// A rival team index that has an anchor on the room beyond this threshold
    /// (Phase 42): rendered as a torch prop plus a brighter frame light than mere
    /// presence, since an anchor is a durable claim rather than a passer-through.
    rival_anchor: Option<usize>,
}

impl ThresholdStyle {
    /// An always-open passage: frame only, no hiding leaf. `rival_presence` tints the
    /// frame light when a rival team's presence pins the connection beyond it;
    /// `rival_anchor` additionally spawns a torch prop and takes priority over mere
    /// presence when both are set (an anchor can outlast the team that placed it).
    pub(crate) fn passage(
        tethered: bool,
        rival_presence: Option<usize>,
        rival_anchor: Option<usize>,
    ) -> Self {
        Self {
            leaf_material: None,
            leaf_name: "Door leaf",
            openable: false,
            tethered,
            rival_presence,
            rival_anchor,
        }
    }

    /// The keystone-locked exit: sealed with the armed-trap red leaf.
    pub(crate) fn locked_exit(assets: &MatchAssets, tethered: bool) -> Self {
        Self {
            leaf_material: Some(assets.trap_active_material.clone()),
            leaf_name: "Locked exit",
            openable: false,
            tethered,
            rival_presence: None,
            rival_anchor: None,
        }
    }

    /// A collapse-sealed threshold: visible rubble, never traversable or tethered.
    pub(crate) fn collapsed(assets: &MatchAssets) -> Self {
        Self {
            leaf_material: Some(assets.rubble_material.clone()),
            leaf_name: "Collapsed rubble",
            openable: false,
            tethered: false,
            rival_presence: None,
            rival_anchor: None,
        }
    }

    /// A sealed side door (decor / future route): a plain closed leaf, never tethered.
    pub(crate) fn side_door(assets: &MatchAssets) -> Self {
        Self {
            leaf_material: Some(assets.door_leaf_material.clone()),
            leaf_name: "Closed door",
            openable: false,
            tethered: false,
            rival_presence: None,
            rival_anchor: None,
        }
    }
}

/// Spawn one threshold gateway: the frame + lintel + tether light, and — for sealed
/// styles — the closed leaf filling the opening.
pub(crate) fn spawn_threshold_gateway(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    threshold_style: ThresholdStyle,
    y_offset: f32,
) {
    spawn_place_frame(
        commands,
        assets,
        gap,
        threshold_style.tethered,
        threshold_style.rival_presence,
        threshold_style.rival_anchor,
        y_offset,
    );
    if let Some(leaf_material) = threshold_style.leaf_material {
        spawn_leaf(
            commands,
            assets,
            gap,
            threshold_style.openable,
            leaf_material,
            threshold_style.leaf_name,
            y_offset,
        );
    }
}

fn gap_yaw(normal: Vec2) -> f32 {
    let along = Vec2::new(-normal.y, normal.x);
    (-along.y).atan2(along.x)
}

/// A rival team's anchor torch prop at a threshold frame (Phase 42): a small emissive
/// box (from the shared placeholder mesh) plus a floor halo, team-coloured, so a
/// rival-anchored doorway reads as "their torch holds this door" rather than only a
/// tinted light. Mirrors `item_visuals::spawn_anchor_torch`'s box + halo shape.
fn spawn_rival_anchor_torch(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    team: usize,
    y_offset: f32,
) {
    let along = Vec2::new(-gap.normal.y, gap.normal.x);
    let pos = gap.center + along * (gap.width * 0.5 + 0.3);
    let root = Transform::from_xyz(pos.x, y_offset + gap.floor_y + 0.55, pos.y)
        .with_scale(Vec3::new(0.18, 1.1, 0.18));
    spawn_rival_anchor_torch_at(commands, assets, root, team);
}

/// The rival-anchor-torch shape (Phase 42, extended Phase 42c for doorway previews): a
/// small emissive box plus a floor halo and point light, team-coloured, spawned at an
/// already-resolved root `Transform` — a world-frame threshold placement (the caller
/// above) or a place-preview's local frame (`super::preview::spawn_preview_anchor_torch`,
/// composed under the preview's parent transform the same way `preview::spawn_room_preview`
/// composes its rival avatars). Also tagged [`super::super::view::components::PassagePreview`]-adjacent
/// callers should add that marker themselves so it despawns/queries like other preview
/// geometry; this helper only owns the shape.
pub(crate) fn spawn_rival_anchor_torch_at(
    commands: &mut Commands,
    assets: &MatchAssets,
    root: Transform,
    team: usize,
) -> Entity {
    let color = team_color(team);
    let material = assets
        .team_materials
        .get(team % assets.team_materials.len())
        .cloned()
        .unwrap_or_else(|| assets.rival_material.clone());
    commands
        .spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(material.clone()),
            root,
            Name::new("Rival anchor torch"),
        ))
        .with_children(|torch| {
            torch.spawn((
                Mesh3d(assets.halo_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, -0.52, 0.0).with_scale(Vec3::new(1.3, 1.0, 1.3)),
                Name::new("Rival anchor torch floor halo"),
            ));
            torch.spawn((
                PointLight {
                    color,
                    intensity: 1_900.0,
                    range: 6.5,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.45, 0.0),
            ));
        })
        .id()
}

/// A neon doorway frame (two posts + a lintel) at a gap, built from primitives and
/// rotated to the gap's wall so it matches the opening at any angle.
fn spawn_place_frame(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    tethered: bool,
    rival_presence: Option<usize>,
    rival_anchor: Option<usize>,
    y_offset: f32,
) {
    let material = assets.doorframe_material.clone();
    let along = Vec2::new(-gap.normal.y, gap.normal.x);
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let half = gap.width * 0.5;
    let base_y = y_offset + gap.floor_y;
    let status = crate::evidence::threshold_status(gap, tethered);
    for offset in [half, -half] {
        let p = gap.center + along * offset;
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            crate::evidence::DiagnosticThresholdVisual {
                threshold: gap.threshold,
                kind: crate::evidence::DiagnosticThresholdVisualKind::Frame,
                status,
            },
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(p.x, base_y + WALL_HEIGHT * 0.5, p.y).with_scale(Vec3::new(
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
        crate::evidence::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::evidence::DiagnosticThresholdVisualKind::Frame,
            status,
        },
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_xyz(gap.center.x, base_y + WALL_HEIGHT - 0.2, gap.center.y)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.3), 0.34, 0.24)),
        Name::new("Doorframe lintel"),
    ));

    // Light priority (Phase 41 collapse override, Phase 42 rival attribution): your
    // own tether (control colour) outranks a rival's anchor, which outranks a rival's
    // mere presence, which outranks the neutral idle frame. Collapse overrides all of
    // it — it's the one force even an anchor cannot hold back. An anchor is a durable
    // claim, so it burns brighter than a passer-through's presence.
    const PRESENCE_INTENSITY: f32 = 1_400.0;
    const ANCHOR_INTENSITY: f32 = 2_200.0;
    let (tether_color, intensity) = match status {
        crate::evidence::DiagnosticThresholdStatus::Collapsed => (
            style::surface(style::SurfaceRole::Rubble)
                .edge
                .unwrap_or(style::marker(MarkerRole::Collapse).base_color),
            PRESENCE_INTENSITY,
        ),
        _ if tethered => (
            style::marker(MarkerRole::Control).base_color,
            PRESENCE_INTENSITY,
        ),
        _ if let Some(team) = rival_anchor => (team_color(team), ANCHOR_INTENSITY),
        _ if let Some(team) = rival_presence => (team_color(team), PRESENCE_INTENSITY),
        _ => (Color::srgb(0.45, 0.62, 0.78), PRESENCE_INTENSITY),
    };
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        crate::evidence::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::evidence::DiagnosticThresholdVisualKind::FrameLight,
            status,
        },
        PointLight {
            color: tether_color,
            intensity,
            range: 5.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(gap.center.x, base_y + WALL_HEIGHT - 0.35, gap.center.y),
        Name::new("Doorframe tether light"),
    ));

    // A rival anchor beyond this threshold also gets a physical torch prop at the
    // frame (unless collapse has sealed the threshold — rubble is the whole read then).
    if status != crate::evidence::DiagnosticThresholdStatus::Collapsed
        && let Some(team) = rival_anchor
    {
        spawn_rival_anchor_torch(commands, assets, gap, team, y_offset);
    }

    if crate::evidence::visual_audit_enabled() {
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            crate::evidence::DiagnosticThresholdVisual {
                threshold: gap.threshold,
                kind: crate::evidence::DiagnosticThresholdVisualKind::Label,
                status,
            },
            Text2d::new(crate::evidence::threshold_label(&gap.threshold)),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(gap.center.x, base_y + WALL_HEIGHT + 0.35, gap.center.y)
                .with_rotation(rot)
                .with_scale(Vec3::splat(0.035)),
            Name::new("Threshold debug label"),
        ));
    }
}

/// A door leaf filling a sealed doorway gap, flush with the wall.
fn spawn_leaf(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    openable: bool,
    material: Handle<StandardMaterial>,
    name: &'static str,
    y_offset: f32,
) {
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let leaf_h = WALL_HEIGHT - DOOR_LINTEL_H;
    let base_y = y_offset + gap.floor_y;
    let closed_y = base_y + leaf_h * 0.5;
    let open_y = closed_y + leaf_h;
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        crate::evidence::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::evidence::DiagnosticThresholdVisualKind::Leaf,
            status: crate::evidence::threshold_status(gap, false),
        },
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
        Name::new(if openable { "Door leaf" } else { name }),
    ));
}
