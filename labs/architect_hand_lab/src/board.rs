//! The placement surface. Walls and route stubs carry topology; hatching,
//! rings and motion carry state so colour is never the only explanation.

use bevy::prelude::*;
use observed_hex::coords::HexCoord;
use observed_hex::faces::HexFace;
use observed_hex::ports::PortClass;
use observed_mechanics::board::Edge;

use crate::art::CardArt;
use crate::model::LabState;

pub const HEX_RADIUS: f32 = 0.92;
const SQRT3: f32 = 1.732_050_8;

#[derive(Component)]
pub struct BoardVisual;

#[derive(Component)]
pub struct BoardCamera;

#[derive(Component)]
pub struct Landing {
    from: Vec2,
    to: Vec2,
    start: f32,
}

#[derive(Component, Clone, Copy)]
pub struct Breath {
    base: f32,
    phase: f32,
}

struct Kit {
    hex: Handle<Mesh>,
    ring: Handle<Mesh>,
    inner_ring: Handle<Mesh>,
    disc: Handle<Mesh>,
    bar: Handle<Mesh>,
}

impl Kit {
    fn new(meshes: &mut Assets<Mesh>) -> Self {
        Self {
            hex: meshes.add(RegularPolygon::new(HEX_RADIUS * 0.96, 6)),
            ring: meshes.add(Annulus::new(HEX_RADIUS * 0.72, HEX_RADIUS * 0.86)),
            inner_ring: meshes.add(Annulus::new(HEX_RADIUS * 0.48, HEX_RADIUS * 0.57)),
            disc: meshes.add(Circle::new(HEX_RADIUS * 0.18)),
            bar: meshes.add(Rectangle::new(HEX_RADIUS * 0.78, HEX_RADIUS * 0.12)),
        }
    }
}

struct Painter<'a> {
    commands: Commands<'a, 'a>,
    materials: &'a mut Assets<ColorMaterial>,
}

impl Painter<'_> {
    fn put(&mut self, mesh: &Handle<Mesh>, color: Color, at: Vec2, z: f32, turn: f32, scale: f32) {
        self.commands.spawn((
            BoardVisual,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(self.materials.add(ColorMaterial::from(color))),
            Transform::from_translation(at.extend(z))
                .with_rotation(Quat::from_rotation_z(turn))
                .with_scale(Vec3::splat(scale)),
        ));
    }

    fn breathe(
        &mut self,
        mesh: &Handle<Mesh>,
        color: Color,
        at: Vec2,
        z: f32,
        scale: f32,
        phase: f32,
    ) {
        self.commands.spawn((
            BoardVisual,
            Breath { base: scale, phase },
            Mesh2d(mesh.clone()),
            MeshMaterial2d(self.materials.add(ColorMaterial::from(color))),
            Transform::from_translation(at.extend(z)).with_scale(Vec3::splat(scale)),
        ));
    }
}

#[must_use]
pub fn world_of(state: &LabState, coord: HexCoord) -> Vec2 {
    let centre = state.state.board.centre();
    let dq = f32::from(coord.q) - f32::from(centre.q);
    let dr = f32::from(coord.r) - f32::from(centre.r);
    Vec2::new(HEX_RADIUS * SQRT3 * (dq + dr * 0.5), -HEX_RADIUS * 1.5 * dr)
}

#[must_use]
pub fn cell_at(state: &LabState, point: Vec2) -> Option<HexCoord> {
    state
        .state
        .board
        .cells()
        .map(|cell| (cell, world_of(state, cell).distance_squared(point)))
        .filter(|(_, distance)| *distance <= HEX_RADIUS * HEX_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(cell, _)| cell)
}

fn face_step(face: HexFace) -> Vec2 {
    let (dq, dr, _) = face.delta();
    Vec2::new(
        HEX_RADIUS * SQRT3 * (dq as f32 + dr as f32 * 0.5),
        -HEX_RADIUS * 1.5 * dr as f32,
    )
}

fn edge_pose(state: &LabState, edge: Edge) -> (Vec2, f32) {
    let step = face_step(edge.face);
    let direction = step.normalize_or_zero();
    (
        world_of(state, edge.cell) + step * 0.5,
        direction.y.atan2(direction.x) + std::f32::consts::FRAC_PI_2,
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "one explicit painter pass preserves visual layering"
)]
pub fn redraw(
    commands: Commands,
    state: Res<LabState>,
    existing: Query<Entity, With<BoardVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !state.is_changed() {
        return;
    }
    let kit = Kit::new(&mut meshes);
    let mut painter = Painter {
        commands,
        materials: &mut materials,
    };
    for entity in &existing {
        painter.commands.entity(entity).despawn();
    }

    let selected = state.selected_card();
    for cell in state.state.board.cells() {
        let at = world_of(&state, cell);
        painter.put(
            &kit.hex,
            Color::srgb(0.075, 0.105, 0.135),
            at,
            0.0,
            0.0,
            1.0,
        );

        if selected.is_some() {
            let preview = state
                .preview_at(cell)
                .expect("selected card previews every cell");
            if preview.is_valid() {
                let phase = (state.state.board.size().index(cell) % 6) as f32 * 0.11;
                painter.breathe(
                    &kit.ring,
                    Color::srgba(0.32, 0.90, 0.84, 0.22),
                    at,
                    1.0,
                    1.0,
                    phase,
                );
            }
        }

        if state.locks.is_held(cell) {
            painter.put(
                &kit.inner_ring,
                Color::srgb(0.85, 0.72, 0.30),
                at,
                1.2,
                0.0,
                1.0,
            );
            painter.put(
                &kit.bar,
                Color::srgb(0.85, 0.72, 0.30),
                at,
                1.3,
                std::f32::consts::FRAC_PI_4,
                0.9,
            );
            painter.put(
                &kit.bar,
                Color::srgb(0.85, 0.72, 0.30),
                at,
                1.3,
                -std::f32::consts::FRAC_PI_4,
                0.9,
            );
        }

        let protected = state.state.flags.iter().any(|flag| flag.at == cell)
            || state.state.prisons.contains(&cell)
            || state.state.spawns.contains(&cell);
        if protected {
            painter.put(
                &kit.ring,
                Color::srgb(0.77, 0.53, 0.92),
                at,
                1.35,
                0.0,
                0.78,
            );
            painter.put(
                &kit.inner_ring,
                Color::srgb(0.77, 0.53, 0.92),
                at,
                1.36,
                0.0,
                0.72,
            );
        }

        if state.goal_cell == Some(cell) {
            painter.breathe(
                &kit.ring,
                Color::srgba(1.0, 0.70, 0.20, 0.95),
                at,
                2.0,
                1.28,
                0.0,
            );
        }
    }

    // Draw every wall as real geometry. Interior edges are canonicalized so a
    // wall shared by two cells never becomes visually heavier than the rim.
    for cell in state.state.board.cells() {
        for face in HexFace::LATERAL {
            let edge = Edge { cell, face };
            let neighbour = state.state.board.size().neighbor(cell, face);
            let rim = !neighbour.is_some_and(|other| state.state.board.on_board(other));
            let canonical = edge.canonical(state.state.board.size());
            if !rim && canonical != edge {
                continue;
            }
            if rim || state.state.board.port(edge) != PortClass::Door {
                let (at, turn) = edge_pose(&state, edge);
                painter.put(&kit.bar, Color::srgb(0.58, 0.64, 0.70), at, 3.0, turn, 1.15);
            }
        }
    }

    // Pawns are context, not controls: a dark body and facing notch make the
    // source of watched hatching readable without another colour legend.
    for pawn in &state.state.pawns {
        if pawn.jailed {
            continue;
        }
        let at = world_of(&state, pawn.at);
        painter.put(&kit.disc, Color::srgb(0.95, 0.95, 0.90), at, 4.0, 0.0, 1.1);
        let direction = face_step(pawn.facing).normalize_or_zero();
        painter.put(
            &kit.bar,
            Color::srgb(0.08, 0.10, 0.12),
            at + direction * 0.22,
            4.1,
            direction.y.atan2(direction.x),
            0.35,
        );
    }

    if let Some(preview) = state.preview() {
        let at = world_of(&state, preview.play.cell);
        let valid = preview.is_valid();
        painter.put(
            &kit.ring,
            if valid {
                Color::srgb(0.32, 0.98, 0.88)
            } else {
                Color::srgb(1.0, 0.48, 0.26)
            },
            at,
            5.0,
            0.0,
            1.18,
        );

        // The route itself is the preview. It is deliberately drawn from the
        // shape's ports rather than from the current board.
        for face in HexFace::LATERAL {
            if preview.play.shape.port(preview.play.rotation, face) != PortClass::Door {
                continue;
            }
            let direction = face_step(face).normalize_or_zero();
            painter.put(
                &kit.bar,
                Color::srgba(0.36, 0.98, 0.90, 0.95),
                at + direction * HEX_RADIUS * 0.43,
                5.2,
                direction.y.atan2(direction.x),
                1.05,
            );
        }
        painter.put(&kit.disc, Color::srgb(1.0, 0.72, 0.24), at, 5.3, 0.0, 0.85);

        for (edge, to) in &preview.changes {
            let (edge_at, turn) = edge_pose(&state, *edge);
            match to {
                PortClass::Door => painter.breathe(
                    &kit.disc,
                    Color::srgba(0.34, 1.0, 0.90, 0.92),
                    edge_at,
                    6.0,
                    0.82,
                    0.0,
                ),
                _ => painter.put(
                    &kit.bar,
                    Color::srgb(1.0, 0.64, 0.24),
                    edge_at,
                    6.0,
                    turn,
                    1.35,
                ),
            }
        }
    } else if let Some(cell) = state.hovered
        && let Some(preview) = state.preview_at(cell)
        && !preview.is_valid()
    {
        painter.put(
            &kit.ring,
            Color::srgba(1.0, 0.48, 0.26, 0.78),
            world_of(&state, cell),
            4.5,
            0.0,
            1.08,
        );
    }
}

pub fn animate(time: Res<Time>, mut breathing: Query<(&Breath, &mut Transform)>) {
    let seconds = time.elapsed_secs();
    for (breath, mut transform) in &mut breathing {
        let wave = ((seconds / 3.4 + breath.phase) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        transform.scale = Vec3::splat(breath.base * (0.94 + wave * 0.11));
    }
}

pub fn spawn_landing(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<LabState>,
    art: Res<CardArt>,
    mut seen: Local<u32>,
) {
    if *seen == state.placement_serial {
        return;
    }
    *seen = state.placement_serial;
    let Some(play) = state.last_placed else {
        return;
    };
    let to = world_of(&state, play.cell);
    commands.spawn((
        Landing {
            from: to + Vec2::new(3.4, -4.2),
            to,
            start: time.elapsed_secs(),
        },
        Sprite {
            image: art.get(play.shape),
            custom_size: Some(Vec2::splat(1.65)),
            ..default()
        },
        Transform::from_translation((to + Vec2::new(3.4, -4.2)).extend(20.0)),
    ));
}

pub fn animate_landing(
    mut commands: Commands,
    time: Res<Time>,
    mut landing: Query<(Entity, &Landing, &mut Transform, &mut Sprite)>,
) {
    for (entity, landing, mut transform, mut sprite) in &mut landing {
        let raw = ((time.elapsed_secs() - landing.start) / 0.52).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - raw).powi(3);
        let at = landing.from.lerp(landing.to, eased);
        transform.translation.x = at.x;
        transform.translation.y = at.y;
        transform.rotation = Quat::from_rotation_z((1.0 - eased) * -0.35);
        transform.scale = Vec3::splat(1.0 - eased * 0.45);
        sprite.color.set_alpha(1.0 - raw.powi(3));
        if raw >= 1.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scenario;

    #[test]
    fn every_board_cell_round_trips_through_world_space() {
        let state = LabState::new(Scenario::ThroughLine);
        for cell in state.state.board.cells() {
            assert_eq!(cell_at(&state, world_of(&state, cell)), Some(cell));
        }
    }
}
