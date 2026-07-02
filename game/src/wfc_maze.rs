use crate::teleport::WallSeg;
use bevy::math::Vec2;
use ghx_proc_gen::{
    generator::{
        RngMode,
        builder::GeneratorBuilder,
        model::{ModelCollection, ModelRotation},
        rules::RulesBuilder,
        socket::{SocketCollection, SocketsCartesian2D},
    },
    ghx_grid::{
        cartesian::{coordinates::Cartesian2D, grid::CartesianGrid},
        grid::{Grid, GridData},
    },
};

fn generate_wfc_grid(
    cols: usize,
    rows: usize,
    entry_cols: &[usize],
    exit_cols: &[usize],
    seed: u64,
) -> Result<
    GridData<
        Cartesian2D,
        ghx_proc_gen::generator::model::ModelInstance,
        CartesianGrid<Cartesian2D>,
    >,
    String,
> {
    let mut sockets = SocketCollection::new();
    let wall = sockets.create();
    let walkable = sockets.create();

    sockets.add_connection(wall, vec![wall]);
    sockets.add_connection(walkable, vec![walkable]);

    let mut models = ModelCollection::<Cartesian2D>::new();

    // Model 0: Void (solid wall block)
    let void_model = models.create(SocketsCartesian2D::Mono(wall)).clone();

    // Model 1: Corridor4Way (4-way intersection)
    let _cross_model = models.create(SocketsCartesian2D::Mono(walkable)).clone();

    // Model 2: CorridorStraight (straight hallway corridor)
    let _straight_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: walkable,
            x_neg: walkable,
            y_pos: wall,
            y_neg: wall,
        })
        .with_all_rotations()
        .clone();

    // Model 3: CorridorCorner (90-degree corner turn)
    let _corner_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: walkable,
            x_neg: wall,
            y_pos: walkable,
            y_neg: wall,
        })
        .with_all_rotations()
        .clone();

    // Model 4: CorridorT (T-junction corridor)
    let _t_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: walkable,
            x_neg: walkable,
            y_pos: walkable,
            y_neg: wall,
        })
        .with_all_rotations()
        .clone();

    // Model 5: CorridorEnd (Dead-end / doorway connector)
    let end_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: wall,
            x_neg: wall,
            y_pos: walkable,
            y_neg: wall,
        })
        .with_all_rotations()
        .clone();

    let rules = RulesBuilder::new_cartesian_2d(models.clone(), sockets)
        .build()
        .map_err(|e| format!("Rules builder error: {:?}", e))?;

    println!(
        "WFC MODEL INDICES -> void: {}, cross: {}, straight: {}, corner: {}, t: {}, end: {}",
        void_model.index(),
        _cross_model.index(),
        _straight_model.index(),
        _corner_model.index(),
        _t_model.index(),
        end_model.index()
    );

    let grid = CartesianGrid::new_cartesian_2d(cols as u32, rows as u32, false, false);

    let mut initial_nodes = Vec::new();

    // Lock left boundary to Void if it doesn't host any entry/exit door columns
    if !entry_cols.contains(&0) && !exit_cols.contains(&0) {
        for y in 1..(rows - 1) {
            initial_nodes.push(((0, y as u32), (void_model.index(), ModelRotation::Rot0)));
        }
    }

    // Lock right boundary to Void if it doesn't host any entry/exit door columns
    if !entry_cols.contains(&(cols - 1)) && !exit_cols.contains(&(cols - 1)) {
        for y in 1..(rows - 1) {
            initial_nodes.push((
                (cols as u32 - 1, y as u32),
                (void_model.index(), ModelRotation::Rot0),
            ));
        }
    }

    // Lock bottom boundary (row 0)
    for x in 0..cols {
        if entry_cols.contains(&x) {
            // Doorway facing North: walkable North, all other sides wall
            initial_nodes.push(((x as u32, 0), (end_model.index(), ModelRotation::Rot0)));
        } else {
            initial_nodes.push(((x as u32, 0), (void_model.index(), ModelRotation::Rot0)));
        }
    }

    // Lock top boundary (row rows - 1)
    for x in 0..cols {
        if exit_cols.contains(&x) {
            // Doorway facing South: walkable South, all other sides wall
            initial_nodes.push((
                (x as u32, rows as u32 - 1),
                (end_model.index(), ModelRotation::Rot180),
            ));
        } else {
            initial_nodes.push((
                (x as u32, rows as u32 - 1),
                (void_model.index(), ModelRotation::Rot0),
            ));
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
    grid_data: &GridData<
        Cartesian2D,
        ghx_proc_gen::generator::model::ModelInstance,
        CartesianGrid<Cartesian2D>,
    >,
    entry_cols: &[usize],
    exit_cols: &[usize],
    void_idx: usize,
) -> bool {
    let grid = grid_data.grid();
    let width = grid.size_x();
    let height = grid.size_y();

    // We check that EVERY entrance can reach ALL exits, ensuring a single unified path component
    for &ec in entry_cols {
        let mut visited = vec![false; grid.total_size()];
        let mut queue = std::collections::VecDeque::new();

        let start_idx = grid.get_index_2d(ec as u32, 0);
        queue.push_back(start_idx);
        visited[start_idx] = true;

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
                    if model_idx != void_idx {
                        visited[n_idx] = true;
                        queue.push_back(n_idx);
                    }
                }
            }
        }

        // Verify this entrance reaches all exits
        for &xc in exit_cols {
            let idx = grid.get_index_2d(xc as u32, height - 1);
            if !visited[idx] {
                return false;
            }
        }

        // Also verify no unreachable corridor/walkable tiles from this entrance
        for idx in grid.indexes() {
            let model_idx = grid_data.get(idx).model_index;
            if model_idx != void_idx && !visited[idx] {
                return false;
            }
        }
    }

    true
}

pub fn generate_wfc_interior_walls(
    cols: usize,
    rows: usize,
    entry_cols: &[usize],
    exit_cols: &[usize],
    seed: u64,
    cell: f32,
    wall_t: f32,
) -> Vec<WallSeg> {
    let void_idx = 0;

    let mut current_seed = seed;
    let mut grid_data = None;

    // Attempt WFC generation & connectivity verification
    for _try in 0..500 {
        if let Ok(data) = generate_wfc_grid(cols, rows, entry_cols, exit_cols, current_seed)
            && check_connectivity(&data, entry_cols, exit_cols, void_idx)
        {
            grid_data = Some(data);
            break;
        }
        current_seed = current_seed.wrapping_add(1);
    }

    let Some(data) = grid_data else {
        return Vec::new();
    };

    let grid = data.grid();

    let footprint_x = cols as f32 * cell * 0.5;
    let footprint_y = rows as f32 * cell * 0.5;

    let cell_center = |cx: i32, cy: i32| -> Vec2 {
        Vec2::new(
            -footprint_x + (cx as f32 + 0.5) * cell,
            -footprint_y + (cy as f32 + 0.5) * cell,
        )
    };

    let is_walkable = |cx: i32, cy: i32| -> bool {
        if cx < 0 || cx >= cols as i32 || cy < 0 || cy >= rows as i32 {
            return false;
        }
        let idx = grid.get_index_2d(cx as u32, cy as u32);
        let model_idx = data.get(idx).model_index;
        model_idx != void_idx
    };

    let mut walls = Vec::new();

    // 1. Generate Wall Panels
    for idx in grid.indexes() {
        let pos = grid.pos_from_index(idx);
        let cx = pos.x as i32;
        let cy = pos.y as i32;

        if is_walkable(cx, cy) {
            let center = cell_center(cx, cy);

            // West neighbor
            if !is_walkable(cx - 1, cy) && cx > 0 {
                walls.push(WallSeg {
                    center: center - Vec2::new(cell * 0.5, 0.0),
                    half: Vec2::new(wall_t, cell * 0.5),
                });
            }
            // East neighbor
            if !is_walkable(cx + 1, cy) && cx + 1 < cols as i32 {
                walls.push(WallSeg {
                    center: center + Vec2::new(cell * 0.5, 0.0),
                    half: Vec2::new(wall_t, cell * 0.5),
                });
            }
            // South neighbor
            if !is_walkable(cx, cy - 1) && cy > 0 {
                walls.push(WallSeg {
                    center: center - Vec2::new(0.0, cell * 0.5),
                    half: Vec2::new(cell * 0.5, wall_t),
                });
            }
            // North neighbor
            if !is_walkable(cx, cy + 1) && cy + 1 < rows as i32 {
                walls.push(WallSeg {
                    center: center + Vec2::new(0.0, cell * 0.5),
                    half: Vec2::new(cell * 0.5, wall_t),
                });
            }
        }
    }

    // 2. Generate Vertical Corner Pillars
    for vy in 0..=rows {
        for vx in 0..=cols {
            let cx = vx as i32;
            let cy = vy as i32;

            let w1 = is_walkable(cx - 1, cy - 1);
            let w2 = is_walkable(cx, cy - 1);
            let w3 = is_walkable(cx - 1, cy);
            let w4 = is_walkable(cx, cy);

            let count = (w1 as u8) + (w2 as u8) + (w3 as u8) + (w4 as u8);
            if count > 0 && count < 4 {
                // Do not spawn pillars on the absolute outer perimeter boundary
                let on_perimeter_x = vx == 0 || vx == cols;
                let on_perimeter_y = vy == 0 || vy == rows;
                if on_perimeter_x || on_perimeter_y {
                    continue;
                }

                let px = -footprint_x + (vx as f32 - 0.5) * cell;
                let py = -footprint_y + (vy as f32 - 0.5) * cell;

                walls.push(WallSeg {
                    center: Vec2::new(px, py),
                    half: Vec2::new(wall_t, wall_t),
                });
            }
        }
    }

    walls
}
