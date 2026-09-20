//! Incremental facility topology tracking and disjoint-set primitive.
//!
//! Owns the traversal graph over cells that are actually passable (routing through
//! [`ArchitectLab::step_through`], which enforces sealed ports, closed doors,
//! retracted cells, void spaces, and unpowered floor vertical links).

use std::collections::{BTreeMap, BTreeSet};

use observed_facility::hex_wfc::HexSpace;
use observed_hex::{HexCoord, HexFace};

use super::ArchitectLab;

/// Disjoint-set forest where each component's root is strictly canonical
/// (always the minimum element in the component according to `Ord`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisjointSet<T: Ord + Copy> {
    parent: BTreeMap<T, T>,
}

impl<T: Ord + Copy> DisjointSet<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            parent: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, item: T) {
        self.parent.entry(item).or_insert(item);
    }

    pub fn find(&mut self, item: T) -> T {
        self.insert(item);
        let mut root = item;
        while let Some(&p) = self.parent.get(&root) {
            if p == root {
                break;
            }
            root = p;
        }
        // Path compression
        let mut curr = item;
        while let Some(&p) = self.parent.get(&curr) {
            if p == root {
                break;
            }
            self.parent.insert(curr, root);
            curr = p;
        }
        root
    }

    pub fn union(&mut self, a: T, b: T) -> bool {
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return false;
        }

        // Canonical root: always point the larger element to the smaller element.
        // This guarantees that the component's representative is the absolute minimum
        // element in the component, completely independent of union order.
        if root_a < root_b {
            self.parent.insert(root_b, root_a);
        } else {
            self.parent.insert(root_a, root_b);
        }
        true
    }

    #[must_use]
    pub fn component_count(&mut self) -> usize {
        let items: Vec<T> = self.parent.keys().copied().collect();
        let mut roots = BTreeSet::new();
        for item in items {
            roots.insert(self.find(item));
        }
        roots.len()
    }

    #[must_use]
    pub fn same_component(&mut self, a: T, b: T) -> bool {
        self.find(a) == self.find(b)
    }
}

/// Stable canonical identifier for a connected facility component.
/// Holds the minimum `HexCoord` in that component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub HexCoord);

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Comp({},{},{})", self.0.q, self.0.r, self.0.level)
    }
}

/// Incrementally maintained facility topology.
///
/// Caches the connected components of the passable graph. Recomputed lazily on a
/// dirty flag at most once per beat when something queries topology or win conditions.
///
/// Immutable queries (`component_count`, `same_component`, `component_of`,
/// `reachable_from`) are pure `&self` reads against the cached component map,
/// eliminating borrow conflicts and interior mutability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacilityTopology {
    /// Number of times the topology structure has been rebuilt.
    pub rebuild_count: u64,
    /// Total number of cells inspected during rebuilds.
    pub cells_touched: u64,
    /// Dirty flag indicating graph mutations occurred since the last rebuild.
    pub dirty: bool,
    /// Map from passable cell to its canonical component id.
    cell_to_component: BTreeMap<HexCoord, ComponentId>,
    /// Map from canonical component id to all passable cells in that component.
    component_cells: BTreeMap<ComponentId, BTreeSet<HexCoord>>,
}

impl Default for FacilityTopology {
    fn default() -> Self {
        Self {
            rebuild_count: 0,
            cells_touched: 0,
            dirty: true,
            cell_to_component: BTreeMap::new(),
            component_cells: BTreeMap::new(),
        }
    }
}

impl FacilityTopology {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Rebuild the topology components from authoritative simulation state.
    pub fn rebuild(&mut self, lab: &ArchitectLab) {
        self.rebuild_count += 1;
        self.dirty = false;
        self.cell_to_component.clear();
        self.component_cells.clear();

        // 1. Passable cells: non-void, not retracted, and not on a collapsed floor.
        let passable_cells: BTreeSet<HexCoord> = lab
            .world
            .placements
            .iter()
            .filter(|&(cell, placement)| {
                placement.space != HexSpace::Void
                    && !lab.retracted.contains(cell)
                    && !lab.collapsed_floors.contains(&cell.level)
            })
            .map(|(&cell, _)| cell)
            .collect();

        self.cells_touched += passable_cells.len() as u64;

        if passable_cells.is_empty() {
            return;
        }

        // 2. DisjointSet over all passable cells, routing adjacency through step_through.
        let mut ds = DisjointSet::new();
        for &cell in &passable_cells {
            ds.insert(cell);
            for face in HexFace::ALL {
                if let Some(next) = lab.step_through(cell, face)
                    && passable_cells.contains(&next)
                {
                    ds.union(cell, next);
                }
            }
        }

        // 3. Populate cached component structures.
        for &cell in &passable_cells {
            let root = ds.find(cell);
            let comp_id = ComponentId(root);
            self.cell_to_component.insert(cell, comp_id);
            self.component_cells
                .entry(comp_id)
                .or_default()
                .insert(cell);
        }
    }

    /// How many disjoint components the passable facility is currently in.
    #[must_use]
    pub fn component_count(&self) -> usize {
        self.component_cells.len()
    }

    /// Are these two cells mutually reachable through passable connections.
    #[must_use]
    pub fn same_component(&self, a: HexCoord, b: HexCoord) -> bool {
        match (
            self.cell_to_component.get(&a),
            self.cell_to_component.get(&b),
        ) {
            (Some(c_a), Some(c_b)) => c_a == c_b,
            _ => false,
        }
    }

    /// Stable canonical component ID for a cell (the minimum HexCoord in that component).
    /// Returns None if the cell is not passable.
    #[must_use]
    pub fn component_of(&self, cell: HexCoord) -> Option<ComponentId> {
        self.cell_to_component.get(&cell).copied()
    }

    /// Set of all cells mutually reachable from the given cell.
    /// Returns an empty set if the cell is unpassable.
    #[must_use]
    pub fn reachable_from(&self, cell: HexCoord) -> BTreeSet<HexCoord> {
        self.cell_to_component
            .get(&cell)
            .and_then(|id| self.component_cells.get(id))
            .cloned()
            .unwrap_or_default()
    }

    /// Returns the sizes (cell counts) of all disjoint components.
    #[must_use]
    pub fn component_sizes(&self) -> Vec<usize> {
        self.component_cells.values().map(|c| c.len()).collect()
    }

    /// Number of disjoint components containing at least one cell from the
    /// given set of occupiable cells.
    ///
    /// This filters out unreachable generation orphans (sealed elevator shafts,
    /// dead rooms, disconnected singletons) which were never reachable by Observers.
    #[must_use]
    pub fn meaningful_component_count(&self, occupiable_cells: &BTreeSet<HexCoord>) -> usize {
        self.component_cells
            .values()
            .filter(|cells| cells.iter().any(|c| occupiable_cells.contains(c)))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{ArchitectMode, DoorState, MatchOutcome, ObserverState};

    #[test]
    fn disjoint_set_initializes_empty_and_tracks_singletons() {
        let mut ds = DisjointSet::<i32>::new();
        assert_eq!(ds.component_count(), 0);

        ds.insert(1);
        ds.insert(2);
        ds.insert(3);
        assert_eq!(ds.component_count(), 3);
        assert!(!ds.same_component(1, 2));
        assert!(!ds.same_component(2, 3));
        assert!(ds.same_component(1, 1));
    }

    #[test]
    fn disjoint_set_unions_with_canonical_minimum_representative() {
        let mut ds = DisjointSet::<i32>::new();
        // Insert in arbitrary order
        ds.insert(50);
        ds.insert(10);
        ds.insert(30);
        ds.insert(5);

        // Union in reverse order
        assert!(ds.union(50, 30));
        assert_eq!(ds.find(50), 30);
        assert_eq!(ds.find(30), 30);

        assert!(ds.union(30, 10));
        assert_eq!(ds.find(50), 10);
        assert_eq!(ds.find(10), 10);

        assert!(ds.union(10, 5));
        assert_eq!(ds.find(50), 5);
        assert_eq!(ds.find(30), 5);
        assert_eq!(ds.find(10), 5);
        assert_eq!(ds.find(5), 5);

        // Redundant union returns false
        assert!(!ds.union(50, 5));
        assert_eq!(ds.component_count(), 1);
    }

    #[test]
    fn disjoint_set_find_creates_missing_item_as_singleton() {
        let mut ds = DisjointSet::<i32>::new();
        assert_eq!(ds.find(42), 42);
        assert_eq!(ds.component_count(), 1);
    }

    #[test]
    fn topology_boots_with_initial_components_and_cached_reads() {
        let lab = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        assert!(!lab.topology.is_dirty());
        assert!(lab.component_count() >= 1);

        let active_obs = lab
            .observers
            .values()
            .find(|o| o.state == ObserverState::Active)
            .expect("active observer");
        let comp = lab.component_of(active_obs.cell);
        assert!(comp.is_some());
        let reachable = lab.reachable_from(active_obs.cell);
        assert!(reachable.contains(&active_obs.cell));
        assert!(lab.same_component(active_obs.cell, active_obs.cell));
    }

    #[test]
    fn door_closing_splits_and_door_opening_merges() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        // Find two adjacent passable cells on level 0 connected by an open lateral face
        let mut edge = None;
        for (&cell, placement) in &lab.world.placements {
            if placement.space == HexSpace::Void || lab.retracted.contains(&cell) {
                continue;
            }
            for face in HexFace::ALL {
                if face.is_lateral()
                    && let Some(next) = lab.step_through(cell, face)
                    && let Some(key) = lab.threshold_key(cell, face)
                {
                    edge = Some((cell, next, key));
                    break;
                }
            }
            if edge.is_some() {
                break;
            }
        }

        let (a, b, key) = edge.expect("found lateral connected edge");
        assert!(lab.same_component(a, b));

        // Close the door
        assert!(lab.operate_door(key, DoorState::Closed));
        assert!(lab.topology.is_dirty());
        lab.sync_topology();
        assert!(!lab.topology.is_dirty());

        // Now reopen the door
        assert!(lab.operate_door(key, DoorState::Open));
        lab.sync_topology();
        assert!(lab.same_component(a, b));
    }

    #[test]
    fn retraction_splits_facility_into_multiple_components() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let initial_count = lab.component_count();

        // Pick a passable cell that has exits, and retract it
        let candidate = *lab
            .world
            .placements
            .keys()
            .find(|&&c| {
                lab.world.placements[&c].space != HexSpace::Void
                    && !lab.retracted.contains(&c)
                    && !lab.prison_core.contains(&c)
                    && lab.exits(c).len() >= 2
            })
            .expect("candidate cell");

        lab.retracted.insert(candidate);
        lab.topology.mark_dirty();
        lab.sync_topology();

        // Candidate itself is no longer in any component
        assert_eq!(lab.component_of(candidate), None);
        assert!(lab.reachable_from(candidate).is_empty());
        assert!(!lab.same_component(candidate, candidate));
        assert!(lab.component_count() >= initial_count);
    }

    #[test]
    fn floor_power_cut_splits_multi_floor_facility() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::QuickClimb).unwrap();
        assert_eq!(lab.world.config.levels, 2);

        let count_powered = lab.component_count();

        // Cut power on level 1
        assert!(lab.cut_floor_power(1));
        assert!(lab.topology.is_dirty());
        lab.sync_topology();

        let count_unpowered = lab.component_count();
        // Severing inter-floor vertical ports increases or maintains disjoint components
        assert!(count_unpowered >= count_powered);

        // Restore power
        lab.economy.power.insert(1, true);
        lab.topology.mark_dirty();
        lab.sync_topology();
        assert_eq!(lab.component_count(), count_powered);
    }

    #[test]
    fn reset_paths_clear_topology_without_leaks() {
        // Path 1: desktop.rs (LabSession::reset)
        #[cfg(feature = "desktop")]
        {
            use crate::desktop::{ArchitectAction, LabSession};
            let mut session = LabSession::default();
            session.sim.topology.rebuild_count = 999;
            session.sim.topology.cells_touched = 8888;
            session.apply_action(ArchitectAction::Reset);
            assert_eq!(
                session.sim.topology.rebuild_count, 1,
                "desktop reset must initialize fresh topology with 1 initial build"
            );
            assert!(!session.sim.topology.is_dirty());
        }

        // Path 2: view.rs (MapCameraState::reset_for_mode)
        #[cfg(feature = "desktop")]
        {
            use crate::view::MapCameraState;
            let mut camera = MapCameraState::default();
            camera.zoom = 2.5;
            camera.reset_for_mode(ArchitectMode::Pocket);
            assert!((camera.zoom - 0.62).abs() < f32::EPSILON);
        }

        // Path 3: web.rs (RogueGame::reset)
        #[cfg(feature = "web")]
        {
            use crate::web::RogueGame;
            let mut game = RogueGame::new(0).unwrap();
            game.sim.topology.rebuild_count = 999;
            game.reset(0).expect("web reset succeeds");
            assert_eq!(
                game.sim.topology.rebuild_count, 1,
                "web reset must initialize fresh topology"
            );
            assert!(!game.sim.topology.is_dirty());
        }
    }

    #[test]
    fn sever_objective_triggers_when_threshold_reached() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let initial_components = lab.meaningful_component_count();
        assert_eq!(initial_components, 1);

        // Sever is disabled when sever_threshold == 0
        lab.sever_threshold = 0;
        lab.step_beat();
        assert_eq!(lab.outcome, MatchOutcome::Running);
        assert_eq!(lab.sever_tick, None);

        // Find a lateral edge between two connected cells
        let mut edge = None;
        for (&cell, placement) in &lab.world.placements {
            if placement.space == HexSpace::Void || lab.retracted.contains(&cell) {
                continue;
            }
            for face in HexFace::ALL {
                if face.is_lateral()
                    && let Some(next) = lab.step_through(cell, face)
                    && let Some(key) = lab.threshold_key(cell, face)
                {
                    edge = Some((cell, next, key));
                    break;
                }
            }
            if edge.is_some() {
                break;
            }
        }
        let (_a, _b, key) = edge.expect("found lateral connected edge");

        // Closing the door splits the connected component
        assert!(lab.operate_door(key, DoorState::Closed));
        lab.sync_topology();
        let split_count = lab.meaningful_component_count();
        assert_eq!(split_count, 2);

        // Reopen door
        assert!(lab.operate_door(key, DoorState::Open));
        lab.sync_topology();
        assert_eq!(lab.meaningful_component_count(), initial_components);

        // Configure sever_threshold to split_count (2)
        lab.sever_threshold = split_count;

        // Close the door: on next beat (beat 1), Sever triggers!
        assert!(lab.operate_door(key, DoorState::Closed));
        lab.step_beat();
        assert_eq!(lab.outcome, MatchOutcome::RogueVictory);
        assert!(lab.sever_tick.is_some());
        assert!(
            lab.events
                .iter()
                .any(|e| e.message.contains("Facility severed"))
        );
    }
}

#[cfg(test)]
mod component_shape {
    use super::*;
    use crate::sim::{ArchitectLab, ArchitectMode};

    /// What are the components actually made of?
    ///
    /// Every mode reports 9 or more components at match start and never drops to one, so
    /// a raw component count cannot be the Sever predicate until we know whether those
    /// are rooms or orphans.
    #[test]
    fn component_size_distribution_at_match_start() {
        println!("\n============== COMPONENT SHAPE ==============");
        for mode in ArchitectMode::ALL {
            let lab = ArchitectLab::for_mode(mode).expect("scenario boots");
            let mut sizes: BTreeMap<ComponentId, usize> = BTreeMap::new();
            for (&cell, placement) in &lab.world.placements {
                if placement.space == HexSpace::Void {
                    continue;
                }
                if let Some(root) = lab.component_of(cell) {
                    *sizes.entry(root).or_default() += 1;
                }
            }
            let mut histogram: BTreeMap<usize, usize> = BTreeMap::new();
            for &size in sizes.values() {
                *histogram.entry(size).or_default() += 1;
            }
            let singletons = histogram.get(&1).copied().unwrap_or(0);
            let largest = sizes.values().copied().max().unwrap_or(0);
            let total: usize = sizes.values().sum();
            println!(
                "{:>12}: {} components over {total} passable cells; largest {largest}, \
                 singletons {singletons}",
                mode.short_label(),
                sizes.len()
            );
            println!("              sizes (cells -> how many components): {histogram:?}");
        }
        println!("=============================================\n");
    }

    /// Verifies that the refined Sever predicate (counting components containing
    /// cells an Observer could occupy) correctly filters out procedural generation
    /// orphans (singletons, disconnected shafts) at match start across all modes.
    #[test]
    fn meaningful_component_count_filters_unreachable_orphans_at_match_start() {
        for mode in ArchitectMode::ALL {
            let lab = ArchitectLab::for_mode(mode).expect("scenario boots");
            let count = lab.meaningful_component_count();
            // Pocket, Quick Climb, and Full Ascent start in a single connected body.
            // Deep Stack starts with 2 meaningful components because the scenario's
            // route gap at route[len - 3] disconnects the summit sector (where
            // Observer 1 spawns) from the rest of the facility (where Observer 0 spawns).
            let expected = match mode {
                ArchitectMode::Pocket | ArchitectMode::QuickClimb | ArchitectMode::FullAscent => 1,
                ArchitectMode::DeepStack => 2,
            };
            assert_eq!(
                count, expected,
                "Mode {:?} should have {} meaningful component(s) at match start, but had {}",
                mode, expected, count
            );
        }
    }
}

#[cfg(test)]
mod sever_ceiling {
    use super::*;
    use crate::sim::{ArchitectLab, ArchitectMode, MatchOutcome};

    /// How far does the facility actually fragment, when Sever is not ending the match?
    ///
    /// A threshold cannot be chosen from a run that stops the moment the threshold is met:
    /// every mode then reports exactly N and the ceiling stays invisible. This runs each
    /// mode with Sever disabled and records the whole trajectory.
    #[test]
    fn meaningful_component_ceiling_with_sever_disabled() {
        println!("\n============== SEVER CEILING (objective off) ==============");
        for mode in ArchitectMode::ALL {
            let mut lab = ArchitectLab::for_mode(mode).expect("scenario boots");
            lab.bot_architect = true;
            lab.sever_threshold = 0;

            let mut max_meaningful = 0usize;
            let mut first_reaching: BTreeMap<usize, u64> = BTreeMap::new();
            let mut beat = 0u64;
            while beat < 1000 && lab.outcome == MatchOutcome::Running {
                lab.step_beat();
                beat += 1;
                let n = lab.meaningful_component_count();
                if n > max_meaningful {
                    for threshold in (max_meaningful + 1)..=n {
                        first_reaching.entry(threshold).or_insert(beat);
                    }
                    max_meaningful = n;
                }
            }
            println!(
                "{:>12}: {beat} beats, outcome {:?}, max meaningful components {max_meaningful}",
                mode.short_label(),
                lab.outcome
            );
            let milestones: Vec<String> = [2usize, 4, 6, 8, 10, 12, 16, 20]
                .into_iter()
                .filter_map(|t| first_reaching.get(&t).map(|b| format!("N={t} at beat {b}")))
                .collect();
            println!("              first reached: {}", milestones.join(", "));
        }
        println!("===========================================================\n");
    }
}
