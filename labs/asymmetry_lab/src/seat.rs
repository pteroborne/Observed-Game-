//! Which chair you are sitting in, and what that chair can see.

use bevy::prelude::*;
use observed_mechanics::knowledge::Knowledge;
use observed_mechanics::spec::{ModeSpec, Rules, deal};
use observed_mechanics::state::{Intent, MatchState, PawnId, TeamId};
use observed_mechanics::tiles::{TilePlay, TileShape};

/// The two chairs.
///
/// They are asymmetric in **information**, which is the whole design. Making
/// them asymmetric in camera as well is a presentation choice the shipped game
/// can make later; it is not what decides whether the pairing works.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Seat {
    /// Sees the whole lattice, holds tiles, cannot move anybody.
    #[default]
    Architect,
    /// Moves the squad, sees only what the squad has looked at.
    Operator,
}

impl Seat {
    /// The seat this client was opened for.
    ///
    /// Two devices join one match by opening different URLs — `?seat=operator`
    /// and `?seat=architect` — which is the whole of the join protocol. There is
    /// no lobby because there is nothing to negotiate: the relay keys a match by
    /// name and a seat by its query string.
    #[must_use]
    pub fn from_environment() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            let query = web_sys::window()
                .and_then(|window| window.location().search().ok())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if query.contains("seat=operator") {
                return Seat::Operator;
            }
            if query.contains("seat=architect") {
                return Seat::Architect;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if std::env::args().any(|arg| arg == "--operator") {
                return Seat::Operator;
            }
        }
        Seat::Architect
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Seat::Architect => "ARCHITECT",
            Seat::Operator => "OPERATOR",
        }
    }

    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Seat::Architect => Seat::Operator,
            Seat::Operator => Seat::Architect,
        }
    }
}

#[derive(Resource)]
pub struct Session {
    pub seat: Seat,
    pub spec: ModeSpec,
    pub rules: Rules,
    pub state: MatchState,
    /// What the operator's team has seen. Rebuilt by replaying the log, so both
    /// seats agree on it even though only one is shown it.
    pub known: Knowledge,
    pub team: TeamId,

    /// This turn's orders, not yet submitted.
    pub queued: Vec<Intent>,
    pub plays: Vec<TilePlay>,
    pub selected_pawn: Option<PawnId>,
    pub selected_tile: Option<TileShape>,
    pub rotation: u8,
    pub face_only: bool,
    pub notice: String,
}

impl Session {
    #[must_use]
    pub fn new(seat: Seat, spec: ModeSpec) -> Self {
        let rules = Rules::from_spec(&spec);
        let state = deal(&spec);
        let mut known = Knowledge::blank(state.board.size());
        known.observe(&state, rules.vision.as_ref(), TeamId(0));
        Self {
            seat,
            spec,
            rules,
            state,
            known,
            team: TeamId(0),
            queued: Vec::new(),
            plays: Vec::new(),
            selected_pawn: None,
            selected_tile: None,
            rotation: 0,
            face_only: false,
            notice: String::new(),
        }
    }

    pub fn restart(&mut self) {
        let seat = self.seat;
        let spec = self.spec.clone();
        *self = Self::new(seat, spec);
    }

    /// Resolve a turn from both seats' orders, then record what the squad now
    /// knows. Knowledge updates *after* the turn, so the picture an operator
    /// carries is always one of consequences rather than intentions.
    pub fn resolve(&mut self, extra: &[Intent], plays: Vec<TilePlay>) {
        if self.state.outcome.is_some() {
            return;
        }
        let mut intents = self.queued.clone();
        intents.extend_from_slice(extra);
        for pawn in self.commandable() {
            if !intents.iter().any(|intent| intent.pawn == pawn) {
                let facing = self.state.pawn(pawn).facing;
                intents.push(Intent {
                    pawn,
                    facing,
                    action: observed_mechanics::state::Action::Hold,
                });
            }
        }
        intents.sort_by_key(|intent| intent.pawn);
        self.state.architect_queue = plays;

        let Session { state, rules, .. } = self;
        observed_mechanics::step::step(state, rules, &intents);

        let vision = self.rules.vision.as_ref();
        self.known.observe(&self.state, vision, self.team);
        self.queued.clear();
        self.plays.clear();
        self.selected_pawn = None;
    }

    #[must_use]
    pub fn commandable(&self) -> Vec<PawnId> {
        let mut order: Vec<PawnId> = self
            .state
            .free_pawns()
            .filter(|pawn| pawn.team == self.team)
            .map(|pawn| pawn.id)
            .collect();
        order.sort_by_key(|id| {
            let at = self.state.pawn(*id).at;
            (self.state.board.size().index(at), *id)
        });
        order
    }

    pub fn set_order(&mut self, intent: Intent) {
        self.queued.retain(|queued| queued.pawn != intent.pawn);
        self.queued.push(intent);
    }

    #[must_use]
    pub fn order_for(&self, pawn: PawnId) -> Option<&Intent> {
        self.queued.iter().find(|intent| intent.pawn == pawn)
    }
}
