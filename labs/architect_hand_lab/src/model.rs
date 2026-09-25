//! Deterministic task boards and the small interaction state machine around a
//! card. The real facility model remains in `observed_mechanics`; this module
//! owns only what a usability trial needs to remember.

use bevy::prelude::*;
use observed_hex::coords::HexCoord;
use observed_mechanics::architect::{PlacementPreview, inspect};
use observed_mechanics::rules::LockSet;
use observed_mechanics::spec::{ModeSpec, deal};
use observed_mechanics::state::{MatchState, TeamId};
use observed_mechanics::tiles::{TilePlay, TileShape};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scenario {
    ThroughLine,
    CornerTurn,
    SafeSite,
    PreserveRoute,
    FreePlay,
}

impl Scenario {
    pub const ALL: [Self; 5] = [
        Self::ThroughLine,
        Self::CornerTurn,
        Self::SafeSite,
        Self::PreserveRoute,
        Self::FreePlay,
    ];

    #[must_use]
    pub const fn number(self) -> usize {
        match self {
            Self::ThroughLine => 1,
            Self::CornerTurn => 2,
            Self::SafeSite => 3,
            Self::PreserveRoute => 4,
            Self::FreePlay => 5,
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::ThroughLine => "THROUGH LINE",
            Self::CornerTurn => "CORNER TURN",
            Self::SafeSite => "SAFE SITE",
            Self::PreserveRoute => "PRESERVE ROUTE",
            Self::FreePlay => "FREE BUILD",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self::ALL[self.number() % Self::ALL.len()]
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        Self::ALL[(self.number() + Self::ALL.len() - 2) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardInstance {
    pub id: CardId,
    pub shape: TileShape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Goal {
    Exact(TilePlay),
    ShapeAnywhere(TileShape),
    Free,
}

impl Goal {
    fn matches(self, play: TilePlay) -> bool {
        match self {
            Self::Exact(wanted) => {
                wanted.cell == play.cell
                    && wanted.shape == play.shape
                    && wanted.shape.normalize_rotation(wanted.rotation)
                        == play.shape.normalize_rotation(play.rotation)
            }
            Self::ShapeAnywhere(shape) => play.shape == shape,
            Self::Free => false,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrialMetrics {
    pub selections: u32,
    pub map_taps: u32,
    pub card_drags: u32,
    pub target_changes: u32,
    pub rotations: u32,
    pub invalid_targets: u32,
    pub cancellations: u32,
    pub commits: u32,
    pub undos: u32,
}

impl TrialMetrics {
    #[must_use]
    pub fn summary(&self, scenario: Scenario, seconds: f32) -> String {
        format!(
            "{} | {:.1}s | select {} | tap {} | drag {} | aim {} | rotate {} | invalid {} | cancel {} | place {} | undo {}",
            scenario.title(),
            seconds,
            self.selections,
            self.map_taps,
            self.card_drags,
            self.target_changes,
            self.rotations,
            self.invalid_targets,
            self.cancellations,
            self.commits,
            self.undos,
        )
    }
}

#[derive(Clone)]
struct UndoSnapshot {
    state: MatchState,
    cards: Vec<CardInstance>,
    discard: Vec<CardInstance>,
    selected: Option<CardId>,
    rotation: u8,
    target: Option<HexCoord>,
}

#[derive(Resource)]
pub struct LabState {
    pub scenario: Scenario,
    pub state: MatchState,
    pub locks: LockSet,
    pub cards: Vec<CardInstance>,
    pub discard: Vec<CardInstance>,
    pub selected: Option<CardId>,
    pub rotation: u8,
    pub target: Option<HexCoord>,
    pub hovered: Option<HexCoord>,
    pub goal_cell: Option<HexCoord>,
    pub danger_play: Option<TilePlay>,
    pub prompt: String,
    pub notice: String,
    pub complete: bool,
    pub metrics: TrialMetrics,
    pub last_placed: Option<TilePlay>,
    pub placement_serial: u32,
    goal: Goal,
    undo: Option<UndoSnapshot>,
}

impl Default for LabState {
    fn default() -> Self {
        Self::new(Scenario::ThroughLine)
    }
}

impl LabState {
    #[must_use]
    pub fn new(scenario: Scenario) -> Self {
        let (state, locks, shapes, goal, goal_cell, danger_play, prompt) = setup(scenario);
        let cards = shapes
            .into_iter()
            .enumerate()
            .map(|(index, shape)| CardInstance {
                id: CardId(index as u16),
                shape,
            })
            .collect();
        Self {
            scenario,
            state,
            locks,
            cards,
            discard: Vec::new(),
            selected: None,
            rotation: 0,
            target: None,
            hovered: None,
            goal_cell,
            danger_play,
            prompt,
            notice: "Choose a card. Its exact door pattern appears here before you spend it."
                .to_string(),
            complete: false,
            metrics: TrialMetrics::default(),
            last_placed: None,
            placement_serial: 0,
            goal,
            undo: None,
        }
    }

    pub fn change_scenario(&mut self, scenario: Scenario) {
        *self = Self::new(scenario);
    }

    #[must_use]
    pub fn selected_card(&self) -> Option<CardInstance> {
        let id = self.selected?;
        self.cards.iter().find(|card| card.id == id).copied()
    }

    #[must_use]
    pub fn preview(&self) -> Option<PlacementPreview> {
        let card = self.selected_card()?;
        let cell = self.target?;
        Some(inspect(
            &self.state,
            &self.locks,
            TilePlay {
                cell,
                shape: card.shape,
                rotation: card.shape.normalize_rotation(self.rotation),
            },
        ))
    }

    #[must_use]
    pub fn preview_at(&self, cell: HexCoord) -> Option<PlacementPreview> {
        let card = self.selected_card()?;
        Some(inspect(
            &self.state,
            &self.locks,
            TilePlay {
                cell,
                shape: card.shape,
                rotation: card.shape.normalize_rotation(self.rotation),
            },
        ))
    }

    pub fn select(&mut self, id: CardId) {
        let Some(card) = self.cards.iter().find(|card| card.id == id).copied() else {
            return;
        };
        if self.selected != Some(id) {
            self.metrics.selections += 1;
        }
        self.selected = Some(id);
        self.rotation = card.shape.normalize_rotation(self.rotation);
        self.notice = format!(
            "{} selected — tap a glowing site or drag the card onto the map.",
            card.shape.label().to_uppercase()
        );
    }

    pub fn aim(&mut self, cell: HexCoord, from_drag: bool) {
        if self.selected_card().is_none() {
            self.notice = "Choose a card first.".to_string();
            return;
        }
        if from_drag {
            self.metrics.card_drags += 1;
        } else {
            self.metrics.map_taps += 1;
        }
        if self.target != Some(cell) {
            self.metrics.target_changes += 1;
        }
        self.target = Some(cell);
        let preview = self.preview().expect("selection and target were just set");
        match preview.refusal {
            None => {
                self.notice = format!(
                    "Valid: {} boundaries will change. Rotate or press PLACE.",
                    preview.changes.len()
                );
            }
            Some(reason) => {
                self.metrics.invalid_targets += 1;
                self.notice = format!("CAN'T PLACE — {}.", reason.label().to_uppercase());
            }
        }
    }

    pub fn rotate(&mut self, clockwise: bool) {
        let Some(shape) = self.selected_card().map(|card| card.shape) else {
            self.notice = "Choose a card before rotating.".to_string();
            return;
        };
        let period = shape.rotation_period();
        if period == 1 {
            self.notice = format!("{} looks the same in every direction.", shape.label());
            return;
        }
        self.rotation = if clockwise {
            (self.rotation + 1) % period
        } else {
            (self.rotation + period - 1) % period
        };
        self.metrics.rotations += 1;
        self.notice = match self.preview() {
            Some(preview) if preview.is_valid() => format!(
                "Orientation {}/{} — valid, {} boundaries change.",
                self.rotation + 1,
                period,
                preview.changes.len()
            ),
            Some(preview) => format!(
                "Orientation {}/{} — {}.",
                self.rotation + 1,
                period,
                preview
                    .refusal
                    .expect("invalid preview has a reason")
                    .label()
            ),
            None => format!(
                "Orientation {}/{} — choose a site.",
                self.rotation + 1,
                period
            ),
        };
    }

    pub fn cancel(&mut self) {
        if self.selected.is_some() || self.target.is_some() {
            self.metrics.cancellations += 1;
        }
        self.selected = None;
        self.target = None;
        self.rotation = 0;
        self.notice = "Preview cancelled. Choose any card in your hand.".to_string();
    }

    pub fn commit(&mut self) {
        let Some(preview) = self.preview() else {
            self.notice = "Choose a card and a site before placing.".to_string();
            return;
        };
        if let Some(refusal) = preview.refusal {
            self.metrics.invalid_targets += 1;
            self.notice = format!("CAN'T PLACE — {}.", refusal.label().to_uppercase());
            return;
        }
        let card = self.selected_card().expect("preview requires a card");
        self.undo = Some(UndoSnapshot {
            state: self.state.clone(),
            cards: self.cards.clone(),
            discard: self.discard.clone(),
            selected: self.selected,
            rotation: self.rotation,
            target: self.target,
        });
        for (edge, port) in preview.changes {
            self.state.board.set_port(edge, port);
        }
        self.cards.retain(|candidate| candidate.id != card.id);
        self.discard.push(card);
        self.metrics.commits += 1;
        self.last_placed = Some(preview.play);
        self.placement_serial = self.placement_serial.wrapping_add(1);
        self.selected = None;
        self.target = None;
        self.rotation = 0;
        if self.goal.matches(preview.play) {
            self.complete = true;
            self.notice = "BRIEF COMPLETE — the card and orientation read correctly.".to_string();
        } else if self.scenario == Scenario::FreePlay {
            self.notice = "Tile placed. Choose another card or undo to compare.".to_string();
        } else {
            self.notice = "Legal construction, but it does not satisfy the brief. Undo and compare the card silhouettes."
                .to_string();
        }
    }

    pub fn undo(&mut self) {
        let Some(snapshot) = self.undo.take() else {
            self.notice = "Nothing to undo yet.".to_string();
            return;
        };
        self.state = snapshot.state;
        self.cards = snapshot.cards;
        self.discard = snapshot.discard;
        self.selected = snapshot.selected;
        self.rotation = snapshot.rotation;
        self.target = snapshot.target;
        self.complete = false;
        self.last_placed = None;
        self.placement_serial = self.placement_serial.wrapping_add(1);
        self.metrics.undos += 1;
        self.notice = "Placement restored as a preview.".to_string();
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    #[must_use]
    pub fn report(&self, elapsed: f32) -> String {
        self.metrics.summary(self.scenario, elapsed)
    }
}

fn base_state(seed: u64, walls: u16) -> MatchState {
    let mut spec = ModeSpec::presets()
        .into_iter()
        .find(|spec| spec.name == "Architect")
        .unwrap_or_else(ModeSpec::plant);
    spec.seed = seed;
    spec.walls = walls;
    spec.guardian_count = 0;
    let mut state = deal(&spec);
    state.guardians.clear();
    state.telegraph.clear();
    state.hands[TeamId(0).0 as usize].cards.clear();
    state
}

#[allow(clippy::type_complexity)]
fn setup(
    scenario: Scenario,
) -> (
    MatchState,
    LockSet,
    Vec<TileShape>,
    Goal,
    Option<HexCoord>,
    Option<TilePlay>,
    String,
) {
    if scenario == Scenario::PreserveRoute {
        return preserve_setup();
    }
    let walls = match scenario {
        Scenario::ThroughLine | Scenario::CornerTurn => 8,
        Scenario::SafeSite => 14,
        Scenario::FreePlay => 12,
        Scenario::PreserveRoute => unreachable!(),
    };
    let state = base_state(0xCA4D_0000 + scenario.number() as u64, walls);
    let mut locks = LockSet::empty(state.board.size());
    if scenario == Scenario::SafeSite {
        for cell in [at(2, 2), at(2, 3), at(3, 2), at(4, 2)] {
            locks.cover(cell);
        }
    }
    match scenario {
        Scenario::ThroughLine => {
            let goal = first_valid(&state, &locks, TileShape::Corridor, 0);
            (
                state,
                locks,
                vec![TileShape::Corridor, TileShape::Bend, TileShape::Junction, TileShape::Room],
                Goal::Exact(goal),
                Some(goal.cell),
                None,
                "Connect the two opposing route marks at the beacon. Choose the straight-through card and orient it east–west."
                    .to_string(),
            )
        }
        Scenario::CornerTurn => {
            let goal = first_valid(&state, &locks, TileShape::Bend, 2);
            (
                state,
                locks,
                vec![TileShape::DeadEnd, TileShape::Bend, TileShape::Corridor, TileShape::Hall],
                Goal::Exact(goal),
                Some(goal.cell),
                None,
                "Turn the route at the beacon. Select BEND and use the on-map arrows until orientation 3/6."
                    .to_string(),
            )
        }
        Scenario::SafeSite => (
            state,
            locks,
            vec![TileShape::DeadEnd, TileShape::Sealed, TileShape::Corridor, TileShape::Room],
            Goal::ShapeAnywhere(TileShape::DeadEnd),
            None,
            None,
            "Place the DEAD END anywhere legal. Watched cells carry an eye hatch; fixed objectives carry a double ring."
                .to_string(),
        ),
        Scenario::FreePlay => (
            state,
            locks,
            vec![TileShape::Corridor, TileShape::Bend, TileShape::Junction, TileShape::Room],
            Goal::Free,
            None,
            None,
            "No brief. Compare cards, rotations and placement explanations freely. Reset deals the same hand for repeatable tests."
                .to_string(),
        ),
        Scenario::PreserveRoute => unreachable!(),
    }
}

fn first_valid(state: &MatchState, locks: &LockSet, shape: TileShape, rotation: u8) -> TilePlay {
    state
        .board
        .cells()
        .map(|cell| TilePlay {
            cell,
            shape,
            rotation,
        })
        .find(|play| inspect(state, locks, *play).is_valid())
        .expect("curated scenario must contain a valid placement")
}

#[allow(clippy::type_complexity)]
fn preserve_setup() -> (
    MatchState,
    LockSet,
    Vec<TileShape>,
    Goal,
    Option<HexCoord>,
    Option<TilePlay>,
    String,
) {
    for attempt in 0..96_u64 {
        let state = base_state(0xCA4D_4000 + attempt, 28);
        let locks = LockSet::empty(state.board.size());
        let cells: Vec<_> = state.board.cells().collect();
        for cell in cells {
            let mut danger = None;
            let mut valid = None;
            for shape in TileShape::ALL {
                for rotation in 0..shape.rotation_period() {
                    let play = TilePlay {
                        cell,
                        shape,
                        rotation,
                    };
                    let preview = inspect(&state, &locks, play);
                    if preview.refusal == Some(observed_mechanics::tiles::Refusal::WouldDisconnect)
                    {
                        danger = Some(play);
                    } else if preview.is_valid()
                        && danger.is_some_and(|candidate| candidate.shape != shape)
                    {
                        valid = Some(play);
                    }
                }
            }
            if let (Some(danger), Some(goal)) = (danger, valid) {
                let mut shapes = vec![danger.shape, goal.shape];
                for shape in TileShape::ALL {
                    if !shapes.contains(&shape) && shapes.len() < 4 {
                        shapes.push(shape);
                    }
                }
                return (
                    state,
                    locks,
                    shapes,
                    Goal::Exact(goal),
                    Some(cell),
                    Some(danger),
                    "At the beacon, choose the card and orientation that preserve every route. One tempting card would sever the objective network."
                        .to_string(),
                );
            }
        }
    }
    panic!("pinned preserve-route search found no valid/refused pair");
}

#[must_use]
pub const fn at(q: u16, r: u16) -> HexCoord {
    HexCoord { q, r, level: 0 }
}

#[must_use]
pub const fn card_subtitle(shape: TileShape) -> &'static str {
    match shape {
        TileShape::Sealed => "0 DOORS · VAULT",
        TileShape::DeadEnd => "1 DOOR · POCKET",
        TileShape::Corridor => "2 DOORS · THROUGH",
        TileShape::Bend => "2 DOORS · TURN",
        TileShape::Junction => "3 DOORS · CHOICE",
        TileShape::Hall => "4 DOORS · GATHER",
        TileShape::Room => "6 DOORS · OPEN",
    }
}

#[must_use]
pub const fn card_description(shape: TileShape) -> &'static str {
    match shape {
        TileShape::Sealed => "Closes every face. Strong control, no passage.",
        TileShape::DeadEnd => "A single-entry pocket that forces a return.",
        TileShape::Corridor => "Carries movement straight through the cell.",
        TileShape::Bend => "Turns a route between adjacent faces.",
        TileShape::Junction => "Three alternating exits create a hard choice.",
        TileShape::Hall => "Four openings make room to regroup and redirect.",
        TileShape::Room => "All six faces open. Maximum access, little control.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curated_scenario_builds_and_its_goal_is_legal() {
        for scenario in Scenario::ALL {
            let lab = LabState::new(scenario);
            assert_eq!(lab.cards.len(), 4);
            if let Some(cell) = lab.goal_cell {
                assert!(lab.state.board.on_board(cell));
            }
            if scenario == Scenario::PreserveRoute {
                let danger = lab.danger_play.expect("danger play");
                assert_eq!(
                    inspect(&lab.state, &lab.locks, danger).refusal,
                    Some(observed_mechanics::tiles::Refusal::WouldDisconnect)
                );
            }
        }
    }

    #[test]
    fn invalid_aim_never_spends_a_card() {
        let mut lab = LabState::new(Scenario::ThroughLine);
        let card = lab.cards[0];
        lab.select(card.id);
        lab.aim(lab.state.spawns[0], false);
        assert!(!lab.preview().expect("preview").is_valid());
        lab.commit();
        assert_eq!(lab.cards.len(), 4);
        assert_eq!(lab.metrics.commits, 0);
    }

    #[test]
    fn commit_and_undo_restore_board_and_hand() {
        let mut lab = LabState::new(Scenario::ThroughLine);
        let before = lab.state.digest();
        let card = lab
            .cards
            .iter()
            .find(|card| card.shape == TileShape::Corridor)
            .copied()
            .expect("corridor");
        lab.select(card.id);
        lab.aim(lab.goal_cell.expect("target"), false);
        lab.commit();
        assert_eq!(lab.cards.len(), 3);
        assert!(lab.complete);
        lab.undo();
        assert_eq!(lab.cards.len(), 4);
        assert_eq!(lab.state.digest(), before);
        assert!(!lab.complete);
    }

    #[test]
    fn metrics_summary_is_stable_and_local() {
        let summary = TrialMetrics {
            selections: 1,
            rotations: 2,
            commits: 1,
            ..default()
        }
        .summary(Scenario::CornerTurn, 4.25);
        assert!(summary.contains("CORNER TURN | 4.2s"));
        assert!(summary.contains("rotate 2"));
    }
}
