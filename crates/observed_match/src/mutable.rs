//! Phase 10 integration: a **mutable facility**. It folds the proven
//! observation (`observation_lab`) and constraint-spine (`constraint_lab`)
//! systems into a facility with an objective — move the team and the power cell
//! to the exit — and shows the team can still finish *while the unobserved
//! structure rewires behind them*.
//!
//! The connection layer is reused wholesale via `constraint_lab::ConstraintWorld`
//! (9-room graph + protected spine + observation-aware decoherence). The team's
//! occupied rooms drive the observation set, so the rooms they stand in freeze
//! while the rest decoheres; the protected spine keeps the exit reachable, so
//! following it always works.

use observed_core::RoomId;
use observed_facility::constraints::ConstraintWorld;
use observed_observation::Side;

pub const TEAM_SIZE: usize = 4;
pub const START_ROOM: u32 = 0;
pub const EXIT_ROOM: u32 = 8;

/// The next room + doorway along the protected spine toward the exit. Matches the
/// spanning path `constraint_lab` protects (0-1-2-5-4-3-6-7-8).
pub fn spine_next(room: RoomId) -> Option<(RoomId, Side)> {
    let (next, side) = match room.0 {
        0 => (1, Side::East),
        1 => (2, Side::East),
        2 => (5, Side::South),
        5 => (4, Side::West),
        4 => (3, Side::West),
        3 => (6, Side::South),
        6 => (7, Side::East),
        7 => (8, Side::East),
        _ => return None,
    };
    Some((RoomId(next), side))
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct MutableFacility {
    /// The reused connection layer: graph + protected spine + observation.
    pub structure: ConstraintWorld,
    pub steps: u32,
    pub decohere_count: u32,
    pub objective_complete: bool,
    pub last_event: String,
}

impl MutableFacility {
    pub fn authored() -> Self {
        let mut structure = ConstraintWorld::authored();
        // The team starts at the entrance; their rooms are the observation set.
        structure.graph.players = vec![RoomId(START_ROOM); TEAM_SIZE];
        structure.recompute_connectivity();
        Self {
            structure,
            steps: 0,
            decohere_count: 0,
            objective_complete: false,
            last_event: "Move the team and the cell to the exit while the structure shifts."
                .to_string(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    pub fn team_rooms(&self) -> &Vec<RoomId> {
        &self.structure.graph.players
    }

    /// The power cell rides with the lead member.
    pub fn cell_room(&self) -> RoomId {
        self.structure.graph.players[0]
    }

    pub fn at_exit(&self) -> usize {
        self.structure
            .graph
            .players
            .iter()
            .filter(|r| r.0 == EXIT_ROOM)
            .count()
    }

    pub fn connected(&self) -> bool {
        self.structure.connected
    }

    fn update_objective(&mut self) {
        let done = self
            .structure
            .graph
            .players
            .iter()
            .all(|r| r.0 == EXIT_ROOM);
        if done && !self.objective_complete {
            self.last_event =
                "Objective complete — the team and cell reached the exit.".to_string();
        }
        self.objective_complete = done;
    }

    /// Advance every team member one step along the protected spine. Because the
    /// spine is never rewired, this works no matter how the rest has decohered.
    pub fn advance(&mut self) {
        for index in 0..self.structure.graph.players.len() {
            let room = self.structure.graph.players[index];
            if let Some((_next, side)) = spine_next(room) {
                self.structure.traverse(index, side);
            }
        }
        self.steps += 1;
        self.update_objective();
    }

    /// Human traversal: move one member through the doorway on `side`.
    pub fn step_member(&mut self, member: usize, side: Side) -> bool {
        let moved = self.structure.traverse(member, side);
        self.update_objective();
        moved
    }

    /// Rewire the unobserved structure (the spine and occupied rooms hold).
    pub fn decohere(&mut self) {
        self.structure.decohere();
        self.decohere_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spine_door_links(
        facility: &MutableFacility,
    ) -> Vec<(observed_observation::DoorId, observed_observation::DoorId)> {
        // The doorways along the spine and their current partners.
        let mut out = Vec::new();
        let mut room = RoomId(START_ROOM);
        while let Some((next, side)) = spine_next(room) {
            let door = facility.structure.graph.door_id(room, side);
            out.push((door, facility.structure.graph.partner(door)));
            room = next;
        }
        out
    }

    #[test]
    fn authored_team_starts_at_the_entrance_and_the_exit_is_reachable() {
        let facility = MutableFacility::authored();
        assert_eq!(facility.team_rooms().len(), TEAM_SIZE);
        assert!(facility.team_rooms().iter().all(|r| r.0 == START_ROOM));
        assert!(facility.connected());
        assert!(!facility.objective_complete);
    }

    #[test]
    fn the_team_reaches_the_exit_while_the_structure_shifts() {
        let mut facility = MutableFacility::authored();
        let initial_links = facility.structure.graph.links.clone();

        for _ in 0..40 {
            if facility.objective_complete {
                break;
            }
            facility.advance();
            facility.decohere();
            assert!(
                facility.connected(),
                "the spine must keep the exit reachable"
            );
        }

        assert!(
            facility.objective_complete,
            "the team completes the objective"
        );
        assert_eq!(facility.at_exit(), TEAM_SIZE);
        assert_eq!(facility.cell_room().0, EXIT_ROOM);
        assert_ne!(
            facility.structure.graph.links, initial_links,
            "the unobserved structure shifted during the run"
        );
    }

    #[test]
    fn the_spine_never_rewires_through_decoherence() {
        let mut facility = MutableFacility::authored();
        let spine = spine_door_links(&facility);
        for _ in 0..30 {
            facility.decohere();
        }
        for (door, partner) in spine {
            assert_eq!(
                facility.structure.graph.partner(door),
                partner,
                "a spine doorway must never rewire"
            );
        }
    }

    #[test]
    fn the_occupied_room_freezes_while_the_rest_shifts() {
        // Move the team to room 1 so room 1 is observed.
        let mut facility = MutableFacility::authored();
        facility.advance(); // team 0 -> 1
        let occupied = facility.team_rooms()[0];
        assert_eq!(occupied.0, 1);

        let watched: Vec<_> = Side::ALL
            .iter()
            .map(|side| {
                let d = facility.structure.graph.door_id(occupied, *side);
                (d, facility.structure.graph.partner(d))
            })
            .collect();
        let before = facility.structure.graph.links.clone();
        facility.decohere();
        for (door, partner) in watched {
            assert_eq!(
                facility.structure.graph.partner(door),
                partner,
                "observed room frozen"
            );
        }
        assert_ne!(
            facility.structure.graph.links, before,
            "the rest still shifts"
        );
    }

    #[test]
    fn the_run_is_deterministic() {
        let mut a = MutableFacility::authored();
        let mut b = MutableFacility::authored();
        for _ in 0..8 {
            a.advance();
            a.decohere();
            b.advance();
            b.decohere();
        }
        assert_eq!(a.structure.graph.links, b.structure.graph.links);
        assert_eq!(a.team_rooms(), b.team_rooms());
    }

    #[test]
    fn reset_restores_the_entrance() {
        let mut facility = MutableFacility::authored();
        for _ in 0..5 {
            facility.advance();
            facility.decohere();
        }
        facility.reset();
        assert!(facility.team_rooms().iter().all(|r| r.0 == START_ROOM));
        assert_eq!(facility.steps, 0);
        assert!(!facility.objective_complete);
    }
}
