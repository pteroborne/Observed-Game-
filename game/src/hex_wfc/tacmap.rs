//! Multi-level hex survivor sketch: the cells the local runner has seen, drawn as hex
//! outline tiles per level in the sketch style. Purely presentation — it reads the
//! runtime's presentation-derived `discovered` set and never drives simulation.

use bevy::prelude::*;
use observed_style::{MarkerRole, ObservedState, SurfaceRole};

use super::sim::HexWfcRuntime;
use crate::GameState;

// Pointy-top hexes use the same axial projection as the world, scaled down to the
// survivor sketch: x = sqrt(3) * side * (q + r/2), y = 1.5 * side * r. Keeping the
// outline as six UI bars (rather than a rotated square) makes the map's topology
// truthful even at this deliberately tiny scale.
const HEX_SIDE: f32 = 4.2;
const HEX_WIDTH: f32 = 7.274_613;
const HEX_HEIGHT: f32 = HEX_SIDE * 2.0;
const Q_PITCH: f32 = HEX_WIDTH;
const R_X_PITCH: f32 = HEX_WIDTH * 0.5;
const R_Y_PITCH: f32 = HEX_SIDE * 1.5;
const LEVEL_COLUMNS: u8 = 4;
const LEVEL_GAP_X: f32 = 18.0;
const LEVEL_GAP_Y: f32 = 26.0;

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
    if !runtime.map_open {
        return;
    }
    spawn_map(&mut commands, &runtime);
}

fn spawn_map(commands: &mut Commands, runtime: &HexWfcRuntime) {
    let world = &runtime.match_state.facility;
    let level_width = f32::from(world.config.cols.saturating_sub(1)) * Q_PITCH
        + f32::from(world.config.rows.saturating_sub(1)) * R_X_PITCH
        + HEX_WIDTH;
    let level_height = f32::from(world.config.rows.saturating_sub(1)) * R_Y_PITCH + HEX_HEIGHT;
    let level_columns = world.config.levels.min(LEVEL_COLUMNS);
    let level_rows = world.config.levels.div_ceil(LEVEL_COLUMNS);
    let map_width = f32::from(level_columns) * level_width
        + f32::from(level_columns.saturating_sub(1)) * LEVEL_GAP_X
        + 32.0;
    let map_height = f32::from(level_rows) * level_height
        + f32::from(level_rows.saturating_sub(1)) * LEVEL_GAP_Y
        + 116.0;
    let you = observed_style::marker(MarkerRole::You).base_color;
    let exit_color = observed_style::marker(MarkerRole::Exit).base_color;
    let runner = observed_style::marker(MarkerRole::Rival).base_color;
    let glimpsed = observed_style::observed_modulate(
        observed_style::surface(SurfaceRole::Spine),
        ObservedState::Unobserved,
    )
    .base_color;
    let exit = world.config.exit();
    let local_cell = runtime.local().cell;
    commands
        .spawn((
            HexWfcTacMap,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                right: px(18),
                bottom: px(18),
                width: px(map_width.min(1180.0)),
                height: px(map_height.min(760.0)),
                border: UiRect::all(px(1)),
                overflow: Overflow::clip(),
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.004, 0.01, 0.024, 0.94)),
            BorderColor::all(glimpsed),
            GlobalZIndex(45),
            Name::new("Hex survivor sketch"),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("SURVIVOR SKETCH  [TAB CLOSE]"),
                TextFont {
                    font_size: 15.0,
                    ..Default::default()
                },
                TextColor(you),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(14),
                    top: px(10),
                    ..Default::default()
                },
            ));
            root.spawn((
                Text::new("hex outline: seen   @ you   X exit   T runner"),
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
                let column = level % LEVEL_COLUMNS;
                let row = level / LEVEL_COLUMNS;
                let level_left = 16.0 + f32::from(column) * (level_width + LEVEL_GAP_X);
                let level_top = 42.0 + f32::from(row) * (level_height + LEVEL_GAP_Y);
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
                        top: px(level_top),
                        ..Default::default()
                    },
                ));
            }
            let others: Vec<_> = runtime
                .match_state
                .players
                .values()
                .filter(|player| player.id != runtime.local_player && !player.escaped)
                .map(|player| player.cell)
                .collect();
            for &cell in &runtime.discovered {
                let column = cell.level % LEVEL_COLUMNS;
                let row = cell.level / LEVEL_COLUMNS;
                let level_left = 16.0 + f32::from(column) * (level_width + LEVEL_GAP_X);
                let level_top = 62.0 + f32::from(row) * (level_height + LEVEL_GAP_Y);
                let left = level_left + f32::from(cell.q) * Q_PITCH + f32::from(cell.r) * R_X_PITCH;
                let top = level_top + f32::from(cell.r) * R_Y_PITCH;
                let (glyph, border_color, glyph_color) = if cell == local_cell {
                    ("@", you, you)
                } else if cell == exit {
                    ("X", exit_color, exit_color)
                } else if others.contains(&cell) {
                    ("T", runner, runner)
                } else {
                    ("", glimpsed, you)
                };
                root.spawn((Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(top),
                    width: px(HEX_WIDTH),
                    height: px(HEX_HEIGHT),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..Default::default()
                },))
                    .with_children(|hex| {
                        spawn_hex_outline(hex, border_color);
                        hex.spawn((
                            Text::new(glyph),
                            TextFont {
                                font_size: 9.0,
                                ..Default::default()
                            },
                            TextColor(glyph_color),
                        ));
                    });
            }
        });
}

/// Six thin UI bars tracing a pointy-top hex. Positions are relative to the owning
/// `HEX_WIDTH × HEX_HEIGHT` tile; rotations match the canonical axial projection.
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
                ..Default::default()
            },
            BackgroundColor(color),
            UiTransform::from_rotation(Rot2::degrees(degrees)),
        ));
    }
}

fn signature(runtime: &HexWfcRuntime) -> u64 {
    let mut signature = u64::from(runtime.map_open);
    signature ^= u64::from(runtime.match_state.facility.generation) << 1;
    signature ^= (runtime.discovered.len() as u64) << 8;
    let local = runtime.local().cell;
    signature ^= u64::from(local.q) | (u64::from(local.r) << 16) | (u64::from(local.level) << 32);
    for player in runtime.match_state.players.values() {
        signature = signature.rotate_left(7)
            ^ (u64::from(player.cell.q)
                | (u64::from(player.cell.r) << 16)
                | (u64::from(player.cell.level) << 32)
                | (u64::from(player.escaped) << 48));
    }
    signature
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_ten_level_sketch_fits_the_capture_viewport() {
        let cols = 28u8;
        let rows = 20u8;
        let levels = 10u8;
        let level_width =
            f32::from(cols - 1) * Q_PITCH + f32::from(rows - 1) * R_X_PITCH + HEX_WIDTH;
        let level_height = f32::from(rows - 1) * R_Y_PITCH + HEX_HEIGHT;
        let width = f32::from(levels.min(LEVEL_COLUMNS)) * level_width
            + f32::from(levels.min(LEVEL_COLUMNS) - 1) * LEVEL_GAP_X
            + 32.0;
        let level_rows = levels.div_ceil(LEVEL_COLUMNS);
        let height =
            f32::from(level_rows) * level_height + f32::from(level_rows - 1) * LEVEL_GAP_Y + 116.0;
        assert!(width <= 1_180.0, "map width {width} exceeds its UI cap");
        assert!(height <= 760.0, "map height {height} exceeds its UI cap");
    }
}
