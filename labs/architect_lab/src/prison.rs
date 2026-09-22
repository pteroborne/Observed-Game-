//! Prison core maze and self-escape simulation mechanics.
//!
//! A Guardian catch sends its target to the lowest level of the prison core
//! at the facility's horizontal center. Jailed Observers remain embodied and
//! loyal, navigating the multi-step vertical maze to self-escape back into
//! the facility.

use observed_facility::hex_wfc::{
    HexArchetype, HexCoord, HexFace, HexPlacement, HexSpace, HexWfcConfig, HexWfcWorld, PortClass,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The prison core state and internal maze topology.
#[derive(Clone, Debug, PartialEq)]
pub struct PrisonState {
    /// All cells belonging to the prison core. Protected against collapse,
    /// retraction, architect play, and guardian entry.
    pub cells: BTreeSet<HexCoord>,
    /// The lowest cell where captured observers are deposited (level 0).
    pub lowest_cell: HexCoord,
    /// The exit threshold inside the core.
    pub exit_threshold: HexCoord,
    /// The facility cell outside the prison core where escaped observers emerge.
    pub facility_exit: HexCoord,
    /// Adjacency graph: from_cell -> map of HexFace to neighbor cell.
    pub graph: BTreeMap<HexCoord, BTreeMap<HexFace, HexCoord>>,
    /// Precomputed deterministic shortest escape path from each prison cell to facility_exit.
    pub escape_routes: BTreeMap<HexCoord, Vec<HexCoord>>,
}

impl PrisonState {
    /// Builds the deterministic prison core maze at the facility's horizontal center.
    pub fn new(config: HexWfcConfig, world: &HexWfcWorld) -> Self {
        let cq = config.cols / 2;
        let cr = config.rows / 2;

        let mut cells = BTreeSet::new();
        let mut graph: BTreeMap<HexCoord, BTreeMap<HexFace, HexCoord>> = BTreeMap::new();
        let lowest_cell: HexCoord;
        let exit_threshold: HexCoord;

        if config.levels >= 2 {
            // Multi-level vertical maze:
            // Level 0: Holding cell -> Corridors -> Junction (with DeadEnd branch) -> Shaft bottom
            let c_lowest = HexCoord {
                q: cq,
                r: cr,
                level: 0,
            };
            let c_corr1 = HexCoord {
                q: cq - 1,
                r: cr,
                level: 0,
            };
            let c_corr2 = HexCoord {
                q: cq - 1,
                r: cr + 1,
                level: 0,
            };
            let c_junc0 = HexCoord {
                q: cq,
                r: cr + 1,
                level: 0,
            };
            let c_dead0 = HexCoord {
                q: cq,
                r: cr + 2,
                level: 0,
            };
            let c_shaft0 = HexCoord {
                q: cq + 1,
                r: cr,
                level: 0,
            };

            // Level 1: Shaft top -> Junction (with DeadEnd branch) -> Corridor -> Exit threshold
            let c_shaft1 = HexCoord {
                q: cq + 1,
                r: cr,
                level: 1,
            };
            let c_junc1 = HexCoord {
                q: cq,
                r: cr,
                level: 1,
            };
            let c_dead1 = HexCoord {
                q: cq,
                r: cr - 1,
                level: 1,
            };
            let c_corr3 = HexCoord {
                q: cq - 1,
                r: cr,
                level: 1,
            };
            let c_exit = HexCoord {
                q: cq - 1,
                r: cr + 1,
                level: 1,
            };

            lowest_cell = c_lowest;
            exit_threshold = c_exit;

            cells.insert(c_lowest);
            cells.insert(c_corr1);
            cells.insert(c_corr2);
            cells.insert(c_junc0);
            cells.insert(c_dead0);
            cells.insert(c_shaft0);
            cells.insert(c_shaft1);
            cells.insert(c_junc1);
            cells.insert(c_dead1);
            cells.insert(c_corr3);
            cells.insert(c_exit);

            // Level 0 connections:
            add_bidirectional_edge(&mut graph, c_lowest, HexFace::West, c_corr1, HexFace::East);
            add_bidirectional_edge(
                &mut graph,
                c_corr1,
                HexFace::SouthEast,
                c_corr2,
                HexFace::NorthWest,
            );
            add_bidirectional_edge(&mut graph, c_corr2, HexFace::East, c_junc0, HexFace::West);
            add_bidirectional_edge(
                &mut graph,
                c_junc0,
                HexFace::SouthEast,
                c_dead0,
                HexFace::NorthWest,
            );
            add_bidirectional_edge(
                &mut graph,
                c_junc0,
                HexFace::NorthEast,
                c_shaft0,
                HexFace::SouthWest,
            );

            // Vertical Shaft connection (level 0 to level 1):
            add_bidirectional_edge(&mut graph, c_shaft0, HexFace::Up, c_shaft1, HexFace::Down);

            // Level 1 connections:
            add_bidirectional_edge(&mut graph, c_shaft1, HexFace::West, c_junc1, HexFace::East);
            add_bidirectional_edge(
                &mut graph,
                c_junc1,
                HexFace::NorthWest,
                c_dead1,
                HexFace::SouthEast,
            );
            add_bidirectional_edge(&mut graph, c_junc1, HexFace::West, c_corr3, HexFace::East);
            add_bidirectional_edge(
                &mut graph,
                c_corr3,
                HexFace::SouthEast,
                c_exit,
                HexFace::NorthWest,
            );
        } else {
            // Single-level lateral maze (for Pocket mode):
            let c_lowest = HexCoord {
                q: cq - 1,
                r: cr,
                level: 0,
            }; // (2, 2)
            let c_corr1 = HexCoord {
                q: cq - 1,
                r: cr + 1,
                level: 0,
            }; // (2, 3)
            let c_junc0 = HexCoord {
                q: cq,
                r: cr + 1,
                level: 0,
            }; // (3, 3)
            let c_dead0 = HexCoord {
                q: cq,
                r: cr + 2,
                level: 0,
            }; // (3, 4)
            let c_corr2 = HexCoord {
                q: cq,
                r: cr,
                level: 0,
            }; // (3, 2)
            let c_corr3 = HexCoord {
                q: cq,
                r: cr - 1,
                level: 0,
            }; // (3, 1)
            let c_exit = HexCoord {
                q: cq - 1,
                r: cr - 1,
                level: 0,
            }; // (2, 1)

            lowest_cell = c_lowest;
            exit_threshold = c_exit;

            cells.insert(c_lowest);
            cells.insert(c_corr1);
            cells.insert(c_junc0);
            cells.insert(c_dead0);
            cells.insert(c_corr2);
            cells.insert(c_corr3);
            cells.insert(c_exit);

            add_bidirectional_edge(
                &mut graph,
                c_lowest,
                HexFace::SouthEast,
                c_corr1,
                HexFace::NorthWest,
            );
            add_bidirectional_edge(&mut graph, c_corr1, HexFace::East, c_junc0, HexFace::West);
            add_bidirectional_edge(
                &mut graph,
                c_junc0,
                HexFace::SouthEast,
                c_dead0,
                HexFace::NorthWest,
            );
            add_bidirectional_edge(
                &mut graph,
                c_junc0,
                HexFace::NorthWest,
                c_corr2,
                HexFace::SouthEast,
            );
            add_bidirectional_edge(
                &mut graph,
                c_corr2,
                HexFace::NorthWest,
                c_corr3,
                HexFace::SouthEast,
            );
            add_bidirectional_edge(&mut graph, c_corr3, HexFace::West, c_exit, HexFace::East);
        }

        // Determine facility exit: a solid cell adjacent to exit_threshold outside prison core
        let mut candidates = Vec::new();
        for face in HexFace::LATERAL {
            let Some(neighbor) = config.grid().neighbor(exit_threshold, face) else {
                continue;
            };
            if !cells.contains(&neighbor) {
                let is_solid = world
                    .placements
                    .get(&neighbor)
                    .is_some_and(|p| p.space.built());
                candidates.push((is_solid, face, neighbor));
            }
        }
        // Prefer solid cell in facility; if multiple, pick deterministically by coordinate
        candidates.sort_by_key(|&(solid, _face, coord)| (!solid, coord.q, coord.r));
        let (exit_face, facility_exit) = candidates
            .first()
            .map(|&(_solid, face, coord)| (face, coord))
            .unwrap_or_else(|| {
                let fallback = HexCoord {
                    q: exit_threshold.q.saturating_sub(1),
                    r: exit_threshold.r,
                    level: exit_threshold.level,
                };
                (HexFace::West, fallback)
            });

        // Add one-way boundary step out of the prison core
        graph
            .entry(exit_threshold)
            .or_default()
            .insert(exit_face, facility_exit);

        // Precompute deterministic escape routes using BFS
        let mut escape_routes = BTreeMap::new();
        for &start in &cells {
            if let Some(route) = find_shortest_escape_path(start, facility_exit, &graph) {
                escape_routes.insert(start, route);
            }
        }

        Self {
            cells,
            lowest_cell,
            exit_threshold,
            facility_exit,
            graph,
            escape_routes,
        }
    }

    /// Returns the deterministic next escape step for an observer at `from`.
    #[must_use]
    pub fn next_escape_step(&self, from: HexCoord) -> Option<HexCoord> {
        self.escape_routes
            .get(&from)
            .and_then(|route| route.first().copied())
    }

    /// Returns the full deterministic escape route from `from` to the facility exit.
    #[must_use]
    pub fn escape_route(&self, from: HexCoord) -> Option<&[HexCoord]> {
        self.escape_routes.get(&from).map(|v| v.as_slice())
    }

    /// Injects solid placements into the world for all prison core cells so presentation
    /// renders them with the canonical prison theme.
    pub fn ensure_placements(&self, world: &mut HexWfcWorld) {
        for &cell in &self.cells {
            world
                .placements
                .entry(cell)
                .and_modify(|p| {
                    if p.space.unbuilt() {
                        p.space = HexSpace::Hall;
                        p.archetype = HexArchetype::Straight;
                    }
                })
                .or_insert_with(|| HexPlacement {
                    coord: cell,
                    space: HexSpace::Hall,
                    doors: 0,
                    up: PortClass::Sealed,
                    down: PortClass::Sealed,
                    archetype: HexArchetype::Straight,
                });
        }
    }
}

fn add_bidirectional_edge(
    graph: &mut BTreeMap<HexCoord, BTreeMap<HexFace, HexCoord>>,
    a: HexCoord,
    face_a_to_b: HexFace,
    b: HexCoord,
    face_b_to_a: HexFace,
) {
    graph.entry(a).or_default().insert(face_a_to_b, b);
    graph.entry(b).or_default().insert(face_b_to_a, a);
}

/// Finds the deterministic shortest path from `start` to `destination` using BFS.
/// Returns the sequence of steps from start (excluding `start`, ending at `destination`).
fn find_shortest_escape_path(
    start: HexCoord,
    destination: HexCoord,
    graph: &BTreeMap<HexCoord, BTreeMap<HexFace, HexCoord>>,
) -> Option<Vec<HexCoord>> {
    if start == destination {
        return Some(Vec::new());
    }

    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();
    let mut parent = BTreeMap::new();

    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        if current == destination {
            let mut path = Vec::new();
            let mut curr = destination;
            while curr != start {
                path.push(curr);
                curr = parent[&curr];
            }
            path.reverse();
            return Some(path);
        }

        if let Some(neighbors) = graph.get(&current) {
            // Sort neighbor transitions deterministically by face index
            let mut transitions: Vec<_> = neighbors.iter().collect();
            transitions.sort_by_key(|(face, coord)| (face.index(), coord.q, coord.r, coord.level));

            for (_face, &next) in transitions {
                if visited.insert(next) {
                    parent.insert(next, current);
                    queue.push_back(next);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ArchitectMode;

    #[test]
    fn prison_state_constructs_for_all_modes() {
        for mode in ArchitectMode::ALL {
            let config = mode.config();
            let world = HexWfcWorld::generate(mode.seed(), config).unwrap();
            let prison = PrisonState::new(config, &world);

            assert!(!prison.cells.is_empty());
            assert_eq!(prison.lowest_cell.level, 0, "lowest cell is at level 0");
            assert!(
                !prison.cells.contains(&prison.facility_exit),
                "facility exit is outside prison"
            );

            let route = prison
                .escape_route(prison.lowest_cell)
                .expect("valid escape route exists");
            assert!(
                route.len() >= 6,
                "escape is a real traversal ({} steps in {:?})",
                route.len(),
                mode
            );
            assert_eq!(*route.last().unwrap(), prison.facility_exit);
        }
    }

    #[test]
    fn dead_ends_and_junctions_exist_for_genuine_maze_difficulty() {
        let mode = ArchitectMode::FullAscent;
        let config = mode.config();
        let world = HexWfcWorld::generate(mode.seed(), config).unwrap();
        let prison = PrisonState::new(config, &world);

        // The maze contains cells that have degree 1 (dead ends)
        let dead_ends: Vec<_> = prison
            .cells
            .iter()
            .filter(|&&cell| prison.graph.get(&cell).map_or(0, |edges| edges.len()) == 1)
            .collect();
        assert!(!dead_ends.is_empty(), "prison maze contains dead ends");

        // The maze contains junctions with degree > 2
        let junctions: Vec<_> = prison
            .cells
            .iter()
            .filter(|&&cell| prison.graph.get(&cell).map_or(0, |edges| edges.len()) > 2)
            .collect();
        assert!(!junctions.is_empty(), "prison maze contains junctions");
    }

    #[test]
    fn escape_routing_is_strictly_deterministic() {
        let mode = ArchitectMode::FullAscent;
        let config = mode.config();
        let world = HexWfcWorld::generate(mode.seed(), config).unwrap();
        let prison_a = PrisonState::new(config, &world);
        let prison_b = PrisonState::new(config, &world);

        assert_eq!(prison_a.escape_routes, prison_b.escape_routes);
        for &cell in &prison_a.cells {
            assert_eq!(
                prison_a.next_escape_step(cell),
                prison_b.next_escape_step(cell)
            );
        }
    }

    #[test]
    fn catch_sends_target_to_the_lowest_prison_level() {
        use crate::sim::{ArchitectLab, ObserverId, ObserverState};

        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");
        let id = ObserverId(0);

        // Position observer on the top level before capture
        lab.observers.get_mut(&id).unwrap().cell = HexCoord {
            q: 5,
            r: 4,
            level: 1,
        };
        lab.observers.get_mut(&id).unwrap().state = ObserverState::Active;

        lab.jail(id);

        let jailed = &lab.observers[&id];
        assert_eq!(
            jailed.cell.level, 0,
            "catch sends target to floor 0, not to where it was caught or nearest floor"
        );
        assert_eq!(
            jailed.cell, lab.prison.lowest_cell,
            "target is deposited at the lowest holding cell"
        );
        assert_eq!(jailed.state, ObserverState::Jailed);
    }

    #[test]
    fn jailed_observer_remains_embodied_and_loyal() {
        use crate::sim::{ArchitectLab, ObserverId, ObserverState};

        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");
        let id = ObserverId(0);
        let initial_roster_count = lab.observers.len();

        lab.jail(id);

        // 1. Not removed from roster:
        assert_eq!(
            lab.observers.len(),
            initial_roster_count,
            "jailed observer is not removed from the match roster"
        );
        assert!(
            lab.observers.contains_key(&id),
            "jailed observer still exists under its original ObserverId"
        );

        // 2. Still embodied:
        let observer = &lab.observers[&id];
        assert_eq!(observer.id, id);
        assert_eq!(observer.state, ObserverState::Jailed);
        assert!(
            lab.prison_core.contains(&observer.cell),
            "jailed observer occupies a concrete hex inside the prison core"
        );
        assert!(
            lab.world.placements.contains_key(&observer.cell),
            "jailed observer's coordinate exists in world placements"
        );
    }

    #[test]
    fn bot_observer_tree_navigates_prison_maze_and_self_escapes_back_to_active() {
        use crate::sim::{ArchitectLab, MatchOutcome, ObserverId, ObserverIntent, ObserverState};

        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");
        let id = ObserverId(0);
        lab.guardians.clear(); // remove guardians to isolate escape behavior

        lab.jail(id);
        assert_eq!(lab.observers[&id].cell, lab.prison.lowest_cell);
        assert_eq!(lab.observers[&id].state, ObserverState::Jailed);

        let mut steps_taken = 0;
        let max_beats = 20;

        for _beat in 0..max_beats {
            if lab.observers[&id].state == ObserverState::Active {
                break;
            }

            let (intent, trace) = lab.observer_intent(id);
            assert_eq!(
                trace.selected,
                Some("escape jail"),
                "bot observer tree evaluates 'escape jail' priority while jailed"
            );

            let ObserverIntent::Step(next) = intent else {
                panic!("expected Step intent while escaping, got {:?}", intent);
            };

            // Intent moves body through authoritative apply_observer_intent
            lab.apply_observer_intent(id, intent);
            steps_taken += 1;

            assert_eq!(lab.observers[&id].cell, next);
        }

        // Must have successfully escaped back to Active:
        assert_eq!(
            lab.observers[&id].state,
            ObserverState::Active,
            "jailed observer successfully escaped back to Active"
        );
        assert_eq!(
            lab.observers[&id].cell, lab.prison.facility_exit,
            "escaping observer emerged at the facility exit boundary"
        );
        assert!(
            steps_taken >= 6,
            "escape required a real multi-step traversal (took {} steps), not an adjacency check",
            steps_taken
        );
        assert_eq!(
            lab.outcome,
            MatchOutcome::Running,
            "match outcome remains Running after escape"
        );

        // Next beat: observer is now Active, so priority 1 "escape jail" no longer fires
        let (_intent, trace) = lab.observer_intent(id);
        assert_ne!(
            trace.selected,
            Some("escape jail"),
            "active observer no longer selects 'escape jail'"
        );
    }

    #[test]
    fn all_jailed_triggers_rogue_victory_while_single_jailed_permits_escape() {
        use crate::sim::{ArchitectLab, MatchOutcome, ObserverId, ObserverState};

        // 1. Single jailed observer does not cause rogue victory
        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");
        lab.jail(ObserverId(0));
        lab.refresh_observation();
        lab.step_beat();
        assert_eq!(
            lab.outcome,
            MatchOutcome::Running,
            "single jailed observer allows match to continue and escape to proceed"
        );

        // 2. Both jailed observers triggers rogue victory
        lab.jail(ObserverId(1));
        lab.refresh_observation();
        lab.step_beat();
        assert_eq!(
            lab.outcome,
            MatchOutcome::RogueVictory,
            "when all observers are jailed simultaneously, rogue architect wins immediately"
        );
        assert!(
            lab.observers
                .values()
                .all(|o| o.state == ObserverState::Jailed),
            "all observers are jailed in the core"
        );
    }

    #[test]
    fn floor_collapse_never_removes_or_rewrites_the_prison_core() {
        use crate::sim::ArchitectLab;
        use observed_facility::hex_wfc::HexSpace;

        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");
        let core_cells: Vec<_> = lab.prison.cells.iter().copied().collect();
        assert!(!core_cells.is_empty());

        // Void all non-prison cells on level 0 to trigger floor collapse
        for (&cell, tile) in &mut lab.world.placements {
            if cell.level == 0 && !lab.prison_core.contains(&cell) {
                tile.space = HexSpace::Void;
                tile.doors = 0;
            }
        }
        lab.observed.clear();
        lab.anchored.clear();
        lab.guardians.clear();
        lab.observers.clear();
        lab.doors.clear();

        // Target a retraction on level 0
        let target = HexCoord {
            q: 0,
            r: 0,
            level: 0,
        };
        lab.contradictions = BTreeSet::from([target]);
        lab.next_retraction_tick = Some(crate::sim::RETRACTION_TICKS);
        lab.tick = crate::sim::RETRACTION_TICKS;
        lab.advance_retraction();

        // Floor 0 collapsed:
        assert!(
            lab.collapsed_floors.contains(&0),
            "floor 0 collapsed permanently"
        );

        // Verify every prison core cell survived untouched
        for cell in core_cells {
            assert!(
                lab.prison_core.contains(&cell),
                "prison core still contains cell {:?}",
                cell
            );
            assert!(
                lab.retraction_protected(cell),
                "prison core cell {:?} is retraction protected",
                cell
            );
            let placement = lab
                .world
                .placements
                .get(&cell)
                .expect("cell exists in world");
            assert_ne!(
                placement.space,
                HexSpace::Void,
                "prison core cell {:?} was not voided by floor collapse",
                cell
            );
        }
    }

    #[test]
    fn identical_snapshot_reproduces_identical_tree_intent_and_trace_for_jailed_observer() {
        use crate::sim::{ArchitectLab, ObserverId};

        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");
        lab.jail(ObserverId(0));

        let (intent_a, trace_a) = lab.observer_intent(ObserverId(0));
        let (intent_b, trace_b) = lab.observer_intent(ObserverId(0));

        assert_eq!(intent_a, intent_b);
        assert_eq!(trace_a.visited, trace_b.visited);
        assert_eq!(trace_a.selected, trace_b.selected);
        assert_eq!(trace_a.selected, Some("escape jail"));
    }

    #[test]
    fn escape_simulation_is_deterministic_from_seed_and_ordered_commands() {
        use crate::sim::{ArchitectLab, ObserverId};

        let mut lab1 =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");
        let mut lab2 =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");

        lab1.guardians.clear();
        lab2.guardians.clear();

        lab1.jail(ObserverId(0));
        lab2.jail(ObserverId(0));

        for _ in 0..12 {
            lab1.step_beat();
            lab2.step_beat();

            let obs1 = &lab1.observers[&ObserverId(0)];
            let obs2 = &lab2.observers[&ObserverId(0)];

            assert_eq!(
                obs1.cell, obs2.cell,
                "observer position matches deterministically"
            );
            assert_eq!(
                obs1.facing, obs2.facing,
                "observer facing matches deterministically"
            );
            assert_eq!(
                obs1.state, obs2.state,
                "observer state matches deterministically"
            );
        }
    }

    #[test]
    fn all_three_reset_paths_clear_and_recreate_prison_state_without_leaking() {
        use crate::desktop::LabSession;
        use crate::sim::{ArchitectLab, ObserverId, ObserverState};

        // 1. Sim direct reset / recreation
        let mut sim = ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("sim boots");
        sim.jail(ObserverId(0));
        assert_eq!(sim.observers[&ObserverId(0)].state, ObserverState::Jailed);

        sim = ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("sim resets");
        assert_eq!(
            sim.observers[&ObserverId(0)].state,
            ObserverState::Active,
            "reset restored fresh ObserverState"
        );
        assert_eq!(sim.prison_core.len(), 11);

        // 2. Desktop session reset
        let mut desktop = LabSession::default();
        desktop.sim.jail(ObserverId(0));
        assert_eq!(
            desktop.sim.observers[&ObserverId(0)].state,
            ObserverState::Jailed
        );

        desktop.reset();
        assert_eq!(
            desktop.sim.observers[&ObserverId(0)].state,
            ObserverState::Active,
            "desktop reset restored active observers"
        );
        assert!(!desktop.sim.prison_core.is_empty());

        // 3. Web session reset (gated by web feature if present)
        #[cfg(feature = "web")]
        {
            let mut web = crate::web::RogueGame::new(2).expect("web boots");
            web.sim.jail(ObserverId(0));
            assert_eq!(
                web.sim.observers[&ObserverId(0)].state,
                ObserverState::Jailed
            );

            web.reset(2).expect("web reset");
            assert_eq!(
                web.sim.observers[&ObserverId(0)].state,
                ObserverState::Active,
                "web reset restored active observers"
            );
            assert_eq!(web.sim.prison_core.len(), 11);
        }
    }
}
