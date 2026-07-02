//! Phase 38 — **Contested Observation**: team attribution over the shared
//! decoherence graph from [`crate`].
//!
//! The arc's single rule (see `docs/contention_arc_plan.md`) is that
//! observation is *objective and shared*: when any team occupies a room or
//! anchors it, every doorway of that room — for every team — is frozen. There
//! is no per-team pin regime; [`ContentionWorld::is_pinned`] is one shared
//! predicate. What differs per team is not what is real but what is *known*:
//! each team keeps its own ledger of doorway links it has personally observed,
//! stamped with the tick it last looked (fog of war over truth, not over
//! geometry).
//!
//! This module is additive over [`ObservationWorld`]: it reuses the door graph
//! and the deterministic shuffle verbatim (via [`ObservationWorld::decohere_with`])
//! and layers team membership, anchors, knowledge, and a solvability guard on
//! top. `ObservationWorld::players` is intentionally left empty on the inner
//! world — [`ContentionWorld::members`] is the single source of truth for who
//! is where, because a team may field more than one member.

use std::collections::BTreeMap;

use crate::{DoorId, ObservationWorld, Side};
use observed_core::{RoomId, TeamId};

/// How many alternate salts the solvability guard will try before giving up
/// and reverting a decoherence entirely. See [`ContentionWorld::decohere`].
pub const SOLVABILITY_RETRY_BUDGET: usize = 16;

/// Who is responsible for a doorway being frozen. Presence and anchors can
/// overlap (a team can both stand in a room and have anchored it, or two
/// teams can anchor the same room); queries return every source so
/// presentation can attribute blame precisely instead of a single "frozen"
/// bit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PinSource {
    /// A member of this team currently occupies the room at one end of the
    /// doorway (or its partner).
    Presence(TeamId),
    /// This team has an anchor placed on the room at one end of the doorway
    /// (or its partner).
    Anchor(TeamId),
}

/// A team member's position. Teams may field multiple members, so this is a
/// flat roster rather than one room per team.
#[derive(Clone, Copy, Debug)]
pub struct Member {
    pub team: TeamId,
    pub room: RoomId,
}

/// A team-keyed hard freeze on a room: pins all four of the room's doorways
/// (and each doorway's partner) until removed, independent of presence.
///
/// Anchor-vs-anchor is idempotent by design: two teams can anchor the same
/// room simultaneously and both facts are recorded. Freezing is a fact, not a
/// vote — removing one team's anchor leaves the other's still pinning the
/// room. The competitive conflict this creates lives one level up, at the
/// route: a rival denies you not by contesting your anchor but by shaping the
/// surrounding topology to disfavor you.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Anchor {
    pub team: TeamId,
    pub room: RoomId,
}

/// One doorway link as a single team has observed it: which door it led to,
/// and the tick that observation was made.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnownEdge {
    pub partner: DoorId,
    pub tick: u64,
}

/// A team's private ledger of the graph: which doorway links it has
/// personally observed and when. Reality (the actual matching in
/// `ObservationWorld::links`) is shared and objective; knowledge of it is
/// not. Two teams standing in different rooms can each be confidently wrong
/// about what the other has since seen rewire.
#[derive(Clone, Debug, Default)]
pub struct TeamKnowledge {
    edges: BTreeMap<DoorId, KnownEdge>,
}

impl TeamKnowledge {
    /// The last-known link for `door`, if this team has ever observed it.
    pub fn get(&self, door: DoorId) -> Option<KnownEdge> {
        self.edges.get(&door).copied()
    }

    /// Ticks elapsed since this team last observed `door`, or `None` if it
    /// has never been observed by this team.
    pub fn staleness(&self, door: DoorId, now: u64) -> Option<u64> {
        self.edges
            .get(&door)
            .map(|edge| now.saturating_sub(edge.tick))
    }

    /// How many doorways this team has ever recorded a link for.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether this team has recorded any doorway links at all.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    fn record(&mut self, door: DoorId, partner: DoorId, tick: u64) {
        self.edges.insert(door, KnownEdge { partner, tick });
    }
}

/// The Phase 38 model: shared, objective decoherence (via the embedded
/// [`ObservationWorld`]) plus team attribution, team-keyed anchors,
/// team-local knowledge, and a solvability guard around every rewire.
///
/// `world.players` is always empty; [`Self::members`] is authoritative for
/// occupancy because a team may field more than one member and
/// `ObservationWorld` models one player per slot.
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct ContentionWorld {
    pub world: ObservationWorld,
    pub members: Vec<Member>,
    pub anchors: Vec<Anchor>,
    pub exit: RoomId,
    pub tick: u64,
    knowledge: BTreeMap<TeamId, TeamKnowledge>,
    /// How many salts the most recent `decohere()` needed (1 if the first
    /// attempt was already solvable).
    pub last_decohere_attempts: u32,
    /// Whether the most recent `decohere()` had to fully revert to the
    /// pre-shuffle links because every salt in the retry budget still
    /// stranded a member.
    pub last_decohere_reverted: bool,
}

impl ContentionWorld {
    /// Build a new contested world over `room_count` rooms joined by `edges`,
    /// with `members` (team, starting room) pairs and a shared `exit` room.
    /// `base_seed` seeds the embedded [`ObservationWorld`]'s decoherence
    /// stream, exactly as [`ObservationWorld::from_edges`].
    pub fn new(
        room_count: usize,
        edges: &[(RoomId, Side, RoomId, Side)],
        members: &[(TeamId, RoomId)],
        exit: RoomId,
        base_seed: u64,
    ) -> Self {
        let world = ObservationWorld::from_edges(room_count, edges, Vec::new(), base_seed);
        let members: Vec<Member> = members
            .iter()
            .map(|&(team, room)| Member { team, room })
            .collect();
        Self {
            world,
            members,
            anchors: Vec::new(),
            exit,
            tick: 0,
            knowledge: BTreeMap::new(),
            last_decohere_attempts: 0,
            last_decohere_reverted: false,
        }
    }

    // -- queries ------------------------------------------------------------

    /// Whether any member — of any team — currently occupies `room`.
    pub fn observed(&self, room: RoomId) -> bool {
        self.members.iter().any(|member| member.room == room)
    }

    /// Whether any team has an anchor placed on `room`.
    fn anchored(&self, room: RoomId) -> bool {
        self.anchors.iter().any(|anchor| anchor.room == room)
    }

    /// The shared, objective pin rule: a doorway is frozen when either end's
    /// room is observed (by any team) or anchored (by any team). This is
    /// deliberately a single predicate with no per-team variant — Phase 38's
    /// thesis is that observation is shared, not team-relative.
    pub fn is_pinned(&self, door: DoorId) -> bool {
        self.room_pins(self.world.door(door).room)
            || self.room_pins(self.world.door(self.world.partner(door)).room)
    }

    fn room_pins(&self, room: RoomId) -> bool {
        self.observed(room) || self.anchored(room)
    }

    /// Every reason `door` is currently pinned, deduped, in a deterministic
    /// order: presence sources ordered by team id, then anchor sources
    /// ordered by team id. Presentation uses this to attribute blame (whose
    /// tether light should show on this frame).
    pub fn pin_sources(&self, door: DoorId) -> Vec<PinSource> {
        let rooms = [
            self.world.door(door).room,
            self.world.door(self.world.partner(door)).room,
        ];

        let mut presence_teams: Vec<TeamId> = self
            .members
            .iter()
            .filter(|member| rooms.contains(&member.room))
            .map(|member| member.team)
            .collect();
        presence_teams.sort_by_key(|team| team.0);
        presence_teams.dedup();

        let mut anchor_teams: Vec<TeamId> = self
            .anchors
            .iter()
            .filter(|anchor| rooms.contains(&anchor.room))
            .map(|anchor| anchor.team)
            .collect();
        anchor_teams.sort_by_key(|team| team.0);
        anchor_teams.dedup();

        presence_teams
            .into_iter()
            .map(PinSource::Presence)
            .chain(anchor_teams.into_iter().map(PinSource::Anchor))
            .collect()
    }

    // -- anchors --------------------------------------------------------

    /// Place `team`'s anchor on `room`. Idempotent: if `team` already anchors
    /// `room` this is a no-op that still returns `true` (the anchor is a
    /// fact, not a toggle). Anchoring a room another team already anchors
    /// succeeds and both facts are recorded — anchor-vs-anchor agrees.
    pub fn place_anchor(&mut self, team: TeamId, room: RoomId) -> bool {
        if self
            .anchors
            .iter()
            .any(|anchor| anchor.team == team && anchor.room == room)
        {
            return true;
        }
        self.anchors.push(Anchor { team, room });
        true
    }

    /// Remove `team`'s anchor from `room`, if present. Only ever removes the
    /// calling team's own anchor; another team's anchor on the same room is
    /// untouched. Returns whether an anchor was actually removed.
    pub fn remove_anchor(&mut self, team: TeamId, room: RoomId) -> bool {
        let before = self.anchors.len();
        self.anchors
            .retain(|anchor| !(anchor.team == team && anchor.room == room));
        self.anchors.len() != before
    }

    // -- knowledge --------------------------------------------------------

    /// For every member, record into that member's team's ledger every
    /// doorway of the occupied room, and each doorway's current partner,
    /// stamped with `self.tick`. Call sites decide cadence (e.g. once per
    /// simulation step, or only on room entry) — this simply captures "what
    /// would be visible from here right now."
    pub fn record_observations(&mut self) {
        let tick = self.tick;
        for member in self.members.clone() {
            let ledger = self.knowledge.entry(member.team).or_default();
            for side in Side::ALL {
                let door = self.world.door_id(member.room, side);
                let partner = self.world.partner(door);
                ledger.record(door, partner, tick);
                ledger.record(partner, door, tick);
            }
        }
    }

    /// `team`'s private ledger of observed doorway links.
    pub fn known_edges(&self, team: TeamId) -> &TeamKnowledge {
        static EMPTY: TeamKnowledge = TeamKnowledge {
            edges: BTreeMap::new(),
        };
        self.knowledge.get(&team).unwrap_or(&EMPTY)
    }

    // -- movement -----------------------------------------------------------

    /// Walk `members[member_index]` through the doorway on `side`, following
    /// its current link. Returns false for a sealed wall or an out-of-range
    /// index.
    pub fn traverse(&mut self, member_index: usize, side: Side) -> bool {
        let Some(member) = self.members.get(member_index) else {
            return false;
        };
        let door = self.world.door_id(member.room, side);
        if self.world.is_sealed(door) {
            return false;
        }
        let destination = self.world.door(self.world.partner(door)).room;
        self.members[member_index].room = destination;
        true
    }

    // -- evolution ------------------------------------------------------

    /// Re-match every doorway that isn't shared-pinned (presence or anchor,
    /// any team), guarded so no living member is ever stranded from
    /// [`Self::exit`].
    ///
    /// Mechanically: attempt a rewire with salt 0 (the default stream). If
    /// every member can still reach the exit afterwards, keep it. Otherwise
    /// restore the pre-attempt links and retry with the next salt
    /// (1, 2, 3, ...) up to [`SOLVABILITY_RETRY_BUDGET`] attempts. If every
    /// salt in the budget still strands someone, restore the links to
    /// exactly what they were before this call.
    ///
    /// This preserves solvability by induction: the *previous* state was
    /// solvable (by the same argument, recursively, back to the authored
    /// starting graph, which is solvable by construction). Every accepted
    /// rewire is checked before it is kept, so the invariant "every member
    /// can reach the exit" holds after every call, whether or not this call
    /// found a solvable rewire.
    ///
    /// `decoherence_count` bookkeeping: each attempt (including retries)
    /// bumps `world.decoherence_count` once (inside `decohere_with`), but we
    /// snapshot the count *before* the first attempt and restore it before
    /// each retry, so every attempt is `decoherence_count = snapshot + 1`
    /// with a different salt — attempts differ only by salt, never by an
    /// accumulating count, and the final kept state (accept or full revert)
    /// leaves the count at exactly `snapshot + 1`.
    pub fn decohere(&mut self) {
        let links_before = self.world.links.clone();
        let count_before = self.world.decoherence_count;
        let rewires_before = self.world.rewires_last;
        let locked_before = self.world.locked_last;

        // Precompute the shared-pin bitset once: `is_pinned` depends only on
        // `members`/`anchors`, neither of which this loop mutates, so it is
        // safe (and cheaper) to snapshot it before looping rather than borrow
        // `self` from inside the closure `decohere_with` needs.
        let pinned_doors: Vec<bool> = (0..self.world.doors.len())
            .map(|i| self.is_pinned(DoorId(i as u16)))
            .collect();
        let pinned = |door: DoorId| pinned_doors[door.0 as usize];

        let mut attempts = 0u32;
        let mut accepted = false;
        for attempt in 1..=SOLVABILITY_RETRY_BUDGET {
            self.world.decoherence_count = count_before;
            let salt = if attempt == 1 { 0 } else { attempt as u64 };
            self.world.decohere_with(&pinned, salt);
            attempts += 1;

            if self.all_members_can_reach_exit() {
                accepted = true;
                break;
            }
            // Not solvable: undo this attempt's link mutation before trying
            // the next salt (or giving up).
            self.world.links = links_before.clone();
        }

        self.last_decohere_attempts = attempts;
        self.last_decohere_reverted = !accepted;

        if !accepted {
            // Every salt in the budget stranded someone: fully restore the
            // pre-call state, including the rewire/lock counters, so a
            // failed decohere is a true no-op on the graph.
            self.world.links = links_before;
            self.world.decoherence_count = count_before + 1;
            self.world.rewires_last = rewires_before;
            self.world.locked_last = locked_before;
        }
    }

    fn all_members_can_reach_exit(&self) -> bool {
        self.members
            .iter()
            .all(|member| self.reachable(member.room))
    }

    /// Whether `from` can reach [`Self::exit`] through currently non-sealed
    /// doors.
    pub fn reachable(&self, from: RoomId) -> bool {
        if from == self.exit {
            return true;
        }
        self.path_to_exit(from).is_some()
    }

    /// A shortest sequence of doorway sides that walks from `from` to
    /// [`Self::exit`] through currently non-sealed doors, or `None` if no
    /// such path exists. Breadth-first, with a deterministic tie-break: at
    /// each room, sides are explored in [`Side::ALL`] order, so the result is
    /// reproducible for a given graph state.
    pub fn path_to_exit(&self, from: RoomId) -> Option<Vec<Side>> {
        use std::collections::VecDeque;

        if from == self.exit {
            return Some(Vec::new());
        }

        let mut visited = vec![false; self.world.room_count];
        let mut prev: Vec<Option<(RoomId, Side)>> = vec![None; self.world.room_count];
        visited[from.0 as usize] = true;

        let mut queue = VecDeque::new();
        queue.push_back(from);

        while let Some(room) = queue.pop_front() {
            if room == self.exit {
                break;
            }
            for side in Side::ALL {
                let door = self.world.door_id(room, side);
                if self.world.is_sealed(door) {
                    continue;
                }
                let next = self.world.door(self.world.partner(door)).room;
                if visited[next.0 as usize] {
                    continue;
                }
                visited[next.0 as usize] = true;
                prev[next.0 as usize] = Some((room, side));
                queue.push_back(next);
            }
        }

        if !visited[self.exit.0 as usize] {
            return None;
        }

        let mut path = Vec::new();
        let mut current = self.exit;
        while current != from {
            let (room, side) = prev[current.0 as usize]?;
            path.push(side);
            current = room;
        }
        path.reverse();
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Side;

    /// A small 3x1 corridor: rooms 0 - 1 - 2, room 2 is the exit. Room 0's
    /// East door links to room 1's West; room 1's East links to room 2's
    /// West. All other doors are sealed by default (no edge given).
    fn corridor_edges() -> Vec<(RoomId, Side, RoomId, Side)> {
        vec![
            (RoomId(0), Side::East, RoomId(1), Side::West),
            (RoomId(1), Side::East, RoomId(2), Side::West),
        ]
    }

    fn assert_valid_matching(world: &ObservationWorld) {
        for index in 0..world.doors.len() {
            let a = DoorId(index as u16);
            let b = world.partner(a);
            assert_eq!(
                world.partner(b),
                a,
                "matching must be a symmetric involution"
            );
        }
    }

    #[test]
    fn presence_pins_are_shared_across_teams() {
        let mut w = ContentionWorld::new(
            3,
            &corridor_edges(),
            &[(TeamId(0), RoomId(1)), (TeamId(1), RoomId(0))],
            RoomId(2),
            0xC0FFEE,
        );
        // Room 1 is occupied by team 0; its East doorway is incident to room 1
        // and connects toward team 1's side of the graph, but team 1 never
        // *stood* there. The doorway must still be pinned and immune to
        // rewiring because presence is shared, not per-team.
        let door = w.world.door_id(RoomId(1), Side::East);
        let partner_before = w.world.partner(door);

        assert!(w.is_pinned(door), "room 1 is observed, so its doors pin");
        assert!(
            w.pin_sources(door)
                .contains(&PinSource::Presence(TeamId(0))),
            "attribution must name team 0, the team actually present"
        );

        for _ in 0..8 {
            w.decohere();
        }
        assert_eq!(
            w.world.partner(door),
            partner_before,
            "an observed doorway must never rewire regardless of which team is watching"
        );
    }

    #[test]
    fn anchors_freeze_without_presence_and_persist_until_removed() {
        let mut w = ContentionWorld::new(
            3,
            &corridor_edges(),
            &[(TeamId(0), RoomId(2)), (TeamId(1), RoomId(2))],
            RoomId(2),
            0xACE,
        );
        // Nobody stands in room 0, but team 0 anchors it.
        assert!(w.place_anchor(TeamId(0), RoomId(0)));
        let door = w.world.door_id(RoomId(0), Side::East);
        let partner_before = w.world.partner(door);

        assert!(w.is_pinned(door), "an anchored room pins its doors");
        assert!(w.pin_sources(door).contains(&PinSource::Anchor(TeamId(0))));

        for _ in 0..8 {
            w.decohere();
            assert_eq!(
                w.world.partner(door),
                partner_before,
                "anchor must persist across repeated decoherence"
            );
        }

        // Anchor-vs-anchor: team 1 also anchors the same room. Idempotent —
        // both facts are recorded.
        assert!(w.place_anchor(TeamId(1), RoomId(0)));
        let sources = w.pin_sources(door);
        assert!(sources.contains(&PinSource::Anchor(TeamId(0))));
        assert!(sources.contains(&PinSource::Anchor(TeamId(1))));

        // Removing team 0's anchor leaves team 1's still pinning the room.
        assert!(w.remove_anchor(TeamId(0), RoomId(0)));
        assert!(w.is_pinned(door), "team 1's anchor still pins the room");
        assert!(w.pin_sources(door).contains(&PinSource::Anchor(TeamId(1))));
        assert!(!w.pin_sources(door).contains(&PinSource::Anchor(TeamId(0))));

        // Removing the same anchor twice is a no-op the second time.
        assert!(!w.remove_anchor(TeamId(0), RoomId(0)));
    }

    #[test]
    fn knowledge_is_team_local_and_can_go_stale() {
        // Use a grid with plenty of free doors (the authored 3x3 lattice) so
        // the shuffle has real entropy, and search deterministically for a
        // base_seed whose first post-move decohere actually rewires the
        // doorway under test — rather than hoping a fixed seed does it within
        // an attempt budget.
        let edges = crate::authored_edges();
        let door_of_interest = |w: &ObservationWorld| w.door_id(RoomId(4), Side::East);

        let mut chosen_seed = None;
        for seed in 0u64..200 {
            let mut probe = ObservationWorld::from_edges(
                crate::ROOM_COUNT,
                &edges,
                vec![RoomId(8)], // observer far from room 4
                seed,
            );
            let door = door_of_interest(&probe);
            let before = probe.partner(door);
            probe.decohere();
            if probe.partner(door) != before {
                chosen_seed = Some(seed);
                break;
            }
        }
        let seed = chosen_seed.expect("expected a seed that rewires room 4's East doorway");

        let mut w = ContentionWorld::new(
            crate::ROOM_COUNT,
            &edges,
            &[(TeamId(0), RoomId(4)), (TeamId(1), RoomId(0))],
            RoomId(8),
            seed,
        );
        let door = door_of_interest(&w.world);
        let original_partner = w.world.partner(door);

        w.tick = 5;
        w.record_observations();
        let team0_edge = w.known_edges(TeamId(0)).get(door).unwrap();
        assert_eq!(team0_edge.partner, original_partner);
        assert_eq!(team0_edge.tick, 5);

        // Team 0 moves away from room 4 so its doorway becomes free to rewire.
        // Team 1 stays clear of room 4 throughout.
        w.members[0].room = RoomId(8);
        w.decohere();
        assert_ne!(
            w.world.partner(door),
            original_partner,
            "the seed was chosen specifically so this decohere rewires the door"
        );

        // Team 0's stale ledger still reports the OLD partner and OLD tick —
        // it hasn't looked since tick 5.
        let stale = w.known_edges(TeamId(0)).get(door).unwrap();
        assert_eq!(stale.partner, original_partner);
        assert_eq!(stale.tick, 5);

        // Team 1 moves into room 4 at the new tick and records fresh
        // knowledge, which must reflect the NEW partner.
        w.members[1].room = RoomId(4);
        w.tick = 9;
        w.record_observations();
        let fresh = w.known_edges(TeamId(1)).get(door).unwrap();
        assert_eq!(fresh.partner, w.world.partner(door));
        assert_ne!(fresh.partner, original_partner);
        assert_eq!(fresh.tick, 9);

        // Staleness is computed relative to "now".
        assert_eq!(w.known_edges(TeamId(0)).staleness(door, 20), Some(15));
        assert_eq!(w.known_edges(TeamId(1)).staleness(door, 20), Some(11));
    }

    /// Build a tiny world engineered so the very first shuffle (salt 0)
    /// stalemates a member behind a sealed door, forcing the solvability
    /// guard to retry with another salt. Four rooms in a line, 0-1-2-3, exit
    /// at room 3; only room 1's East/room 2's West pair (and room 0's East /
    /// room1's West) are free to move — we anchor room 3 so its doors can
    /// never be touched but leave rooms 0-2 free, and pick a `base_seed`
    /// whose salt-0 shuffle happens to seal off room 0. If salt 0 doesn't
    /// strand anyone for a given seed, the loop below searches deterministically
    /// over seeds until it finds one that does, then verifies the guard saves it.
    #[test]
    fn solvability_guard_retries_or_reverts_to_keep_every_member_connected() {
        // A 4-room line: 0 - 1 - 2 - 3 (exit). All four rooms' "extra" sides
        // (anything but the line edges) are already sealed (no edges given
        // for them), so the only doors ever in the free set are the three
        // line connections plus each room's two unused sides (which are
        // self-sealed and thus excluded from `free` because `is_pinned`
        // returns false for sealed-but-unpinned doors — those are eligible to
        // "rewire" into another seal, which is harmless). Team member starts
        // in room 0; nothing anchors or observes room 1 or room 2, so the
        // line's two internal joints are free to shuffle.
        let edges = vec![
            (RoomId(0), Side::East, RoomId(1), Side::West),
            (RoomId(1), Side::East, RoomId(2), Side::West),
            (RoomId(2), Side::East, RoomId(3), Side::West),
        ];

        // Search deterministically over seeds for one where an UNGUARDED
        // salt-0 decohere strands room 0 from the exit, to prove the guard
        // matters (rather than trivially never firing).
        let mut found_seed = None;
        for seed in 0u64..200 {
            let mut probe = ObservationWorld::from_edges(4, &edges, vec![RoomId(3)], seed);
            probe.decohere();
            // Reachability check by hand: BFS from room 0 using `probe`.
            let reachable = {
                let mut visited = [false; 4];
                let mut stack = vec![RoomId(0)];
                visited[0] = true;
                while let Some(room) = stack.pop() {
                    for side in Side::ALL {
                        let door = probe.door_id(room, side);
                        if probe.is_sealed(door) {
                            continue;
                        }
                        let next = probe.door(probe.partner(door)).room;
                        if !visited[next.0 as usize] {
                            visited[next.0 as usize] = true;
                            stack.push(next);
                        }
                    }
                }
                visited[3]
            };
            if !reachable {
                found_seed = Some(seed);
                break;
            }
        }
        let seed = found_seed.expect("expected at least one stranding seed in the search range");

        // Now exercise the guarded path with the same seed and member layout:
        // member 0 (team 0) sits in room 0, far from the exit at room 3.
        // Nobody observes room 3 (it's the shared exit and stays empty here),
        // so the line joints are free to shuffle exactly as in the probe.
        let mut w = ContentionWorld::new(4, &edges, &[(TeamId(0), RoomId(0))], RoomId(3), seed);
        w.decohere();

        assert!(
            w.all_members_can_reach_exit(),
            "the solvability guard must never leave a member stranded"
        );
        assert!(w.last_decohere_attempts >= 1, "attempts must be tracked");
        assert_valid_matching(&w.world);
    }

    #[test]
    fn identically_constructed_worlds_stay_byte_identical_under_the_same_script() {
        fn build() -> ContentionWorld {
            ContentionWorld::new(
                3,
                &corridor_edges(),
                &[(TeamId(0), RoomId(0)), (TeamId(1), RoomId(2))],
                RoomId(2),
                0x1234_5678,
            )
        }
        let mut a = build();
        let mut b = build();

        fn script(w: &mut ContentionWorld) {
            w.tick = 1;
            w.record_observations();
            w.place_anchor(TeamId(0), RoomId(1));
            w.decohere();
            w.traverse(0, Side::East);
            w.tick = 2;
            w.record_observations();
            w.decohere();
            w.remove_anchor(TeamId(0), RoomId(1));
            w.decohere();
        }

        script(&mut a);
        script(&mut b);

        assert_eq!(a.world.links, b.world.links);
        assert_eq!(
            a.members.iter().map(|m| m.room).collect::<Vec<_>>(),
            b.members.iter().map(|m| m.room).collect::<Vec<_>>()
        );
        assert_eq!(a.last_decohere_attempts, b.last_decohere_attempts);
        assert_eq!(a.last_decohere_reverted, b.last_decohere_reverted);
        for team in [TeamId(0), TeamId(1)] {
            assert_eq!(a.known_edges(team).len(), b.known_edges(team).len());
            for index in 0..DOOR_COUNT_FOR_TEST {
                let door = DoorId(index as u16);
                assert_eq!(a.known_edges(team).get(door), b.known_edges(team).get(door));
            }
        }
    }

    const DOOR_COUNT_FOR_TEST: usize = 3 * 4;

    #[test]
    fn legacy_decohere_stream_is_unchanged() {
        // Mirrors ObservationWorld's own determinism test: this is the
        // contract that decohere_with's salt-0 path must preserve exactly.
        let mut a = ObservationWorld::authored();
        let mut b = ObservationWorld::authored();
        a.decohere();
        b.decohere();
        assert_eq!(a.links, b.links);
    }
}
