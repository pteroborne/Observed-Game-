//! A deterministic driver.
//!
//! It exists so a seam can be *compared*: run the same scripted squad under two
//! `ModeSpec`s and diff the digests. It is a simple player, not a good one, but
//! it has to be good enough to exercise each mechanic, or a seam test compares
//! two matches in which that mechanic never fired. Three behaviours earn their
//! place on exactly that ground: overwatch (or no flag is ever planted), rescue
//! (or the prison is a one-way door), and refresh (or every pawn stays stale
//! forever and stink base recency never decides anything).
//!
//! Each team is planned independently. Planning both as one squad — which an
//! earlier draft did — has team 1 escorting team 0's planter, and the mode
//! looks broken when only the driver is.

use observed_hex::coords::{HexCoord, lateral_distance};
use observed_hex::faces::HexFace;

use crate::sim::state::{Action, Intent, MatchState, PawnId, TeamId};
use crate::spec::Rules;

/// The lateral face pointing from `from` at its neighbour `to`, if adjacent.
fn face_towards(state: &MatchState, from: HexCoord, to: HexCoord) -> Option<HexFace> {
    HexFace::LATERAL
        .into_iter()
        .find(|&face| state.board.size().neighbor(from, face) == Some(to))
}

fn hold(state: &MatchState, pawn: PawnId) -> Intent {
    Intent {
        pawn,
        facing: state.pawn(pawn).facing,
        action: Action::Hold,
    }
}

/// Within reach of a guardian's next move.
///
/// Guardians close one hex at a time, so any cell adjacent to one is a cell you
/// may not still be standing in. Without this the driver walks straight into
/// them and every trace reads as "the guardians are unbeatable" when what is
/// actually unbeatable is a squad that cannot see them.
fn exposed(state: &MatchState, cell: HexCoord) -> bool {
    state
        .guardians
        .iter()
        .any(|guardian| lateral_distance(guardian.at, cell) <= 1)
}

/// One step towards `goal`, preferring cover over speed.
///
/// Candidates are ranked `(exposed, distance-to-goal, index)`: a safe slower
/// step beats a fast one into a guardian's reach, and holding is itself a
/// candidate, so a cornered pawn stands rather than walking into the arms of
/// whatever cornered it.
fn advance(state: &MatchState, pawn: PawnId, goal: HexCoord) -> Intent {
    let at = state.pawn(pawn).at;
    let size = state.board.size();
    let field = state.board.distance_field(goal);

    let mut options: Vec<(bool, u32, usize, Option<HexFace>)> = state
        .board
        .open_neighbours(at)
        .map(|(face, cell)| {
            (
                exposed(state, cell),
                field[size.index(cell)],
                size.index(cell),
                Some(face),
            )
        })
        .filter(|&(_, distance, _, _)| distance != u32::MAX)
        .collect();
    options.push((
        exposed(state, at),
        field[size.index(at)],
        size.index(at),
        None,
    ));
    options.sort_unstable();

    match options.first() {
        Some(&(_, _, _, Some(face))) => Intent {
            pawn,
            facing: face,
            action: Action::Step(face),
        },
        _ => hold(state, pawn),
    }
}

/// The freshest rival within two hexes that could take this pawn.
fn threatening_rival(state: &MatchState, pawn: PawnId) -> Option<u16> {
    let me = state.pawn(pawn);
    let mine = state.freshness(pawn);
    state
        .free_pawns()
        .filter(|other| other.team != me.team)
        .filter(|other| lateral_distance(me.at, other.at) <= 2)
        .map(|other| state.freshness(other.id))
        .filter(|&theirs| theirs > mine)
        .max()
}

#[must_use]
pub fn intents(state: &MatchState, _rules: &Rules) -> Vec<Intent> {
    let mut out: Vec<Intent> = state
        .teams()
        .into_iter()
        .flat_map(|team| team_intents(state, team))
        .collect();
    out.sort_by_key(|intent| intent.pawn);
    out
}

/// One team's plan. Public so a human can drive one side while the bot drives
/// the rest, which is what the browser build does.
#[must_use]
pub fn team_intents(state: &MatchState, team: TeamId) -> Vec<Intent> {
    let free: Vec<PawnId> = state
        .free_pawns()
        .filter(|pawn| pawn.team == team)
        .map(|pawn| pawn.id)
        .collect();
    if free.is_empty() {
        return Vec::new();
    }
    let base = state.base_of(team);
    let prison = state.prison_for(team);
    let held = state
        .pawns
        .iter()
        .any(|pawn| pawn.team == team && pawn.jailed);

    // One target for the squad: splitting up cannot satisfy overwatch.
    let target = state
        .unplanted_flags()
        .filter(|flag| {
            free.iter()
                .any(|&id| state.board.connected(state.pawn(id).at, flag.at))
        })
        .min_by_key(|flag| {
            free.iter()
                .map(|&id| lateral_distance(state.pawn(id).at, flag.at))
                .min()
                .unwrap_or(u32::MAX)
        })
        .map(|flag| flag.at);

    // Spare a pawn for the prison when teammates are held, but never the last
    // one — a squad that empties itself to rescue is only ever half a squad.
    let rescuer = (held && free.len() > 1)
        .then(|| {
            free.iter()
                .copied()
                .min_by_key(|&id| (lateral_distance(state.pawn(id).at, prison), id))
        })
        .flatten();

    // If the whole squad is stale, send the pawn nearest home to refresh.
    // Nobody would otherwise, and recency would never decide anything.
    let all_stale = free
        .iter()
        .all(|&id| state.freshness(id) == 0 && state.pawn(id).at != base);
    let refresher = (all_stale && free.len() > 1 && state.turn > 1)
        .then(|| {
            free.iter()
                .copied()
                .filter(|&id| Some(id) != rescuer)
                .min_by_key(|&id| (lateral_distance(state.pawn(id).at, base), id))
        })
        .flatten();

    let Some(target) = target else {
        return free.into_iter().map(|id| hold(state, id)).collect();
    };

    let planter = free
        .iter()
        .copied()
        .filter(|&id| Some(id) != rescuer && Some(id) != refresher)
        .min_by_key(|&id| (lateral_distance(state.pawn(id).at, target), id))
        .or_else(|| free.first().copied())
        .expect("free is non-empty");

    let mut posts: Vec<HexCoord> = state
        .board
        .open_neighbours(target)
        .map(|(_, cell)| cell)
        .collect();

    free.into_iter()
        .map(|pawn| {
            // Outranked and close enough to be taken: go home and come back
            // fresher. This is the trade stink base exists to create.
            if threatening_rival(state, pawn).is_some() {
                return advance(state, pawn, base);
            }
            if Some(pawn) == rescuer {
                return advance(state, pawn, prison);
            }
            if Some(pawn) == refresher {
                return advance(state, pawn, base);
            }
            if pawn == planter {
                let at = state.pawn(pawn).at;
                return if at == target {
                    Intent {
                        pawn,
                        facing: state.pawn(pawn).facing,
                        action: Action::Plant,
                    }
                } else {
                    advance(state, pawn, target)
                };
            }
            let at = state.pawn(pawn).at;
            if let Some(face) = face_towards(state, at, target) {
                posts.retain(|&post| post != at);
                return Intent {
                    pawn,
                    facing: face,
                    action: Action::Hold,
                };
            }
            let Some(index) = posts
                .iter()
                .enumerate()
                .min_by_key(|&(_, &post)| lateral_distance(at, post))
                .map(|(index, _)| index)
            else {
                return advance(state, pawn, target);
            };
            let post = posts.remove(index);
            advance(state, pawn, post)
        })
        .collect()
}
