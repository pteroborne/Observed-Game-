//! Hybrid match unit and integration tests.

#[cfg(test)]
mod tests {
    use super::super::match_state::tile_world;
    use super::super::round_step::route_all;
    use super::super::{CONTROL_ROOM, HybridMatch, HybridTape, LOCAL_TEAM, LocalAction};
    use crate::facility::{EXIT_CAPACITY, START_ROOM, TEAM_COUNT};
    use crate::maze::{GRID_H, GRID_W};
    use bevy::math::Vec3;
    use observed_core::RoomId;
    use observed_traversal::FpsConfig;
    use player_input::PlayerIntent;
    use std::collections::HashSet;

    #[test]
    fn the_hybrid_match_resolves_to_the_competitive_result() {
        let tape = HybridTape::record_demo(1);
        let end = tape.replay_to(tape.len());
        assert!(end.competitive.finished);
        assert_eq!(end.competitive.escaped_count(), EXIT_CAPACITY as usize);
        assert_eq!(end.competitive.winner, Some(LOCAL_TEAM));
        assert_eq!(
            end.competitive.escaped_count() + end.competitive.absorbed_count(),
            TEAM_COUNT
        );
    }

    #[test]
    fn replay_reproduces_match_maze_and_first_person_pose_exactly() {
        let tape = HybridTape::record_demo(2);
        for round in [
            0,
            1,
            tape.len() / 2,
            tape.len().saturating_sub(1),
            tape.len(),
        ] {
            assert!(tape.exact_at(round), "exact at round {round}");
            assert_eq!(tape.replay_to(round).snapshot(), tape.snapshots[round]);
        }
    }

    #[test]
    fn sequential_playback_matches_seek() {
        let tape = HybridTape::record_demo(3);
        let mut session = HybridMatch::authored(tape.seed);
        for round in 0..=tape.len() {
            assert_eq!(session.snapshot(), tape.replay_to(round).snapshot());
            if let Some(frame) = tape.frames.get(round) {
                session.apply_action(frame.local);
            }
        }
    }

    #[test]
    fn advance_uses_a_contiguous_real_floor_path() {
        let mut session = HybridMatch::authored(4);
        let before = session.maze_tiles.clone();
        let start = session.rooms[START_ROOM as usize].center_tile();
        let target = session.local_target().expect("spine continues");
        let goal = session.rooms[target.0 as usize].center_tile();
        assert!(session.apply_action(LocalAction::Advance));
        assert_eq!(session.last_traversal.first(), Some(&start));
        assert_eq!(session.last_traversal.last(), Some(&goal));
        for tile in &session.last_traversal {
            assert!(before[tile.1 * GRID_W + tile.0].is_floor());
        }
        for pair in session.last_traversal.windows(2) {
            assert_eq!(
                pair[0].0.abs_diff(pair[1].0) + pair[0].1.abs_diff(pair[1].1),
                1
            );
        }
    }

    #[test]
    fn live_advance_is_spatially_gated_to_the_target_room() {
        let mut session = HybridMatch::authored(5);
        assert_eq!(session.local_room(), RoomId(START_ROOM));
        assert_eq!(
            session.step_player(PlayerIntent::default(), false),
            None,
            "standing at spawn does not advance the match"
        );
        let target = session.local_target().expect("spine target");
        session.place_body_in_room(target);
        assert_eq!(
            session.step_player(PlayerIntent::default(), false),
            Some(LocalAction::Advance)
        );
        assert_eq!(session.local_room(), target);
    }

    #[test]
    fn reroutes_defer_in_view_and_commit_atomically_off_camera() {
        let mut session = HybridMatch::authored(6);
        session.competitive.advance_round(&[]);
        session.target = route_all(&session.competitive, &session.base, &session.rooms);
        let affected = session.affected_tiles();
        assert!(!affected.is_empty());
        let before = session.maze_tiles.clone();
        assert!(!session.try_commit_reroute(&affected, false));
        assert_eq!(session.maze_tiles, before);
        session.place_body_in_room(session.local_room());
        assert!(session.try_commit_reroute(&HashSet::new(), false));
        assert!(session.in_sync());
        assert!(session.navigable());
    }

    #[test]
    fn a_reroute_never_changes_the_player_footprint() {
        let mut session = HybridMatch::authored(7);
        session.competitive.advance_round(&[]);
        session.target = route_all(&session.competitive, &session.base, &session.rooms);
        let affected = session.affected_tiles();
        let &(x, y) = affected.iter().next().expect("reroute changes tiles");
        let point = tile_world(x, y);
        session.body.position.x = point.x;
        session.body.position.z = point.y;
        assert!(!session.try_commit_reroute(&HashSet::new(), false));
        assert!(!session.in_sync());
    }

    #[test]
    fn the_rendered_maze_stays_navigable_every_round() {
        let tape = HybridTape::record_demo(8);
        for round in 0..=tape.len() {
            let session = tape.replay_to(round);
            assert!(session.navigable(), "navigable at round {round}");
            assert!(session.player_on_floor(), "player remains on floor");
        }
    }

    #[test]
    fn match_replay_and_network_snapshots_include_multi_level_elevation() {
        let tape = HybridTape::record_demo(81);
        assert!(
            tape.snapshots
                .iter()
                .all(|snapshot| snapshot.elevation_steps.len() == GRID_W * GRID_H)
        );
        assert!(
            tape.snapshots
                .iter()
                .any(|snapshot| snapshot.body_position.y > FpsConfig::default().half_height + 0.5),
            "the canonical match path reaches an elevated room"
        );
        for round in 0..=tape.len() {
            assert_eq!(
                tape.replay_to(round).snapshot().elevation_steps,
                tape.snapshots[round].elevation_steps,
                "elevation field is replay-exact at round {round}"
            );
        }
    }

    #[test]
    fn every_spine_leg_offers_a_short_trapped_route_and_long_safe_bypass() {
        for seed in [1, 2, 17, 82, 999] {
            let session = HybridMatch::authored(seed);
            let spine = session
                .rendered
                .iter()
                .filter(|route| route.spine)
                .collect::<Vec<_>>();
            assert!(!spine.is_empty());
            for route in spine {
                assert!(
                    !route.safe_path.is_empty() && !route.trap_tiles.is_empty(),
                    "seed {seed} spine leg {:?} must expose both choices",
                    route.rooms
                );
                assert!(
                    route.safe_path.len() > route.path.len(),
                    "safe route is the deliberate detour"
                );
                assert!(
                    route
                        .safe_path
                        .iter()
                        .all(|tile| !route.trap_tiles.contains(tile)),
                    "safe bypass avoids its pressure gate"
                );
            }
        }
    }

    #[test]
    fn scripted_replay_uses_the_safe_route() {
        let mut session = HybridMatch::authored(83);
        let traps = session.trap_tiles.clone();
        assert!(session.apply_action(LocalAction::Advance));
        assert!(
            session
                .last_traversal
                .iter()
                .all(|tile| !traps.contains(tile)),
            "canonical replay path takes the safe bypass"
        );
    }

    #[test]
    fn active_pressure_gate_sets_back_without_removing_progress() {
        let mut session = HybridMatch::authored(84);
        let trap = *session.trap_tiles.iter().next().expect("generated trap");
        let point = tile_world(trap.0, trap.1);
        session.body.position = Vec3::new(
            point.x,
            session.floor_height(trap.0, trap.1) + session.config.half_height,
            point.y,
        );
        session.body.velocity = Vec3::ZERO;
        session.body.grounded = true;
        let room_before = session.local_room();
        let round_before = session.competitive.round;

        assert_eq!(session.step_player(PlayerIntent::default(), false), None);
        assert_eq!(session.trap_hits, 1);
        assert_eq!(session.local_room(), room_before);
        assert_eq!(
            session.competitive.round, round_before,
            "trap costs time and position, never earned progress"
        );
        assert_eq!(session.player_room(), Some(room_before));
        assert!(session.trap_cooldown_ticks > 0);
    }

    #[test]
    fn inactive_pressure_gate_allows_the_risky_shortcut() {
        let mut session = HybridMatch::authored(85);
        let trap = *session.trap_tiles.iter().next().expect("generated trap");
        let point = tile_world(trap.0, trap.1);
        session.body.position = Vec3::new(
            point.x,
            session.floor_height(trap.0, trap.1) + session.config.half_height,
            point.y,
        );
        session.body.velocity = Vec3::ZERO;
        session.body.grounded = true;
        session.hazard_tick = super::super::TRAP_ACTIVE_TICKS;

        session.step_player(PlayerIntent::default(), false);
        assert_eq!(session.trap_hits, 0);
        assert_eq!(session.player_tile(), Some(trap));
    }

    #[test]
    fn committed_reroute_emits_first_person_feedback() {
        let mut session = HybridMatch::authored(86);
        session.competitive.advance_round(&[]);
        session.target = route_all(&session.competitive, &session.base, &session.rooms);
        session.place_body_in_room(session.local_room());
        assert!(session.try_commit_reroute(&HashSet::new(), false));
        assert_eq!(
            session.reroute_feedback_ticks,
            super::super::REROUTE_FEEDBACK_TICKS
        );
    }

    #[test]
    fn the_rendered_maze_actually_reroutes_during_the_match() {
        let tape = HybridTape::record_demo(9);
        let first = &tape.snapshots[0].rendered_routes;
        assert!(
            tape.snapshots
                .iter()
                .any(|snapshot| &snapshot.rendered_routes != first),
            "at least one authoritative reroute commits to the rendered maze"
        );
    }

    #[test]
    fn the_control_is_spatially_gated() {
        let mut session = HybridMatch::authored(10);
        assert!(!session.can_seize());
        while session.local_room() != CONTROL_ROOM {
            assert!(session.apply_action(LocalAction::Advance));
        }
        assert!(session.can_seize());
        assert!(session.apply_action(LocalAction::Seize));
        assert_eq!(session.competitive.control_holder, Some(LOCAL_TEAM));
    }

    #[test]
    fn scripted_recordings_are_identical() {
        let a = HybridTape::record_demo(11);
        let b = HybridTape::record_demo(11);
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.snapshots, b.snapshots);
    }
}
