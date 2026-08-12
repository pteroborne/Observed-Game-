//! Presentation. Reads [`crate::sim`]; the simulation never reads back.
//!
//! Both views draw from one classification pass ([`paint`]) so they cannot
//! disagree about what a cell *is* while disagreeing about how to draw it. A
//! difference between the isometric and flat views is therefore always a drawing
//! bug, never a rules bug — which is the whole reason for having two.
//!
//! They also share the *geometry* builder in [`board`] rather than owning a
//! renderer each. The two modes differ in exactly three things — which levels are
//! drawn, whether a cell gets height, and where the camera sits ([`camera`]) —
//! and writing that as two files would have been two files to keep in agreement
//! about a lattice they are both reading from the same place.

pub mod board;
pub mod camera;
pub mod hud;
pub mod overlay;
pub mod setup;

use bevy::prelude::*;
use observed_facility::hex_wfc::{HexArchetype, HexSpace};
use observed_hex::HexCoord;
use observed_match::hex_wfc::HexMapDiscovery;
use observed_style::{HexSketchRole, MarkerRole, SchematicRole, Treatment, marker, schematic};

use crate::sim::TacticsGame;
use crate::sim::unit::PLAYER_TEAM;

/// Which renderer is drawing the board.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMode {
    /// Authored geometry with quarter-height perimeter walls for one active deck.
    #[default]
    Deck,
    /// Top-down schematic of one deck for strategic orientation.
    Overview,
}

impl ViewMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ViewMode::Deck => "Quarter-wall deck",
            ViewMode::Overview => "Operations map",
        }
    }

    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            ViewMode::Deck => ViewMode::Overview,
            ViewMode::Overview => ViewMode::Deck,
        }
    }
}

/// What a cell means to the player right now, in the order that matters.
///
/// The ordering *is* the design: a cell that is both anchored and about to shift
/// is drawn as anchored, because the anchor is why the shift will fail. Painting
/// it as volatile would tell the player the opposite of what is true.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellPaint {
    /// Not discovered. Drawn as nothing at all under fog.
    Unknown,
    /// Held by a deployed anchor — guaranteed to survive the next shift.
    Anchored,
    /// In sight this turn, so held for this turn.
    Observed,
    /// Telegraphed to re-collapse when the turn ends.
    Volatile,
    /// Remembered, but the facility has rewritten it since.
    Stale,
    /// Remembered and believed current.
    Known,
}

impl CellPaint {
    /// The semantic treatment, always from `observed_style`.
    #[must_use]
    pub fn treatment(self) -> Treatment {
        match self {
            // Never drawn; the caller skips it. Grid keeps it defined rather
            // than making this function partial.
            CellPaint::Unknown => schematic(SchematicRole::Grid),
            CellPaint::Anchored => marker(MarkerRole::Control),
            CellPaint::Observed => schematic(SchematicRole::Pinned),
            CellPaint::Volatile => schematic(SchematicRole::Volatile),
            CellPaint::Stale => schematic(SchematicRole::Grid),
            CellPaint::Known => schematic(SchematicRole::Pinned),
        }
    }

    /// Remembered-but-unheld cells are drawn dimmer than held ones, so "I know
    /// this" and "I am holding this" are never the same mark.
    #[must_use]
    pub const fn dimmed(self) -> bool {
        matches!(self, CellPaint::Known | CellPaint::Stale)
    }

    /// The legend entry. Every mark on screen has one — the Legibility Contract
    /// forbids an unlabelled coloured marker.
    #[must_use]
    pub const fn legend(self) -> &'static str {
        match self {
            CellPaint::Unknown => "undiscovered",
            CellPaint::Anchored => "anchored - will not change",
            CellPaint::Observed => "in sight - held this turn",
            CellPaint::Volatile => "will re-collapse this turn",
            CellPaint::Stale => "remembered - has since changed",
            CellPaint::Known => "remembered",
        }
    }

    /// Everything a legend needs to list, in reading order.
    pub const ALL: [CellPaint; 6] = [
        CellPaint::Observed,
        CellPaint::Anchored,
        CellPaint::Volatile,
        CellPaint::Known,
        CellPaint::Stale,
        CellPaint::Unknown,
    ];
}

/// Classify one cell for drawing.
///
/// With `reveal_full_map` an undiscovered cell is reported as `Known` rather
/// than `Unknown`, which is the *only* thing that setting changes. It cannot
/// reach the simulation from here — this function has no `&mut` — which is the
/// structural half of the promise that the whole-map view measures the same game.
#[must_use]
pub fn paint(game: &TacticsGame, cell: HexCoord) -> CellPaint {
    let anchored = game.anchor_in(cell).is_some();
    if anchored {
        return CellPaint::Anchored;
    }
    let observed = game.observation.visible_cells.contains(&cell)
        || game
            .observation
            .occupied_cells
            .values()
            .any(|&at| at == cell);
    if observed {
        return CellPaint::Observed;
    }
    let telegraphed = game
        .telegraph
        .as_ref()
        .is_some_and(|telegraph| telegraph.cells().contains(&cell));
    let known = game
        .knowledge
        .get(&PLAYER_TEAM)
        .and_then(|map| map.cells.get(&cell));
    match known {
        // Telegraph wins over plain memory: what is about to change is the one
        // thing the player has to act on this turn.
        Some(_) if telegraphed => CellPaint::Volatile,
        Some(known) if known.is_stale(&game.world, cell) => CellPaint::Stale,
        Some(_) => CellPaint::Known,
        None if game.settings.reveal_full_map => {
            if telegraphed {
                CellPaint::Volatile
            } else {
                CellPaint::Known
            }
        }
        None => CellPaint::Unknown,
    }
}

/// Whether the player's map says this cell has ever been walked, as opposed to
/// merely seen. Rooms only name themselves to someone who has been inside.
#[must_use]
pub fn traversed(game: &TacticsGame, cell: HexCoord) -> bool {
    game.knowledge
        .get(&PLAYER_TEAM)
        .and_then(|map| map.cells.get(&cell))
        .is_some_and(|known| known.discovery == HexMapDiscovery::Traversed)
}

/// Map the solver's archetype onto the style crate's drawing vocabulary.
///
/// `observed_style` asks callers to do this themselves — its own doc says the
/// solver's `HexArchetype` lives in `observed_facility` and that the style crate
/// stays render-free and dependency-light. So this match is the sanctioned
/// duplication, not an oversight: `iso_observer_lab` carries the same one.
#[must_use]
pub fn sketch_role(archetype: HexArchetype, space: HexSpace, in_room: bool) -> HexSketchRole {
    if in_room || space == HexSpace::Room {
        return HexSketchRole::Room;
    }
    match archetype {
        HexArchetype::Void => HexSketchRole::Void,
        HexArchetype::Straight | HexArchetype::Corner => HexSketchRole::Corridor,
        HexArchetype::Junction => HexSketchRole::Junction,
        HexArchetype::RampUp => HexSketchRole::Ramp,
        HexArchetype::RampHead => HexSketchRole::RampHead,
        HexArchetype::Shaft => HexSketchRole::Shaft,
        HexArchetype::Expanse => HexSketchRole::Expanse,
        HexArchetype::Room => HexSketchRole::Room,
    }
}

/// Marker treatment for a unit.
#[must_use]
pub fn unit_marker(is_selected: bool, is_rival: bool) -> Treatment {
    marker(if is_rival {
        MarkerRole::Rival
    } else if is_selected {
        MarkerRole::You
    } else {
        MarkerRole::Teammate
    })
}

/// Anything spawned by a board renderer, so a rebuild can clear the board
/// without touching the HUD or the camera.
#[derive(Component)]
pub struct BoardVisual;

/// Lightweight pointer feedback, rebuilt independently from authored hulls.
#[derive(Component)]
pub struct BoardOverlay;

/// Marks the entity roots each screen owns.
#[derive(Component)]
pub struct HudRoot;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{MatchSettings, Sight};
    use crate::sim::TacticsGame;

    fn game(sight: Sight, reveal_full_map: bool) -> TacticsGame {
        TacticsGame::new(MatchSettings {
            sight,
            reveal_full_map,
            seed: 0x0000_0000_000c_0ffe,
            ..MatchSettings::standard()
        })
        .expect("solves")
    }

    /// A cell nobody has found reads as nothing under fog and as ordinary known
    /// ground when the presentation-only full-map setting is enabled.
    #[test]
    fn full_map_paints_undiscovered_ground_and_fog_hides_it() {
        let fogged = game(Sight::Corridor, false);
        let revealed = game(Sight::Corridor, true);
        let far = fogged.world.config.exit();
        assert_eq!(paint(&fogged, far), CellPaint::Unknown);
        assert_ne!(paint(&revealed, far), CellPaint::Unknown);
    }

    #[test]
    fn a_unit_stands_on_ground_it_is_holding() {
        let game = game(Sight::Adjacent, false);
        let cell = game.units[&observed_core::PlayerId(0)].cell;
        assert_eq!(paint(&game, cell), CellPaint::Observed);
    }

    /// An anchor outranks everything, because the anchor is the reason a
    /// telegraphed cell will not change.
    #[test]
    fn an_anchor_outranks_every_other_reading() {
        let mut game = game(Sight::Adjacent, false);
        let id = observed_core::PlayerId(0);
        game.apply(id, crate::sim::action::TacticsAction::DeployAnchor)
            .expect("deploys");
        let cell = game.units[&id].cell;
        assert_eq!(paint(&game, cell), CellPaint::Anchored);
    }

    #[test]
    fn every_paint_state_has_a_legend_entry() {
        for state in CellPaint::ALL {
            assert!(!state.legend().is_empty(), "{state:?} has no legend");
        }
        assert_eq!(CellPaint::ALL.len(), 6, "a new state needs a legend entry");
    }
}
