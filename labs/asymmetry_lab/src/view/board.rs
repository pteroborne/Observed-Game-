//! Drawing whichever seat you are in.
//!
//! One renderer, two truths. The architect is drawn from the board; the
//! operator is drawn from [`Knowledge`] — what their squad has *seen*, which
//! may be out of date. Rendering the operator from the board with a mask over
//! it would have been simpler and would have quietly lied: the operator would
//! see stale ground redrawn correctly, which is precisely the mistake the
//! design is built to punish.

use bevy::prelude::*;
use observed_hex::coords::HexCoord;
use observed_hex::faces::HexFace;
use observed_hex::ports::PortClass;
use observed_mechanics::board::Edge;
use observed_mechanics::state::ChangeSource;
use observed_style::{ColorVisionMode, MarkerRole, TacticsRole, marker, tactics, team};

use crate::seat::{Seat, Session};

use super::animate::Pulse;
use super::art::{ArtAtlas, Icon};
use super::{HEX_RADIUS, world_of};

#[derive(Component)]
pub struct BoardVisual;

/// How a cell is known, which is the operator's whole world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fog {
    /// Being looked at right now. What you see is true.
    Live,
    /// Seen before, not now. What you see is a memory and may be wrong.
    Remembered,
    /// Never seen.
    Dark,
}

impl Fog {
    pub const ALL: [Fog; 3] = [Fog::Live, Fog::Remembered, Fog::Dark];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Fog::Live => "watched now - what you see is true",
            Fog::Remembered => "remembered - may have been rebuilt since",
            Fog::Dark => "never seen",
        }
    }
}

const Z_CELL: f32 = 0.0;
const Z_ROLE: f32 = 2.0;
const Z_ORDER: f32 = 3.0;
const Z_WALL: f32 = 4.0;
const Z_ACTOR: f32 = 5.0;
const Z_FACING: f32 = 6.0;

struct Kit {
    hex: Handle<Mesh>,
    ring: Handle<Mesh>,
    inner: Handle<Mesh>,
    disc: Handle<Mesh>,
    hatch: Handle<Mesh>,
}

impl Kit {
    fn new(meshes: &mut Assets<Mesh>) -> Self {
        let r = HEX_RADIUS;
        Self {
            hex: meshes.add(RegularPolygon::new(r * 0.97, 6)),
            ring: meshes.add(Annulus::new(r * 0.74, r * 0.86)),
            inner: meshes.add(Annulus::new(r * 0.50, r * 0.60)),
            disc: meshes.add(Circle::new(r * 0.30)),
            hatch: meshes.add(Rectangle::new(r * 1.2, r * 0.06)),
        }
    }
}

struct Painter<'a> {
    commands: Commands<'a, 'a>,
    materials: &'a mut Assets<ColorMaterial>,
    atlas: &'a ArtAtlas,
    vision: ColorVisionMode,
}

impl Painter<'_> {
    fn put(&mut self, mesh: &Handle<Mesh>, color: Color, at: Vec2, z: f32, turn: f32, scale: f32) {
        let seen: Color = observed_style::simulate_color_vision(color, self.vision).into();
        let mut adjusted = seen;
        adjusted.set_alpha(color.alpha());
        self.commands.spawn((
            BoardVisual,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(self.materials.add(ColorMaterial::from(adjusted))),
            Transform::from_translation(at.extend(z))
                .with_rotation(Quat::from_rotation_z(turn))
                .with_scale(Vec3::splat(scale)),
        ));
    }

    fn icon(&mut self, icon: Icon, at: Vec2, z: f32, turn: f32, size: f32, fade: f32) {
        let Some(image) = self.atlas.get(icon, self.vision) else {
            return;
        };
        self.commands.spawn((
            BoardVisual,
            Sprite {
                image,
                custom_size: Some(Vec2::splat(size)),
                color: Color::WHITE.with_alpha(fade),
                ..default()
            },
            Transform::from_translation(at.extend(z)).with_rotation(Quat::from_rotation_z(turn)),
        ));
    }
}

fn edge_pose(state: &observed_mechanics::state::MatchState, edge: Edge) -> (Vec2, f32) {
    let from = world_of(state, edge.cell);
    let (dq, dr, _) = edge.face.delta();
    let step = Vec2::new(
        HEX_RADIUS * 1.732_050_8 * (dq as f32 + dr as f32 * 0.5),
        -HEX_RADIUS * 1.5 * dr as f32,
    );
    let dir = step.normalize_or_zero();
    (
        from + step * 0.5,
        dir.y.atan2(dir.x) + std::f32::consts::FRAC_PI_2,
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "one pass over one board; splitting it would hide the draw order"
)]
pub fn redraw(
    commands: Commands,
    session: Res<Session>,
    atlas: Res<ArtAtlas>,
    existing: Query<Entity, With<BoardVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !session.is_changed() {
        return;
    }
    let kit = Kit::new(&mut meshes);
    let mut p = Painter {
        commands,
        materials: &mut materials,
        atlas: &atlas,
        vision: ColorVisionMode::Normal,
    };
    for entity in &existing {
        p.commands.entity(entity).despawn();
    }

    let state = &session.state;
    let locks = session.rules.vision.locks(state);
    let architect = session.seat == Seat::Architect;

    let fog_of = |cell: HexCoord| -> Fog {
        if architect {
            return Fog::Live;
        }
        match session.known.remembered(cell) {
            None => Fog::Dark,
            Some(seen) if seen.seen_on >= state.turn => Fog::Live,
            Some(_) => Fog::Remembered,
        }
    };

    // Floor. A remembered cell is dimmer and hatched; a dark one is not drawn
    // at all, because an operator has no business seeing its outline.
    for cell in state.board.cells() {
        let fog = fog_of(cell);
        if fog == Fog::Dark {
            continue;
        }
        let at = world_of(state, cell);
        let held = locks.is_held(cell);
        let base = if held {
            Color::srgb(0.10, 0.16, 0.20)
        } else {
            tactics(TacticsRole::DevSurface).base_color
        };
        let fade = if fog == Fog::Remembered { 0.42 } else { 1.0 };
        p.put(&kit.hex, base.with_alpha(fade), at, Z_CELL, 0.0, 1.0);
        if held {
            p.put(
                &kit.inner,
                tactics(TacticsRole::ReachableRoute).base_color,
                at,
                Z_CELL + 0.1,
                0.0,
                1.0,
            );
        }
        if fog == Fog::Remembered {
            // Hatching, not just dimming: staleness must survive being read
            // without colour, and it is the single most important thing an
            // operator can misjudge.
            p.put(
                &kit.hatch,
                Color::srgb(0.45, 0.42, 0.30),
                at,
                Z_CELL + 0.15,
                std::f32::consts::FRAC_PI_4,
                1.0,
            );
        }
    }

    // Board roles. The operator only sees the ones it has been near.
    for &base in &state.spawns {
        if fog_of(base) != Fog::Dark {
            p.icon(
                Icon::Base,
                world_of(state, base),
                Z_ROLE,
                0.0,
                HEX_RADIUS * 1.7,
                1.0,
            );
        }
    }
    for &prison in &state.prisons {
        if fog_of(prison) != Fog::Dark {
            p.icon(
                Icon::Prison,
                world_of(state, prison),
                Z_ROLE,
                0.0,
                HEX_RADIUS * 1.25,
                1.0,
            );
        }
    }
    for flag in &state.flags {
        if fog_of(flag.at) == Fog::Dark {
            continue;
        }
        let icon = if flag.planted_by.is_some() {
            Icon::FlagPlanted
        } else {
            Icon::Flag
        };
        p.icon(
            icon,
            world_of(state, flag.at),
            Z_ROLE + 0.1,
            0.0,
            HEX_RADIUS * 1.35,
            1.0,
        );
    }

    // Walls. Drawn from memory for the operator, so a rebuilt boundary keeps
    // showing the wall that used to be there until somebody looks again.
    for cell in state.board.cells() {
        let fog = fog_of(cell);
        if fog == Fog::Dark {
            continue;
        }
        for face in HexFace::LATERAL {
            let edge = Edge { cell, face };
            let neighbour = state.board.size().neighbor(cell, face);
            let rim = !neighbour.is_some_and(|other| state.board.on_board(other));
            let port = if architect || fog == Fog::Live {
                state.board.port(edge)
            } else {
                session
                    .known
                    .remembered(cell)
                    .map_or(PortClass::Sealed, |seen| seen.ports[face.index()])
            };
            if rim || port != PortClass::Door {
                let (at, turn) = edge_pose(state, edge);
                let fade = if fog == Fog::Remembered { 0.5 } else { 1.0 };
                p.icon(Icon::Wall, at, Z_WALL, turn, HEX_RADIUS * 1.05, fade);
            }
        }
    }

    // The telegraph. The architect sees everything coming; the operator sees
    // only what is about to happen where somebody is looking.
    for change in &state.telegraph {
        if !architect && fog_of(change.edge.cell) != Fog::Live {
            continue;
        }
        let (at, turn) = edge_pose(state, change.edge);
        let pulse = match (change.source, change.to) {
            (ChangeSource::Architect(_), PortClass::Sealed) => Pulse::closing(1.0),
            (ChangeSource::Architect(_), _) => Pulse::opening(1.0),
            (_, PortClass::Sealed) => Pulse::closing(1.0),
            (_, _) => Pulse::opening(1.0),
        };
        let Some(image) = atlas.get(Icon::Ghost, ColorVisionMode::Normal) else {
            continue;
        };
        p.commands.spawn((
            BoardVisual,
            pulse,
            Sprite {
                image,
                custom_size: Some(Vec2::splat(HEX_RADIUS * 1.05)),
                ..default()
            },
            Transform::from_translation(at.extend(Z_WALL + 0.2))
                .with_rotation(Quat::from_rotation_z(turn)),
        ));
    }

    // Where the architect is about to build.
    for play in &session.plays {
        p.put(
            &kit.ring,
            tactics(TacticsRole::ClickPulse).base_color,
            world_of(state, play.cell),
            Z_ORDER,
            0.0,
            1.0,
        );
    }
    for intent in &session.queued {
        let pawn = state.pawn(intent.pawn);
        let aim = match intent.action {
            observed_mechanics::state::Action::Step(face) => state
                .board
                .size()
                .neighbor(pawn.at, face)
                .unwrap_or(pawn.at),
            _ => pawn.at,
        };
        p.put(
            &kit.ring,
            tactics(TacticsRole::ClickPulse).base_color,
            world_of(state, aim),
            Z_ORDER,
            0.0,
            0.72,
        );
    }

    // Actors.
    for guardian in &state.guardians {
        if fog_of(guardian.at) != Fog::Live && !architect {
            continue;
        }
        p.icon(
            Icon::Guardian,
            world_of(state, guardian.at),
            Z_ACTOR,
            0.0,
            HEX_RADIUS * 1.35,
            1.0,
        );
    }
    for pawn in &state.pawns {
        let at = world_of(state, pawn.at);
        let mine = pawn.team == session.team;
        if !mine && !architect && fog_of(pawn.at) != Fog::Live {
            continue;
        }
        p.put(
            &kit.disc,
            Color::srgb(0.02, 0.03, 0.04),
            at,
            Z_ACTOR - 0.1,
            0.0,
            1.42,
        );
        if session.selected_pawn == Some(pawn.id) {
            p.put(&kit.disc, Color::WHITE, at, Z_ACTOR - 0.05, 0.0, 1.6);
        }
        let icon = match (pawn.jailed, mine) {
            (true, _) => Icon::Held,
            (false, true) => Icon::Pawn,
            (false, false) => Icon::Rival,
        };
        p.icon(icon, at, Z_ACTOR, 0.0, HEX_RADIUS * 1.3, 1.0);
        if !pawn.jailed {
            let (edge_at, turn) = edge_pose(
                state,
                Edge {
                    cell: pawn.at,
                    face: pawn.facing,
                },
            );
            let toward = (edge_at - at).normalize_or_zero();
            p.icon(
                Icon::Facing,
                at + toward * HEX_RADIUS * 0.5,
                Z_FACING,
                turn - std::f32::consts::FRAC_PI_2,
                HEX_RADIUS * 0.62,
                1.0,
            );
        }
    }

    // The architect's aiming reticle: where the selected tile would land.
    if architect && let Some(shape) = session.selected_tile {
        let _ = shape;
        let colour = marker(MarkerRole::NextRoom).base_color;
        if let Some(play) = session.plays.last() {
            p.put(
                &kit.ring,
                colour,
                world_of(state, play.cell),
                Z_ORDER + 0.1,
                0.0,
                1.1,
            );
        }
    }
    let _ = team(0);
}
