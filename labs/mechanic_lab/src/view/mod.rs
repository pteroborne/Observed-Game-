//! Presentation. Reads [`crate::sim`]; the simulation never reads back.
//!
//! Flat, single-scale, and **cameraless by design**. The board is 37 cells and
//! is sized to fit whatever viewport it lands in, which removes the entire
//! pan/zoom/pose layer `tactics_lab` needed and is what makes the touch build
//! tractable: there is no gesture that can lose the board.

pub mod animate;
pub mod art;
pub mod board;
pub mod hud;
pub mod input;

use bevy::prelude::*;
use observed_hex::coords::HexCoord;
use observed_hex::faces::HexFace;
use observed_style::ColorVisionMode;

use crate::sim::state::{Intent, MatchState, PawnId, TeamId};
use crate::spec::{ModeSpec, Rules, deal};

/// Everything the running lab is: which mode, its rules, the match, and the
/// orders the human has declared but not yet resolved.
#[derive(Resource)]
pub struct Session {
    pub presets: Vec<ModeSpec>,
    pub preset: usize,
    pub spec: ModeSpec,
    pub rules: Rules,
    pub state: MatchState,
    /// The human team's declared orders for this turn, by pawn.
    pub queued: Vec<Intent>,
    pub selected: Option<PawnId>,
    /// While set, tapping a neighbour turns the pawn instead of moving it.
    /// Facing is free but declared, so it needs a control of its own.
    pub face_only: bool,
    pub human: TeamId,
    /// Which colour-vision simulation the board is drawn through. Every colour
    /// passes through it, so this checks the real board rather than a swatch
    /// page — and it is the tool for telling me which marks still collide.
    pub vision: ColorVisionMode,
    pub menu_open: bool,
    pub more_open: bool,
    /// Both sides driven by the scripted driver, advancing on a clock.
    ///
    /// The point is not a demo. A match you *watch* is the cheapest way to see
    /// whether a mode has a shape: whether turns differ from one another,
    /// whether anything ever gets held, whether the objective is reachable at
    /// all. The sweep answers that as a number; this answers it as a thing you
    /// can look at.
    pub watching: bool,
    pub notice: String,
}

impl Session {
    #[must_use]
    pub fn new(presets: Vec<ModeSpec>, preset: usize) -> Self {
        let spec = presets[preset].clone();
        let rules = Rules::from_spec(&spec);
        let state = deal(&spec);
        Self {
            presets,
            preset,
            spec,
            rules,
            state,
            queued: Vec::new(),
            selected: None,
            face_only: false,
            human: TeamId(0),
            vision: ColorVisionMode::Normal,
            menu_open: false,
            more_open: false,
            watching: false,
            notice: String::new(),
        }
    }

    /// Rebuild from the current preset. This is what "swap a mechanic at
    /// runtime" means here — a mode is data, so switching one is dealing a new
    /// match from an edited [`ModeSpec`], never mutating a live trait object.
    pub fn restart(&mut self) {
        let spec = self.presets[self.preset].clone();
        self.rules = Rules::from_spec(&spec);
        self.state = deal(&spec);
        self.spec = spec;
        self.queued.clear();
        self.selected = None;
        self.notice.clear();
    }

    /// Advance one turn with every team driven by the scripted driver.
    pub fn play_one_turn(&mut self) {
        if self.state.outcome.is_some() {
            return;
        }
        let mut intents = Vec::new();
        for team in self.state.teams() {
            intents.extend(crate::sim::bot::team_intents(&self.state, team));
        }
        intents.sort_by_key(|intent| intent.pawn);
        let Session { state, rules, .. } = self;
        crate::sim::step::step(state, rules, &intents);
        self.queued.clear();
    }

    pub fn load_mode(&mut self, index: usize) {
        self.preset = index.min(self.presets.len() - 1);
        self.menu_open = false;
        self.restart();
    }

    #[must_use]
    pub fn order_for(&self, pawn: PawnId) -> Option<&Intent> {
        self.queued.iter().find(|intent| intent.pawn == pawn)
    }

    pub fn set_order(&mut self, intent: Intent) {
        self.queued.retain(|queued| queued.pawn != intent.pawn);
        self.queued.push(intent);
    }

    /// Where a pawn will stand and which way it will look once this turn
    /// resolves, given the order it currently has.
    ///
    /// This is what the cone preview draws from. Answering it from the pawn's
    /// *current* cell would show the player the consequence of a facing they
    /// are no longer going to have, which is worse than showing nothing.
    #[must_use]
    pub fn projected_pose(&self, pawn: PawnId) -> (HexCoord, HexFace) {
        let actor = self.state.pawn(pawn);
        let Some(intent) = self.order_for(pawn) else {
            return (actor.at, actor.facing);
        };
        let at = match intent.action {
            crate::sim::state::Action::Step(face) => self
                .state
                .board
                .passable(actor.at, face)
                .then(|| self.state.board.size().neighbor(actor.at, face))
                .flatten()
                .unwrap_or(actor.at),
            _ => actor.at,
        };
        (at, intent.facing)
    }

    /// Pawns of the human team that can still be given an order.
    pub fn commandable(&self) -> Vec<PawnId> {
        let mut order: Vec<PawnId> = self
            .state
            .free_pawns()
            .filter(|pawn| pawn.team == self.human)
            .map(|pawn| pawn.id)
            .collect();
        // Group by cell so `Next` walks a stack before leaving it.
        order.sort_by_key(|id| {
            let at = self.state.pawn(*id).at;
            (self.state.board.size().index(at), *id)
        });
        order
    }
}

/// Layout for a pointy-top axial lattice, in world units of one hex radius.
pub const HEX_RADIUS: f32 = 1.0;
const SQRT3: f32 = 1.732_050_8;

/// Axial `(q, r)` to world, centred on the board's middle cell.
#[must_use]
pub fn world_of(state: &MatchState, coord: HexCoord) -> Vec2 {
    let centre = state.board.centre();
    let dq = f32::from(coord.q) - f32::from(centre.q);
    let dr = f32::from(coord.r) - f32::from(centre.r);
    Vec2::new(HEX_RADIUS * SQRT3 * (dq + dr * 0.5), -HEX_RADIUS * 1.5 * dr)
}

/// The cell nearest a world position, if it is on the board.
#[must_use]
pub fn cell_at(state: &MatchState, point: Vec2) -> Option<HexCoord> {
    state
        .board
        .cells()
        .map(|cell| (cell, world_of(state, cell).distance_squared(point)))
        .filter(|&(_, d)| d <= HEX_RADIUS * HEX_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(cell, _)| cell)
}
