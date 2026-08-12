//! Fog of war: what a squad knows, as distinct from what is true.
//!
//! The *types* are the shipped ones — [`HexPlayerMapKnowledge`],
//! [`HexMapCellKnowledge`], [`HexMapDiscovery`], and above all
//! `HexMapCellKnowledge::is_stale`, which is the subtle part: a remembered cell
//! goes stale when its **revision** moves, not when the facility's generation
//! does, so an unrelated shift across the map does not blank your notes.
//!
//! The recording loop is the lab's own, because `Sight` is a setting this game
//! has and the shipped one does not. `observe_team` hardcodes one hop of glimpse
//! around each body; reusing it would pin `Sight` to `Adjacent` forever. The one
//! rule it enforces that must survive being rewritten here: **glimpsing a cell
//! does not reveal what the room does.** You learn a room's function by standing
//! in it or by surveying it from a monitor, never by seeing its doorway.

use std::collections::BTreeSet;

use observed_facility::hex_wfc::HexWfcWorld;
use observed_hex::HexCoord;
use observed_match::hex_wfc::{HexMapCellKnowledge, HexMapDiscovery, HexPlayerMapKnowledge};

use super::objectives::room_at;

/// Record a turn's observation into a squad's map.
///
/// `occupied` are the cells units are standing in — traversed, role revealed.
/// `observed` is everything the squad's sight reaches, including those cells —
/// glimpsed, role withheld. `anchored` cells are marked so the map can show
/// which notes are guaranteed still accurate.
pub fn record_turn(
    knowledge: &mut HexPlayerMapKnowledge,
    world: &HexWfcWorld,
    occupied: &BTreeSet<HexCoord>,
    observed: &BTreeSet<HexCoord>,
    anchored: &BTreeSet<HexCoord>,
) {
    // Anchoring is re-derived every turn rather than accumulated: an anchor that
    // was picked up must stop claiming to hold its cell, or the map would show a
    // guarantee the facility is no longer honouring.
    for known in knowledge.cells.values_mut() {
        known.anchored = false;
    }
    for &cell in observed {
        let traversed = occupied.contains(&cell);
        record(
            knowledge,
            world,
            cell,
            if traversed {
                HexMapDiscovery::Traversed
            } else {
                HexMapDiscovery::Glimpsed
            },
            anchored.contains(&cell),
            traversed,
        );
    }
    for &cell in anchored {
        record(
            knowledge,
            world,
            cell,
            HexMapDiscovery::Traversed,
            true,
            true,
        );
    }
}

/// Reveal a bounded schematic around a monitor room.
///
/// This is the one place a role is learned at a distance, and it is the payoff
/// for spending an action in a monitor room. Delegates to the shipped
/// `survey_local` so the two games agree on what a survey is worth.
pub fn survey(
    knowledge: &mut HexPlayerMapKnowledge,
    world: &HexWfcWorld,
    origin: HexCoord,
    hops: usize,
) {
    knowledge.survey_local(world, origin, hops);
}

fn record(
    knowledge: &mut HexPlayerMapKnowledge,
    world: &HexWfcWorld,
    cell: HexCoord,
    discovery: HexMapDiscovery,
    anchored: bool,
    role_known: bool,
) {
    let Some(placement) = world.placements.get(&cell) else {
        return;
    };
    let revision = world.cell_revisions.get(&cell).copied().unwrap_or(0);
    let known_ports = placement.ports();
    let room_role = role_known
        .then(|| room_at(world, cell).map(|(_, role, _)| role))
        .flatten();
    knowledge
        .cells
        .entry(cell)
        .and_modify(|known| {
            if discovery == HexMapDiscovery::Traversed {
                known.discovery = HexMapDiscovery::Traversed;
            }
            known.last_confirmed_revision = revision;
            known.known_ports = known_ports;
            known.anchored |= anchored;
            if room_role.is_some() {
                known.room_role = room_role;
            }
        })
        .or_insert(HexMapCellKnowledge {
            discovery,
            last_confirmed_revision: revision,
            known_ports,
            anchored,
            room_role,
        });
}
