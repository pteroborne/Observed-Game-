//! Active-level, player-local survivor sketch for the hex facility.

use std::collections::BTreeSet;

use bevy::prelude::*;
use observed_hex::{HexCoord, HexFace, PortClass};
use observed_match::hex_wfc::HexMapDiscovery;
use observed_style::{MarkerRole, ObservedState, SurfaceRole};

use super::sim::HexWfcRuntime;
use crate::GameState;

const HEX_SIDE: f32 = 9.0;
const HEX_WIDTH: f32 = 15.588_457;
const HEX_HEIGHT: f32 = HEX_SIDE * 2.0;
const Q_PITCH: f32 = HEX_WIDTH;
const R_X_PITCH: f32 = HEX_WIDTH * 0.5;
const R_Y_PITCH: f32 = HEX_SIDE * 1.5;
const PANEL_PADDING: f32 = 24.0;

#[derive(Component)]
pub(super) struct HexWfcTacMap;

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
    runtime: Res<HexWfcRuntime>,
    mut projection: ResMut<TacMapProjection>,
    existing: Query<Entity, With<HexWfcTacMap>>,
) {
    let signature = signature(&runtime);
    if projection.signature == signature {
        return;
    }
    projection.signature = signature;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if runtime.map_open {
        spawn_map(&mut commands, &runtime);
    }
}

fn spawn_map(commands: &mut Commands, runtime: &HexWfcRuntime) {
    let world = &runtime.match_state.facility;
    let knowledge = runtime
        .match_state
        .player_map(runtime.local_player)
        .expect("local player owns survivor knowledge");
    let discovered = knowledge.cells.keys().copied().collect::<BTreeSet<_>>();
    let map_width = f32::from(world.config.cols.saturating_sub(1)) * Q_PITCH
        + f32::from(world.config.rows.saturating_sub(1)) * R_X_PITCH
        + HEX_WIDTH;
    let map_height = f32::from(world.config.rows.saturating_sub(1)) * R_Y_PITCH + HEX_HEIGHT;
    let panel_width = (map_width + PANEL_PADDING * 2.0).min(1_180.0);
    let panel_height = (map_height + 132.0).min(760.0);
    let you = observed_style::marker(MarkerRole::You).base_color;
    let exit_color = observed_style::marker(MarkerRole::Exit).base_color;
    let anchored = observed_style::marker(MarkerRole::Control).base_color;
    let glimpsed = observed_style::observed_modulate(
        observed_style::surface(SurfaceRole::Spine),
        ObservedState::Unobserved,
    )
    .base_color;
    let stale = observed_style::observed_modulate(
        observed_style::surface(SurfaceRole::Wall),
        ObservedState::Unobserved,
    )
    .base_color;
    let panel = stale.with_alpha(0.94);
    let local_cell = runtime.local().cell;
    let viewed = runtime.map_level;
    let exit = world.config.exit();
    let discovered_exit = knowledge.cells.contains_key(&exit);
    let lower = adjacent_discovered_level(&discovered, viewed, -1);
    let upper = adjacent_discovered_level(&discovered, viewed, 1);
    let level_hint = format!(
        "{}  FLOOR {viewed}  {}",
        lower.map_or("--".to_string(), |level| format!("L{level} down")),
        upper.map_or("--".to_string(), |level| format!("up L{level}")),
    );

    commands
        .spawn((
            HexWfcTacMap,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                right: px(18),
                bottom: px(18),
                width: px(panel_width),
                height: px(panel_height),
                border: UiRect::all(px(1)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(panel),
            BorderColor::all(glimpsed),
            GlobalZIndex(45),
            Name::new("Active-level hex survivor sketch"),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("SURVIVOR SKETCH  [TAB CLOSE]  [PGUP/PGDN FLOOR]"),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(you),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(14),
                    top: px(10),
                    ..default()
                },
            ));
            root.spawn((
                Text::new(level_hint),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(glimpsed),
                Node {
                    position_type: PositionType::Absolute,
                    right: px(14),
                    top: px(10),
                    ..default()
                },
            ));
            root.spawn((
                Text::new(
                    "hollow glimpsed | bright traversed | ? stale | # anchored | @ you | X exit | V vertical",
                ),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(glimpsed),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(14),
                    bottom: px(10),
                    ..default()
                },
            ));

            for (&cell, known) in knowledge
                .cells
                .iter()
                .filter(|(cell, _)| cell.level == viewed)
            {
                for face in [HexFace::East, HexFace::SouthEast, HexFace::SouthWest] {
                    if known.known_ports.port(face) == PortClass::Sealed {
                        continue;
                    }
                    let Some(neighbor) = world.config.grid().neighbor(cell, face) else {
                        continue;
                    };
                    let Some(neighbor_known) = knowledge.cells.get(&neighbor) else {
                        continue;
                    };
                    if neighbor.level != viewed
                        || neighbor_known.known_ports.port(face.opposite()) == PortClass::Sealed
                    {
                        continue;
                    }
                    let color = if known.is_stale(world, cell)
                        || neighbor_known.is_stale(world, neighbor)
                    {
                        stale
                    } else if known.discovery == HexMapDiscovery::Traversed
                        && neighbor_known.discovery == HexMapDiscovery::Traversed
                    {
                        you
                    } else {
                        glimpsed
                    };
                    spawn_connection(root, cell_center(cell), cell_center(neighbor), color);
                }
            }

            for (&cell, known) in knowledge
                .cells
                .iter()
                .filter(|(cell, _)| cell.level == viewed)
            {
                let left = PANEL_PADDING
                    + f32::from(cell.q) * Q_PITCH
                    + f32::from(cell.r) * R_X_PITCH;
                let top = 54.0 + f32::from(cell.r) * R_Y_PITCH;
                let placement = &world.placements[&cell];
                let vertical = placement.up != observed_hex::PortClass::Sealed
                    || placement.down != observed_hex::PortClass::Sealed;
                let is_stale = known.is_stale(world, cell);
                let treatment = if known.anchored {
                    anchored
                } else if is_stale {
                    stale
                } else if known.discovery == HexMapDiscovery::Traversed {
                    you
                } else {
                    glimpsed
                };
                let (glyph, color) = if cell == local_cell {
                    ("@", you)
                } else if discovered_exit && cell == exit {
                    ("X", exit_color)
                } else if known.anchored {
                    ("#", anchored)
                } else if is_stale {
                    ("?", stale)
                } else if vertical {
                    ("V", treatment)
                } else {
                    ("", treatment)
                };
                root.spawn((Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(top),
                    width: px(HEX_WIDTH),
                    height: px(HEX_HEIGHT),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },))
                    .with_children(|hex| {
                        spawn_hex_outline(hex, color);
                        hex.spawn((
                            Text::new(glyph),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(color),
                        ));
                    });
            }
        });
}

fn cell_center(cell: HexCoord) -> Vec2 {
    Vec2::new(
        PANEL_PADDING
            + f32::from(cell.q) * Q_PITCH
            + f32::from(cell.r) * R_X_PITCH
            + HEX_WIDTH * 0.5,
        54.0 + f32::from(cell.r) * R_Y_PITCH + HEX_HEIGHT * 0.5,
    )
}

fn spawn_connection(root: &mut ChildSpawnerCommands, from: Vec2, to: Vec2, color: Color) {
    let delta = to - from;
    let midpoint = (from + to) * 0.5;
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(midpoint.x - delta.length() * 0.5),
            top: px(midpoint.y - 1.0),
            width: px(delta.length()),
            height: px(2.0),
            ..default()
        },
        BackgroundColor(color),
        UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x))),
    ));
}

fn adjacent_discovered_level(
    discovered: &BTreeSet<HexCoord>,
    current: u8,
    direction: i8,
) -> Option<u8> {
    if direction > 0 {
        discovered
            .iter()
            .map(|cell| cell.level)
            .filter(|&level| level > current)
            .min()
    } else {
        discovered
            .iter()
            .map(|cell| cell.level)
            .filter(|&level| level < current)
            .max()
    }
}

fn spawn_hex_outline(hex: &mut ChildSpawnerCommands, color: Color) {
    let half_w = HEX_WIDTH * 0.5;
    let half_h = HEX_HEIGHT * 0.5;
    let segments: [(f32, f32, f32); 6] = [
        (half_w + HEX_SIDE * 0.433, half_h - HEX_SIDE * 0.75, 30.0),
        (half_w + HEX_SIDE * 0.866, half_h, 90.0),
        (half_w + HEX_SIDE * 0.433, half_h + HEX_SIDE * 0.75, -30.0),
        (half_w - HEX_SIDE * 0.433, half_h + HEX_SIDE * 0.75, 30.0),
        (half_w - HEX_SIDE * 0.866, half_h, 90.0),
        (half_w - HEX_SIDE * 0.433, half_h - HEX_SIDE * 0.75, -30.0),
    ];
    for (center_x, center_y, degrees) in segments {
        hex.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(center_x - HEX_SIDE * 0.5),
                top: px(center_y - 0.75),
                width: px(HEX_SIDE),
                height: px(1.5),
                ..default()
            },
            BackgroundColor(color),
            UiTransform::from_rotation(Rot2::degrees(degrees)),
        ));
    }
}

fn signature(runtime: &HexWfcRuntime) -> u64 {
    let mut signature = u64::from(runtime.map_open);
    signature ^= u64::from(runtime.map_level) << 1;
    signature ^= u64::from(runtime.match_state.facility.generation) << 8;
    let local = runtime.local().cell;
    signature ^= u64::from(local.q) | (u64::from(local.r) << 16) | (u64::from(local.level) << 32);
    if let Some(knowledge) = runtime.match_state.player_map(runtime.local_player) {
        signature ^= (knowledge.cells.len() as u64) << 24;
        for (&cell, known) in &knowledge.cells {
            signature = signature.rotate_left(7)
                ^ u64::from(cell.q)
                ^ (u64::from(cell.r) << 16)
                ^ (u64::from(cell.level) << 32)
                ^ (u64::from(known.last_confirmed_revision) << 40)
                ^ (u64::from(known.known_ports.0) << 47)
                ^ (u64::from(known.discovery == HexMapDiscovery::Traversed) << 62)
                ^ (u64::from(known.anchored) << 63);
        }
    }
    signature
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_level_panel_fits_the_capture_viewport() {
        let cols = 28u8;
        let rows = 20u8;
        let width = f32::from(cols - 1) * Q_PITCH
            + f32::from(rows - 1) * R_X_PITCH
            + HEX_WIDTH
            + PANEL_PADDING * 2.0;
        let height = f32::from(rows - 1) * R_Y_PITCH + HEX_HEIGHT + 132.0;
        assert!(width <= 1_180.0, "map width {width} exceeds UI cap");
        assert!(height <= 760.0, "map height {height} exceeds UI cap");
    }

    #[test]
    fn adjacent_hints_skip_undiscovered_floors() {
        let cells = BTreeSet::from([
            HexCoord {
                q: 0,
                r: 0,
                level: 0,
            },
            HexCoord {
                q: 0,
                r: 0,
                level: 4,
            },
            HexCoord {
                q: 0,
                r: 0,
                level: 9,
            },
        ]);
        assert_eq!(adjacent_discovered_level(&cells, 4, -1), Some(0));
        assert_eq!(adjacent_discovered_level(&cells, 4, 1), Some(9));
        assert_eq!(adjacent_discovered_level(&cells, 9, 1), None);
    }
}
