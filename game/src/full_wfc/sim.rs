use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use observed_facility::full_wfc::{
    CellCoord, FullWfcConfig, FullWfcWorld, ModuleFace, ModuleSpace, ObservationFrame, ThresholdKey,
};
use player_input::{PlayerId, PlayerIntent};

use crate::flow::ActiveMatchSeed;

pub(super) const CELL_SIZE: f32 = 12.0;
pub(super) const LEVEL_HEIGHT: f32 = 5.0;
pub(super) const EYE_HEIGHT: f32 = 1.72;
const WALK_SPEED: f32 = 4.8;
const SPRINT_SPEED: f32 = 7.2;
const CLIMB_SPEED: f32 = 2.5;
const LOCAL_PLAYER: PlayerId = PlayerId(0);

#[derive(Resource, Default)]
pub(super) struct FullWfcIntent(pub PlayerIntent);

#[derive(Clone, Copy, Debug)]
pub(super) struct FullWfcPlayer {
    pub id: PlayerId,
    pub cell: CellCoord,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub climb_target: Option<CellCoord>,
}

#[derive(Resource)]
pub(super) struct FullWfcRuntime {
    pub world: FullWfcWorld,
    pub player: FullWfcPlayer,
    pub ticks_until_pulse: u32,
    pub pending_visual_changes: BTreeSet<CellCoord>,
    pub status: String,
    pub escaped: bool,
}

pub(super) fn setup_runtime(mut commands: Commands, seed: Option<Res<ActiveMatchSeed>>) {
    let requested_seed = seed.as_deref().map_or(0xF011_FAC1_1177, |seed| seed.0);
    let config = FullWfcConfig::default();
    let (world, seed_offset) = (0..64u64)
        .find_map(|offset| {
            FullWfcWorld::new(requested_seed.wrapping_add(offset), config)
                .ok()
                .map(|world| (world, offset))
        })
        .expect("the full-WFC default corpus must contain a solvable nearby seed");
    let spawn = world.spawn();
    let pulse_ticks = world.config.pulse_ticks;
    commands.insert_resource(FullWfcRuntime {
        player: FullWfcPlayer {
            id: LOCAL_PLAYER,
            cell: spawn,
            position: world_position(spawn),
            yaw: initial_yaw(&world, spawn),
            pitch: 0.0,
            climb_target: None,
        },
        pending_visual_changes: world.placements.keys().copied().collect(),
        status: if seed_offset == 0 {
            "initial WFC solve".to_string()
        } else {
            format!("initial seed advanced by {seed_offset} after contradictions")
        },
        world,
        ticks_until_pulse: pulse_ticks,
        escaped: false,
    });
    commands.insert_resource(FullWfcIntent::default());
}

fn initial_yaw(world: &FullWfcWorld, cell: CellCoord) -> f32 {
    [
        (ModuleFace::East, -std::f32::consts::FRAC_PI_2),
        (ModuleFace::West, std::f32::consts::FRAC_PI_2),
        (ModuleFace::South, std::f32::consts::PI),
        (ModuleFace::North, 0.0),
    ]
    .into_iter()
    .find_map(|(face, yaw)| world.placements[&cell].is_open(face).then_some(yaw))
    .unwrap_or(0.0)
}

pub(super) fn cleanup_runtime(mut commands: Commands) {
    commands.remove_resource::<FullWfcRuntime>();
    commands.remove_resource::<FullWfcIntent>();
}

pub(super) fn step_runtime(
    time: Res<Time<Fixed>>,
    mut intent: ResMut<FullWfcIntent>,
    mut runtime: ResMut<FullWfcRuntime>,
) {
    if runtime.escaped {
        intent.0.look = Vec2::ZERO;
        return;
    }
    runtime.player.yaw -= intent.0.look.x;
    runtime.player.pitch = (runtime.player.pitch + intent.0.look.y).clamp(-1.25, 1.25);
    intent.0.look = Vec2::ZERO;

    move_player(&mut runtime, intent.0, time.delta_secs());
    let observation = observation_frame(&runtime);
    runtime.world.update_observation(observation.clone());

    runtime.ticks_until_pulse = runtime.ticks_until_pulse.saturating_sub(1);
    if runtime.ticks_until_pulse == 0 {
        pulse(&mut runtime, observation);
    }
    runtime.escaped = runtime.player.cell == runtime.world.exit();
    if runtime.escaped {
        runtime.status = "EXIT REACHED - experiment complete".to_string();
    }
}

fn pulse(runtime: &mut FullWfcRuntime, observation: ObservationFrame) {
    match runtime.world.propose_relayout(&observation) {
        Ok(candidate) => {
            let changed = candidate.changed_cells.clone();
            match runtime.world.commit_relayout(candidate, observation) {
                Ok(()) => {
                    runtime.pending_visual_changes.extend(changed);
                    runtime.status = format!(
                        "decoherence pulse {} accepted in {} attempt(s)",
                        runtime.world.accepted_pulses, runtime.world.last_attempts
                    );
                }
                Err(error) => runtime.status = format!("pulse held: {error:?}"),
            }
        }
        Err(error) => runtime.status = format!("WFC contradiction: {error:?}"),
    }
    runtime.ticks_until_pulse = runtime.world.config.pulse_ticks;
}

fn move_player(runtime: &mut FullWfcRuntime, intent: PlayerIntent, dt: f32) {
    if let Some(target) = runtime.player.climb_target {
        let target_y = world_position(target).y;
        let delta = target_y - runtime.player.position.y;
        let step = CLIMB_SPEED * dt;
        if delta.abs() <= step {
            runtime.player.position.y = target_y;
            runtime.player.cell = target;
            runtime.player.climb_target = None;
        } else {
            runtime.player.position.y += delta.signum() * step;
        }
        return;
    }

    let climb_face = if intent.jump_pressed {
        Some(ModuleFace::Up)
    } else if intent.interact_held {
        Some(ModuleFace::Down)
    } else {
        None
    };
    if let Some(face) = climb_face {
        let center = world_position(runtime.player.cell);
        let near_shaft = Vec2::new(
            runtime.player.position.x - center.x,
            runtime.player.position.z - center.z,
        )
        .length_squared()
            <= 2.2 * 2.2;
        let open = runtime
            .world
            .placement(runtime.player.cell)
            .is_some_and(|placement| placement.is_open(face));
        if near_shaft && open {
            runtime.player.climb_target = runtime.world.config.neighbor(runtime.player.cell, face);
            return;
        }
    }

    let speed = if intent.sprint_held {
        SPRINT_SPEED
    } else {
        WALK_SPEED
    };
    let forward = Vec3::new(-runtime.player.yaw.sin(), 0.0, -runtime.player.yaw.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let delta = (right * intent.movement.x + forward * intent.movement.y) * speed * dt;
    try_axis(runtime, delta.x, true);
    try_axis(runtime, delta.z, false);
}

fn try_axis(runtime: &mut FullWfcRuntime, delta: f32, x_axis: bool) {
    if delta == 0.0 {
        return;
    }
    let mut proposed = runtime.player.position;
    if x_axis {
        proposed.x += delta;
    } else {
        proposed.z += delta;
    }
    let Some(next) = horizontal_cell(runtime.world.config, proposed, runtime.player.cell.level)
    else {
        return;
    };
    if next == runtime.player.cell {
        runtime.player.position = proposed;
        return;
    }
    let Some(face) = face_between(runtime.player.cell, next) else {
        return;
    };
    let open = runtime
        .world
        .placement(runtime.player.cell)
        .is_some_and(|placement| placement.is_open(face));
    let active = runtime
        .world
        .placement(next)
        .is_some_and(|placement| placement.space != ModuleSpace::Void);
    let exit_face_reserved = (next == runtime.world.exit()
        && runtime.world.reserved_exit_faces.contains(&face.opposite()))
        || (runtime.player.cell == runtime.world.exit()
            && runtime.world.reserved_exit_faces.contains(&face));
    if open && active && !exit_face_reserved {
        runtime.player.position = proposed;
        runtime.player.cell = next;
    }
}

fn horizontal_cell(config: FullWfcConfig, position: Vec3, level: u8) -> Option<CellCoord> {
    let x = ((position.x + CELL_SIZE * 0.5) / CELL_SIZE).floor() as i32;
    let z = ((position.z + CELL_SIZE * 0.5) / CELL_SIZE).floor() as i32;
    (x >= 0 && z >= 0 && x < i32::from(config.cols) && z < i32::from(config.rows))
        .then(|| CellCoord::new(x as u16, z as u16, level))
}

fn face_between(a: CellCoord, b: CellCoord) -> Option<ModuleFace> {
    match (
        i32::from(b.x) - i32::from(a.x),
        i32::from(b.z) - i32::from(a.z),
    ) {
        (1, 0) => Some(ModuleFace::East),
        (-1, 0) => Some(ModuleFace::West),
        (0, 1) => Some(ModuleFace::South),
        (0, -1) => Some(ModuleFace::North),
        _ => None,
    }
}

pub(super) fn observation_frame(runtime: &FullWfcRuntime) -> ObservationFrame {
    let mut visible_cells = BTreeSet::from([runtime.player.cell]);
    if let Some(target) = runtime.player.climb_target {
        visible_cells.insert(target);
    }
    let mut visible_thresholds = BTreeSet::new();
    if runtime
        .world
        .placement(runtime.player.cell)
        .is_some_and(|placement| placement.space == ModuleSpace::Room)
    {
        visible_thresholds.insert(ThresholdKey {
            room: runtime.player.cell,
            face: look_face(runtime.player.yaw, runtime.player.pitch),
        });
    }
    ObservationFrame {
        visible_cells,
        visible_thresholds,
        occupied_cells: BTreeMap::from([(runtime.player.id, runtime.player.cell)]),
    }
}

fn look_face(yaw: f32, pitch: f32) -> ModuleFace {
    if pitch > 0.72 {
        return ModuleFace::Up;
    }
    if pitch < -0.72 {
        return ModuleFace::Down;
    }
    let direction = Vec2::new(-yaw.sin(), -yaw.cos());
    if direction.x.abs() > direction.y.abs() {
        if direction.x > 0.0 {
            ModuleFace::East
        } else {
            ModuleFace::West
        }
    } else if direction.y > 0.0 {
        ModuleFace::South
    } else {
        ModuleFace::North
    }
}

pub(super) fn world_position(coord: CellCoord) -> Vec3 {
    Vec3::new(
        f32::from(coord.x) * CELL_SIZE,
        f32::from(coord.level) * LEVEL_HEIGHT + EYE_HEIGHT,
        f32::from(coord.z) * CELL_SIZE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(seed: u64) -> FullWfcRuntime {
        let world = FullWfcWorld::new(seed, FullWfcConfig::default()).expect("default solve");
        let spawn = world.spawn();
        FullWfcRuntime {
            player: FullWfcPlayer {
                id: LOCAL_PLAYER,
                cell: spawn,
                position: world_position(spawn),
                yaw: -std::f32::consts::FRAC_PI_2,
                pitch: 0.0,
                climb_target: None,
            },
            ticks_until_pulse: world.config.pulse_ticks,
            pending_visual_changes: BTreeSet::new(),
            status: String::new(),
            escaped: false,
            world,
        }
    }

    #[test]
    fn simulation_reports_player_id_and_only_the_faced_threshold() {
        let runtime = runtime(11);
        let frame = observation_frame(&runtime);
        assert_eq!(frame.occupied_cells[&LOCAL_PLAYER], runtime.player.cell);
        assert_eq!(frame.visible_thresholds.len(), 1);
        assert_eq!(
            frame.visible_thresholds.iter().next().unwrap().face,
            ModuleFace::East
        );
    }

    #[test]
    fn closed_cell_boundary_blocks_continuous_motion() {
        let mut runtime = runtime(12);
        let closed = ModuleFace::ALL
            .into_iter()
            .filter(|face| !matches!(face, ModuleFace::Up | ModuleFace::Down))
            .find(|&face| !runtime.world.placements[&runtime.player.cell].is_open(face))
            .expect("spawn room has a closed horizontal face");
        runtime.player.position = match closed {
            ModuleFace::East => Vec3::new(CELL_SIZE * 0.49, EYE_HEIGHT, 0.0),
            ModuleFace::West => Vec3::new(-CELL_SIZE * 0.49, EYE_HEIGHT, 0.0),
            ModuleFace::South => Vec3::new(0.0, EYE_HEIGHT, CELL_SIZE * 0.49),
            ModuleFace::North => Vec3::new(0.0, EYE_HEIGHT, -CELL_SIZE * 0.49),
            _ => unreachable!(),
        };
        let before = runtime.player.position;
        try_axis(
            &mut runtime,
            if matches!(closed, ModuleFace::East | ModuleFace::South) {
                1.0
            } else {
                -1.0
            },
            matches!(closed, ModuleFace::East | ModuleFace::West),
        );
        assert_eq!(runtime.player.position, before);
    }
}
