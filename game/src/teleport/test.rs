#[cfg(test)]
mod tests {
    use crate::hallway;
    use crate::teleport::geom::outward_normal;
    use crate::teleport::*;
    use bevy::math::{Vec2, Vec3};
    use observed_core::RoomId;
    use observed_match::mutable::EXIT_ROOM;
    use observed_traversal::FpsArena;
    use std::f32::consts::PI;

    fn nav(connections: &[u32], target: Option<u32>) -> Nav {
        Nav {
            connections: connections.iter().map(|&r| RoomId(r)).collect(),
            connection_slots: connections
                .iter()
                .enumerate()
                .map(|(slot, &target)| RoomConnectionSlot {
                    target: RoomId(target),
                    slot: ThresholdSlotId(slot as u8),
                })
                .collect(),
            hallway_entry_room_slot: None,
            hallway_exit_room_slot: None,
            target_room: target.map(RoomId),
            seed: 1,
            version: 0,
            exit_locked: false,
            pins: Vec::new(),
        }
    }

    fn test_threshold(room: RoomId, target: RoomId) -> ThresholdLink {
        ThresholdLink {
            room: RoomThreshold {
                room,
                slot: ThresholdSlotId(0),
            },
            hall: HallThreshold {
                hall: HallId::new(room, target),
                side: room,
                slot: ThresholdSlotId(0),
            },
            local_side: ThresholdLocalSide::Room,
        }
    }

    #[test]
    fn room_geom_has_a_gap_per_connection_and_marks_the_forward_one() {
        let geom = room_geom(
            RoomId(0),
            &[RoomId(1), RoomId(3), RoomId(5)],
            Some(RoomId(3)),
            7,
        );
        assert_eq!(geom.gaps.len(), 3);
        let forward = geom
            .forward_gap()
            .expect("a forward gap toward the objective");
        assert_eq!(forward.target, RoomId(3));
        assert_eq!(
            geom.gaps
                .iter()
                .filter(|g| g.kind == GapKind::Forward)
                .count(),
            1
        );
    }

    #[test]
    fn room_threshold_slots_are_stable_across_connection_changes() {
        let seed = 17;
        let full = room_geom_with_slots(
            RoomId(0),
            &[RoomId(1), RoomId(3)],
            &[
                RoomConnectionSlot {
                    target: RoomId(1),
                    slot: ThresholdSlotId(1),
                },
                RoomConnectionSlot {
                    target: RoomId(3),
                    slot: ThresholdSlotId(3),
                },
            ],
            Some(RoomId(1)),
            seed,
        );
        let reduced = room_geom_with_slots(
            RoomId(0),
            &[RoomId(1)],
            &[RoomConnectionSlot {
                target: RoomId(1),
                slot: ThresholdSlotId(1),
            }],
            Some(RoomId(1)),
            seed,
        );

        let full_gap = full.gaps.iter().find(|g| g.target == RoomId(1)).unwrap();
        let reduced_gap = reduced.gaps.iter().find(|g| g.target == RoomId(1)).unwrap();

        assert_eq!(
            full_gap.threshold.room,
            RoomThreshold {
                room: RoomId(0),
                slot: ThresholdSlotId(1),
            }
        );
        assert!((full_gap.center - reduced_gap.center).length() < 0.001);
        assert!(full_gap.normal.dot(reduced_gap.normal) > 0.999);
    }

    #[test]
    fn rooms_are_convex_polygons_with_enough_edges_for_their_doorways() {
        // Across seeds, a room is a 4â€“8 sided convex polygon with at least one edge per
        // connection, and its gaps sit on distinct edges (their centres differ).
        for seed in 0..40u64 {
            let geom = room_geom(
                RoomId(0),
                &[RoomId(1), RoomId(3), RoomId(5)],
                Some(RoomId(3)),
                seed,
            );
            let poly = geom.poly.as_ref().expect("a room is a polygon");
            assert!(
                (4..=8).contains(&poly.len()) && poly.len() >= geom.gaps.len(),
                "seed {seed}: {} sides for {} doors",
                poly.len(),
                geom.gaps.len()
            );
            // Distinct doorway edges.
            for i in 0..geom.gaps.len() {
                for j in (i + 1)..geom.gaps.len() {
                    assert!(
                        (geom.gaps[i].center - geom.gaps[j].center).length() > 0.5,
                        "seed {seed}: doorways share an edge"
                    );
                }
            }
        }
    }

    #[test]
    fn observation_rooms_have_monitor_walls_plus_doorway_edges() {
        for room in [RoomId(5), RoomId(6)] {
            let geom = room_geom(
                room,
                &[RoomId(1), RoomId(3), RoomId(7), RoomId(8)],
                Some(RoomId(8)),
                7,
            );
            let poly = geom.poly.as_ref().expect("a room is a polygon");
            assert_eq!(poly.len(), 13);
            assert!(
                poly.len() - geom.gaps.len() >= 9,
                "observation rooms need nine non-door wall faces for monitors"
            );
        }
    }

    #[test]
    fn varied_straight_hallways_have_distinct_lengths() {
        // The straight connector renders at visibly different depths per edge seed.
        let template = hallway::template(0);
        let a = hallway_geom(RoomId(0), RoomId(1), template, 11, false)
            .half
            .y;
        let differ = (0..64u64).any(|s| {
            (hallway_geom(RoomId(0), RoomId(1), template, s, false)
                .half
                .y
                - a)
                .abs()
                > 1.0
        });
        assert!(
            differ,
            "straight hallway length should vary with the edge seed"
        );
    }

    #[test]
    fn hallway_geom_has_an_entry_and_an_exit() {
        let template = hallway::template(0);
        let geom = hallway_geom(RoomId(0), RoomId(1), template, 0, false);
        assert!(
            geom.gaps
                .iter()
                .any(|g| g.kind == GapKind::Entry && g.target == RoomId(0))
        );
        assert!(
            geom.gaps
                .iter()
                .any(|g| g.kind == GapKind::Exit && g.target == RoomId(1))
        );
    }

    #[test]
    fn room_preview_and_crossing_align_to_the_same_hall_threshold_slot() {
        let template = maze_templates()
            .into_iter()
            .find(|template| template.grid.is_some())
            .expect("at least one maze hallway template");
        let hall = (0..64_u64)
            .map(|seed| {
                hallway_geom_with_slots(
                    RoomId(0),
                    RoomId(1),
                    ThresholdSlotId(2),
                    ThresholdSlotId(1),
                    template,
                    seed,
                    false,
                )
            })
            .find(|geom| {
                geom.gaps
                    .iter()
                    .filter(|g| g.kind == GapKind::Entry)
                    .count()
                    > 1
            })
            .expect("maze template exposes multiple entry apertures");
        let room = room_geom_with_slots(
            RoomId(0),
            &[RoomId(1)],
            &[RoomConnectionSlot {
                target: RoomId(1),
                slot: ThresholdSlotId(2),
            }],
            Some(RoomId(1)),
            29,
        );
        let room_gap = *room.forward_gap().expect("room has a forward gap");
        let selected_hall_gap = hall
            .gaps
            .iter()
            .find(|gap| gap.threshold.hall == room_gap.threshold.hall)
            .expect("hall contains the selected threshold slot");

        let align = hallway_alignment(&room_gap, &hall).expect("slot alignment resolves");
        let selected_world = align.apply(selected_hall_gap.center);
        let expected = room_gap.center + room_gap.normal * PREVIEW_OUTSET;

        assert_eq!(
            room_gap.threshold.room,
            RoomThreshold {
                room: RoomId(0),
                slot: ThresholdSlotId(2),
            }
        );
        assert_eq!(selected_hall_gap.threshold.hall, room_gap.threshold.hall);
        assert!((selected_world - expected).length() < 0.001);
    }

    #[test]
    fn crossing_detects_an_outward_pass_through_the_gap() {
        let gap = DoorGap {
            center: Vec2::new(0.0, -ROOM_HALF),
            normal: Vec2::new(0.0, -1.0),
            width: THRESHOLD_WIDTH,
            target: RoomId(2),
            kind: GapKind::Forward,
            threshold: test_threshold(RoomId(0), RoomId(2)),
        };
        // Walk from inside (z > -ROOM_HALF) to outside (z < -ROOM_HALF), on-centre.
        assert!(crossed(
            Vec2::new(0.0, -ROOM_HALF + 0.5),
            Vec2::new(0.0, -ROOM_HALF - 0.5),
            &gap
        ));
        // Moving away (inward) does not cross.
        assert!(!crossed(
            Vec2::new(0.0, -ROOM_HALF + 0.5),
            Vec2::new(0.0, 0.0),
            &gap
        ));
        // Crossing the threshold plane but outside the gap width does not count.
        assert!(!crossed(
            Vec2::new(THRESHOLD_WIDTH, -ROOM_HALF + 0.5),
            Vec2::new(THRESHOLD_WIDTH, -ROOM_HALF - 0.5),
            &gap
        ));
    }

    #[test]
    fn the_room_hallway_room_loop_advances_to_the_target() {
        // In room 0, objective is room 1; connections 0â†”1 and 0â†”3.
        let nav = nav(&[1, 3], Some(1));
        let place = Place::Room(RoomId(0));
        let forward = *geom_for(place, &nav).forward_gap().unwrap();
        assert_eq!(forward.target, RoomId(1));

        // Cross the forward doorway â†’ enter the 0â†’1 hallway with the edge's variation.
        let (place, crossing) = apply_crossing(place, &forward, &nav);
        assert_eq!(
            crossing,
            Crossing::EnteredHallway {
                from: RoomId(0),
                to: RoomId(1)
            }
        );
        assert_eq!(
            place,
            Place::Hallway {
                from: RoomId(0),
                to: RoomId(1),
                variation: hallway::variation_for(RoomId(0), RoomId(1), nav.seed, nav.version),
            }
        );

        // Walk to the hallway's exit and cross â†’ arrive in room 1.
        let exit = *geom_for(place, &nav)
            .gaps
            .iter()
            .find(|g| g.kind == GapKind::Exit)
            .unwrap();
        let (place, crossing) = apply_crossing(place, &exit, &nav);
        assert_eq!(crossing, Crossing::ArrivedRoom(RoomId(1)));
        assert_eq!(place, Place::Room(RoomId(1)));
    }

    #[test]
    fn an_anchored_edge_keeps_its_hallway_through_decoherence() {
        let mut n = nav(&[1, 3], Some(1));
        n.version = 5; // the live structure has rerolled five times
        // Without a pin, edge (0,1) follows the live decohere version.
        assert_eq!(n.effective_version(RoomId(0), RoomId(1)), 5);
        // Pin edge (0,1) at version 2 (when the torch was dropped).
        n.pins.push(PinnedEdge {
            a: RoomId(0),
            b: RoomId(1),
            version: 2,
        });
        assert_eq!(n.effective_version(RoomId(0), RoomId(1)), 2);
        assert_eq!(
            n.effective_version(RoomId(1), RoomId(0)),
            2,
            "the pin is edge-unordered"
        );
        // A different edge is unaffected â€” it still re-rolls.
        assert_eq!(n.effective_version(RoomId(0), RoomId(3)), 5);
        // Crossing into the pinned edge yields the frozen variation, not the live one.
        let gap = *room_geom(RoomId(0), &n.connections, n.target_room, 1)
            .forward_gap()
            .unwrap();
        let (place, _) = apply_crossing(Place::Room(RoomId(0)), &gap, &n);
        let pinned = match place {
            Place::Hallway { variation, .. } => variation,
            _ => panic!("entered a hallway"),
        };
        assert_eq!(
            pinned,
            hallway::variation_for(RoomId(0), RoomId(1), n.seed, 2)
        );
    }

    #[test]
    fn entry_spawn_places_the_body_just_inside_the_arrival_gap() {
        // Arriving in a room from room 0: spawn just inside the doorway back to 0.
        let geom = room_geom(RoomId(1), &[RoomId(0), RoomId(2)], Some(RoomId(2)), 5);
        let spawn = entry_spawn(&geom, RoomId(0));
        let back = geom.gaps.iter().find(|g| g.target == RoomId(0)).unwrap();
        // Spawn is inset inward from the gap (closer to the room centre).
        assert!(spawn.length() < back.center.length());
    }

    #[test]
    fn align2d_inverse_round_trips() {
        let a = Align2d {
            yaw: 0.9,
            offset: Vec2::new(3.0, -4.0),
        };
        for p in [Vec2::new(1.0, 2.0), Vec2::new(-5.0, 0.3), Vec2::ZERO] {
            let back = a.inverse_apply(a.apply(p));
            assert!((back - p).length() < 1e-4, "round trip {p:?} -> {back:?}");
        }
    }

    #[test]
    fn crossing_a_doorway_carries_the_body_in_continuously() {
        // Room 0 â†’ its 0â†’1 hallway across the forward gap: the alignment maps the body's
        // pose continuously into the hallway frame â€” no snap, no view flip.
        let nav = nav(&[1, 3], Some(1));
        let room = Place::Room(RoomId(0));
        let gap = *geom_for(room, &nav).forward_gap().unwrap();
        let (hall, _) = apply_crossing(room, &gap, &nav);
        let hgeom = geom_for(hall, &nav);
        let align = crossing_alignment(&hgeom, hall, &gap, RoomId(0)).expect("hallway alignment");

        // A body just past the room doorway (outward, along the gap normal) maps to just
        // inside the hallway entry (âˆ’Z side of its footprint), not snapped elsewhere.
        let threshold = gap.center + gap.normal * 0.3;
        let inside = align.inverse_apply(threshold);
        assert!(
            inside.y < 0.0 && inside.y > -hgeom.half.y,
            "lands just inside the hallway entry: {inside:?}"
        );
        assert!(inside.x.abs() <= hgeom.half.x, "within the hallway width");

        // Heading carries through: walking out through the gap (forward == gap normal)
        // becomes walking +Z into the hallway, regardless of the doorway's facing.
        let old_yaw = gap.normal.x.atan2(-gap.normal.y); // forward(old_yaw) == gap.normal
        let new_yaw = old_yaw + align.yaw;
        let fwd = Vec2::new(new_yaw.sin(), -new_yaw.cos());
        assert!(fwd.y > 0.9, "now facing into the hallway (+Z): {fwd:?}");
    }

    #[test]
    fn entering_a_room_keeps_the_arrival_doorway_an_open_passage() {
        // Room 1 connects back to 0 (where we came from) and on to 2 (the objective).
        let mut geom = room_geom(RoomId(1), &[RoomId(0), RoomId(2)], Some(RoomId(2)), 5);
        // By default the doorway back toward 0 is a sealed Side wall.
        let back = geom.gaps.iter().find(|g| g.target == RoomId(0)).unwrap();
        assert_eq!(back.kind, GapKind::Side);
        // Re-opening the arrival doorway makes it a real passage (so it doesn't pop into a
        // wall behind you) while the forward objective doorway is untouched.
        open_entry(&mut geom, Some(RoomId(0)));
        let back = geom.gaps.iter().find(|g| g.target == RoomId(0)).unwrap();
        assert_eq!(back.kind, GapKind::Entry);
        assert!(back.kind.is_passage());
        assert!(geom.forward_gap().is_some(), "forward doorway is preserved");
        // The start room (no arrival doorway) keeps every non-forward door sealed.
        let mut start = room_geom(RoomId(1), &[RoomId(0), RoomId(2)], Some(RoomId(2)), 5);
        open_entry(&mut start, None);
        assert!(start.gaps.iter().all(|g| g.kind != GapKind::Entry));
    }

    #[test]
    fn crossing_a_hallway_exit_carries_the_body_into_the_room_continuously() {
        // Hallway 0â†’1 exit into room 1 (which connects back to 0 and on to 2).
        let nav1 = nav(&[0, 2], Some(2));
        let hall = Place::Hallway {
            from: RoomId(0),
            to: RoomId(1),
            variation: hallway::variation_for(RoomId(0), RoomId(1), nav1.seed, nav1.version),
        };
        let hgeom = geom_for(hall, &nav1);
        let exit = *hgeom.gaps.iter().find(|g| g.kind == GapKind::Exit).unwrap();
        let mut rgeom = geom_for(Place::Room(RoomId(1)), &nav1);
        open_entry(&mut rgeom, Some(RoomId(0)));
        let align = crossing_alignment(&rgeom, Place::Room(RoomId(1)), &exit, RoomId(0))
            .expect("the arrival doorway resolves an alignment");
        // A body just past the hallway exit maps to inside the destination room footprint.
        let threshold = exit.center + exit.normal * 0.3;
        let inside = align.inverse_apply(threshold);
        assert!(
            inside.x.abs() <= rgeom.half.x + 0.6 && inside.y.abs() <= rgeom.half.y + 0.6,
            "lands inside the room footprint: {inside:?} (half {:?})",
            rgeom.half,
        );
    }

    #[test]
    fn an_edge_rolls_its_hallway_by_decohere_version() {
        let nav = nav(&[1], Some(1));
        let gap = *room_geom(RoomId(0), &nav.connections, nav.target_room, 1)
            .forward_gap()
            .unwrap();
        let (place, _) = apply_crossing(Place::Room(RoomId(0)), &gap, &nav);
        let v0 = match place {
            Place::Hallway { variation, .. } => variation,
            _ => panic!("entered a hallway"),
        };
        assert_eq!(
            v0,
            hallway::variation_for(RoomId(0), RoomId(1), nav.seed, nav.version)
        );
        // The selection is version-keyed, so an unobserved re-roll can change it.
        assert!((1..32).any(|v| hallway::variation_for(RoomId(0), RoomId(1), nav.seed, v) != v0));
    }

    fn inside_any_solid(arena: &FpsArena, p: Vec3) -> bool {
        arena.solids.iter().any(|s| {
            p.x >= s.min.x
                && p.x <= s.max.x
                && p.y >= s.min.y
                && p.y <= s.max.y
                && p.z >= s.min.z
                && p.z <= s.max.z
        })
    }

    /// The most-violated wall signed distance for `p` (positive = inside), ignoring open
    /// doorway edges. >= radius means the body is safely contained.
    fn deepest_inside(geom: &PlaceGeom, p: Vec2) -> f32 {
        let poly = geom.poly.as_ref().unwrap();
        let n = poly.len();
        let mut worst = f32::INFINITY;
        for i in 0..n {
            let a = poly[i];
            let b = poly[(i + 1) % n];
            let mid = (a + b) * 0.5;
            let is_door = geom
                .gaps
                .iter()
                .any(|g| g.kind.is_passage() && (g.center - mid).length() < 0.05);
            if is_door {
                continue;
            }
            worst = worst.min((p - a).dot(-outward_normal(a, b)));
        }
        worst
    }

    #[test]
    fn a_polygon_room_contains_the_body_but_opens_at_the_doorway() {
        let geom = room_geom(RoomId(0), &[RoomId(1)], Some(RoomId(1)), 4);
        let r = 0.4;
        // A polygon room has no AABB walls â€” its angled walls are the `contain` clamp.
        assert!(
            place_arena(&geom, 0.0, 3.4).solids.is_empty(),
            "a polygon room collides only with the floor"
        );
        let gap = *geom.forward_gap().unwrap();
        // A body driven far outside a wall (away from the door) is pulled back inside.
        let clamped = contain(&geom, -gap.normal * 100.0, r);
        assert!(
            deepest_inside(&geom, clamped) >= r - 0.1,
            "a body outside a wall is contained inside the room"
        );
        // Stepping out through the doorway is allowed (not clamped back).
        let at_door = gap.center + gap.normal * 0.3;
        let after = contain(&geom, at_door, r);
        assert!(
            (after - at_door).length() < 0.01,
            "the doorway stays open so the body can cross"
        );
    }

    #[test]
    fn hallway_arena_opens_both_ends_and_walls_the_sides() {
        let template = hallway::template(0);
        let geom = hallway_geom(RoomId(0), RoomId(1), template, 0, false);
        let arena = place_arena(&geom, 0.0, 3.4);
        let y = 1.0;
        // Entry (âˆ’Z) and exit (+Z) are open at the centreline.
        assert!(!inside_any_solid(&arena, Vec3::new(0.0, y, -geom.half.y)));
        assert!(!inside_any_solid(&arena, Vec3::new(0.0, y, geom.half.y)));
        // The long side wall is solid.
        assert!(inside_any_solid(&arena, Vec3::new(geom.half.x, y, 0.0)));
    }

    /// The templates whose flavour is a generated labyrinth.
    fn maze_templates() -> Vec<&'static hallway::HallwayTemplate> {
        hallway::TEMPLATES
            .iter()
            .filter(|t| t.flavor == hallway::HallwayFlavor::Maze)
            .collect()
    }

    #[test]
    fn a_maze_hallway_has_entrances_and_exits_with_interior_walls() {
        for template in maze_templates() {
            for seed in 0..6u64 {
                let geom = hallway_geom(RoomId(2), RoomId(7), template, seed, false);
                let entries: Vec<_> = geom
                    .gaps
                    .iter()
                    .filter(|g| g.kind == GapKind::Entry)
                    .collect();
                let exits: Vec<_> = geom
                    .gaps
                    .iter()
                    .filter(|g| g.kind == GapKind::Exit)
                    .collect();
                assert!(!entries.is_empty(), "{} has an entrance", template.name);
                assert!(!exits.is_empty(), "{} has an exit", template.name);
                assert!(
                    entries.iter().all(|g| g.target == RoomId(2)),
                    "every entrance leads back to `from`"
                );
                assert!(
                    exits.iter().all(|g| g.target == RoomId(7)),
                    "every exit leads on to `to`"
                );
                assert!(
                    !geom.interior.is_empty(),
                    "{} is a real maze with interior walls",
                    template.name
                );
            }
        }
    }

    /// Can a body of the controller's radius reach the exit from the entry through the
    /// built collision arena? Flood the free space on a fine lattice, confined to the
    /// footprint, and require the exit cell to be reachable from the entry spawn. This
    /// exercises the whole pipeline: maze â†’ interior walls â†’ arena â†’ walkable.
    fn maze_is_walkable(geom: &PlaceGeom) -> bool {
        const STEP: f32 = 0.25;
        const R: f32 = 0.4; // controller body radius
        const HH: f32 = 0.9; // controller half-height
        let arena = place_arena(geom, 0.0, 3.4);
        let half = geom.half;
        let blocked = |px: f32, pz: f32| -> bool {
            let (cy, hy) = (HH, HH); // feet on the floor (floor_y = 0)
            arena.solids.iter().any(|s| {
                px - R < s.max.x
                    && px + R > s.min.x
                    && cy - hy < s.max.y
                    && cy + hy > s.min.y
                    && pz - R < s.max.z
                    && pz + R > s.min.z
            })
        };
        let entry = geom.gaps.iter().find(|g| g.kind == GapKind::Entry).unwrap();
        let exit = geom.gaps.iter().find(|g| g.kind == GapKind::Exit).unwrap();
        let start = entry.center - entry.normal * ENTRY_INSET;
        let goal = exit.center - exit.normal * ENTRY_INSET;
        let key = |x: f32, z: f32| -> (i32, i32) {
            ((x / STEP).round() as i32, (z / STEP).round() as i32)
        };
        let goal_key = key(goal.x, goal.y);
        let start_key = key(start.x, start.y);
        if blocked(start_key.0 as f32 * STEP, start_key.1 as f32 * STEP) {
            return false; // spawn itself must be clear
        }
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        seen.insert(start_key);
        queue.push_back(start_key);
        while let Some((ix, iz)) = queue.pop_front() {
            if (ix, iz) == goal_key {
                return true;
            }
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (ix + dx, iz + dz);
                let (wx, wz) = (nx as f32 * STEP, nz as f32 * STEP);
                // Stay strictly inside the footprint so the flood can't leak out a gap.
                if wx.abs() >= half.x || wz.abs() >= half.y {
                    continue;
                }
                if seen.contains(&(nx, nz)) || blocked(wx, wz) {
                    continue;
                }
                seen.insert((nx, nz));
                queue.push_back((nx, nz));
            }
        }
        false
    }

    #[test]
    fn a_maze_hallway_is_walkable_from_entry_to_exit() {
        for template in maze_templates() {
            for seed in 0..12u64 {
                let geom = hallway_geom(RoomId(1), RoomId(4), template, seed, false);
                assert!(
                    maze_is_walkable(&geom),
                    "{} (seed {seed}) must be walkable entryâ†’exit",
                    template.name
                );
            }
        }
    }

    fn chicane_template() -> &'static hallway::HallwayTemplate {
        hallway::TEMPLATES
            .iter()
            .find(|t| t.flavor == hallway::HallwayFlavor::Chicane)
            .expect("a chicane template exists")
    }

    #[test]
    fn a_chicane_hallway_is_a_walkable_s_bend() {
        let template = chicane_template();
        for seed in 0..16u64 {
            let geom = hallway_geom(RoomId(1), RoomId(4), template, seed, false);
            let entry = geom.gaps.iter().find(|g| g.kind == GapKind::Entry).unwrap();
            let exit = geom.gaps.iter().find(|g| g.kind == GapKind::Exit).unwrap();
            assert_eq!(entry.target, RoomId(1));
            assert_eq!(exit.target, RoomId(4));
            assert_eq!(geom.interior.len(), 2, "two staggered baffles form the S");
            // The slalom: entry and exit doorways sit on opposite sides of the corridor.
            assert!(
                entry.center.x * exit.center.x < 0.0,
                "seed {seed}: entry and exit are offset to opposite sides"
            );
            assert!(
                maze_is_walkable(&geom),
                "chicane (seed {seed}) must be walkable entryâ†’exit"
            );
        }
    }

    fn colonnade_templates() -> Vec<&'static hallway::HallwayTemplate> {
        hallway::TEMPLATES
            .iter()
            .filter(|t| t.flavor == hallway::HallwayFlavor::Colonnade)
            .collect()
    }

    #[test]
    fn a_colonnade_is_a_walkable_pillared_pseudo_room() {
        for template in colonnade_templates() {
            for seed in 0..16u64 {
                let geom = hallway_geom(RoomId(1), RoomId(4), template, seed, false);
                // It is a real pillared volume (a grid of interior columns), open at both
                // ends, and reachable entryâ†’exit down the clear central lane.
                assert!(
                    geom.interior.len() >= 4,
                    "{} (seed {seed}) has a grid of pillars",
                    template.name
                );
                assert!(
                    geom.gaps.iter().any(|g| g.kind == GapKind::Entry)
                        && geom.gaps.iter().any(|g| g.kind == GapKind::Exit),
                    "{} is open at both ends",
                    template.name
                );
                // The central lane (x = 0) is clear: no pillar straddles it.
                assert!(
                    geom.interior
                        .iter()
                        .all(|p| p.center.x.abs() - p.half.x > 0.0),
                    "{} keeps a clear central lane",
                    template.name
                );
                assert!(
                    maze_is_walkable(&geom),
                    "{} (seed {seed}) must be walkable entryâ†’exit",
                    template.name
                );
            }
        }
    }

    #[test]
    fn room_footprints_vary_in_size_across_seeds() {
        // Rooms aren't all one size â€” some read as tight, some as hub-like.
        let areas: Vec<f32> = (0..24u64)
            .map(|seed| {
                let g = room_geom(
                    RoomId(0),
                    &[RoomId(1), RoomId(2), RoomId(3)],
                    Some(RoomId(1)),
                    seed,
                );
                g.half.x * g.half.y
            })
            .collect();
        let min = areas.iter().copied().fold(f32::INFINITY, f32::min);
        let max = areas.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            max > min * 1.3,
            "room footprints should vary in size (min {min}, max {max})"
        );
    }

    #[test]
    fn walking_any_hallway_never_climbs_onto_the_roof() {
        use observed_traversal::{FIXED_DT, FpsBody, FpsConfig, step_body};
        use player_input::PlayerIntent;
        let config = FpsConfig::default();
        for (i, template) in hallway::TEMPLATES.iter().enumerate() {
            for seed in 0..8u64 {
                let geom = hallway_geom(RoomId(0), RoomId(1), template, seed, false);
                let arena = place_arena(&geom, 0.0, 3.4);
                let spawn = entry_spawn(&geom, RoomId(0));
                // Face into the hall (+Z, toward the exit), as `place_body` does.
                let mut body =
                    FpsBody::spawned(Vec3::new(spawn.x, config.half_height, spawn.y), PI);
                // Drive forward with a weaving strafe to provoke corner wedging against
                // the perimeter and any interior (maze/baffle) walls.
                for tick in 0..480u32 {
                    let strafe = if (tick / 30) % 2 == 0 { 1.0 } else { -1.0 };
                    step_body(
                        &mut body,
                        PlayerIntent {
                            movement: Vec2::new(strafe, 1.0),
                            ..Default::default()
                        },
                        &arena,
                        &config,
                        FIXED_DT,
                    );
                    let feet = body.position.y - config.half_height;
                    assert!(
                        feet < 0.5,
                        "template {i} ({}) seed {seed} tick {tick}: roofed at feet y={feet}",
                        template.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_hallway_to_the_exit_locks_its_door_when_the_gate_is_shut() {
        let template = hallway::template(0); // a straight connector
        // Heading into the exit room with the gate locked â†’ a solid LockedExit door.
        let locked = hallway_geom(RoomId(7), RoomId(EXIT_ROOM), template, 0, true);
        let exit = locked
            .gaps
            .iter()
            .find(|g| matches!(g.kind, GapKind::LockedExit))
            .expect("a locked exit door");
        assert!(!exit.kind.is_passage(), "a locked exit cannot be crossed");
        // place_arena must wall it off (no void to walk into).
        let arena = place_arena(&locked, 0.0, 3.4);
        assert!(
            inside_any_solid(&arena, Vec3::new(exit.center.x, 1.0, exit.center.y)),
            "the locked exit doorway is solid"
        );

        // Unlocked (gate open) â†’ a normal, crossable Exit at the same place.
        let open = hallway_geom(RoomId(7), RoomId(EXIT_ROOM), template, 0, false);
        assert!(
            open.gaps.iter().any(|g| g.kind == GapKind::Exit),
            "an unlocked exit is a normal passage"
        );
        assert!(!open.gaps.iter().any(|g| g.kind == GapKind::LockedExit));

        // The lock only applies to the exit room â€” other destinations stay open.
        let elsewhere = hallway_geom(RoomId(1), RoomId(4), template, 0, true);
        assert!(elsewhere.gaps.iter().any(|g| g.kind == GapKind::Exit));
    }
}
