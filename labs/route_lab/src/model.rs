//! Pure-logic feasibility model for **persistent player routes** — deepening the
//! authored spine of `constraint_lab` into routes the *players* build.
//!
//! The structure still rewires when unobserved (reusing `observation_lab`'s
//! graph). A team can lay a **cable** on a doorway to pin its current connection,
//! so that route survives every decoherence — a reliable highway through the
//! churn. Cables are budget-limited per team and **contestable**: an opponent can
//! cut one, after which the doorways are free to rewire again. The owner can
//! recover their own cable to reclaim the budget.

use bevy::prelude::*;
use observation_lab::model::{DOOR_COUNT, DoorId, ObservationWorld};
use observed_core::{SplitMix, TeamId};

pub const TEAM_COUNT: usize = 2;
pub const CABLE_CAPACITY: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CableId(pub u16);

#[derive(Clone, Copy, Debug)]
pub struct Cable {
    pub id: CableId,
    pub owner: TeamId,
    pub a: DoorId,
    pub b: DoorId,
}

#[derive(Resource, Clone, Debug)]
pub struct RouteWorld {
    /// The mutable connection graph reused from `observation_lab` (no observers —
    /// persistence here comes from cables, not from watching).
    pub graph: ObservationWorld,
    pub teams: Vec<TeamId>,
    /// Remaining cable budget per team (parallel to `teams`).
    pub budget: Vec<u8>,
    pub cables: Vec<Cable>,
    pub next_cable: u16,
    pub base_seed: u64,
    pub decohere_count: u32,
    pub contests: u32,
    pub last_event: String,
}

impl RouteWorld {
    pub fn authored() -> Self {
        let mut graph = ObservationWorld::authored();
        // No observers: every doorway is free unless a cable pins it.
        graph.players.clear();
        let teams: Vec<TeamId> = (0..TEAM_COUNT).map(|i| TeamId(i as u8)).collect();
        Self {
            budget: vec![CABLE_CAPACITY; TEAM_COUNT],
            graph,
            teams,
            cables: Vec::new(),
            next_cable: 0,
            base_seed: 0x520C_AB1E_1234_5678,
            decohere_count: 0,
            contests: 0,
            last_event: "Lay cable on a doorway to keep its route through the churn.".to_string(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    fn team_index(&self, team: TeamId) -> Option<usize> {
        self.teams.iter().position(|t| *t == team)
    }

    pub fn budget_of(&self, team: TeamId) -> u8 {
        self.team_index(team).map(|i| self.budget[i]).unwrap_or(0)
    }

    pub fn is_cabled(&self, door: DoorId) -> bool {
        self.cables.iter().any(|c| c.a == door || c.b == door)
    }

    pub fn cable_on(&self, door: DoorId) -> Option<&Cable> {
        self.cables.iter().find(|c| c.a == door || c.b == door)
    }

    /// Lay a cable that pins the doorway's current connection so it survives
    /// decoherence. Costs the team one cable from its budget.
    pub fn deploy_cable(&mut self, team: TeamId, door: DoorId) -> bool {
        let Some(index) = self.team_index(team) else {
            return false;
        };
        if self.budget[index] == 0 {
            self.last_event = format!("{} is out of cable.", team.label());
            return false;
        }
        if self.graph.is_sealed(door) {
            self.last_event = "Cannot cable a sealed wall.".to_string();
            return false;
        }
        if self.is_cabled(door) {
            self.last_event = "That doorway is already cabled.".to_string();
            return false;
        }
        let partner = self.graph.partner(door);
        let id = CableId(self.next_cable);
        self.next_cable += 1;
        self.cables.push(Cable {
            id,
            owner: team,
            a: door,
            b: partner,
        });
        self.budget[index] -= 1;
        self.last_event = format!("{} laid cable {}.", team.label(), id.0);
        true
    }

    /// Cut a cable. The owner recovers the budget; an opponent's cut is a contest.
    pub fn cut_cable(&mut self, by: TeamId, cable: CableId) -> bool {
        let Some(position) = self.cables.iter().position(|c| c.id == cable) else {
            return false;
        };
        let cable = self.cables.remove(position);
        if by == cable.owner {
            if let Some(index) = self.team_index(cable.owner) {
                self.budget[index] += 1;
            }
            self.last_event = format!("{} recovered cable {}.", by.label(), cable.id.0);
        } else {
            self.contests += 1;
            self.last_event = format!(
                "{} cut {}'s cable {}.",
                by.label(),
                cable.owner.label(),
                cable.id.0
            );
        }
        true
    }

    /// Convenience: cut whichever cable touches `door`.
    pub fn cut_on(&mut self, by: TeamId, door: DoorId) -> bool {
        let Some(id) = self.cable_on(door).map(|c| c.id) else {
            return false;
        };
        self.cut_cable(by, id)
    }

    /// Re-match every doorway that is not pinned by a cable. Cabled routes persist.
    pub fn decohere(&mut self) {
        self.decohere_count += 1;
        let mut free: Vec<DoorId> = (0..DOOR_COUNT)
            .map(|i| DoorId(i as u16))
            .filter(|d| !self.is_cabled(*d))
            .collect();

        let mut rng = SplitMix(
            self.base_seed ^ (self.decohere_count as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        for i in (1..free.len()).rev() {
            free.swap(i, rng.below(i + 1));
        }

        let mut chunks = free.chunks_exact(2);
        for pair in chunks.by_ref() {
            let (a, b) = (pair[0], pair[1]);
            self.graph.links[a.0 as usize] = b;
            self.graph.links[b.0 as usize] = a;
        }
        if let [leftover] = chunks.remainder() {
            self.graph.links[leftover.0 as usize] = *leftover;
        }
        self.last_event = format!("Decohered ({} cabled routes held).", self.cables.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observation_lab::model::Side;
    use observed_core::RoomId;

    fn assert_valid_matching(world: &RouteWorld) {
        for index in 0..DOOR_COUNT {
            let a = DoorId(index as u16);
            let b = world.graph.partner(a);
            assert_eq!(
                world.graph.partner(b),
                a,
                "matching must stay a valid involution"
            );
        }
    }

    fn east_door(world: &RouteWorld, room: u32) -> DoorId {
        world.graph.door_id(RoomId(room), Side::East)
    }

    #[test]
    fn authored_has_two_teams_with_full_budget() {
        let world = RouteWorld::authored();
        assert_eq!(world.teams.len(), TEAM_COUNT);
        assert_eq!(world.budget_of(TeamId(0)), CABLE_CAPACITY);
        assert!(world.cables.is_empty());
        assert_valid_matching(&world);
    }

    #[test]
    fn a_cabled_route_survives_decoherence() {
        let mut world = RouteWorld::authored();
        let door = east_door(&world, 0);
        let partner = world.graph.partner(door);
        assert!(world.deploy_cable(TeamId(0), door));

        for _ in 0..50 {
            world.decohere();
        }
        assert_eq!(
            world.graph.partner(door),
            partner,
            "the cabled route must persist"
        );
        assert_valid_matching(&world);
    }

    #[test]
    fn uncabled_doors_still_rewire() {
        let mut world = RouteWorld::authored();
        world.deploy_cable(TeamId(0), east_door(&world, 0));
        let before = world.graph.links.clone();
        world.decohere();
        assert_ne!(world.graph.links, before, "non-cabled doors should rewire");
    }

    #[test]
    fn cable_budget_is_limited_per_team() {
        let mut world = RouteWorld::authored();
        assert!(world.deploy_cable(TeamId(0), east_door(&world, 0)));
        assert!(world.deploy_cable(TeamId(0), east_door(&world, 1)));
        assert!(world.deploy_cable(TeamId(0), east_door(&world, 3)));
        assert_eq!(world.budget_of(TeamId(0)), 0);
        // Out of budget: the fourth is denied.
        assert!(!world.deploy_cable(TeamId(0), east_door(&world, 4)));
    }

    #[test]
    fn an_opponent_cut_is_a_contest_and_frees_the_route() {
        let mut world = RouteWorld::authored();
        let door = east_door(&world, 0);
        world.deploy_cable(TeamId(0), door);
        assert_eq!(world.budget_of(TeamId(0)), CABLE_CAPACITY - 1);

        assert!(world.cut_on(TeamId(1), door));
        assert_eq!(world.contests, 1);
        // The owner does not get the cable back when an opponent cuts it.
        assert_eq!(world.budget_of(TeamId(0)), CABLE_CAPACITY - 1);
        assert!(!world.is_cabled(door));

        // Now that doorway is free again — it can rewire.
        let before = world.graph.partner(door);
        let mut changed = false;
        for _ in 0..50 {
            world.decohere();
            if world.graph.partner(door) != before {
                changed = true;
                break;
            }
        }
        assert!(changed, "a cut route rejoins the churn");
    }

    #[test]
    fn recovering_your_own_cable_refunds_the_budget() {
        let mut world = RouteWorld::authored();
        let door = east_door(&world, 0);
        world.deploy_cable(TeamId(0), door);
        assert_eq!(world.budget_of(TeamId(0)), CABLE_CAPACITY - 1);
        assert!(world.cut_on(TeamId(0), door));
        assert_eq!(world.budget_of(TeamId(0)), CABLE_CAPACITY);
        assert_eq!(world.contests, 0);
    }

    #[test]
    fn decoherence_is_deterministic() {
        let mut a = RouteWorld::authored();
        let mut b = RouteWorld::authored();
        a.deploy_cable(TeamId(0), east_door(&a, 0));
        b.deploy_cable(TeamId(0), east_door(&b, 0));
        a.decohere();
        b.decohere();
        assert_eq!(a.graph.links, b.graph.links);
    }

    #[test]
    fn reset_restores_authored_state() {
        let mut world = RouteWorld::authored();
        world.deploy_cable(TeamId(0), east_door(&world, 0));
        world.decohere();
        world.reset();
        assert!(world.cables.is_empty());
        assert_eq!(world.budget_of(TeamId(0)), CABLE_CAPACITY);
        assert_eq!(world.decohere_count, 0);
    }
}
