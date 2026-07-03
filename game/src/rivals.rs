//! Presentation-only **rival avatars** for the teleport match.
//!
//! The teleport model shows the player one discrete place at a time, so the old
//! whole-maze view's rival avatars were dropped. This brings them back where they can
//! actually be seen: when a rival team's clump occupies the *same room* the player is
//! standing in, it appears as a figure **walking** the room (toward the way out), rather
//! than popping in and out. Every team starts clumped at the entrance and the local team
//! (the fastest) pulls ahead, so you see rivals at the start and whenever a slower team
//! passes through your room.
//!
//! This is pure projection of the deterministic brain — it reads `team_room` /
//! `active_runner` and never writes match state, so determinism, replay, and lockstep are
//! untouched. The functions here are the testable core; `screens::sync_rival_avatars`
//! drives the Bevy entities from them.

use bevy::prelude::Vec2;
use observed_core::RoomId;
use observed_match::facility::{CompetitiveFacility, TEAM_COUNT};

use crate::flow::LOCAL_TEAM;
use crate::teleport::PlaceGeom;

/// How fast a rival paces its room (cycles of the walk per second). Cosmetic only.
pub const RIVAL_PACE_SPEED: f32 = 0.45;

/// The rival team indices whose clump currently occupies `room` and are still active
/// runners — i.e. the rivals you would physically see sharing your room. The local team
/// is never included (you don't watch yourself), nor are escaped/absorbed teams.
pub fn rivals_in_room(facility: &CompetitiveFacility, room: RoomId) -> Vec<usize> {
    (0..TEAM_COUNT)
        .filter(|&i| i as u8 != LOCAL_TEAM.0)
        .filter(|&i| facility.teams.get(i).is_some_and(|t| t.active_runner()))
        .filter(|&i| facility.team_room(i) == room)
        .collect()
}

/// A triangle wave of period 2: `0 → 1 → 0`. Used to pace a rival back and forth along
/// its walk segment with no seam — so it never jumps from the end back to the start (the
/// very "teleport" we're replacing).
pub fn triangle_wave(t: f32) -> f32 {
    let p = t.rem_euclid(2.0);
    if p < 1.0 { p } else { 2.0 - p }
}

/// The line segment a rival paces inside the current place's local frame (centred at 0):
/// from the back of the room to just inside the spine-forward doorway, so the walk reads
/// as "heading for the exit". Falls back to the +Z axis when there is no forward doorway
/// (e.g. the exit room). Both endpoints stay inside the footprint.
pub fn pace_segment(geom: &PlaceGeom) -> (Vec2, Vec2) {
    let extent = geom.half.min_element() * 0.6;
    let dir = geom
        .forward_gap()
        .map(|g| g.normal)
        .unwrap_or(Vec2::Y)
        .normalize_or_zero();
    (-dir * extent, dir * extent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_match_start_all_three_rivals_share_the_entrance() {
        let facility = CompetitiveFacility::authored();
        // Everyone is clumped at the entrance; the player (team 0) is here too.
        let entrance = facility.team_room(0);
        let rivals = rivals_in_room(&facility, entrance);
        assert_eq!(
            rivals,
            vec![1, 2, 3],
            "the three rivals are co-present at start"
        );
        assert!(
            !rivals.contains(&(LOCAL_TEAM.0 as usize)),
            "you never see your own team"
        );
    }

    #[test]
    fn listed_rivals_are_actually_here_and_active_and_never_the_local_team() {
        let mut facility = CompetitiveFacility::authored();
        // Advance the match so teams spread along the spine.
        for _ in 0..6 {
            if facility.finished {
                break;
            }
            facility.advance_round(&[]);
        }
        // For every spine room, the invariants must hold.
        for index in 0..TEAM_COUNT {
            let room = facility.team_room(index);
            for &i in &rivals_in_room(&facility, room) {
                assert_ne!(i as u8, LOCAL_TEAM.0, "the local team is never listed");
                assert!(facility.teams[i].active_runner(), "only active runners");
                assert_eq!(facility.team_room(i), room, "only rivals actually here");
            }
        }
    }

    #[test]
    fn triangle_wave_stays_in_range_and_has_no_seam() {
        assert!((triangle_wave(0.0) - 0.0).abs() < 1e-6);
        assert!((triangle_wave(1.0) - 1.0).abs() < 1e-6);
        assert!(
            (triangle_wave(2.0) - 0.0).abs() < 1e-6,
            "period 2, returns to 0"
        );
        // Across a fine sweep it always stays within [0, 1] (no out-of-bounds jump).
        for k in 0..400 {
            let v = triangle_wave(k as f32 * 0.013);
            assert!((0.0..=1.0).contains(&v), "wave stays in range at {k}");
        }
    }

    #[test]
    fn pace_segment_stays_inside_the_footprint() {
        let geom = PlaceGeom {
            half: Vec2::new(6.0, 4.0),
            gaps: Vec::new(), // no forward gap → +Z fallback
            interior: Vec::new(),
            poly: None,
            decks: Vec::new(),
        };
        let (a, b) = pace_segment(&geom);
        let bound = geom.half.min_element();
        assert!(
            a.length() <= bound && b.length() <= bound,
            "endpoints stay inside"
        );
        assert!(
            (a + b).length() < 1e-6,
            "the segment is centred on the room"
        );
    }
}
