use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use ghx_proc_gen::{
    generator::{
        RngMode,
        builder::GeneratorBuilder,
        model::{ModelCollection, ModelInstance, ModelRotation},
        rules::RulesBuilder,
        socket::{SocketCollection, SocketsCartesian2D},
    },
    ghx_grid::{
        cartesian::{coordinates::Cartesian2D, grid::CartesianGrid},
        grid::{Grid, GridData},
    },
};
use wfc_proc_gen_lab::map_topology::{
    MapTopologyState, handle_map_topology_input, render_map_topology, setup_map_topology_ui,
};

// TILE_SIZE for sprite rendering
const VIEW_TILE_SIZE: f32 = 45.0;
const GRID_SIZE: u32 = 10;

#[derive(Component)]
struct GridTile;

#[derive(Component)]
struct SeedText;

#[derive(Resource)]
struct MapState {
    grid: Option<GridData<Cartesian2D, ModelInstance, CartesianGrid<Cartesian2D>>>,
    seed: u64,
    solve_status: String,
    tries: u32,
    void_idx: usize,
    corridor_idx: usize,
    entrance_idx: usize,
    exit_idx: usize,
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.015, 0.02, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — WFC Procedural Generation Lab".to_string(),
                resolution: WindowResolution::new(1000, 800),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(MapState {
            grid: None,
            seed: 42, // starting seed
            solve_status: "Init".to_string(),
            tries: 0,
            void_idx: 0,
            corridor_idx: 0,
            entrance_idx: 0,
            exit_idx: 0,
        })
        .init_resource::<MapTopologyState>()
        .add_systems(
            Startup,
            (setup_camera, setup_ui, init_map, setup_map_topology_ui),
        )
        .add_systems(
            Update,
            (
                handle_map_topology_input,
                handle_input,
                render_map,
                update_ui,
                render_map_topology,
            )
                .chain(),
        )
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("WFC Lab Camera")));
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        SeedText,
        Text::new(""),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.92, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            left: Val::Px(20.0),
            ..default()
        },
    ));
}

fn init_map(mut state: ResMut<MapState>) {
    state.void_idx = 0;
    state.corridor_idx = 1;
    state.entrance_idx = 2;
    state.exit_idx = 3;

    solve_new_map(&mut state);
}

fn solve_new_map(state: &mut MapState) {
    state.solve_status = "Solving...".to_string();
    let mut current_seed = state.seed;
    let mut tries = 0;
    loop {
        tries += 1;
        match generate_wfc_grid(current_seed, GRID_SIZE, GRID_SIZE) {
            Ok(data) => {
                if check_connectivity(
                    &data,
                    state.void_idx,
                    state.corridor_idx,
                    state.entrance_idx,
                    state.exit_idx,
                ) {
                    print_ascii_map(
                        &data,
                        current_seed,
                        state.void_idx,
                        state.corridor_idx,
                        state.entrance_idx,
                        state.exit_idx,
                    );
                    state.grid = Some(data);
                    state.seed = current_seed;
                    state.tries = tries;
                    state.solve_status = "SUCCESS".to_string();
                    break;
                }
            }
            Err(_) => {
                // WFC contradiction / failed to solve
            }
        }
        current_seed = current_seed.wrapping_add(1);
        if tries > 500 {
            state.solve_status = "FAILED (Max retries exceeded)".to_string();
            state.tries = tries;
            state.grid = None;
            break;
        }
    }
}

fn print_ascii_map(
    grid_data: &GridData<Cartesian2D, ModelInstance, CartesianGrid<Cartesian2D>>,
    seed: u64,
    void_idx: usize,
    corridor_idx: usize,
    entrance_idx: usize,
    exit_idx: usize,
) {
    let grid = grid_data.grid();
    let width = grid.size_x();
    let height = grid.size_y();

    println!("\n--- Solved Map (Seed: {}) ---", seed);
    for y in (0..height).rev() {
        let mut line = String::new();
        for x in 0..width {
            let idx = grid.get_index_2d(x, y);
            let model_idx = grid_data.get(idx).model_index;
            if model_idx == void_idx {
                line.push('.');
            } else if model_idx == corridor_idx {
                line.push('#');
            } else if model_idx == entrance_idx {
                line.push('<');
            } else if model_idx == exit_idx {
                line.push('>');
            } else {
                line.push('?');
            }
        }
        println!("{}", line);
    }
    println!("-----------------------------\n");
}

fn generate_wfc_grid(
    seed: u64,
    width: u32,
    height: u32,
) -> Result<GridData<Cartesian2D, ModelInstance, CartesianGrid<Cartesian2D>>, String> {
    let mut sockets = SocketCollection::new();
    let wall = sockets.create();
    let walkable = sockets.create();

    sockets.add_connection(wall, vec![wall]);
    sockets.add_connection(walkable, vec![walkable]);

    let mut models = ModelCollection::<Cartesian2D>::new();
    let void_model = models.create(SocketsCartesian2D::Mono(wall)).clone();
    let _corridor_model = models.create(SocketsCartesian2D::Mono(walkable)).clone();

    let entrance_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: walkable,
            x_neg: wall,
            y_pos: wall,
            y_neg: wall,
        })
        .clone();

    let exit_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: wall,
            x_neg: walkable,
            y_pos: wall,
            y_neg: wall,
        })
        .clone();

    let rules = RulesBuilder::new_cartesian_2d(models, sockets)
        .build()
        .map_err(|e| format!("Rules builder error: {:?}", e))?;

    let grid = CartesianGrid::new_cartesian_2d(width, height, false, false);

    let mut initial_nodes = Vec::new();

    // Lock top and bottom boundaries to Void
    for x in 0..width {
        initial_nodes.push(((x, 0), (void_model.index(), ModelRotation::Rot0)));
        initial_nodes.push(((x, height - 1), (void_model.index(), ModelRotation::Rot0)));
    }

    // Lock left boundary
    for y in 1..(height - 1) {
        if y == 3 || y == 4 {
            initial_nodes.push(((0, y), (entrance_model.index(), ModelRotation::Rot0)));
        } else {
            initial_nodes.push(((0, y), (void_model.index(), ModelRotation::Rot0)));
        }
    }

    // Lock right boundary
    for y in 1..(height - 1) {
        if y == 3 || y == 4 {
            initial_nodes.push(((width - 1, y), (exit_model.index(), ModelRotation::Rot0)));
        } else {
            initial_nodes.push(((width - 1, y), (void_model.index(), ModelRotation::Rot0)));
        }
    }

    let mut builder = GeneratorBuilder::new()
        .with_rules(rules)
        .with_grid(grid)
        .with_max_retry_count(0)
        .with_rng(RngMode::Seeded(seed));

    builder = builder
        .with_initial_nodes(initial_nodes)
        .map_err(|e| format!("Initial nodes error: {:?}", e))?;

    let mut generator = builder
        .build()
        .map_err(|e| format!("Generator build error: {:?}", e))?;

    let (_, grid_data) = generator
        .generate_grid()
        .map_err(|e| format!("Contradiction: {:?}", e))?;

    Ok(grid_data)
}

fn check_connectivity(
    grid_data: &GridData<Cartesian2D, ModelInstance, CartesianGrid<Cartesian2D>>,
    _void_idx: usize,
    corridor_idx: usize,
    entrance_idx: usize,
    exit_idx: usize,
) -> bool {
    let grid = grid_data.grid();
    let width = grid.size_x();
    let height = grid.size_y();

    let mut visited = vec![false; grid.total_size()];
    let mut queue = std::collections::VecDeque::new();

    // Add starts to queue
    for y in [3, 4] {
        let idx = grid.get_index_2d(0, y);
        queue.push_back(idx);
        visited[idx] = true;
    }

    while let Some(current_idx) = queue.pop_front() {
        let pos = grid.pos_from_index(current_idx);

        let mut neighbors = Vec::new();
        if pos.x > 0 {
            neighbors.push(grid.get_index_2d(pos.x - 1, pos.y));
        }
        if pos.x < width - 1 {
            neighbors.push(grid.get_index_2d(pos.x + 1, pos.y));
        }
        if pos.y > 0 {
            neighbors.push(grid.get_index_2d(pos.x, pos.y - 1));
        }
        if pos.y < height - 1 {
            neighbors.push(grid.get_index_2d(pos.x, pos.y + 1));
        }

        for n_idx in neighbors {
            if !visited[n_idx] {
                let model_idx = grid_data.get(n_idx).model_index;
                if model_idx == corridor_idx || model_idx == entrance_idx || model_idx == exit_idx {
                    visited[n_idx] = true;
                    queue.push_back(n_idx);
                }
            }
        }
    }

    // 1. Verify exit reached
    for y in [3, 4] {
        let idx = grid.get_index_2d(width - 1, y);
        if !visited[idx] {
            return false;
        }
    }

    // 2. Verify no unreachable corridors
    for idx in grid.indexes() {
        let model_idx = grid_data.get(idx).model_index;
        if model_idx == corridor_idx && !visited[idx] {
            return false;
        }
    }

    true
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    topology: Res<MapTopologyState>,
    mut state: ResMut<MapState>,
) {
    if !topology.active && keys.just_pressed(KeyCode::Space) {
        state.seed = state.seed.wrapping_add(37);
        solve_new_map(&mut state);
    }
}

fn update_ui(
    state: Res<MapState>,
    topology: Res<MapTopologyState>,
    mut query: Query<&mut Text, With<SeedText>>,
) {
    if !state.is_changed() && !topology.is_changed() {
        return;
    }
    for mut text in &mut query {
        if topology.active {
            text.0.clear();
            continue;
        }
        text.0 = format!(
            "Archived abstract tile-WFC feasibility proof\n\
             -------------------------\n\
             Status: {}\n\
             Seed: {}\n\
             Tries: {}\n\
             Grid Size: {}x{}\n\
             Entry Slot (Left): Y=3, Y=4\n\
             Exit Slot (Right): Y=3, Y=4\n\n\
             [Controls]\n\
             Press SPACE to randomize seed\n\
             Press M to return to catalogue-v2 MapSpec proof",
            state.solve_status, state.seed, state.tries, GRID_SIZE, GRID_SIZE
        );
    }
}

fn render_map(
    mut commands: Commands,
    state: Res<MapState>,
    topology: Res<MapTopologyState>,
    tiles_query: Query<Entity, With<GridTile>>,
) {
    if !state.is_changed() && !topology.is_changed() {
        return;
    }

    for ent in &tiles_query {
        commands.entity(ent).despawn();
    }

    if topology.active {
        return;
    }

    let Some(ref grid_data) = state.grid else {
        return;
    };

    let grid = grid_data.grid();
    let width = grid.size_x() as f32;
    let height = grid.size_y() as f32;

    let offset_x = -(width * VIEW_TILE_SIZE) / 2.0 + VIEW_TILE_SIZE / 2.0;
    let offset_y = -(height * VIEW_TILE_SIZE) / 2.0 + VIEW_TILE_SIZE / 2.0;

    // Helper to check if a cell coordinate is walkable
    let is_walkable = |cx: i32, cy: i32| -> bool {
        if cx < 0 || cx >= grid.size_x() as i32 || cy < 0 || cy >= grid.size_y() as i32 {
            return false;
        }
        let idx = grid.get_index_2d(cx as u32, cy as u32);
        let model_idx = grid_data.get(idx).model_index;
        model_idx == state.corridor_idx
            || model_idx == state.entrance_idx
            || model_idx == state.exit_idx
    };

    // 1. Render floor backgrounds
    for idx in grid.indexes() {
        let pos = grid.pos_from_index(idx);
        let model_idx = grid_data.get(idx).model_index;

        let color = if model_idx == state.void_idx {
            // Very dark background for Void/Wall volumes
            Color::srgb(0.005, 0.008, 0.015)
        } else {
            // Dark plain floor albedo for Walkable cells
            observed_style::surface(observed_style::SurfaceRole::Plain).base_color
        };

        commands.spawn((
            GridTile,
            Sprite {
                color,
                custom_size: Some(Vec2::new(VIEW_TILE_SIZE, VIEW_TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(
                offset_x + pos.x as f32 * VIEW_TILE_SIZE,
                offset_y + pos.y as f32 * VIEW_TILE_SIZE,
                0.0,
            ),
        ));
    }

    // 2. Render wall panels along walkable boundaries
    let wall_thickness = 4.0;
    let wall_color = observed_style::surface(observed_style::SurfaceRole::Wall)
        .edge
        .unwrap_or(Color::srgb(0.12, 0.4, 0.62)); // Glow outline blue

    for idx in grid.indexes() {
        let pos = grid.pos_from_index(idx);
        let cx = pos.x as i32;
        let cy = pos.y as i32;

        if is_walkable(cx, cy) {
            let px = offset_x + cx as f32 * VIEW_TILE_SIZE;
            let py = offset_y + cy as f32 * VIEW_TILE_SIZE;

            // Check West edge
            if !is_walkable(cx - 1, cy) {
                commands.spawn((
                    GridTile,
                    Sprite {
                        color: wall_color,
                        custom_size: Some(Vec2::new(wall_thickness, VIEW_TILE_SIZE)),
                        ..default()
                    },
                    Transform::from_xyz(px - VIEW_TILE_SIZE * 0.5, py, 1.0),
                ));
            }
            // Check East edge
            if !is_walkable(cx + 1, cy) {
                commands.spawn((
                    GridTile,
                    Sprite {
                        color: wall_color,
                        custom_size: Some(Vec2::new(wall_thickness, VIEW_TILE_SIZE)),
                        ..default()
                    },
                    Transform::from_xyz(px + VIEW_TILE_SIZE * 0.5, py, 1.0),
                ));
            }
            // Check South edge
            if !is_walkable(cx, cy - 1) {
                commands.spawn((
                    GridTile,
                    Sprite {
                        color: wall_color,
                        custom_size: Some(Vec2::new(VIEW_TILE_SIZE, wall_thickness)),
                        ..default()
                    },
                    Transform::from_xyz(px, py - VIEW_TILE_SIZE * 0.5, 1.0),
                ));
            }
            // Check North edge
            if !is_walkable(cx, cy + 1) {
                commands.spawn((
                    GridTile,
                    Sprite {
                        color: wall_color,
                        custom_size: Some(Vec2::new(VIEW_TILE_SIZE, wall_thickness)),
                        ..default()
                    },
                    Transform::from_xyz(px, py + VIEW_TILE_SIZE * 0.5, 1.0),
                ));
            }
        }
    }

    // 3. Render vertical corner pillars on intersections
    let pillar_size = 8.0;
    // Glow orange/gold for corner posts
    let pillar_color = observed_style::marker(observed_style::MarkerRole::NextRoom).base_color;

    for vy in 0..=grid.size_y() {
        for vx in 0..=grid.size_x() {
            let cx = vx as i32;
            let cy = vy as i32;

            // Check the 4 surrounding cells of vertex (vx, vy)
            let w1 = is_walkable(cx - 1, cy - 1);
            let w2 = is_walkable(cx, cy - 1);
            let w3 = is_walkable(cx - 1, cy);
            let w4 = is_walkable(cx, cy);

            let count = (w1 as u8) + (w2 as u8) + (w3 as u8) + (w4 as u8);
            if count > 0 && count < 4 {
                // Mixed transition: spawn a pillar
                let px = offset_x + (vx as f32 - 0.5) * VIEW_TILE_SIZE;
                let py = offset_y + (vy as f32 - 0.5) * VIEW_TILE_SIZE;

                commands.spawn((
                    GridTile,
                    Sprite {
                        color: pillar_color,
                        custom_size: Some(Vec2::new(pillar_size, pillar_size)),
                        ..default()
                    },
                    Transform::from_xyz(px, py, 2.0),
                ));
            }
        }
    }
}
