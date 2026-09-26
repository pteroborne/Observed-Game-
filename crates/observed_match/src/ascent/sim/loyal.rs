//! The loyal Architect bot: a behaviour tree that builds for its own team.
//!
//! The design admits a loyal bot only through the command boundary the Rogue bot proved
//! (`docs/architect_ascent_design.md`, section 9): it asks the same legality query a
//! human's preview asks, and its choice is submitted exactly as a human play is. It is
//! called in its team's context - that team's hand, cooldown and knowledge - by the
//! session, so it can neither see nor play what its human counterpart could not.
//!
//! It judges a play without making it. Previewing each candidate on a copy of the rules
//! cost 190 ms a decision on a production facility, because every copy carries the whole
//! facility. A tile play changes one cell, so what it does is local: the doorways around
//! that cell, and whether its doors join a cell the team can reach to a cell nearer the
//! summit. One search from the summit and one from each Observer answer the second for
//! every candidate at once.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_facility::hex_wfc::HexPlacement;
use observed_hex::{HexCoord, HexFace, PortClass, ports_compatible, travel_distance};

use super::{
    ArchitectCommand, ArchitectLab, BehaviorTrace, CardKind, ObserverState, TeamId, command_key,
};

/// How far from its own Observers the bot looks for a play, in cells. A route is built
/// where the team is, and the bound keeps a production facility's decision in budget.
const REACH: u32 = 3;
/// What an Observer with no way up costs the score: far more than any real route.
const NO_WAY_UP: usize = 10_000;
/// How far an Observer's own search runs, in steps: past every cell a candidate within
/// [`REACH`] can join, with room for the walk to wind.
const OWN_SEARCH_STEPS: usize = 12;

/// One candidate, judged: mismatched doorways around its cell after the play, before
/// it, and the team's way up after it.
struct Judged {
    after: usize,
    before: usize,
    way_up: usize,
    command: ArchitectCommand,
}

impl ArchitectLab {
    /// The loyal Architect's intent for `team`, in that team's hand context.
    ///
    /// 1. Wait for the cooldown.
    /// 2. Repair: play what mends mismatched doorways, before a floor starts retracting.
    /// 3. Answer: build within reach of a route the team `asked` for, nearest the oldest
    ///    ask first, breaking nothing and losing none of the way up.
    /// 4. Build up: play what shortens the team's way to the summit, breaking nothing.
    /// 5. Otherwise hold the card.
    #[must_use]
    pub fn loyal_intent(
        &self,
        team: TeamId,
        asked: &[HexCoord],
    ) -> (Option<ArchitectCommand>, BehaviorTrace) {
        let mut trace = BehaviorTrace::default();
        if trace.test("wait for cooldown", self.cooldown > 0) {
            return (None, trace);
        }
        let own: Vec<HexCoord> = self
            .observers
            .values()
            .filter(|o| o.team == team && o.state == ObserverState::Active)
            .map(|o| o.cell)
            .collect();
        // Candidates around the team, and around where it has asked for a route.
        let anchors: Vec<HexCoord> = own.iter().chain(asked).copied().collect();
        let candidates = self.candidates(&anchors);
        if candidates.is_empty() {
            trace.test("hold card", true);
            return (None, trace);
        }
        // One search from the summit over the whole facility. Walks are symmetric, so it
        // also says how far each Observer is from the summit now. Each Observer's own
        // search need only reach the cells a candidate can touch.
        let from_summit = self.distances_from(self.world.config.exit(), usize::MAX);
        let from_own: Vec<BTreeMap<HexCoord, usize>> = own
            .iter()
            .map(|&cell| self.distances_from(cell, OWN_SEARCH_STEPS))
            .collect();
        let current: Vec<usize> = own
            .iter()
            .map(|cell| from_summit.get(cell).copied().unwrap_or(NO_WAY_UP))
            .collect();
        // Replacing a cell on a shortest way up could cut the way the team has.
        let in_use = |cell: HexCoord| {
            from_own.iter().zip(&current).any(|(reach, &now)| {
                matches!(
                    (reach.get(&cell), from_summit.get(&cell)),
                    (Some(to), Some(on)) if to + on == now
                )
            })
        };

        let judged: Vec<Judged> = candidates
            .into_iter()
            .filter_map(|(command, placement)| {
                let ArchitectCommand::Play { target, .. } = command else {
                    return None;
                };
                let (before, after) = self.mismatches_around(target, placement);
                let way_up = if in_use(target) {
                    current.iter().sum()
                } else {
                    from_own
                        .iter()
                        .zip(&current)
                        .map(|(reach, &now)| {
                            now.min(self.through(target, placement, reach, &from_summit))
                        })
                        .sum()
                };
                Some(Judged {
                    after,
                    before,
                    way_up,
                    command,
                })
            })
            .collect();

        let key = |judged: &&Judged| command_key(judged.command);
        let repair = judged
            .iter()
            .filter(|j| j.after < j.before)
            .min_by_key(|j| (j.after, j.way_up, key(j)));
        if trace.test("repair a contradiction", repair.is_some()) {
            return (repair.map(|j| j.command), trace);
        }
        let now: usize = current.iter().sum();
        // How near a play is to an ask: to the oldest ask it answers, then how near.
        let answers = |j: &Judged| {
            let ArchitectCommand::Play { target, .. } = j.command else {
                return None;
            };
            asked
                .iter()
                .enumerate()
                .map(|(age, &at)| (age, travel_distance(at, target)))
                .filter(|&(_, distance)| distance <= REACH)
                .min()
        };
        let answer = judged
            .iter()
            .filter(|j| j.after == 0 && j.way_up <= now)
            .filter_map(|j| Some((answers(j)?, j)))
            .min_by_key(|(near, j)| (*near, j.way_up, key(j)))
            .map(|(_, j)| j);
        if trace.test("answer a request", answer.is_some()) {
            return (answer.map(|j| j.command), trace);
        }
        let build = judged
            .iter()
            .filter(|j| j.after == 0 && j.way_up < now)
            .min_by_key(|j| (j.way_up, key(j)));
        if trace.test("shorten the way up", build.is_some()) {
            return (build.map(|j| j.command), trace);
        }
        trace.test("hold card", true);
        (None, trace)
    }

    /// Every legal tile play within reach of the team, with the cell it would build.
    fn candidates(&self, own: &[HexCoord]) -> Vec<(ArchitectCommand, HexPlacement)> {
        let near: BTreeSet<HexCoord> = self
            .known
            .iter()
            .copied()
            .filter(|&cell| own.iter().any(|&at| travel_distance(at, cell) <= REACH))
            .collect();
        let mut out = Vec::new();
        for card in &self.deck.hand {
            let CardKind::Tile(shape) = card.kind else {
                continue;
            };
            for &target in &near {
                for rotation in 0..6 {
                    let command = ArchitectCommand::Play {
                        card: card.id,
                        target,
                        rotation,
                    };
                    if self.refusal(command).is_none() {
                        out.push((command, self.played_placement(shape, target, rotation)));
                    }
                }
            }
        }
        out.sort_by_key(|(command, _)| command_key(*command));
        out
    }

    /// Mismatched doorways between `cell` and its neighbours: as it stands, and if it
    /// held `placement`. A doorway is mismatched where two built cells' ports disagree, or
    /// where an open port faces a retracted cell.
    fn mismatches_around(&self, cell: HexCoord, placement: HexPlacement) -> (usize, usize) {
        let grid = self.world.config.grid();
        let current = self.world.placements[&cell];
        let bad = |here: &HexPlacement, retracted: bool, face: HexFace| {
            let Some(next) = grid.neighbor(cell, face) else {
                return false;
            };
            let Some(other) = self.world.placements.get(&next) else {
                return false;
            };
            let mine = here.ports().port(face);
            let theirs = other.ports().port(face.opposite());
            (here.space.built() && other.space.built() && !ports_compatible(mine, theirs))
                || (here.space.built()
                    && self.retracted.contains(&next)
                    && mine != PortClass::Sealed)
                || (retracted && other.space.built() && theirs != PortClass::Sealed)
        };
        let before = HexFace::ALL
            .into_iter()
            .filter(|&face| bad(&current, self.retracted.contains(&cell), face))
            .count();
        let after = HexFace::ALL
            .into_iter()
            .filter(|&face| bad(&placement, false, face))
            .count();
        (before, after)
    }

    /// The shortest walk from an Observer to the summit through `cell` holding
    /// `placement`, in steps, given the Observer's and the summit's distances to every
    /// cell as the facility stands.
    fn through(
        &self,
        cell: HexCoord,
        placement: HexPlacement,
        from_observer: &BTreeMap<HexCoord, usize>,
        from_summit: &BTreeMap<HexCoord, usize>,
    ) -> usize {
        let grid = self.world.config.grid();
        let linked: Vec<HexCoord> = HexFace::LATERAL
            .into_iter()
            .filter(|&face| placement.is_open(face))
            .filter_map(|face| {
                let next = grid.neighbor(cell, face)?;
                let other = self.world.placements.get(&next)?;
                (other.space.built() && other.is_open(face.opposite())).then_some(next)
            })
            .collect();
        let mut best = NO_WAY_UP;
        for &entry in &linked {
            for &exit in &linked {
                if entry == exit {
                    continue;
                }
                if let (Some(&to), Some(&onward)) =
                    (from_observer.get(&entry), from_summit.get(&exit))
                {
                    best = best.min(to + 2 + onward);
                }
            }
        }
        best
    }

    /// Steps from `origin` to every cell a walk of at most `limit` steps reaches, through
    /// the rules' own exits.
    fn distances_from(&self, origin: HexCoord, limit: usize) -> BTreeMap<HexCoord, usize> {
        let mut distance = BTreeMap::from([(origin, 0)]);
        let mut queue = VecDeque::from([origin]);
        while let Some(cell) = queue.pop_front() {
            let steps = distance[&cell];
            if steps >= limit {
                continue;
            }
            for next in self.exits(cell) {
                if let std::collections::btree_map::Entry::Vacant(entry) = distance.entry(next) {
                    entry.insert(steps + 1);
                    queue.push_back(next);
                }
            }
        }
        distance
    }
}
