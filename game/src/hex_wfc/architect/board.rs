//! The Architect's board: the building as the team knows it, at the isometric pitch.
//!
//! Drawn by its own camera on its own render layer over the world, and lit by its own
//! key and fill, the way `architect_lab` lights its board. The building itself - the
//! real tiles, cut away, as remembered - is `building`'s; this module frames it and draws
//! what sits on it:
//!
//! - **The floor's marks** (`BoardMark`): the team's Observers as eyes and the Guardians
//!   they can see as pyramids, contradictions, the prison lobby and the summit as rings on
//!   the deck, deployed doors (a bar across the doorway closed, two posts open), and a
//!   chevron on a stair or ramp, green up and dim down. Rebuilt when any of them change.
//! - **The play being made** (`BoardOverlay`): a ring on every cell the picked card can be
//!   played on at this rotation, and on the cell under the cursor an amber ghost of the
//!   tile itself, turned as it would be played, with an amber ring if the rules would take
//!   it and a red one if not. Rebuilt when the card, the rotation or the cell changes.
//!
//! Nothing here decides legality: the rings are the rules' own answers
//! (`AscentSession::architect_refusal`) for this seat.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use observed_hex::{HexCoord, PortClass, hex_origin};
use observed_match::ascent::sim::{ArchitectCommand, CardKind, DoorState, ObserverState};
use observed_match::hex_wfc::project_hypothetical_cell;
use observed_style::architect::{Role, color};

use super::ArchitectDesk;
use super::building;
use super::pick::{self, BOARD_ORIGIN, CELL_RADIUS, Framing, Margins};
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
pub(super) struct BoardMark;

#[derive(Component)]
pub(super) struct BoardOverlay;

/// Shared meshes, what was last drawn, and where the camera stands.
#[derive(Resource)]
pub(in crate::hex_wfc) struct Board {
    ring: Handle<Mesh>,
    plate: Handle<Mesh>,
    thin_ring: Handle<Mesh>,
    bar: Handle<Mesh>,
    eye: Handle<Mesh>,
    pyramid: Handle<Mesh>,
    marks_signature: u64,
    play_signature: u64,
    /// The framing and the window it was made for.
    pub framing: Option<(Framing, Vec2)>,
    /// Where the camera stands for that framing, for picking.
    pub camera: Transform,
}

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    board: Option<Res<Board>>,
) {
    if board.is_some() {
        return;
    }
    let pyramid = Mesh::from(Tetrahedron::new(
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::new(-2.6, 0.0, -1.5),
        Vec3::new(2.6, 0.0, -1.5),
        Vec3::new(0.0, 0.0, 3.0),
    ));
    commands.insert_resource(Board {
        ring: meshes.add(hex_ring(CELL_RADIUS - 0.5, CELL_RADIUS - 1.5, 0.05, 0.35)),
        plate: meshes.add(hex_prism(CELL_RADIUS - 0.6, CELL_RADIUS - 0.6, -0.4, 0.0)),
        thin_ring: meshes.add(hex_ring(CELL_RADIUS - 0.6, CELL_RADIUS - 1.1, 0.02, 0.2)),
        bar: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        eye: meshes.add(Sphere::new(1.6)),
        pyramid: meshes.add(pyramid),
        marks_signature: 0,
        play_signature: 0,
        framing: None,
        camera: Transform::default(),
    });
    commands.spawn((
        BoardCamera,
        DespawnOnExit(GameState::HexWfc),
        Camera3d::default(),
        Camera {
            order: BOARD_ORDER,
            clear_color: color(Role::Background).into(),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection::default_3d()),
        // The lab's studio ambience, whatever the facility's own is doing.
        AmbientLight {
            color: color(Role::Text),
            brightness: 360.0,
            ..default()
        },
        RenderLayers::layer(BOARD_LAYER),
        Transform::default(),
        Name::new("Architect board camera"),
    ));
    // The lab's studio: a key from high over the camera's shoulder.
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        DirectionalLight {
            illuminance: 6_500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        RenderLayers::layer(BOARD_LAYER),
        Transform::from_xyz(40.0, 90.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Architect board key"),
    ));
}

/// Frame what the team knows of the floor in view, and ease the camera to it: over the
/// map as the team explores, and up and down the climb as the floor changes.
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
    let config = runtime.match_state.facility.config;
    let known = runtime.ascent.as_ref().and_then(|ascent| {
        let rules = ascent.rules();
        let cells = rules.team_knowledge.get(&desk.team)?.cells.keys();
        let observers = rules
            .observers
            .values()
            .filter(|observer| observer.team == desk.team)
            .map(|observer| &observer.cell);
        pick::frame(
            cells
                .chain(observers)
                .filter(|cell| cell.level == desk.floor)
                .copied(),
            desk.floor,
            size,
            MARGINS,
        )
    });
    // Nothing known on this floor yet: the whole of it.
    let target = known.unwrap_or_else(|| {
        let corners = [
            (0, 0),
            (config.cols - 1, 0),
            (0, config.rows - 1),
            (config.cols - 1, config.rows - 1),
        ];
        pick::frame(
            corners.map(|(q, r)| HexCoord {
                q,
                r,
                level: desk.floor,
            }),
            desk.floor,
            size,
            MARGINS,
        )
        .expect("a lattice has corners")
    });
    let framing = match board.framing {
        Some((from, was)) if was == size => pick::ease(from, target, 4.0, time.delta_secs()),
        _ => target,
    };
    board.framing = Some((framing, size));
    board.camera = pick::camera(framing, MARGINS);
    *transform = board
        .camera
        .with_translation(board.camera.translation + BOARD_ORIGIN);
    *projection = Projection::Orthographic(OrthographicProjection {
        scale: framing.metres_per_pixel,
        near: 1.0,
        far: 3_000.0,
        ..OrthographicProjection::default_3d()
    });
}

/// Redraw the floor's marks, when they have changed.
pub(super) fn draw_marks(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut board: ResMut<Board>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    drawn: Query<Entity, With<BoardMark>>,
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
    for (&cell, known) in knowledge
        .cells
        .iter()
        .filter(|(cell, _)| cell.level == floor)
    {
        let believed = desk.believed(cell, Some(known));
        (cell, believed.map(|p| (p.up, p.down, p.space.built()))).hash(&mut hasher);
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
    if signature == board.marks_signature {
        return;
    }
    board.marks_signature = signature;
    for entity in &drawn {
        commands.entity(entity).despawn();
    }

    let layer = RenderLayers::layer(BOARD_LAYER);
    let mut paint = |role: Role| materials.add(signal(role));
    let spawn = |commands: &mut Commands, mesh: &Handle<Mesh>, material, at: Transform| {
        commands.spawn((
            BoardMark,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            at,
            layer.clone(),
        ));
    };
    let on_deck = |cell: HexCoord, rise: f32| {
        let at = hex_origin(cell);
        BOARD_ORIGIN + Vec3::new(at[0], pick::deck(cell.level) + rise, at[2])
    };

    // A known cell with nothing built in it is a dark plate, so a play there has
    // somewhere to land; stairs and ramps get a chevron up where the climb goes on, green
    // because it is the way to the summit, and a muted one down.
    let plate = paint(Role::Context);
    let climb = paint(Role::Valid);
    let descend = paint(Role::Muted);
    for (&cell, known) in knowledge
        .cells
        .iter()
        .filter(|(cell, _)| cell.level == floor)
    {
        let Some(placement) = desk.believed(cell, Some(known)) else {
            continue;
        };
        if !placement.space.built() {
            spawn(
                &mut commands,
                &board.plate,
                plate.clone(),
                Transform::from_translation(on_deck(cell, 0.0)),
            );
            continue;
        }
        for (port, up, material) in [
            (placement.up, true, &climb),
            (placement.down, false, &descend),
        ] {
            if port != PortClass::Sealed {
                for bar in chevron(on_deck(cell, 3.2), up) {
                    spawn(&mut commands, &board.bar, material.clone(), bar);
                }
            }
        }
    }

    // Deployed doors on doorways the team knows: closed is a bar across the doorway, open
    // is its two posts either side, so the state reads by shape before colour.
    let door = paint(Role::Fixture);
    for (key, state) in rules
        .doors
        .iter()
        .filter(|(key, _)| key.cell.level == floor)
    {
        if !knowledge.cells.contains_key(&key.cell) {
            continue;
        }
        let angle = pick::face_angle(key.face);
        let outward = Vec3::new(angle.cos(), 0.0, angle.sin());
        let across = Quat::from_rotation_y(-angle);
        let at = on_deck(key.cell, 1.6) + outward * 6.9;
        match state {
            DoorState::Closed => spawn(
                &mut commands,
                &board.bar,
                door.clone(),
                Transform::from_translation(at)
                    .with_rotation(across)
                    .with_scale(Vec3::new(0.7, 3.2, 5.6)),
            ),
            DoorState::Open => {
                let side = across * Vec3::Z * 2.6;
                for post in [at + side, at - side] {
                    spawn(
                        &mut commands,
                        &board.bar,
                        door.clone(),
                        Transform::from_translation(post)
                            .with_rotation(across)
                            .with_scale(Vec3::new(0.7, 3.2, 0.7)),
                    );
                }
            }
        }
    }

    // The lab's legend: red is trouble, violet the prison, green the way up, cyan an eye
    // and a red pyramid a Guardian.
    let mut ring = |commands: &mut Commands, role: Role, cell: HexCoord| {
        let material = paint(role);
        spawn(
            commands,
            &board.ring,
            material,
            Transform::from_translation(on_deck(cell, 0.1)),
        );
    };
    for &cell in rules.contradictions.iter().filter(|c| c.level == floor) {
        if knowledge.cells.contains_key(&cell) {
            ring(&mut commands, Role::Guardian, cell);
        }
    }
    for &cell in rules.prison.cells.iter().filter(|c| c.level == floor) {
        ring(&mut commands, Role::Prison, cell);
    }
    let summit = rules.world.config.exit();
    if summit.level == floor && knowledge.cells.contains_key(&summit) {
        ring(&mut commands, Role::Valid, summit);
    }
    let eye = paint(Role::Observer);
    for observer in rules.observers.values() {
        if observer.team == desk.team
            && observer.state == ObserverState::Active
            && observer.cell.level == floor
        {
            spawn(
                &mut commands,
                &board.eye,
                eye.clone(),
                Transform::from_translation(on_deck(observer.cell, 2.2)),
            );
        }
    }
    let hunter = paint(Role::Guardian);
    for id in &knowledge.visible_guardians {
        if let Some(guardian) = rules.guardians.get(id)
            && guardian.cell.level == floor
        {
            spawn(
                &mut commands,
                &board.pyramid,
                hunter.clone(),
                Transform::from_translation(on_deck(guardian.cell, 0.2)),
            );
        }
    }
}

/// The lab's signal material: the role's own colour, unlit, so it reads the same in
/// light and shadow.
fn signal(role: Role) -> StandardMaterial {
    StandardMaterial {
        base_color: color(role),
        unlit: true,
        ..default()
    }
}

/// Redraw the play being made: legal targets, and the ghost under the cursor.
pub(super) fn draw_play(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut board: ResMut<Board>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    drawn: Query<Entity, With<BoardOverlay>>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let mut hasher = std::hash::DefaultHasher::new();
    (desk.selected, desk.rotation, desk.hovered, desk.floor).hash(&mut hasher);
    board.marks_signature.hash(&mut hasher);
    runtime.match_state.geometry.generation.hash(&mut hasher);
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
    let on_deck = |cell: HexCoord, rise: f32| {
        let at = hex_origin(cell);
        BOARD_ORIGIN + Vec3::new(at[0], pick::deck(cell.level) + rise, at[2])
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
    let valid = materials.add(signal(Role::Valid));
    for &cell in &legal {
        commands.spawn((
            BoardOverlay,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(board.thin_ring.clone()),
            MeshMaterial3d(valid.clone()),
            Transform::from_translation(on_deck(cell, 0.15)),
            layer.clone(),
        ));
    }
    let Some(hovered) = desk.hovered else {
        return;
    };
    let takes_it = legal.contains(&hovered);
    let edge = materials.add(signal(if takes_it {
        Role::Selected
    } else {
        Role::Guardian
    }));
    commands.spawn((
        BoardOverlay,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(board.ring.clone()),
        MeshMaterial3d(edge),
        Transform::from_translation(on_deck(hovered, 0.2)),
        layer.clone(),
    ));
    // The tile itself, as it would be played: the lab's amber ghost of its real hulls,
    // floors and walls, turned as the desk has it.
    let CardKind::Tile(shape) = card.kind else {
        return;
    };
    let placement = rules.played_placement(shape, hovered, desk.rotation);
    let physical = &runtime.match_state;
    let Ok(pieces) = project_hypothetical_cell(
        &physical.facility,
        hovered,
        placement,
        physical.content().cells(),
    ) else {
        return;
    };
    let ghost = materials.add(StandardMaterial {
        base_color: color(Role::Selected).with_alpha(0.45),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for floors in [true, false] {
        if let Some(mesh) = building::cutaway_mesh(&pieces, floors, building::bearing()) {
            commands.spawn((
                BoardOverlay,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(ghost.clone()),
                // Just proud of whatever stands there now, so it is seen over it.
                Transform::from_translation(BOARD_ORIGIN + Vec3::Y * 0.08),
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

/// The two bars of a chevron standing over a cell, pointing up (`up`) or down.
fn chevron(at: Vec3, up: bool) -> [Transform; 2] {
    let tilt = if up { 0.75 } else { -0.75 };
    let bar = |x: f32, lean: f32| {
        Transform::from_translation(at + Vec3::new(x, 0.0, 0.0))
            .with_rotation(Quat::from_rotation_z(lean))
            .with_scale(Vec3::new(3.4, 0.9, 0.9))
    };
    [bar(-1.1, tilt), bar(1.1, -tilt)]
}
