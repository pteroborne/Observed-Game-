//! The place rebuild entry point: `rebuild_place` tears down and respawns a place's
//! entire presentation (shell, thresholds, monitors, items, lighting, guardian) whenever
//! the teleport signature changes, plus the room-tinted floor material/light colour
//! helpers it uses to give each room a distinct hue.

use bevy::prelude::*;
use observed_core::{RoomId, SplitMix};
use observed_facility::map_spec::RoomRole;

use crate::GameState;
use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::layout::WALL_HEIGHT;
use crate::screens::match_runtime;
use crate::sim::director::MatchDirector;
use crate::sim::state::{RivalSightings, TeleportState};
use crate::teleport::{self, GapKind, Place};
use crate::view::assets::MatchAssets;
use crate::view::components::PlaceGeometry;

use super::monitors::{
    GuardianConsole, monitor_page_for, spawn_guardian_observation_monitors,
    spawn_tether_camera_monitors,
};
use super::{item_visuals, lighting, preview, shell};

pub(crate) fn room_color(room_id: RoomId) -> Color {
    let r = (((room_id.0 * 17 + 5) % 255) as f32) / 255.0;
    let g = (((room_id.0 * 31 + 13) % 255) as f32) / 255.0;
    let b = (((room_id.0 * 59 + 29) % 255) as f32) / 255.0;
    Color::srgb(0.3 + r * 0.7, 0.3 + g * 0.7, 0.3 + b * 0.7)
}

pub(crate) fn room_floor_material(
    room_id: RoomId,
    base_handle: &Handle<StandardMaterial>,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let mut mat = (*materials.get(base_handle).unwrap()).clone();
    let col = room_color(room_id);
    if mat.base_color_texture.is_some() {
        mat.base_color = Color::WHITE;
        mat.emissive = LinearRgba::from(col) * 0.45;
    } else {
        mat.base_color = col;
        mat.emissive = LinearRgba::from(col) * 3.0;
    }
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
    images: Res<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    guardian: Res<crate::guardian::Guardian>,
    runtime: Res<MatchDirector>,
    mut sightings: ResMut<RivalSightings>,
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
            if nav.is_tethered(game.local_room(), conn) {
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
    let mut geom = teleport::geom_for(tp.place, &nav);
    if matches!(tp.place, Place::Room(_)) {
        teleport::open_entry(&mut geom, tp.arrived_from);
    }
    tp.arena = teleport::place_arena(&geom, y_offset, WALL_HEIGHT);
    if geom.poly.is_some() {
        let clamped = teleport::contain(
            &geom,
            Vec2::new(tp.body.position.x, tp.body.position.z),
            tp.config.radius,
        );
        tp.body.position.x = clamped.x;
        tp.body.position.z = clamped.y;
    }
    tp.gap_dests = match_runtime::compute_gap_dests(seed_val, tp.place, &geom, game, &keys, &items);
    tp.geom = geom.clone();

    // The place's structural shell (floor/ceiling/walls), by place shape.
    match tp.place {
        Place::Room(room) => {
            let floor_material = room_floor_material(room, &assets.floor_material, &mut materials);
            shell::spawn_room_shell(
                &mut commands,
                &assets,
                &mut meshes,
                &geom,
                floor_material,
                y_offset,
            );
        }
        Place::Hallway { .. } => shell::spawn_hallway_shell(
            &mut commands,
            &assets,
            &geom,
            assets.floor_material.clone(),
            &tp.arena.solids,
            y_offset,
        ),
    }

    // The threshold gateways cut into that shell.
    for gap in &geom.gaps {
        if gap.kind == teleport::GapKind::OneWayEntry {
            continue;
        }
        let (ea, eb) = match tp.place {
            Place::Room(room) => (room, gap.target),
            Place::Hallway { from, to, .. } => (from, to),
        };
        let tethered = nav.is_tethered(ea, eb);
        // Phase 38 contested observation: a rival team standing in the room beyond
        // this threshold pins it for everyone; Phase 42 also attributes a rival's
        // *anchor* there (`rival_signals` is the one reconciliation point between
        // that heuristic and `CompetitiveFacility::pin_sources`' anchor bookkeeping).
        let rival_signal = rival_signals
            .iter()
            .find(|signal| signal.neighbor == gap.target)
            .copied();
        let rival_presence = rival_signal.and_then(|s| s.presence).map(|t| t.0 as usize);
        let rival_anchor = rival_signal.and_then(|s| s.anchor).map(|t| t.0 as usize);

        if gap.kind.is_passage() {
            let dest = tp
                .gap_dests
                .iter()
                .find(|d| d.threshold == gap.threshold)
                .or_else(|| {
                    tp.gap_dests
                        .iter()
                        .find(|d| (d.gap_center - gap.center).length() < 0.05)
                })
                .cloned()
                .unwrap_or_else(|| preview::fallback_dest(tp.place, gap, &nav, game));

            // A maze hallway can expose multiple apertures to the same room-side
            // threshold. Keep one full preview for the canonical aperture and use short
            // stubs for secondary apertures instead of drawing overlapping room copies.
            let multi_aperture_room_preview = matches!(tp.place, Place::Hallway { .. })
                && matches!(dest.place, Place::Room(_))
                && gap.threshold.hall.slot.0 != 0
                && geom
                    .gaps
                    .iter()
                    .filter(|other| {
                        other.kind.is_passage()
                            && other.target == gap.target
                            && other.normal.dot(gap.normal) > 0.99
                    })
                    .count()
                    > 1;
            if multi_aperture_room_preview {
                preview::spawn_passage_stub(&mut commands, &assets, gap, y_offset);
            } else {
                preview::spawn_passage_preview(
                    &mut commands,
                    &assets,
                    &mut meshes,
                    &mut materials,
                    gap,
                    tp.place,
                    &dest,
                    &nav,
                    game,
                    match_runtime::countdown_klaxon_active(&runtime),
                );
            }

            shell::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                shell::ThresholdStyle::passage(tethered, rival_presence, rival_anchor),
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
            // The Monitor room's panel page splits across two co-located banks (the
            // tether-camera wall + the guardian-observation wall) on disjoint wall
            // mounts, so both legacy camera "systems" keep working under a map with a
            // single semantic Monitor room instead of the old two-room split.
            let klaxon_active = match_runtime::countdown_klaxon_active(&runtime);
            let split = page.len().div_ceil(2);
            let (tether_page, guardian_page) = page.split_at(split);
            spawn_tether_camera_monitors(
                &mut commands,
                &assets,
                &mut meshes,
                &mut materials,
                &geom,
                y_offset,
                seed_val,
                room,
                0,
                tether_page,
                game,
                klaxon_active,
                &mut sightings,
            );
            spawn_guardian_observation_monitors(
                &mut commands,
                &assets,
                &mut meshes,
                &mut materials,
                &geom,
                y_offset,
                seed_val,
                room,
                tether_page.len(),
                guardian_page,
                game,
                klaxon_active,
                &mut sightings,
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
        item_visuals::spawn_keystone_item(&mut commands, &assets, room, y_offset);
    }

    // Spawn dropped items
    for item in items.placed_in(tp.place) {
        item_visuals::spawn_dropped_item(&mut commands, &assets, &images, item, y_offset);
    }

    // Lighting and surface details
    let palette = match_runtime::palette_for_match(seed_val, tp.place, &runtime);
    let light_color = palette.light_color;

    let place_transform = Transform::from_xyz(0.0, y_offset, 0.0);
    lighting::spawn_place_lighting(
        &mut commands,
        &assets,
        &geom,
        light_color,
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
        && room == guardian.room
    {
        spawn_guardian_model(&mut commands, &assets, &images, guardian.pos);
    }
}

pub(crate) fn spawn_guardian_model(
    commands: &mut Commands,
    assets: &MatchAssets,
    images: &Assets<Image>,
    pos: Vec3,
) {
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
