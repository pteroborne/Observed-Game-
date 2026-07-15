//! Multi-level team-local survivor sketch.

use bevy::prelude::*;
use observed_match::full_wfc::{MapDiscovery, TeamMapKnowledge};
use observed_style::{MarkerRole, ObservedState, SurfaceRole};

use super::sim::FullWfcRuntime;
use crate::GameState;

const CELL_PX: f32 = 16.0;
const CELL_GAP: f32 = 3.0;
const LEVEL_GAP: f32 = 28.0;

#[derive(Component)]
pub(super) struct FullWfcTacMap;

#[derive(Resource, Default)]
pub(super) struct TacMapProjection {
    signature: u64,
}

pub(super) fn setup(mut commands: Commands) {
    commands.insert_resource(TacMapProjection::default());
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<TacMapProjection>();
}

pub(super) fn sync(
    mut commands: Commands,
    runtime: Res<FullWfcRuntime>,
    mut projection: ResMut<TacMapProjection>,
    existing: Query<Entity, With<FullWfcTacMap>>,
) {
    let player = runtime.local();
    let Some(knowledge) = runtime.match_state.team_map(player.team) else {
        return;
    };
    let signature = signature(&runtime, knowledge);
    if projection.signature == signature {
        return;
    }
    projection.signature = signature;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if !runtime.map_open {
        return;
    }
    spawn_map(&mut commands, &runtime, knowledge);
}

fn spawn_map(commands: &mut Commands, runtime: &FullWfcRuntime, knowledge: &TeamMapKnowledge) {
    let world = &runtime.match_state.facility;
    let level_width = f32::from(world.config.cols) * (CELL_PX + CELL_GAP);
    let map_width = f32::from(world.config.levels) * (level_width + LEVEL_GAP) + 32.0;
    let map_height = f32::from(world.config.rows) * (CELL_PX + CELL_GAP) + 92.0;
    let traversed = observed_style::marker(MarkerRole::You).base_color;
    let glimpsed = observed_style::observed_modulate(
        observed_style::surface(SurfaceRole::Spine),
        ObservedState::Unobserved,
    )
    .base_color;
    let stale = observed_style::surface(SurfaceRole::Wall)
        .edge
        .unwrap_or(Color::WHITE);
    let anchor = observed_style::marker(MarkerRole::Control).base_color;
    commands
        .spawn((
            FullWfcTacMap,
            DespawnOnExit(GameState::FullWfc),
            Node {
                position_type: PositionType::Absolute,
                right: px(18),
                bottom: px(18),
                width: px(map_width),
                height: px(map_height),
                border: UiRect::all(px(1)),
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.004, 0.01, 0.024, 0.94)),
            BorderColor::all(glimpsed),
            GlobalZIndex(45),
            Name::new("Team survivor tac-map"),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("SURVIVOR SKETCH  [TAB CLOSE]"),
                TextFont {
                    font_size: 15.0,
                    ..Default::default()
                },
                TextColor(traversed),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(14),
                    top: px(10),
                    ..Default::default()
                },
            ));
            root.spawn((
                Text::new(
                    "[+] traversed   [ ] glimpsed   X stale   A anchored   @ you   T teammate",
                ),
                TextFont {
                    font_size: 12.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.76, 0.84, 0.94)),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(14),
                    bottom: px(10),
                    ..Default::default()
                },
            ));
            for level in 0..world.config.levels {
                let level_left = 16.0 + f32::from(level) * (level_width + LEVEL_GAP);
                root.spawn((
                    Text::new(format!("L{level}")),
                    TextFont {
                        font_size: 12.0,
                        ..Default::default()
                    },
                    TextColor(Color::srgb(0.76, 0.84, 0.94)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(level_left),
                        top: px(35),
                        ..Default::default()
                    },
                ));
            }
            for (cell, known) in &knowledge.cells {
                let left = 16.0
                    + f32::from(cell.level) * (level_width + LEVEL_GAP)
                    + f32::from(cell.x) * (CELL_PX + CELL_GAP);
                let top = 54.0 + f32::from(cell.z) * (CELL_PX + CELL_GAP);
                let is_stale = known.is_stale(world.generation);
                let border_color = if known.anchored {
                    anchor
                } else if is_stale {
                    stale
                } else {
                    glimpsed
                };
                let fill = if known.discovery == MapDiscovery::Traversed && !is_stale {
                    traversed.with_alpha(0.58)
                } else {
                    Color::srgba(0.006, 0.018, 0.038, 0.82)
                };
                let glyph = if *cell == runtime.local().cell {
                    "@"
                } else if runtime.match_state.players.values().any(|other| {
                    other.team == runtime.local().team
                        && other.id != runtime.local_player
                        && other.cell == *cell
                }) {
                    "T"
                } else if known.anchored {
                    "A"
                } else if is_stale {
                    "X"
                } else {
                    ""
                };
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(left),
                        top: px(top),
                        width: px(CELL_PX),
                        height: px(CELL_PX),
                        border: UiRect::all(px(if known.anchored { 2.0 } else { 1.0 })),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..Default::default()
                    },
                    BackgroundColor(fill),
                    BorderColor::all(border_color),
                ))
                .with_child((
                    Text::new(glyph),
                    TextFont {
                        font_size: 12.0,
                        ..Default::default()
                    },
                    TextColor(if known.anchored { anchor } else { traversed }),
                ));
            }
        });
}

fn signature(runtime: &FullWfcRuntime, knowledge: &TeamMapKnowledge) -> u64 {
    let mut signature = u64::from(runtime.map_open);
    signature ^= u64::from(runtime.match_state.facility.generation) << 1;
    signature ^= u64::from(runtime.local().cell.x)
        | (u64::from(runtime.local().cell.z) << 16)
        | (u64::from(runtime.local().cell.level) << 32);
    for (cell, known) in &knowledge.cells {
        let mut value = u64::from(cell.x)
            | (u64::from(cell.z) << 12)
            | (u64::from(cell.level) << 24)
            | (u64::from(known.last_confirmed_generation) << 32);
        value ^= match known.discovery {
            MapDiscovery::Glimpsed => 0x51,
            MapDiscovery::Traversed => 0xA2,
        };
        value ^= u64::from(known.anchored) << 63;
        signature = signature.rotate_left(7) ^ value;
    }
    signature
}
