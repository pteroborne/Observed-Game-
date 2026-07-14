//! The place rebuild entry point: `rebuild_place` tears down and respawns a place's
//! entire presentation (shell, thresholds, monitors, items, lighting, guardian) whenever
//! the teleport signature changes, plus the room-tinted floor material/light colour
//! helpers it uses to give each room a distinct hue.

use bevy::prelude::*;
use bevy_sprite3d::prelude::Sprite3d;
use observed_core::{RoomId, SplitMix};
use observed_facility::map_spec::RoomRole;
use observed_style::{self as style, SurfaceRole};

use crate::GameState;
use crate::hallway;
use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::layout::WALL_HEIGHT;
use crate::screens::match_runtime;
use crate::sim::director::MatchDirector;
use crate::sim::state::TeleportState;
use crate::teleport::{self, GapKind, Place};
use crate::view::assets::MatchAssets;
use crate::view::components::PlaceGeometry;

use super::monitors::{
    GuardianConsole, ObservationBankSpec, monitor_page_for, spawn_observation_monitors,
};
use super::{item_visuals, lighting, modules, preview, shell};

pub(crate) fn place_surface_material(
    role: SurfaceRole,
    palette: &style::DistrictPalette,
    base_handle: &Handle<StandardMaterial>,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let mut mat = (*materials.get(base_handle).unwrap()).clone();
    crate::view::assets::apply_surface_palette(&mut mat, &style::surface(role), palette);
    materials.add(mat)
}

/// The place-rebuild signature: everything that must be identical for a cached place
/// to stay valid. `u64` is the tether hash, the trailing `u64` is `rival_signature_hash`
/// (Phase 42: refreshes the place when a rival's presence/anchor changes on any
/// neighbour, closing the stale-frame-light gap the tuple used to leave open).
type PlaceSignature = (
    Place,
    u64,
    usize,
    Vec<teleport::ThresholdSlotId>,
    observed_match::facility::CollapseState,
    bool,
    u64,
);

#[derive(Resource, Default)]
pub(crate) struct LastRenderedSignature(pub(crate) Option<PlaceSignature>);

/// Mix every neighbour's rival presence/anchor attribution into one hash so the place
/// signature changes whenever a rival moves into/out of a neighbour or plants/removes
/// an anchor there — otherwise a rival-state change with everything else unchanged
/// (sealed slots, item count, collapse state, klaxon) would leave the cached signature
/// untouched and the stale frame light would never refresh (mirrors how `tethers_hash`
/// above folds per-connection state into the signature).
fn rival_signature_hash(signals: &[crate::sim::nav::RivalSignal]) -> u64 {
    let mut mix = SplitMix(0x5257_4956_414C_2148);
    for signal in signals {
        mix.0 = mix.0.wrapping_add(signal.neighbor.0 as u64).wrapping_add(1);
        let _ = mix.next_u64();
        if let Some(team) = signal.presence {
            mix.0 = mix
                .0
                .wrapping_add((team.0 as u64).wrapping_mul(0x1000_0001));
            let _ = mix.next_u64();
        }
        if let Some(team) = signal.anchor {
            mix.0 = mix
                .0
                .wrapping_add((team.0 as u64).wrapping_mul(0x2000_0003));
            let _ = mix.next_u64();
        }
    }
    mix.0
}

/// Rebuild the place presentation geometry (floors, walls, ceiling, previews, lights, items).
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_place(
    assets: Res<MatchAssets>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    guardian: Option<Res<crate::guardian::Guardian>>,
    runtime: Res<MatchDirector>,
    existing: Query<Entity, With<PlaceGeometry>>,
    last_sig: Option<ResMut<LastRenderedSignature>>,
    mut commands: Commands,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
) {
    let tp = tp.into_inner();
    let game = runtime.live.host_match();
    let seed_val = seed.map(|s| s.0).unwrap_or(crate::flow::MATCH_SEED);
    let nav = crate::sim::nav::nav_for_place(seed_val, game, &keys, &items, tp.place);

    // The room whose thresholds are on screen right now — the room side of a
    // `Place::Room`, or the room the player is standing in for a hallway.
    let signal_room = match tp.place {
        Place::Room(room) => room,
        Place::Hallway { from, .. } => from,
    };
    let local_team = crate::flow::LOCAL_TEAM.0 as usize;
    let rival_signals = crate::sim::nav::rival_signals(game, local_team, signal_room);

    // Signature to detect if the place needs rebuilding (e.g. dropped items or tethers changed)
    let signature = {
        let mut tethers_hash = 0u64;
        for &conn in &nav.connections {
            if nav.is_tethered_corridor(crate::teleport::corridor_id_for(game.local_room(), conn)) {
                tethers_hash += 1;
            }
        }
        let item_count = items.placed_in(tp.place).len();
        (
            tp.place,
            tethers_hash,
            item_count,
            nav.sealed_slots.clone(),
            match_runtime::collapse_state_for_place(game, tp.place),
            match_runtime::countdown_klaxon_active(&runtime),
            rival_signature_hash(&rival_signals),
        )
    };

    let mut rebuild = true;
    if let Some(mut sig) = last_sig {
        if tp.rendered == Some(tp.place) && sig.0.as_ref() == Some(&signature) {
            rebuild = false;
        } else {
            sig.0 = Some(signature);
        }
    } else {
        commands.insert_resource(LastRenderedSignature(Some(signature)));
    }

    if tp.rendered == Some(tp.place) && !rebuild {
        return;
    }
    tp.rendered = Some(tp.place);

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let y_offset = teleport::place_y_offset(tp.place);
    // Rendering consumes the exact transaction installed by crossing. Presentation-only
    // changes may rebuild this scene, but may never regenerate geometry or collision.
    let geom = tp.geom.clone();

    let palette = match_runtime::palette_for_match(seed_val, tp.place, &runtime);

    // The place's structural shell (floor/ceiling/walls), by place shape.
    match tp.place {
        Place::Room(room) => {
            let floor_material = place_surface_material(
                SurfaceRole::Plain,
                &palette,
                &assets.floor_material,
                &mut materials,
            );
            let ceiling_material = place_surface_material(
                SurfaceRole::Ceiling,
                &palette,
                &assets.ceiling_material,
                &mut materials,
            );

            let wall_material = if let Some(spec) = &game.competitive.map_spec
                && let Some(r_spec) = spec.room(room)
                && (r_spec.role == RoomRole::Monitor || r_spec.role == RoomRole::GuardianControl)
                && let Some(ref lab_tex) = assets.wall_albedo_lab
            {
                let custom_wall = crate::view::assets::palette_tinted_neon_material(
                    &style::surface(SurfaceRole::Wall),
                    &palette,
                    Some(lab_tex.clone()),
                );
                materials.add(custom_wall)
            } else {
                place_surface_material(
                    SurfaceRole::Wall,
                    &palette,
                    &assets.wall_material,
                    &mut materials,
                )
            };

            shell::spawn_room_shell(
                &mut commands,
                &assets,
                &mut meshes,
                &geom,
                floor_material,
                wall_material,
                ceiling_material,
                y_offset,
            );
        }
        Place::Hallway { variation, .. } => {
            let floor_material = place_surface_material(
                SurfaceRole::Plain,
                &palette,
                &assets.floor_material,
                &mut materials,
            );
            let wall_material = place_surface_material(
                SurfaceRole::Wall,
                &palette,
                &assets.wall_material,
                &mut materials,
            );
            let ceiling_material = place_surface_material(
                SurfaceRole::Ceiling,
                &palette,
                &assets.ceiling_material,
                &mut materials,
            );
            if tp.layout.is_none() {
                let primitives =
                    teleport::place_structural_primitives(&geom, y_offset, WALL_HEIGHT);
                shell::spawn_hallway_shell(
                    &mut commands,
                    &assets,
                    &mut meshes,
                    &geom,
                    floor_material,
                    wall_material,
                    ceiling_material,
                    &primitives,
                    y_offset,
                );
            } else {
                shell::spawn_hallway_floor_ceiling(
                    &mut commands,
                    &mut meshes,
                    &geom,
                    floor_material,
                    ceiling_material,
                    y_offset,
                );
                let spec = tp
                    .layout
                    .as_ref()
                    .and_then(|layout| {
                        tp.collision_catalog
                            .arena_for_layout(layout, &geom, y_offset)
                    })
                    .expect("authored layout has valid arena spec");
                super::authored::spawn_collision_shell(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &spec,
                    &palette,
                    super::authored::ShellMaterials {
                        floor: &assets.floor_material,
                        wall: &assets.wall_material,
                        interior: (hallway::template(variation).flavor
                            == hallway::HallwayFlavor::Gantry)
                            .then_some((SurfaceRole::GantryDeck, &assets.gantry_deck_material)),
                    },
                );
            }
            // The WFC-composed light-module layer (Arc I Phase 71): decoration
            // and light only, solved from the finished geometry — walls, gaps,
            // and thresholds are already final by the time this runs.
            if let Place::Hallway { from, to, .. } = tp.place {
                let district = match_runtime::district_for_place(seed_val, tp.place);
                let placements = modules::solve_hallway_modules(
                    modules::hall_module_seed(seed_val, from.0, to.0),
                    &geom,
                    teleport::MAZE_CELL,
                    district,
                );
                modules::spawn_hallway_modules(
                    &mut commands,
                    &assets,
                    &mut materials,
                    modules::ModuleSpawn {
                        palette: &palette,
                        placements: &placements,
                        cell: teleport::MAZE_CELL,
                        xform: Transform::from_xyz(0.0, y_offset, 0.0),
                        preview: false,
                    },
                );
            }
        }
    }

    // The threshold gateways cut into that shell.
    let mut portal_index = 0;
    for gap in &geom.gaps {
        if gap.kind == teleport::GapKind::OneWayEntry {
            continue;
        }
        let (ea, eb) = match tp.place {
            Place::Room(room) => (room, gap.target),
            Place::Hallway { from, to, .. } => (from, to),
        };
        let tethered = nav.is_tethered_corridor(crate::teleport::corridor_id_for(ea, eb));
        // Phase 38 contested observation: a rival team standing in the room beyond
        // this threshold pins it for everyone; Phase 42 also attributes a rival's
        // *anchor* there (`rival_signals` is the one reconciliation point between
        // that heuristic and `CompetitiveFacility::pin_sources`' anchor bookkeeping).
        let rival_signal = rival_signals
            .iter()
            .find(|signal| signal.neighbor == gap.target)
            .copied();
        let rival_anchor = rival_signal.and_then(|s| s.anchor).map(|t| t.0 as usize);

        if gap.kind.is_passage() {
            let transit = tp
                .transits
                .iter()
                .find(|transit| transit.source_gap.threshold == gap.threshold)
                .cloned();
            preview::spawn_passage_preview(
                &mut commands,
                &assets,
                &mut meshes,
                &mut materials,
                &mut images,
                gap,
                tp.place,
                transit.as_ref(),
                portal_index,
                seed_val,
                game,
                match_runtime::countdown_klaxon_active(&runtime),
            );
            portal_index += 1;

            shell::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                shell::ThresholdStyle::passage(tethered, rival_anchor),
                y_offset,
            );
        } else if gap.kind == GapKind::LockedExit {
            shell::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                shell::ThresholdStyle::locked_exit(&assets, tethered),
                y_offset,
            );
            spawn_exit_unlock_iconography(&mut commands, &assets, &images, &keys, gap, y_offset);
        } else if gap.kind == GapKind::Collapsed {
            shell::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                shell::ThresholdStyle::collapsed(&assets),
                y_offset,
            );
        } else {
            shell::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                shell::ThresholdStyle::side_door(&assets),
                y_offset,
            );
        }
    }

    // Spawn specialized room visual elements (monitors, interactive console) — driven by
    // the active map spec's semantic roles, never a literal room id (Arc D stage D1 fix:
    // the old `room.0 == 5/6/3` checks silently stopped matching once the map moved to
    // `sector_relay_v1`, which is why the observation rooms stopped showing anything).
    if let Place::Room(room) = tp.place
        && let Some(spec) = &game.competitive.map_spec
    {
        if spec.room(room).is_some_and(|r| r.role == RoomRole::Monitor)
            && let Some(page) = monitor_page_for(spec, room)
        {
            // One unified 3x3 camera bank: every target room can show both anchor and
            // guardian overlays, rather than the old arbitrary tether/guardian half-page.
            spawn_observation_monitors(
                &mut commands,
                &assets,
                &mut materials,
                ObservationBankSpec {
                    geom: &geom,
                    y_offset,
                    seed: seed_val,
                    room,
                    page: &page,
                },
            );
        } else if spec
            .room(room)
            .is_some_and(|r| r.role == RoomRole::GuardianControl)
        {
            // Guardian Control Room: central interactive console
            let console_inactive = materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.2, 0.2),
                emissive: LinearRgba::new(0.2, 0.2, 0.2, 1.0),
                ..default()
            });
            commands.spawn((
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                GuardianConsole,
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(console_inactive),
                Transform::from_xyz(0.0, 1.0 + y_offset, 0.0).with_scale(Vec3::new(1.0, 2.0, 1.0)),
                Name::new("Guardian Control Console"),
            ));
        }
    }

    // Spawn keystone item
    if let Place::Room(room) = tp.place
        && keys.has_uncollected(room)
    {
        item_visuals::spawn_keystone_item(&mut commands, &assets, &images, room, y_offset);
    }

    // Spawn dropped items
    for item in items.placed_in(tp.place) {
        item_visuals::spawn_dropped_item(&mut commands, &assets, &images, item, y_offset);
    }

    // Lighting and surface details
    let place_transform = Transform::from_xyz(0.0, y_offset, 0.0);
    lighting::spawn_place_lighting(
        &mut commands,
        &assets,
        &geom,
        &palette,
        place_transform,
        false,
    );

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
    lighting::spawn_surface_detail(
        &mut commands,
        &assets,
        &geom,
        accent,
        place_transform,
        false,
    );

    // Spawn guardian
    if let Place::Room(room) = tp.place
        && let Some(guardian) = guardian.as_ref()
        && room == guardian.room
    {
        spawn_guardian_model(&mut commands, &assets, &images, guardian.pos);
    }

    // Spawn dressing props
    match tp.place {
        Place::Room(room) => {
            if let Some(spec) = &game.competitive.map_spec
                && let Some(r_spec) = spec.room(room)
            {
                spawn_room_dressing(
                    &mut commands,
                    &assets,
                    &images,
                    room,
                    r_spec.role,
                    &geom,
                    y_offset,
                    seed_val,
                );
            }
        }
        Place::Hallway { .. } => {
            spawn_hallway_dressing(&mut commands, &assets, &images, &geom, y_offset, seed_val);
        }
    }
}

#[allow(clippy::collapsible_if)]
pub(crate) fn spawn_guardian_model(
    commands: &mut Commands,
    assets: &MatchAssets,
    images: &Assets<Image>,
    pos: Vec3,
) {
    if let (Some(sheet), Some(layout), Some(meta)) = (
        &assets.guardian_actor_sheet,
        &assets.guardian_actor_layout,
        &assets.guardian_actor_meta,
    ) {
        if images.contains(sheet) {
            commands
                .spawn((
                    crate::guardian::GuardianModel,
                    PlaceGeometry,
                    DespawnOnExit(GameState::Match),
                    Sprite {
                        image: sheet.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: layout.clone(),
                            index: 0,
                        }),
                        ..default()
                    },
                    Sprite3d {
                        pixels_per_metre: meta.pixels_per_metre,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        emissive: observed_style::marker(observed_style::MarkerRole::Director)
                            .emissive,
                        pivot: Some(Vec2::new(meta.pivot.0, meta.pivot.1)),
                        double_sided: true,
                        ..default()
                    },
                    crate::view::components::BillboardSprite,
                    Transform::from_xyz(pos.x, (pos.y - 0.76).max(0.0) + 0.02, pos.z),
                    Name::new("Guardian sheet sprite"),
                ))
                .with_children(|g| {
                    g.spawn((
                        PointLight {
                            color: Color::srgb(1.0, 0.05, 0.05),
                            intensity: 3000.0,
                            range: 9.0,
                            shadows_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.9, 0.0),
                    ));
                });
            return;
        }
    }

    if let Some(image) = assets.guardian_sprite(images) {
        commands
            .spawn((
                crate::guardian::GuardianModel,
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                crate::view::sprites::sprite3d_components(
                    image,
                    &observed_style::marker(observed_style::MarkerRole::Director),
                    crate::view::sprites::ACTOR_PIXELS_PER_METRE,
                ),
                Transform::from_xyz(pos.x, (pos.y - 0.76).max(0.0) + 0.02, pos.z),
                Name::new("Guardian sprite"),
            ))
            .with_children(|g| {
                g.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.05, 0.05),
                        intensity: 3000.0,
                        range: 9.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.9, 0.0),
                ));
            });
        return;
    }

    commands
        .spawn((
            crate::guardian::GuardianModel,
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.rival_body_mesh.clone()),
            MeshMaterial3d(assets.trap_active_material.clone()),
            Transform::from_translation(pos),
            Name::new("Guardian"),
        ))
        .with_children(|g| {
            g.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.05, 0.05),
                    intensity: 3000.0,
                    range: 9.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.8, 0.0),
            ));
        });
}

fn spawn_exit_unlock_iconography(
    commands: &mut Commands,
    assets: &MatchAssets,
    images: &Assets<Image>,
    keys: &KeystoneState,
    gap: &teleport::DoorGap,
    y_offset: f32,
) {
    let card_image = assets.keystone_card_sprite(images);
    let core_image = assets.keystone_core_sprite(images);
    let exit_card_image = assets.exit_access_card_sprite(images);

    let (Some(card_img), Some(core_img)) = (card_image, core_image) else {
        return;
    };

    let along = Vec2::new(-gap.normal.y, gap.normal.x);

    let required = keys.required as usize;
    let held = keys.held as usize;

    let spacing = 0.35;
    let start_offset = -((required - 1) as f32) * spacing * 0.5;

    for i in 0..required {
        let is_collected = i < held;

        let (img, name) = if i % 3 == 0 {
            (card_img.clone(), "Required card icon")
        } else if i % 3 == 1 {
            (core_img.clone(), "Required core icon")
        } else if let Some(ref exit_img) = exit_card_image {
            (exit_img.clone(), "Required exit card icon")
        } else {
            (card_img.clone(), "Required card icon")
        };

        let treatment = if is_collected {
            observed_style::marker(observed_style::MarkerRole::You)
        } else {
            observed_style::marker(observed_style::MarkerRole::Rival)
        };

        let offset_2d = along * (start_offset + (i as f32) * spacing) + gap.normal * 0.12;
        let pos = gap.center + offset_2d;

        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            crate::view::sprites::sprite3d_components_with_pivot(
                img,
                &treatment,
                crate::view::sprites::DEVICE_PIXELS_PER_METRE,
                Vec2::new(0.5, 0.5),
            ),
            Transform::from_xyz(pos.x, y_offset + gap.floor_y + 1.2, pos.y),
            Name::new(name),
        ));
    }
}

fn is_safe_placement(p: Vec2, geom: &teleport::PlaceGeom, placed: &[Vec2], is_room: bool) -> bool {
    // 1. Wall clearance: must be inside the room's polygon (if it is a room) and not too close to the boundary.
    if is_room {
        let clamped = teleport::contain(geom, p, 0.8); // 0.8 units clear of walls
        if (clamped - p).length() > 0.01 {
            return false;
        }
    } else {
        // Hallway: check bounding box half-extents
        let margin = 0.8;
        if p.x.abs() >= geom.half.x - margin || p.y.abs() >= geom.half.y - margin {
            return false;
        }
    }

    // 2. Center clearance (for keystones/consoles/teleport pads at center)
    if is_room && p.length() < 1.8 {
        return false;
    }

    // 3. Threshold clearance
    for gap in &geom.gaps {
        let along = Vec2::new(-gap.normal.y, gap.normal.x);
        let to_p = p - gap.center;
        let dist_along = to_p.dot(along).abs();
        let dist_normal = to_p.dot(gap.normal);

        // Clearance box: width of gap + 0.5 margin, and extending 2.5 units into the room, 1.0 units out
        if dist_along < (gap.width * 0.5 + 0.5) && dist_normal > -2.5 && dist_normal < 1.0 {
            return false;
        }
    }

    // 4. Distance to other placed props
    for other in placed {
        if p.distance(*other) < 1.5 {
            return false;
        }
    }

    true
}

#[allow(clippy::too_many_arguments)]
fn spawn_room_dressing(
    commands: &mut Commands,
    assets: &MatchAssets,
    images: &Assets<Image>,
    room: RoomId,
    role: RoomRole,
    geom: &teleport::PlaceGeom,
    y_offset: f32,
    seed_val: u64,
) {
    // 1. Initialize deterministic RNG
    let mut rng = SplitMix(seed_val ^ (room.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));

    // 2. Select props by room role
    let props = match role {
        RoomRole::Monitor => vec![
            assets.decor_lab_table.clone(),
            assets.decor_column.clone(),
            assets.decor_lab_crate.clone(),
        ],
        RoomRole::GuardianControl => vec![
            assets.decor_lab_table.clone(),
            assets.decor_column.clone(),
            assets.decor_torch.clone(),
        ],
        RoomRole::Keystone => vec![assets.decor_lab_crate.clone(), assets.decor_column.clone()],
        RoomRole::Start | RoomRole::Exit => {
            vec![assets.decor_column.clone(), assets.decor_torch.clone()]
        }
        _ => vec![assets.decor_column.clone(), assets.decor_lab_crate.clone()],
    };

    // Filter out None handles
    let props: Vec<Handle<Image>> = props.into_iter().flatten().collect();
    if props.is_empty() {
        return;
    }

    // 3. Determine number of props to place (e.g. 2 to 4)
    let count = 2 + (rng.next_u64() % 3) as usize; // 2, 3, or 4 props
    let mut placed = Vec::new();

    // 4. Sample and place props
    for _ in 0..count {
        let mut best_pos = None;
        // Try up to 50 times to find a safe position
        for _ in 0..50 {
            // Sample a point within the half-extents
            let rx = ((rng.next_u64() % 2000) as f32 / 1000.0 - 1.0) * geom.half.x;
            let ry = ((rng.next_u64() % 2000) as f32 / 1000.0 - 1.0) * geom.half.y;
            let p = Vec2::new(rx, ry);

            if is_safe_placement(p, geom, &placed, true) {
                best_pos = Some(p);
                break;
            }
        }

        if let Some(pos) = best_pos {
            placed.push(pos);
            // Select prop handle deterministically
            let prop_idx = (rng.next_u64() as usize) % props.len();
            let image = props[prop_idx].clone();

            if images.contains(&image) {
                commands.spawn((
                    PlaceGeometry,
                    DespawnOnExit(GameState::Match),
                    Sprite { image, ..default() },
                    Sprite3d {
                        pixels_per_metre: 64.0,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        // Dim emission so it is dimmer than interactables and doesn't steal signal colors
                        emissive: LinearRgba::rgb(0.015, 0.015, 0.015),
                        pivot: Some(Vec2::new(0.5, 0.0)),
                        double_sided: true,
                        ..default()
                    },
                    crate::view::components::BillboardSprite,
                    Transform::from_xyz(pos.x, y_offset + 0.02, pos.y),
                    Name::new("Room dressing prop"),
                ));
            }
        }
    }
}

fn spawn_hallway_dressing(
    commands: &mut Commands,
    assets: &MatchAssets,
    images: &Assets<Image>,
    geom: &teleport::PlaceGeom,
    y_offset: f32,
    seed_val: u64,
) {
    let mut rng = SplitMix(seed_val ^ 0x1234_5678_ABCD_EF01);
    let props = vec![assets.decor_column.clone(), assets.decor_lab_crate.clone()];
    let props: Vec<Handle<Image>> = props.into_iter().flatten().collect();
    if props.is_empty() {
        return;
    }

    // Spawn at most 1 prop in hallways to keep it minimal and clear
    let placed = Vec::new();
    let mut best_pos = None;
    for _ in 0..50 {
        let rx = ((rng.next_u64() % 2000) as f32 / 1000.0 - 1.0) * geom.half.x;
        let ry = ((rng.next_u64() % 2000) as f32 / 1000.0 - 1.0) * geom.half.y;
        let p = Vec2::new(rx, ry);

        if is_safe_placement(p, geom, &placed, false) {
            best_pos = Some(p);
            break;
        }
    }

    if let Some(pos) = best_pos {
        let prop_idx = (rng.next_u64() as usize) % props.len();
        let image = props[prop_idx].clone();

        if images.contains(&image) {
            commands.spawn((
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                Sprite { image, ..default() },
                Sprite3d {
                    pixels_per_metre: 64.0,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    emissive: LinearRgba::rgb(0.015, 0.015, 0.015),
                    pivot: Some(Vec2::new(0.5, 0.0)),
                    double_sided: true,
                    ..default()
                },
                crate::view::components::BillboardSprite,
                Transform::from_xyz(pos.x, y_offset + 0.02, pos.y),
                Name::new("Hallway dressing prop"),
            ));
        }
    }
}
