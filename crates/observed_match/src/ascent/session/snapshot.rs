//! Allowlisted seat reads. A client view receives remembered geometry rather than
//! a live world plus a mask, so stale cells cannot silently update behind fog.

use std::collections::BTreeMap;

use observed_core::PlayerId;
use observed_facility::hex_wfc::HexPlacement;
use observed_hex::HexCoord;

use super::{AscentSession, Refusal, Role, TeamRequest};
use crate::ascent::sim::{
    Card, GuardianId, GuardianKind, MatchOutcome, ObserverId, ObserverState, TeamId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellRead {
    pub placement: HexPlacement,
    pub seen_at: u64,
    pub fresh: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObserverRead {
    pub id: ObserverId,
    pub team: TeamId,
    pub cell: HexCoord,
    pub state: ObserverState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardianRead {
    pub id: GuardianId,
    pub cell: HexCoord,
    pub kind: GuardianKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeatSnapshot {
    pub tick: u64,
    pub role: Role,
    pub cells: BTreeMap<HexCoord, CellRead>,
    pub observers: Vec<ObserverRead>,
    pub guardians: Vec<GuardianRead>,
    pub hand: Vec<Card>,
    pub cooldown: u32,
    pub requests: Vec<TeamRequest>,
    pub outcome: MatchOutcome,
}

impl AscentSession {
    pub fn snapshot(&self, player: PlayerId) -> Result<SeatSnapshot, Refusal> {
        let role = self.seats.get(&player).ok_or(Refusal::UnknownSeat)?.role;
        let (cells, observers, guardians) = if role == Role::Rogue {
            let knowledge = self.sim.rogue_knowledge();
            let cells = self
                .sim
                .world
                .placements
                .iter()
                .map(|(&cell, &placement)| {
                    (
                        cell,
                        CellRead {
                            placement,
                            seen_at: self.sim.tick,
                            fresh: true,
                        },
                    )
                })
                .collect();
            (cells, knowledge.known_observers, knowledge.guardians)
        } else {
            let team = self.team(player).ok_or(Refusal::WrongRole)?;
            let knowledge = self.sim.team_knowledge(team);
            let cells = knowledge
                .cells
                .iter()
                .map(|(&cell, known)| {
                    (
                        cell,
                        CellRead {
                            placement: known.placement,
                            seen_at: known.seen_at,
                            fresh: knowledge.visible_cells.contains(&cell),
                        },
                    )
                })
                .collect();
            (
                cells,
                knowledge.known_observers,
                knowledge.visible_guardians,
            )
        };
        let (hand, cooldown) = match role {
            Role::Architect(team) => {
                let hand = &self.hands[&team];
                (hand.deck.hand.clone(), hand.cooldown)
            }
            Role::Rogue => (self.sim.deck.hand.clone(), self.sim.cooldown),
            _ => (Vec::new(), 0),
        };
        let team = self.team(player);
        Ok(SeatSnapshot {
            tick: self.sim.tick,
            role,
            cells,
            observers: observers
                .into_iter()
                .filter_map(|(id, cell)| {
                    let observer = self.sim.observers.get(&id)?;
                    Some(ObserverRead {
                        id,
                        team: observer.team,
                        cell,
                        state: observer.state,
                    })
                })
                .collect(),
            guardians: guardians
                .into_iter()
                .filter_map(|id| {
                    let guardian = self.sim.guardians.get(&id)?;
                    Some(GuardianRead {
                        id,
                        cell: guardian.cell,
                        kind: guardian.kind,
                    })
                })
                .collect(),
            hand,
            cooldown,
            requests: self
                .requests
                .values()
                .filter(|r| Some(r.team) == team)
                .copied()
                .collect(),
            outcome: self.sim.outcome,
        })
    }
}
