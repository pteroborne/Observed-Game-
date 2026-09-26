//! The Architect's board: one floor of what the team knows, seen from above.
//!
//! Drawn by its own camera on its own render layer over the world, like the survivor map,
//! and lit by its own key. Two layers of entities:
//!
//! - **The floor as known** (`BoardCell`): a slab per known cell in its district's colour,
//!   bright where the team is looking now and dim where it is only remembered; a spoke
//!   per doorway; the team's Observers, the Guardians they can see, contradictions, the
//!   prison's lobby and the summit. Rebuilt when what is known, the floor, or the
//!   facility changes.
//! - **The play being made** (`BoardOverlay`): a ring on every cell the picked card can be
//!   played on at this rotation, and on the cell under the cursor a ghost of the doors the
//!   card would give it, green if the rules would take it and red if not. Rebuilt when the
//!   card, the rotation or the cell under the cursor changes.
//!
//! Nothing here decides legality: the rings are the rules' own answers
//! (`AscentSession::architect_refusal`) for this seat.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use observed_hex::{HexCoord, HexFace, TILE_LEVEL_HEIGHT, hex_origin};
use observed_match::ascent::sim::{ArchitectCommand, CardKind, DoorState, ObserverState};
use observed_style::{MarkerRole, marker};

use super::ArchitectDesk;
use super::pick::{self, CELL_RADIUS, Framing, Margins};
use crate::GameState;
use crate::hex_wfc::equipment::{hex_prism, hex_ring};
use crate::hex_wfc::sim::HexWfcRuntime;

/// The board's render layer, apart from the world's and the survivor map's.
pub(super) const BOARD_LAYER: usize = 3;
/// Above the world and the survivor map.
const BOARD_ORDER: isize = 2;
/// The panel and the hand the board leaves room for, in pixels.
pub(super) const MARGINS: Margins = Margins {
    left: 330.0,
    bottom: 230.0,
    top: 24.0,
};

#[derive(Component)]
pub(super) struct BoardCamera;

#[derive(Component)]
pub(super) struct BoardCell;

#[derive(Component)]
pub(super) struct BoardOverlay;

/// Shared meshes and what was last drawn.
#[derive(Resource)]
pub(in crate::hex_wfc) struct Board {
    slab: Handle<Mesh>,
    ring: Handle<Mesh>,
    thin_ring: Handle<Mesh>,
    spoke: Handle<Mesh>,
    disc: Handle<Mesh>,
    floor_signature: u64,
    play_signature: u64,
    /// The framing last used, for picking.
    pub framing: Option<(Framing, Vec2)>,
}

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    board: Option<Res<Board>>,
) {
    if board.is_some() {
        return;
    }
    commands.insert_resource(Board {
        slab: meshes.add(hex_prism(CELL_RADIUS - 0.7, CELL_RADIUS - 0.7, 0.0, 0.4)),
        ring: meshes.add(hex_ring(CELL_RADIUS - 0.4, CELL_RADIUS - 1.6, 0.5, 0.7)),
        thin_ring: meshes.add(hex_ring(CELL_RADIUS - 0.5, CELL_RADIUS - 1.9, 0.8, 0.9)),
        spoke: meshes.add(Cuboid::new(6.4, 0.3, 2.2)),
        disc: meshes.add(Cylinder::new(3.0, 0.6)),
        floor_signature: 0,
        play_signature: 0,
        framing: None,
    });
    commands.spawn((
        BoardCamera,
        DespawnOnExit(GameState::HexWfc),
        Camera3d::default(),
        Camera {
            order: BOARD_ORDER,
            clear_color: Color::srgb(0.012, 0.016, 0.028).into(),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection::default_3d()),
        RenderLayers::layer(BOARD_LAYER),
        Transform::default(),
        Name::new("Architect board camera"),
    ));
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        DirectionalLight {
            illuminance: 4_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        RenderLayers::layer(BOARD_LAYER),
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.6, -1.2, 0.0)),
        Name::new("Architect board key"),
    ));
}

/// Frame the floor in view and point the camera at it.
pub(super) fn frame(
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    windows: Query<&Window>,
    time: Res<Time>,
    mut board: ResMut<Board>,
    mut camera: Query<(&mut Transform, &mut Projection), With<BoardCamera>>,
) {
    let (Ok(window), Ok((mut transform, mut projection))) = (windows.single(), camera.single_mut())
    else {
        return;
    };
    let size = Vec2::new(window.width(), window.height());
    // What the team knows of the floor, and where its Observers are: the whole lattice
    // is mostly unknown, and framing it all shrinks the known to a corner.
    let known = runtime.ascent.as_ref().and_then(|ascent| {
        let rules = ascent.rules();
        let cells = rules.team_knowledge.get(&desk.team)?.cells.keys();
        let observers = rules
            .observers
            .values()
            .filter(|observer| observer.team == desk.team)
            .map(|observer| &observer.cell);
        pick::frame_cells(
            cells
                .chain(observers)
                .filter(|cell| cell.level == desk.floor)
                .copied(),
            pick::MIN_VIEW,
            size,
            MARGINS,
        )
    });
    let target = known
        .unwrap_or_else(|| pick::frame_floor(runtime.match_state.facility.config, size, MARGINS));
    let framing = match board.framing {
        Some((from, was)) if was == size => pick::ease(from, target, 4.0, time.delta_secs()),
        _ => target,
    };
    board.framing = Some((framing, size));
    let height = f32::from(desk.floor) * TILE_LEVEL_HEIGHT;
    *transform = Transform::from_xyz(framing.centre.x, height + 200.0, framing.centre.y)
        .looking_at(
            Vec3::new(framing.centre.x, height, framing.centre.y),
            Vec3::NEG_Z,
        );
    *projection = Projection::Orthographic(OrthographicProjection {
        scale: framing.metres_per_pixel,
        near: 1.0,
        far: 400.0,
        ..OrthographicProjection::default_3d()
    });
}

/// Rebuild the floor as the team knows it, when that has changed.
pub(super) fn draw_floor(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut board: ResMut<Board>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    drawn: Query<Entity, With<BoardCell>>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let rules = ascent.rules();
    let Some(knowledge) = rules.team_knowledge.get(&desk.team) else {
        return;
    };
    let floor = desk.floor;
    let mut hasher = std::hash::DefaultHasher::new();
    floor.hash(&mut hasher);
    rules.world.generation.hash(&mut hasher);
    for (cell, known) in knowledge
        .cells
        .iter()
        .filter(|(cell, _)| cell.level == floor)
    {
        (cell, known.placement.doors, known.placement.space.built()).hash(&mut hasher);
        knowledge.visible_cells.contains(cell).hash(&mut hasher);
    }
    rules.contradictions.hash(&mut hasher);
    for observer in rules.observers.values() {
        (observer.cell, observer.state == ObserverState::Active).hash(&mut hasher);
    }
    knowledge.visible_guardians.hash(&mut hasher);
    for (key, state) in &rules.doors {
        (key.cell, key.face, *state == DoorState::Open).hash(&mut hasher);
    }
    let signature = hasher.finish();
    if signature == board.floor_signature {
        return;
    }
    board.floor_signature = signature;
    for entity in &drawn {
        commands.entity(entity).despawn();
    }

    let layer = RenderLayers::layer(BOARD_LAYER);
    let mut paint = |color: Color, emissive: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: color,
            emissive,
            ..default()
        })
    };
    let spawn = |commands: &mut Commands, mesh: &Handle<Mesh>, material, at: Transform| {
        commands.spawn((
            BoardCell,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            at,
            layer.clone(),
        ));
    };
    for (&cell, known) in knowledge
        .cells
        .iter()
        .filter(|(cell, _)| cell.level == floor)
    {
        if !known.placement.space.built() {
            continue;
        }
        let register = rules
            .world
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(observed_content::ArchitectureRegister::ALL[0]);
        let accent = observed_style::architecture(register).accent;
        // Seen now is lit; remembered is dim, and may no longer be true.
        let gain = if knowledge.visible_cells.contains(&cell) {
            1.0
        } else {
            0.45
        };
        let origin = Vec3::from_array(hex_origin(cell));
        let slab = paint(
            Color::LinearRgba(accent * (0.35 * gain)),
            accent * (0.3 * gain),
        );
        spawn(
            &mut commands,
            &board.slab,
            slab,
            Transform::from_translation(origin),
        );
        let door = paint(
            Color::WHITE,
            accent * (2.4 * gain) + LinearRgba::gray(0.4 * gain),
        );
        for face in HexFace::LATERAL
            .into_iter()
            .filter(|&f| known.placement.is_open(f))
        {
            spawn(
                &mut commands,
                &board.spoke,
                door.clone(),
                spoke(origin, face, 0.45),
            );
        }
    }
    // Marks, over the slabs.
    let door = marker(MarkerRole::Control);
    let door = paint(door.base_color, door.emissive);
    let mut mark = |commands: &mut Commands, mesh: &Handle<Mesh>, role: MarkerRole, cell| {
        let t = marker(role);
        let material = paint(t.base_color, t.emissive);
        spawn(
            commands,
            mesh,
            material,
            Transform::from_translation(Vec3::from_array(hex_origin(cell)) + Vec3::Y * 1.5),
        );
    };
    for &cell in rules.contradictions.iter().filter(|c| c.level == floor) {
        if knowledge.cells.contains_key(&cell) {
            mark(&mut commands, &board.ring, MarkerRole::Collapse, cell);
        }
    }
    // Deployed doors, on the doorways the team knows: closed is a bar across the doorway,
    // open is its two posts either side, so the state reads by shape before colour.
    for (key, state) in rules
        .doors
        .iter()
        .filter(|(key, _)| key.cell.level == floor)
    {
        if !knowledge.cells.contains_key(&key.cell) {
            continue;
        }
        let origin = Vec3::from_array(hex_origin(key.cell));
        let angle = pick::face_angle(key.face);
        let across =
            Quat::from_rotation_y(-angle) * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let at = origin + Vec3::new(angle.cos(), 0.0, angle.sin()) * 6.9 + Vec3::Y * 1.2;
        match state {
            DoorState::Closed => {
                spawn(
                    &mut commands,
                    &board.spoke,
                    door.clone(),
                    Transform::from_translation(at)
                        .with_rotation(across)
                        .with_scale(Vec3::new(0.8, 2.0, 1.4)),
                );
            }
            DoorState::Open => {
                let side = across * Vec3::X * 2.6;
                for post in [at + side, at - side] {
                    spawn(
                        &mut commands,
                        &board.spoke,
                        door.clone(),
                        Transform::from_translation(post)
                            .with_rotation(across)
                            .with_scale(Vec3::new(0.18, 2.0, 1.4)),
                    );
                }
            }
        }
    }
    for &cell in rules.prison.cells.iter().filter(|c| c.level == floor) {
        mark(&mut commands, &board.ring, MarkerRole::Prison, cell);
    }
    let summit = rules.world.config.exit();
    if summit.level == floor && knowledge.cells.contains_key(&summit) {
        mark(&mut commands, &board.ring, MarkerRole::Exit, summit);
    }
    for observer in rules.observers.values() {
        if observer.team == desk.team
            && observer.state == ObserverState::Active
            && observer.cell.level == floor
        {
            mark(
                &mut commands,
                &board.disc,
                MarkerRole::Teammate,
                observer.cell,
            );
        }
    }
    for id in &knowledge.visible_guardians {
        if let Some(guardian) = rules.guardians.get(id)
            && guardian.cell.level == floor
        {
            mark(
                &mut commands,
                &board.disc,
                MarkerRole::Collapse,
                guardian.cell,
            );
        }
    }
}

/// Redraw the play being made: legal targets, and the ghost under the cursor.
pub(super) fn draw_play(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut board: ResMut<Board>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    drawn: Query<Entity, With<BoardOverlay>>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let mut hasher = std::hash::DefaultHasher::new();
    (desk.selected, desk.rotation, desk.hovered, desk.floor).hash(&mut hasher);
    board.floor_signature.hash(&mut hasher);
    let hand = ascent.session().hands.get(&desk.team);
    hand.map(|hand| (hand.cooldown == 0, hand.deck.hand.len()))
        .hash(&mut hasher);
    let signature = hasher.finish();
    if signature == board.play_signature {
        return;
    }
    board.play_signature = signature;
    for entity in &drawn {
        commands.entity(entity).despawn();
    }
    let Some(card) = desk
        .selected
        .and_then(|index| hand?.deck.hand.get(index))
        .copied()
    else {
        return;
    };
    let rules = ascent.rules();
    let Some(knowledge) = rules.team_knowledge.get(&desk.team) else {
        return;
    };
    let layer = RenderLayers::layer(BOARD_LAYER);
    let mut paint = |role: MarkerRole, gain: f32| {
        let t = marker(role);
        materials.add(StandardMaterial {
            base_color: Color::LinearRgba(t.base_color.to_linear() * gain),
            emissive: t.emissive * gain,
            ..default()
        })
    };
    let legal = legal_targets(
        ascent,
        &desk,
        card.id,
        knowledge
            .cells
            .keys()
            .copied()
            .filter(|cell| cell.level == desk.floor),
    );
    let target = paint(MarkerRole::NextRoom, 0.9);
    for &cell in &legal {
        commands.spawn((
            BoardOverlay,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(board.thin_ring.clone()),
            MeshMaterial3d(target.clone()),
            Transform::from_translation(Vec3::from_array(hex_origin(cell))),
            layer.clone(),
        ));
    }
    let Some(hovered) = desk.hovered else {
        return;
    };
    let origin = Vec3::from_array(hex_origin(hovered));
    let verdict = if legal.contains(&hovered) {
        MarkerRole::Exit
    } else {
        MarkerRole::Collapse
    };
    let edge = paint(verdict, 1.0);
    commands.spawn((
        BoardOverlay,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(board.ring.clone()),
        MeshMaterial3d(edge.clone()),
        Transform::from_translation(origin + Vec3::Y * 0.4),
        layer.clone(),
    ));
    if let CardKind::Tile(shape) = card.kind {
        let doors = shape.doors(desk.rotation);
        for face in HexFace::LATERAL
            .into_iter()
            .filter(|face| doors & (1 << face.index()) != 0)
        {
            commands.spawn((
                BoardOverlay,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(board.spoke.clone()),
                MeshMaterial3d(edge.clone()),
                spoke(origin, face, 1.2),
                layer.clone(),
            ));
        }
    }
}

/// The cells among `cells` where the rules would take card `card` at the desk's rotation.
fn legal_targets(
    ascent: &observed_match::ascent::facility::AscentRules,
    desk: &ArchitectDesk,
    card: observed_match::ascent::sim::CardId,
    cells: impl Iterator<Item = HexCoord>,
) -> BTreeSet<HexCoord> {
    cells
        .filter(|&target| {
            ascent
                .session()
                .architect_refusal(
                    desk.seat,
                    ArchitectCommand::Play {
                        card,
                        target,
                        rotation: desk.rotation,
                    },
                )
                .is_none()
        })
        .collect()
}

/// A spoke from a cell's centre out toward `face`, `rise` above the slab.
fn spoke(origin: Vec3, face: HexFace, rise: f32) -> Transform {
    let angle = pick::face_angle(face);
    let reach = 3.4;
    Transform::from_translation(origin + Vec3::new(angle.cos() * reach, rise, angle.sin() * reach))
        .with_rotation(Quat::from_rotation_y(-angle))
}
