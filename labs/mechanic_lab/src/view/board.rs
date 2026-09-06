//! Drawing the board.
//!
//! **Colour is never the only channel.** You are as likely to be reading this
//! board without full hue discrimination as with it, so every distinction is
//! carried by *shape* first and colour second: actors differ by silhouette,
//! cell states by ring and hatch, and passability — the most important
//! information on screen — by geometry, because a wall is a bar and a doorway
//! is a gap. `observed_style::outline` already states the principle, putting
//! width in the semantic treatment for exactly this reason.
//!
//! [`CellPaint::ALL`] and [`MarkPaint::ALL`] are what the legend is built from,
//! so a mark cannot ship without a name.

use bevy::prelude::*;
use observed_hex::coords::HexCoord;
use observed_hex::faces::HexFace;
use observed_hex::ports::PortClass;
use observed_style::{
    ColorVisionMode, MarkerRole, TacticsRole, marker, simulate_color_vision, tactics, team,
};

use super::animate::Pulse;
use super::art::{ArtAtlas, Icon};
use crate::sim::board::Edge;
use crate::sim::rules::LockSet;
use crate::sim::state::MatchState;
use crate::spec::MutationPreview;

use super::{HEX_RADIUS, Session, world_of};

#[derive(Component)]
pub struct BoardVisual;

/// What a cell is. With passability living on edges, a cell has far less to say
/// than it used to — which is the point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellPaint {
    /// Held against change by somebody's vision. Drawn with an inner ring.
    Observed,
    /// No doorway at all: floor you cannot reach or leave.
    Isolated,
    Open,
}

impl CellPaint {
    pub const ALL: [CellPaint; 3] = [CellPaint::Observed, CellPaint::Open, CellPaint::Isolated];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            CellPaint::Observed => "observed - inner ring - held against change",
            CellPaint::Isolated => "isolated - hatched - no doorway in or out",
            CellPaint::Open => "open floor",
        }
    }

    #[must_use]
    pub fn color(self) -> Color {
        match self {
            // Deliberately dim, and *not* the bright cone colour. Filling an
            // observed cell with it put the same periwinkle under a pawn as on
            // the pawn, which under deuteranopia left the two separable only by
            // silhouette. The inner ring carries the signal at full strength
            // instead, so the state reads as a shape on a quiet floor rather
            // than as a second thing the colour of a pawn.
            CellPaint::Observed => Color::srgb(0.10, 0.16, 0.20),
            CellPaint::Isolated => tactics(TacticsRole::DevContext).base_color,
            CellPaint::Open => tactics(TacticsRole::DevSurface).base_color,
        }
    }
}

#[must_use]
pub fn paint(state: &MatchState, locks: &LockSet, cell: HexCoord) -> CellPaint {
    if state.board.doorway_count(cell) == 0 {
        return CellPaint::Isolated;
    }
    if locks.is_held(cell) {
        return CellPaint::Observed;
    }
    CellPaint::Open
}

/// Every non-cell mark, and the silhouette that identifies it without colour.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkPaint {
    Pawn,
    Rival,
    Held,
    Guardian,
    Flag,
    FlagPlanted,
    Prison,
    Base,
    Wall,
    WallClosing,
    WallOpening,
    ChangePending,
    Cone,
    Selected,
    Ordered,
}

impl MarkPaint {
    pub const ALL: [MarkPaint; 15] = [
        MarkPaint::Pawn,
        MarkPaint::Rival,
        MarkPaint::Held,
        MarkPaint::Guardian,
        MarkPaint::Flag,
        MarkPaint::FlagPlanted,
        MarkPaint::Prison,
        MarkPaint::Base,
        MarkPaint::Wall,
        MarkPaint::WallClosing,
        MarkPaint::WallOpening,
        MarkPaint::ChangePending,
        MarkPaint::Cone,
        MarkPaint::Selected,
        MarkPaint::Ordered,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            MarkPaint::Pawn => "your pawn - filled disc",
            MarkPaint::Rival => "rival pawn - diamond",
            MarkPaint::Held => "held in prison - hollow ring",
            MarkPaint::Guardian => "guardian - triangle",
            MarkPaint::Flag => "flag, unplanted - hollow pentagon",
            MarkPaint::FlagPlanted => "flag, planted - filled pentagon",
            MarkPaint::Prison => "prison - square",
            MarkPaint::Base => "base - hexagon ring - you are untouchable here",
            MarkPaint::Wall => "wall - solid bar",
            MarkPaint::WallClosing => "this doorway WILL WALL UP next turn",
            MarkPaint::WallOpening => "this wall WILL OPEN next turn",
            MarkPaint::ChangePending => "this boundary will change - outcome hidden",
            MarkPaint::Cone => "what the selected pawn can see",
            MarkPaint::Selected => "selected",
            MarkPaint::Ordered => "order declared for this turn",
        }
    }

    #[must_use]
    pub fn color(self) -> Color {
        match self {
            MarkPaint::Pawn => team(0).base_color,
            MarkPaint::Rival => team(1).base_color,
            MarkPaint::Held => team(2).base_color,
            MarkPaint::Guardian => marker(MarkerRole::Director).base_color,
            MarkPaint::Flag | MarkPaint::FlagPlanted => marker(MarkerRole::Exit).base_color,
            MarkPaint::Prison => marker(MarkerRole::Collapse).base_color,
            MarkPaint::Base => marker(MarkerRole::NextRoom).base_color,
            MarkPaint::Wall => Color::srgb(0.55, 0.60, 0.66),
            MarkPaint::WallClosing => tactics(TacticsRole::RouteLimit).base_color,
            MarkPaint::WallOpening => tactics(TacticsRole::ReachableRoute).base_color,
            MarkPaint::ChangePending => tactics(TacticsRole::Shifted).base_color,
            MarkPaint::Cone => tactics(TacticsRole::ReachableRoute).base_color,
            MarkPaint::Selected => Color::WHITE,
            MarkPaint::Ordered => tactics(TacticsRole::ClickPulse).base_color,
        }
    }
}

const Z_CELL: f32 = 0.0;
const Z_CONE: f32 = 1.0;
const Z_ROLE: f32 = 2.0;
const Z_ORDER: f32 = 3.0;
const Z_WALL: f32 = 4.0;
const Z_ACTOR: f32 = 5.0;
const Z_FACING: f32 = 6.0;

/// The few primitives that survived the move to authored icons: washes, rings
/// and hatches that belong to the *floor* rather than to a thing standing on
/// it. Everything with an identity is an SVG in `art/`.
struct Kit {
    hex: Handle<Mesh>,
    hex_ring: Handle<Mesh>,
    disc: Handle<Mesh>,
    ring: Handle<Mesh>,
    inner_ring: Handle<Mesh>,
    hatch: Handle<Mesh>,
}

impl Kit {
    fn new(meshes: &mut Assets<Mesh>) -> Self {
        let r = HEX_RADIUS;
        Self {
            hex: meshes.add(RegularPolygon::new(r * 0.97, 6)),
            hex_ring: meshes.add(Annulus::new(r * 0.74, r * 0.86)),
            disc: meshes.add(Circle::new(r * 0.30)),
            ring: meshes.add(Annulus::new(r * 0.19, r * 0.30)),
            inner_ring: meshes.add(Annulus::new(r * 0.50, r * 0.60)),
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
    /// Every colour passes through the vision simulation, so the `Vision`
    /// control checks the real board rather than a swatch page.
    fn put(&mut self, mesh: &Handle<Mesh>, color: Color, at: Vec2, z: f32, turn: f32, scale: f32) {
        let seen: Color = simulate_color_vision(color, self.vision).into();
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
}

impl Painter<'_> {
    /// A mesh mark that breathes. Used for the cell wash under a telegraphed
    /// boundary, where a sprite would be the wrong shape.
    fn put_pulsing(&mut self, mesh: &Handle<Mesh>, color: Color, at: Vec2, z: f32, pulse: Pulse) {
        let seen: Color = simulate_color_vision(color, self.vision).into();
        let mut adjusted = seen;
        adjusted.set_alpha(color.alpha());
        self.commands.spawn((
            BoardVisual,
            pulse,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(self.materials.add(ColorMaterial::from(adjusted))),
            Transform::from_translation(at.extend(z)),
        ));
    }

    /// Draw an authored icon. Silhouette does the identifying, so these are
    /// left at their authored colours rather than tinted — the vision
    /// simulation is baked into the texture instead.
    fn icon(&mut self, icon: Icon, at: Vec2, z: f32, turn: f32, size: f32, pulse: Option<Pulse>) {
        let Some(image) = self.atlas.get(icon, self.vision) else {
            return;
        };
        let mut entity = self.commands.spawn((
            BoardVisual,
            Sprite {
                image,
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_translation(at.extend(z)).with_rotation(Quat::from_rotation_z(turn)),
        ));
        if let Some(pulse) = pulse {
            entity.insert(pulse);
        }
    }
}

/// Where a boundary sits on screen, and which way it lies.
fn edge_pose(state: &MatchState, edge: Edge) -> (Vec2, f32) {
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
        vision: session.vision,
    };
    for entity in &existing {
        p.commands.entity(entity).despawn();
    }

    let state = &session.state;
    let locks = session.rules.vision.locks(state);

    // Floor, then the state markings that ride on it.
    for cell in state.board.cells() {
        let at = world_of(state, cell);
        let paint = paint(state, &locks, cell);
        p.put(&kit.hex, paint.color(), at, Z_CELL, 0.0, 1.0);
        match paint {
            CellPaint::Observed => p.put(
                &kit.inner_ring,
                MarkPaint::Cone.color(),
                at,
                Z_CELL + 0.1,
                0.0,
                1.0,
            ),
            CellPaint::Isolated => {
                for i in 0..3 {
                    p.put(
                        &kit.hatch,
                        Color::srgb(0.30, 0.33, 0.37),
                        at,
                        Z_CELL + 0.1,
                        std::f32::consts::FRAC_PI_4,
                        1.0,
                    );
                    let _ = i;
                }
            }
            CellPaint::Open => {}
        }
    }

    // What the selected pawn can see, so facing is never a guess.
    if let Some(selected) = session.selected
        && !state.pawn(selected).jailed
    {
        let pawn = state.pawn(selected);
        for cell in state.board.cells() {
            if cell != pawn.at
                && session
                    .rules
                    .vision
                    .covers(state, pawn.at, pawn.facing, cell)
            {
                p.put(
                    &kit.hex,
                    MarkPaint::Cone.color().with_alpha(0.20),
                    world_of(state, cell),
                    Z_CONE,
                    0.0,
                    1.0,
                );
            }
        }
    }

    // Board roles: base ring, prison square, flag pentagon.
    for (index, &base) in state.spawns.iter().enumerate() {
        let color = if index == session.human.0 as usize {
            MarkPaint::Base.color()
        } else {
            MarkPaint::Rival.color()
        };
        p.put(
            &kit.hex_ring,
            color,
            world_of(state, base),
            Z_ROLE,
            0.0,
            1.0,
        );
    }
    for &prison in &state.prisons {
        p.icon(
            Icon::Prison,
            world_of(state, prison),
            Z_ROLE,
            0.0,
            HEX_RADIUS * 1.25,
            None,
        );
    }
    for flag in &state.flags {
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
            None,
        );
    }

    // Declared orders.
    for intent in &session.queued {
        let pawn = state.pawn(intent.pawn);
        if pawn.jailed {
            continue;
        }
        let aim = match intent.action {
            crate::sim::state::Action::Step(face) => state
                .board
                .size()
                .neighbor(pawn.at, face)
                .unwrap_or(pawn.at),
            _ => pawn.at,
        };
        p.put(
            &kit.hex_ring,
            MarkPaint::Ordered.color().with_alpha(0.75),
            world_of(state, aim),
            Z_ORDER,
            0.0,
            0.72,
        );
    }

    // Walls. This is the load-bearing read on the board and it is pure
    // geometry: a bar is a wall, a gap is a doorway, no hue required.
    for cell in state.board.cells() {
        for face in HexFace::LATERAL {
            let edge = Edge { cell, face };
            let neighbour = state.board.size().neighbor(cell, face);
            let rim = !neighbour.is_some_and(|other| state.board.on_board(other));
            if rim || state.board.port(edge) != PortClass::Door {
                let (at, turn) = edge_pose(state, edge);
                p.icon(Icon::Wall, at, Z_WALL, turn, HEX_RADIUS * 1.05, None);
            }
        }
    }

    // The telegraph, drawn as ghost geometry: what the facility will do to
    // itself, in the same visual language as what it has already done.
    if session.spec.preview != MutationPreview::Hidden {
        for change in &state.telegraph {
            let (at, turn) = edge_pose(state, change.edge);
            // Rhythm carries the outcome: a boundary about to wall up flashes
            // hard and fast, one about to open breathes slowly. Strip the
            // colour away entirely and the two are still told apart, which is
            // the point — motion is the one channel hue cannot take.
            let pulse = match session.spec.preview {
                MutationPreview::Outcome if change.to == PortClass::Sealed => Pulse::closing(1.0),
                MutationPreview::Outcome => Pulse::opening(1.0),
                _ => Pulse::pending(1.0),
            };
            p.icon(
                Icon::Ghost,
                at,
                Z_WALL + 0.2,
                turn,
                HEX_RADIUS * 1.05,
                Some(pulse),
            );
            // Both cells the boundary joins breathe too, because a bar on an
            // edge is a small thing to notice on a phone.
            for cell in [
                Some(change.edge.cell),
                state
                    .board
                    .size()
                    .neighbor(change.edge.cell, change.edge.face),
            ]
            .into_iter()
            .flatten()
            {
                if !state.board.on_board(cell) {
                    continue;
                }
                p.put_pulsing(
                    &kit.hex,
                    MarkPaint::ChangePending.color().with_alpha(0.5),
                    world_of(state, cell),
                    Z_CONE + 0.1,
                    Pulse {
                        scale: (0.9, 1.0),
                        ..pulse
                    },
                );
            }
        }
    }

    // Actors, each with its own silhouette.
    for guardian in &state.guardians {
        p.icon(
            Icon::Guardian,
            world_of(state, guardian.at),
            Z_ACTOR,
            0.0,
            HEX_RADIUS * 1.35,
            None,
        );
    }
    for pawn in &state.pawns {
        let at = world_of(state, pawn.at);
        let mine = pawn.team == session.human;
        p.put(
            &kit.disc,
            Color::srgb(0.02, 0.03, 0.04),
            at,
            Z_ACTOR - 0.1,
            0.0,
            1.42,
        );
        if session.selected == Some(pawn.id) {
            p.put(
                &kit.ring,
                MarkPaint::Selected.color(),
                at,
                Z_ACTOR - 0.05,
                0.0,
                1.55,
            );
        }
        let icon = match (pawn.jailed, mine) {
            (true, _) => Icon::Held,
            (false, true) => Icon::Pawn,
            (false, false) => Icon::Rival,
        };
        p.icon(icon, at, Z_ACTOR, 0.0, HEX_RADIUS * 1.30, None);

        if !pawn.jailed {
            // Facing as a wedge on the faced edge: big enough to read at a
            // glance, because a cone you cannot see the direction of is a cone
            // you cannot plan with.
            let (edge_at, turn) = edge_pose(
                state,
                Edge {
                    cell: pawn.at,
                    face: pawn.facing,
                },
            );
            let toward = (edge_at - at).normalize_or_zero();
            // The arrow art points up at rotation zero, so it needs the pose
            // turned back by a quarter to point along the faced direction.
            p.icon(
                Icon::Facing,
                at + toward * HEX_RADIUS * 0.50,
                Z_FACING,
                turn - std::f32::consts::FRAC_PI_2,
                HEX_RADIUS * 0.62,
                None,
            );
        }
    }
}
