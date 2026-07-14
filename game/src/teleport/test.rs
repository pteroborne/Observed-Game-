#[cfg(test)]
mod tests {
    use crate::hallway;
    use crate::teleport::geom::{
        grid_interior, outward_normal, room_geom_with_slots_and_seals_for_role,
    };
    use crate::teleport::*;
    use bevy::math::{Vec2, Vec3};
    use observed_core::RoomId;
    use observed_facility::map_spec::{CorridorRole, RoomRole};
    use observed_match::mutable::EXIT_ROOM;
    use observed_traversal::rapier_controller::step_character;
    use observed_traversal::{FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};
    use player_input::PlayerIntent;
    use std::f32::consts::PI;

    fn nav(connections: &[u32], target: Option<u32>) -> Nav {
        Nav {
            connections: connections.iter().map(|&r| RoomId(r)).collect(),
            connection_slots: connections
                .iter()
                .enumerate()
                .map(|(slot, &target)| RoomConnectionSlot {
                    target: RoomId(target),
                    slot: ThresholdSlotId(slot as u16),
                })
                .collect(),
            sealed_slots: Vec::new(),
            hallway_entry_room_slot: None,
            hallway_exit_room_slot: None,
            target_room: target.map(RoomId),
            room_role: None,
            corridor_roles: Vec::new(),
            seed: 1,
            version: 0,
            exit_locked: false,
            exit_room: RoomId(EXIT_ROOM),
            pinned_corridors: Vec::new(),
            map_spec: None,
        }
    }

    fn test_threshold(room: RoomId, target: RoomId) -> ThresholdLink {
        ThresholdLink {
            room: RoomThreshold {
                room,
                slot: ThresholdSlotId(0),
            },
            hall: HallThreshold {
                corridor: corridor_id_for(room, target),
                slot: ThresholdSlotId(0),
            },
            local_side: ThresholdLocalSide::Room,
        }
    }

    fn drive_wellshaft_body_to(
        body: &mut FpsBody,
        target: Vec2,
        target_feet: f32,
        ascending: bool,
        arena: &FpsArena,
        config: &FpsConfig,
    ) {
        for _ in 0..480 {
            let here = Vec2::new(body.position.x, body.position.z);
            let delta = target - here;
            let feet = body.position.y - config.half_height;
            if delta.length() < 0.72 && (!ascending || feet >= target_feet - 0.05) {
                return;
            }
            let direction = delta.normalize_or_zero();
            body.yaw = direction.x.atan2(-direction.y);
            step_body(
                body,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..Default::default()
                },
                arena,
                config,
                FIXED_DT,
            );
        }
        panic!(
            "wellshaft body missed {target:?} at {target_feet} ascending={ascending}; stopped {:?}",
            body.position
        );
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
        // Role-driven only (no more legacy room-id arm): a room's Monitor footprint
        // comes from its `RoomRole`, not a hardcoded id.
        for room in [RoomId(5), RoomId(6)] {
            let geom = room_geom_with_slots_and_seals_for_role(
                room,
                &[RoomId(1), RoomId(3), RoomId(7), RoomId(8)],
                &[],
                &[],
                Some(RoomId(8)),
                Some(RoomRole::Monitor),
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
    fn liminal_room_footprints_scale_by_room_role() {
        let seed = 7;
        let connections = &[RoomId(1), RoomId(3), RoomId(5)];
        let target = Some(RoomId(3));
        let standard = room_geom_with_slots_and_seals_for_role(
            RoomId(0),
            connections,
            &[],
            &[],
            target,
            Some(RoomRole::Keystone),
            seed,
        );
        let no_role = room_geom_with_slots_and_seals_for_role(
            RoomId(0),
            connections,
            &[],
            &[],
            target,
            None,
            seed,
        );
        let hub = room_geom_with_slots_and_seals_for_role(
            RoomId(0),
            connections,
            &[],
            &[],
            target,
            Some(RoomRole::Start),
            seed,
        );

        assert_eq!(
            no_role.half, standard.half,
            "unknown/non-special roles use the standard liminal scale"
        );
        let standard_area = standard.half.x * standard.half.y;
        let hub_area = hub.half.x * hub.half.y;
        assert!(
            hub_area > standard_area * 1.4,
            "hub rooms should breathe larger than standard rooms ({hub_area} vs {standard_area})"
        );
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
    fn hallway_geom_uses_liminal_scaled_connector_dimensions() {
        let template = hallway::TEMPLATES
            .iter()
            .find(|template| {
                template.grid.is_none() && template.flavor != hallway::HallwayFlavor::Gantry
            })
            .expect("a non-grid, non-gantry template exists");
        let seed = 5;
        let geom = hallway_geom(RoomId(0), RoomId(1), template, seed, false);
        let (scaled_len, scaled_width) = hallway::scaled_dims(template);
        let expected_len = (scaled_len * hallway::length_scale(seed)).max(hallway::MIN_HALL_LENGTH);

        assert!(
            geom.half.x > template.width * 0.5,
            "the authored width should be widened in geometry"
        );
        assert!(
            geom.half.y > template.length * 0.5,
            "the authored length should be stretched in geometry"
        );
        assert!((geom.half.x - scaled_width * 0.5).abs() < 1e-4);
        assert!((geom.half.y - expected_len * 0.5).abs() < 1e-4);
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
                    HallwayGeomEndpoints {
                        from: RoomId(0),
                        to: RoomId(1),
                        from_room_slot: ThresholdSlotId(2),
                        to_room_slot: ThresholdSlotId(1),
                        exit_room: RoomId(EXIT_ROOM),
                    },
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
            floor_y: 0.0,
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
                corridor: corridor_id_for(RoomId(0), RoomId(1)),
                from: RoomId(0),
                to: RoomId(1)
            }
        );
        assert_eq!(
            place,
            Place::legacy_hallway(
                RoomId(0),
                RoomId(1),
                hallway::variation_for(RoomId(0), RoomId(1), nav.seed, nav.version),
            )
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
        // Pin corridor (0,1) at version 2 (when the torch was dropped) — expressed as the
        // derived corridor identity, not the `(a, b)` room pair.
        n.pinned_corridors.push(PinnedCorridor {
            corridor: corridor_id_for(RoomId(0), RoomId(1)),
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
        let hall = Place::legacy_hallway(
            RoomId(0),
            RoomId(1),
            hallway::variation_for(RoomId(0), RoomId(1), nav1.seed, nav1.version),
        );
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
            let is_door = geom
                .gaps
                .iter()
                .any(|g| g.kind.is_passage() && is_point_on_segment(g.center, a, b, 0.05));
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
    fn rapier_blocks_an_angled_room_wall_and_preserves_its_doorway() {
        let geom = room_geom(RoomId(0), &[RoomId(1)], Some(RoomId(1)), 4);
        let gap = *geom.forward_gap().unwrap();
        let config = FpsConfig::default();
        let scene = place_rapier_scene(&geom, 0.0, 3.4);
        let poly = geom.poly.as_ref().unwrap();
        let (wall_a, wall_b) = (0..poly.len())
            .map(|i| (poly[i], poly[(i + 1) % poly.len()]))
            .find(|(a, b)| {
                !geom.gaps.iter().any(|candidate| {
                    candidate.kind.is_passage()
                        && is_point_on_segment(candidate.center, *a, *b, 0.05)
                })
            })
            .expect("one edge without the only passage");
        let wall_normal = outward_normal(wall_a, wall_b);
        let wall_plane = wall_a.dot(wall_normal);
        let mut body = FpsBody::spawned(
            Vec3::new(0.0, config.half_height, 0.0),
            wall_normal.x.atan2(-wall_normal.y),
        );
        for _ in 0..360 {
            step_character(
                &scene,
                &mut body,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..Default::default()
                },
                &config,
                FIXED_DT,
            );
        }
        let body_xz = Vec2::new(body.position.x, body.position.z);
        assert!(
            body_xz.dot(wall_normal) < wall_plane,
            "Rapier must stop the capsule at the angled wall: body={body_xz:?}, plane={wall_plane}"
        );

        let mut through_door = FpsBody::spawned(
            Vec3::new(
                gap.center.x - gap.normal.x * 2.0,
                config.half_height,
                gap.center.y - gap.normal.y * 2.0,
            ),
            gap.normal.x.atan2(-gap.normal.y),
        );
        for _ in 0..120 {
            step_character(
                &scene,
                &mut through_door,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..Default::default()
                },
                &config,
                FIXED_DT,
            );
        }
        let door_plane = gap.center.dot(gap.normal);
        let door_xz = Vec2::new(through_door.position.x, through_door.position.z);
        assert!(
            door_xz.dot(gap.normal) > door_plane + config.radius,
            "the physical doorway must remain crossable: body={door_xz:?}, plane={door_plane}"
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

    // --- Phase 47: WFC vs. DFS maze interior selection ------------------------------

    /// A `Mystery`-role edge selects the WFC labyrinth (`crate::wfc_interior`) instead
    /// of the DFS+braid maze (`crate::maze`): the interior it produces exactly matches
    /// `crate::wfc_interior::generate`'s own output for the same grid/seed, which the
    /// DFS maze would not (different algorithm, different wall count in general).
    #[test]
    fn a_mystery_edge_selects_the_wfc_interior() {
        for template in maze_templates() {
            let Some((cols, rows)) = template.grid else {
                continue;
            };
            let seed = 0u64;
            let geom = hallway_geom_with_slots_and_role(
                HallwayGeomEndpoints {
                    from: RoomId(1),
                    to: RoomId(4),
                    from_room_slot: ThresholdSlotId(0),
                    to_room_slot: ThresholdSlotId(0),
                    exit_room: RoomId(EXIT_ROOM),
                },
                template,
                seed,
                false,
                Some(CorridorRole::Mystery),
            );
            let wfc = crate::wfc_interior::generate(
                cols as usize,
                rows as usize,
                seed,
                MAZE_CELL,
                MAZE_WALL_T,
            )
            .expect("pinned seed 0 converges on every template grid size");
            assert_eq!(
                geom.interior.len(),
                wfc.walls.len(),
                "{} (seed {seed}): a Mystery edge's interior wall count matches the WFC generator's own output",
                template.name
            );
            assert!(
                maze_is_walkable(&geom),
                "{} (seed {seed}): a WFC-selected interior must be walkable entry→exit",
                template.name
            );
        }
    }

    /// Every non-vertical, non-mystery corridor role (including `None`, the authored/dev-map fallback with
    /// no `MapSpec`) keeps the DFS+braid maze — byte-identical to
    /// `hallway_geom`/`hallway_geom_with_slots` (which always pass `None`).
    #[test]
    fn a_non_mystery_edge_keeps_the_dfs_maze() {
        let template = maze_templates()[0];
        let seed = 0u64;
        let baseline = hallway_geom(RoomId(1), RoomId(4), template, seed, false);
        for role in [
            None,
            Some(CorridorRole::Connector),
            Some(CorridorRole::LongRoute),
            Some(CorridorRole::Bypass),
        ] {
            let geom = hallway_geom_with_slots_and_role(
                HallwayGeomEndpoints {
                    from: RoomId(1),
                    to: RoomId(4),
                    from_room_slot: ThresholdSlotId(0),
                    to_room_slot: ThresholdSlotId(0),
                    exit_room: RoomId(EXIT_ROOM),
                },
                template,
                seed,
                false,
                role,
            );
            assert_eq!(
                geom.interior.len(),
                baseline.interior.len(),
                "{role:?}: a non-Mystery role keeps the DFS maze's wall count"
            );
            assert_eq!(
                geom.gaps.len(),
                baseline.gaps.len(),
                "{role:?}: same door layout"
            );
        }
    }

    /// The DFS-maze fallback: `grid_interior` (the WFC/DFS selection point) must fall
    /// back to a real, walkable DFS maze rather than ever emitting an empty interior,
    /// even when asked for a `Mystery`-role WFC interior on a grid too small to ever
    /// converge. Real `HallwayTemplate`s never hit this (every catalog grid size
    /// converges under WFC — see `wfc_interior`'s pinned-seed test); this proves the
    /// fallback wiring itself via `grid_interior`'s direct `pub(crate)` test hook,
    /// since no authored template can force the condition.
    #[test]
    fn wfc_failure_falls_back_to_the_dfs_maze() {
        // A 1x1 grid's single cell cannot host both a door-locked entry and a
        // door-locked exit as distinct rows (the archived contradiction shape:
        // row 0 and row `rows - 1` collapse to the same cell), so WFC can never
        // converge here, forcing the fallback branch.
        let (cols, rows) = (1u8, 1u8);
        let seed = 0u64;
        let wfc_direct = crate::wfc_interior::generate(
            cols as usize,
            rows as usize,
            seed,
            MAZE_CELL,
            MAZE_WALL_T,
        );
        assert!(
            wfc_direct.is_err(),
            "a 1x1 grid must fail to converge, proving this test exercises the fallback"
        );
        let interior = grid_interior(cols, rows, seed, Some(CorridorRole::Mystery));
        let dfs = crate::maze::Maze::generate(cols as usize, rows as usize, seed);
        assert_eq!(
            interior.entry_cols, dfs.entry_cols,
            "falls back to the DFS maze's own door columns"
        );
        assert_eq!(
            interior.exit_cols, dfs.exit_cols,
            "falls back to the DFS maze's own door columns"
        );
    }

    /// The fallback decision is a pure function of `(cols, rows, layout_seed,
    /// corridor_role)`: calling `grid_interior` twice with the same inputs on a grid
    /// that forces the WFC failure produces byte-identical output, so "same seed ->
    /// same choice every run" holds even on the fallback path.
    #[test]
    fn the_fallback_decision_is_deterministic_for_the_same_seed() {
        let (cols, rows) = (1u8, 1u8);
        let seed = 3u64;
        let a = grid_interior(cols, rows, seed, Some(CorridorRole::Mystery));
        let b = grid_interior(cols, rows, seed, Some(CorridorRole::Mystery));
        assert_eq!(a.entry_cols, b.entry_cols);
        assert_eq!(a.exit_cols, b.exit_cols);
        assert_eq!(a.interior.len(), b.interior.len());
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

    fn gantry_template() -> &'static hallway::HallwayTemplate {
        hallway::TEMPLATES
            .iter()
            .find(|t| t.flavor == hallway::HallwayFlavor::Gantry)
            .expect("a gantry template exists")
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

    /// The gantry projection is a real two-level hall: five gaps across four thresholds
    /// (a deck-level entry that delivers the body straight onto the deck, per the
    /// no-stairs design ruling, plus the ground-level understory-return it now shares its
    /// wall with; the upper exit on the deck; the safe-bypass exit; and an understory side
    /// exit that recovers back to `from`), six platform decks plus the upper and entry
    /// landings, and no interior walls.
    #[test]
    fn a_gantry_hallway_projects_five_gaps_with_distinct_slots_and_decks() {
        use observed_traversal::gantry;
        let template = gantry_template();
        for seed in 0..16u64 {
            let geom = hallway_geom(RoomId(1), RoomId(4), template, seed, false);
            assert_eq!(geom.gaps.len(), 5, "gantry hall has exactly five gaps");

            // Distinct threshold slots on every gap (no two apertures share an identity).
            for i in 0..geom.gaps.len() {
                for j in (i + 1)..geom.gaps.len() {
                    assert_ne!(
                        geom.gaps[i].threshold, geom.gaps[j].threshold,
                        "gaps {i} and {j} must have distinct threshold slots"
                    );
                }
            }

            let entry = geom.gaps.iter().find(|g| g.kind == GapKind::Entry).unwrap();
            assert_eq!(entry.target, RoomId(1));
            assert_eq!(
                entry.floor_y,
                gantry::UPPER_DECK_Y,
                "the entry threshold now delivers the body directly onto the deck"
            );

            let to_exits: Vec<_> = geom
                .gaps
                .iter()
                .filter(|g| g.kind.is_passage() && g.target == RoomId(4))
                .collect();
            assert_eq!(to_exits.len(), 2, "two passages lead onward to `to`");
            assert!(
                to_exits.iter().any(|g| g.floor_y > 0.0),
                "one onward exit is deck-level (the upper exit)"
            );
            assert!(
                to_exits.iter().any(|g| (g.floor_y - 0.0).abs() < 1e-6),
                "the other onward exit is ground level (the safe bypass)"
            );
            assert!(
                to_exits[0].floor_y != to_exits[1].floor_y,
                "the two `to` exits sit at different floor heights"
            );

            let from_exits: Vec<_> = geom
                .gaps
                .iter()
                .filter(|g| g.kind.is_passage() && g.target == RoomId(1))
                .collect();
            assert_eq!(
                from_exits.len(),
                3,
                "the deck entry, the ground return, and the understory side exit all lead back to `from`"
            );
            assert!(
                from_exits.iter().any(|g| g.floor_y > 0.0),
                "the deck entry is deck-level"
            );
            assert_eq!(
                from_exits.iter().filter(|g| g.floor_y == 0.0).count(),
                2,
                "the ground return and the understory side exit are both ground level"
            );

            assert!(!geom.decks.is_empty(), "the gantry hall has walkable decks");
            assert!(
                geom.interior.is_empty(),
                "the gantry hall has no interior walls (decks replace the old platform walls)"
            );
            // Platforms are the deep decks (half.y matching the authored platform depth);
            // the landings are shallower. One deep deck per authored platform.
            let platform_decks = geom
                .decks
                .iter()
                .filter(|d| (d.half.y - gantry::PLATFORM_HALF_LENGTH).abs() < 1e-3)
                .count();
            assert_eq!(
                platform_decks,
                gantry::PLATFORM_COUNT,
                "one deep deck per authored jump-map platform"
            );
        }
    }

    /// `place_arena` extrudes each deck into a solid whose top sits at `floor_y + top_y`.
    #[test]
    fn gantry_decks_extrude_to_solids_at_the_deck_height() {
        use observed_traversal::gantry;
        let template = gantry_template();
        let geom = hallway_geom(RoomId(1), RoomId(4), template, 0, false);
        let floor_y = 5.0; // an arbitrary place floor offset (hallways use place_y_offset)
        let arena = place_arena(&geom, floor_y, 3.4);

        // A known platform (the first authored one) yields a solid whose top is exactly
        // floor_y + UPPER_DECK_Y.
        let platform0 = geom
            .decks
            .iter()
            .find(|d| (d.top_y - gantry::UPPER_DECK_Y).abs() < 1e-3 && d.center.y < -10.0)
            .expect("platform 0 deck exists");
        let solid = arena
            .solids
            .iter()
            .find(|s| {
                (s.max.x - (platform0.center.x + platform0.half.x)).abs() < 1e-3
                    && (s.max.z - (platform0.center.y + platform0.half.y)).abs() < 1e-3
            })
            .expect("platform 0 has a matching extruded solid");
        assert!(
            (solid.max.y - (floor_y + gantry::UPPER_DECK_Y)).abs() < 1e-3,
            "deck top sits at floor_y + top_y: {} vs {}",
            solid.max.y,
            floor_y + gantry::UPPER_DECK_Y
        );
        assert!(
            (solid.min.y - (floor_y + platform0.bottom_y)).abs() < 1e-3,
            "deck bottom sits at floor_y + bottom_y: {} vs {}",
            solid.min.y,
            floor_y + platform0.bottom_y
        );
        assert!(
            solid.max.y - solid.min.y < 0.25,
            "jump platforms are thin slabs, not floor-to-top blocks"
        );
    }

    /// Thresholds teleport the body directly (user design ruling: no stairs), so every
    /// deck in the projection — including the entry landing — sits flush at `UPPER_DECK_Y`;
    /// there is no sub-deck-height mount stair left to climb, and the entry gap sits over
    /// the entry landing rather than at the ground. This replaces the old
    /// `gantry_decks_extrude_to_solids_and_the_stair_is_walkable` stair-rise assertion,
    /// which no longer applies now the mount stair is deleted.
    #[test]
    fn the_entry_landing_sits_flush_with_every_other_deck_and_no_stair_remains() {
        use observed_traversal::gantry;
        let template = gantry_template();
        let geom = hallway_geom(RoomId(1), RoomId(4), template, 0, false);

        assert!(
            geom.decks
                .iter()
                .all(|d| (d.top_y - gantry::UPPER_DECK_Y).abs() < 1e-3),
            "every deck (platforms + both landings) sits at UPPER_DECK_Y; no sub-deck-height stair treads remain"
        );

        let course = gantry::GantryCourse::authored();
        let entry_landing = course.entry_landing;
        let entry_deck = geom
            .decks
            .iter()
            .find(|d| (d.center - entry_landing.center).length() < 1e-3)
            .expect("the entry landing deck is projected");
        assert!(
            (entry_deck.half - entry_landing.half).length() < 1e-3,
            "the projected entry landing matches the authored course dimensions"
        );

        let entry = geom
            .gaps
            .iter()
            .find(|g| g.kind == GapKind::Entry)
            .expect("a deck-level entry gap exists");
        assert_eq!(
            entry.floor_y,
            gantry::UPPER_DECK_Y,
            "the entry threshold delivers the body directly onto the deck"
        );
        assert!(
            entry.center.y >= entry_landing.min_z() - 0.01
                && entry.center.y <= entry_landing.max_z() + 0.01,
            "the entry gap sits over the entry landing's footprint"
        );
    }

    /// The Y-gate: a body's feet must sit at a gap's `floor_y` (within tolerance) to use
    /// it. A ground-level body cannot "cross" the deck-level upper exit even if it walks
    /// under its XZ span, but the ground-level safe-bypass exit still works exactly as
    /// before (regression).
    #[test]
    fn feet_at_gap_floor_gates_the_upper_exit_but_not_the_ground_bypass() {
        use observed_traversal::gantry;
        let template = gantry_template();
        let geom = hallway_geom(RoomId(1), RoomId(4), template, 0, false);
        let upper = geom
            .gaps
            .iter()
            .find(|g| g.kind.is_passage() && g.target == RoomId(4) && g.floor_y > 0.0)
            .expect("upper exit gap");
        let bypass = geom
            .gaps
            .iter()
            .find(|g| g.kind.is_passage() && g.target == RoomId(4) && g.floor_y == 0.0)
            .expect("safe-bypass exit gap");

        let place_floor_y = 0.0; // hallways offset the whole place; the gate is local
        // A body at ground-level feet height does NOT satisfy the upper exit's gate.
        assert!(!feet_at_gap_floor(0.0, place_floor_y, upper));
        // A body at deck-level feet height DOES satisfy it.
        assert!(feet_at_gap_floor(
            gantry::UPPER_DECK_Y,
            place_floor_y,
            upper
        ));
        // A ground body still crosses the bypass exit (today's behaviour, unmodified).
        assert!(feet_at_gap_floor(0.0, place_floor_y, bypass));
    }

    #[test]
    fn gantry_wall_cuts_are_height_aware_and_raise_the_ceiling() {
        use observed_traversal::gantry;
        let template = gantry_template();
        let geom = hallway_geom(RoomId(1), RoomId(4), template, 0, false);
        let wall_height = 3.4;
        let arena = place_arena(&geom, 0.0, wall_height);
        let upper = geom
            .gaps
            .iter()
            .find(|g| g.kind.is_passage() && g.target == RoomId(4) && g.floor_y > 0.0)
            .expect("upper exit gap");
        let safe = geom
            .gaps
            .iter()
            .find(|g| g.kind.is_passage() && g.target == RoomId(4) && g.floor_y == 0.0)
            .expect("safe-bypass exit gap");

        assert!(
            (structural_height(&geom, wall_height) - (wall_height + gantry::UPPER_DECK_Y)).abs()
                < 1e-3,
            "a raised threshold lifts the hallway shell height"
        );

        let lower_wall_under_upper = arena.solids.iter().any(|solid| {
            (solid.max.z - geom.half.y).abs() < 0.5
                && solid.min.y <= 0.01
                && solid.max.y <= upper.floor_y + 0.01
                && solid.min.x < upper.center.x
                && solid.max.x > upper.center.x
        });
        assert!(
            lower_wall_under_upper,
            "the upper exit must have real wall below the raised opening"
        );

        let upper_lintel_over_safe = arena.solids.iter().any(|solid| {
            (solid.max.z - geom.half.y).abs() < 0.5
                && solid.min.y >= wall_height - 0.01
                && solid.max.y <= wall_height + gantry::UPPER_DECK_Y + 0.01
                && solid.min.x < safe.center.x
                && solid.max.x > safe.center.x
        });
        assert!(
            upper_lintel_over_safe,
            "the lower safe-bypass exit keeps an upper wall/lintel in the taller gantry shell"
        );
    }

    /// Solvability: from the entry, both `to`-exits are reachable in principle. The
    /// ground/bypass lane at `x = SAFE_BYPASS_X` is clear of every deck across its full
    /// length, and the stair+platform chain climbs contiguously to `UPPER_DECK_Y`.
    #[test]
    fn gantry_projection_keeps_the_bypass_lane_clear_and_the_deck_chain_reaches_the_deck() {
        use observed_traversal::gantry;
        let template = gantry_template();
        let geom = hallway_geom(RoomId(1), RoomId(4), template, 0, false);
        let hz = geom.half.y;
        let bypass_x = gantry::SAFE_BYPASS_X;
        let body_radius = 0.4;

        // No deck overlaps the bypass strip at body height (ground-level clearance) — the
        // ground return gap also opens in this lane, so it must stay deck-free too.
        for deck in &geom.decks {
            let overlaps_x = (bypass_x - deck.center.x).abs() < deck.half.x + body_radius;
            assert!(
                !overlaps_x,
                "deck at {:?} intrudes on the bypass lane x={}",
                deck.center, bypass_x
            );
        }
        let _ = hz; // the bypass run spans the full -hz..hz length by construction

        // The platform + landing chain reaches UPPER_DECK_Y.
        let max_deck_top = geom.decks.iter().map(|d| d.top_y).fold(0.0_f32, f32::max);
        assert!(
            (max_deck_top - gantry::UPPER_DECK_Y).abs() < 1e-3,
            "the deck chain reaches the upper deck height"
        );
    }

    /// Arrival-height regression (deck case): crossing a room's forward doorway into a
    /// Gantry hallway resolves the **deck-level** entry gap (not the ground-level return
    /// that now shares `target == from`), so `place_body`'s
    /// `arrival_floor_y = arrival_gap(...).floor_y` lands the body on the entry landing,
    /// not sunk to the hallway's ground floor. This is the site `crossing.rs::place_body`
    /// reads: `gap.floor_y` feeds directly into the spawn Y (`y_offset + floor_y +
    /// half_height`).
    #[test]
    fn arrival_gap_resolves_the_deck_level_entry_not_the_ground_return() {
        use observed_traversal::gantry;
        let template = gantry_template();
        let from = RoomId(1);
        let to = RoomId(4);
        let room_gap = *room_geom(from, &[to], Some(to), 3)
            .forward_gap()
            .expect("room has a forward gap toward the gantry hallway");
        // `arrival_gap`'s `Place::Hallway` branch only matches on the `Place` variant, not
        // the `variation`/`to` fields, so any Gantry-flavoured hallway place works here.
        let hall = Place::legacy_hallway(from, to, 0);
        let hgeom = hallway_geom(from, to, template, 0, false);

        let arrived = arrival_gap(&hgeom, hall, &room_gap, from).expect("entry gap resolves");
        assert_eq!(arrived.kind, GapKind::Entry);
        assert_eq!(arrived.target, from);
        assert_eq!(
            arrived.floor_y,
            gantry::UPPER_DECK_Y,
            "the resolved arrival gap is the deck-level entry, not the ground-level return"
        );

        // The ground-level return shares `target == from` but is a distinct threshold —
        // a naive "first gap targeting `from`" lookup (what `entry_spawn` still does for
        // the no-`crossed` snap fallback) would be ambiguous; `arrival_gap` disambiguates
        // by matching the crossed doorway's threshold identity instead.
        let ground_return = hgeom
            .gaps
            .iter()
            .find(|g| g.kind == GapKind::Exit && g.target == from && g.floor_y == 0.0)
            .expect("a ground-level return gap exists");
        assert_ne!(arrived.threshold, ground_return.threshold);
    }

    /// Arrival-height regression (ground case): every room-side gap keeps `floor_y == 0.0`,
    /// so `arrival_gap`'s resolved floor_y for an ordinary room arrival is unaffected —
    /// `place_body`'s spawn Y stays exactly `y_offset + half_height` as before.
    #[test]
    fn arrival_gap_stays_ground_level_for_an_ordinary_room_arrival() {
        let nav1 = nav(&[0, 2], Some(2));
        let hall = Place::legacy_hallway(
            RoomId(0),
            RoomId(1),
            hallway::variation_for(RoomId(0), RoomId(1), nav1.seed, nav1.version),
        );
        let hgeom = geom_for(hall, &nav1);
        let exit = *hgeom.gaps.iter().find(|g| g.kind == GapKind::Exit).unwrap();
        let mut rgeom = geom_for(Place::Room(RoomId(1)), &nav1);
        open_entry(&mut rgeom, Some(RoomId(0)));

        let arrived = arrival_gap(&rgeom, Place::Room(RoomId(1)), &exit, RoomId(0))
            .expect("the arrival doorway resolves");
        assert_eq!(
            arrived.floor_y, 0.0,
            "a room arrival's floor_y is unaffected by the gantry deck-entry change"
        );
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
            // The wellshaft is a deliberately climbable spiral stair, so walking it
            // raises the feet by design; the flat-roof invariant (meant to catch a
            // body climbing a box hallway's perimeter/baffle walls onto the roof)
            // does not apply. Its controlled vertical traversal is proven by
            // `production_controller_traverses_the_projected_wfc_wellshaft_both_ways`.
            if template.flavor == hallway::HallwayFlavor::Wellshaft {
                continue;
            }
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
    fn rapier_projects_every_room_and_hallway_structure() {
        use observed_facility::map_spec::RoomRole;

        // Every procedural room shape, including the 13-sided monitor footprint,
        // produces a finite Rapier scene. Polygon containment remains a known bridge
        // until its angled perimeter is promoted to oriented primitives, but all
        // authored structural solids already share the Rapier scene path.
        for role in [
            None,
            Some(RoomRole::Start),
            Some(RoomRole::Decision),
            Some(RoomRole::Monitor),
        ] {
            for seed in 0..8 {
                let room = room_geom_with_slots_and_seals_for_role(
                    RoomId(0),
                    &[RoomId(1), RoomId(2), RoomId(3)],
                    &[],
                    &[],
                    Some(RoomId(1)),
                    role,
                    seed,
                );
                let primitives = place_structural_primitives(&room, 0.0, 3.4);
                let scene = place_rapier_scene(&room, 0.0, 3.4);
                assert_eq!(scene.collider_count(), primitives.len() + 1);
            }
        }

        for template in hallway::TEMPLATES {
            for seed in 0..4 {
                let geom = hallway_geom(RoomId(0), RoomId(1), &template, seed, false);
                let arena = place_arena(&geom, 0.0, 3.4);
                let primitives = place_structural_primitives(&geom, 0.0, 3.4);
                let scene = place_rapier_scene(&geom, 0.0, 3.4);
                assert_eq!(
                    scene.collider_count(),
                    primitives.len() + 1,
                    "{} seed {seed}",
                    template.name
                );
                let _legacy_characterization = arena.solids.len();
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

    #[test]
    fn a_vertical_wfc_edge_projects_the_bidirectional_wellshaft() {
        let endpoints = HallwayGeomEndpoints {
            from: RoomId(2),
            to: RoomId(3),
            from_room_slot: ThresholdSlotId(1),
            to_room_slot: ThresholdSlotId(2),
            exit_room: RoomId(EXIT_ROOM),
        };
        let geom = hallway_geom_with_slots_and_role(
            endpoints,
            hallway::template(0),
            77,
            false,
            Some(CorridorRole::Vertical),
        );
        assert!(geom.is_wellshaft());
        assert_eq!(
            geom.half,
            Vec2::new(
                hallway::WELL_SHAFT_OUTER_APOTHEM,
                hallway::WELL_SHAFT_OUTER_RADIUS
            )
        );
        assert_eq!(geom.poly.as_ref().map(Vec::len), Some(6), "hexagonal shell");
        assert_eq!(geom.gaps.len(), 2);
        assert!(geom.gaps.iter().any(|gap| {
            gap.kind == GapKind::Entry && (gap.floor_y - hallway::WELL_SHAFT_HEIGHT).abs() < 0.01
        }));
        assert!(
            geom.gaps
                .iter()
                .any(|gap| gap.kind == GapKind::Exit && gap.floor_y.abs() < 0.01)
        );
        let spawn = entry_spawn(&geom, RoomId(2));
        assert!(
            geom.decks.iter().any(|deck| {
                (deck.top_y - hallway::WELL_SHAFT_HEIGHT).abs() < 0.01
                    && (spawn.x - deck.center.x).abs() <= deck.half.x
                    && (spawn.y - deck.center.y).abs() <= deck.half.y
            }),
            "the elevated entry spawn is supported by the top landing"
        );

        // Treads are the radial slabs whose vertical extent is one riser's closure.
        let treads: Vec<&DeckSeg> = geom
            .decks
            .iter()
            .filter(|deck| {
                (deck.top_y - deck.bottom_y - hallway::WELL_SHAFT_TREAD_CLOSURE).abs() < 0.01
            })
            .collect();
        assert_eq!(
            treads.len(),
            (hallway::WELL_SHAFT_LEVELS - 1) * hallway::WELL_SHAFT_STEPS_PER_FLIGHT,
            "five eight-tread flights"
        );
        for level in 0..hallway::WELL_SHAFT_LEVELS - 1 {
            for step in 0..hallway::WELL_SHAFT_STEPS_PER_FLIGHT {
                let center = hallway::wellshaft_stair_center(level, step);
                let top = level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT
                    + step as f32 * hallway::WELL_SHAFT_STEP_RISE;
                assert!(treads.iter().any(|deck| {
                    deck.center.distance(Vec2::new(center.0, center.1)) < 0.01
                        && (deck.top_y - top).abs() < 0.01
                }));
            }
        }

        // One guard rail per mid-flight tread (end treads abut landings, stay open).
        assert_eq!(
            geom.decks
                .iter()
                .filter(|deck| {
                    (deck.top_y - deck.bottom_y - hallway::WELL_SHAFT_GUARD_HEIGHT).abs() < 0.01
                })
                .count(),
            (hallway::WELL_SHAFT_LEVELS - 1) * (hallway::WELL_SHAFT_STEPS_PER_FLIGHT - 2),
        );

        // The ground landing and bridge (level zero) dip a deck's thickness below the
        // base floor so they meet it cleanly.
        assert_eq!(
            geom.decks
                .iter()
                .filter(|deck| {
                    (deck.bottom_y + hallway::WELL_SHAFT_DECK_THICKNESS).abs() < 0.01
                })
                .count(),
            2,
            "the ground landing and bridge meet the base floor"
        );
        assert!(geom.decks.iter().any(|deck| {
            deck.center == Vec2::ZERO
                && deck.half == Vec2::splat(hallway::WELL_SHAFT_PILLAR_COLLISION_HALF)
                && deck.top_y > hallway::WELL_SHAFT_HEIGHT
        }));

        let again = hallway_geom_with_slots_and_role(
            endpoints,
            hallway::template(0),
            77,
            false,
            Some(CorridorRole::Vertical),
        );
        let signature = |geom: &PlaceGeom| {
            geom.decks
                .iter()
                .map(|deck| (deck.center, deck.half, deck.bottom_y, deck.top_y))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            signature(&geom),
            signature(&again),
            "same edge inputs, same shaft"
        );

        let bottom = geom.gaps.iter().find(|gap| gap.floor_y == 0.0).unwrap();
        let top = geom
            .gaps
            .iter()
            .find(|gap| gap.floor_y == hallway::WELL_SHAFT_HEIGHT)
            .unwrap();
        let expected_bottom = hallway::wellshaft_level_direction(0);
        let expected_top = hallway::wellshaft_level_direction(hallway::WELL_SHAFT_LEVELS - 1);
        assert!(
            bottom
                .normal
                .distance(Vec2::new(expected_bottom.0, expected_bottom.1))
                < 0.01
        );
        assert!(
            top.normal
                .distance(Vec2::new(expected_top.0, expected_top.1))
                < 0.01
        );
    }

    #[test]
    fn production_controller_traverses_the_projected_wfc_wellshaft_both_ways() {
        let geom = hallway_geom_with_slots_and_role(
            HallwayGeomEndpoints {
                from: RoomId(2),
                to: RoomId(3),
                from_room_slot: ThresholdSlotId(1),
                to_room_slot: ThresholdSlotId(2),
                exit_room: RoomId(EXIT_ROOM),
            },
            hallway::template(0),
            77,
            false,
            Some(CorridorRole::Vertical),
        );
        let config = FpsConfig::default();
        let arena = place_arena(&geom, 0.0, crate::layout::WALL_HEIGHT);
        let spawn = entry_spawn(&geom, RoomId(2));
        let mut body = FpsBody::spawned(
            Vec3::new(
                spawn.x,
                hallway::WELL_SHAFT_HEIGHT + config.half_height,
                spawn.y,
            ),
            0.0,
        );

        for upper_level in (1..hallway::WELL_SHAFT_LEVELS).rev() {
            let lower_level = upper_level - 1;
            for step in (0..hallway::WELL_SHAFT_STEPS_PER_FLIGHT).rev() {
                let point = hallway::wellshaft_stair_center(lower_level, step);
                drive_wellshaft_body_to(
                    &mut body,
                    Vec2::new(point.0, point.1),
                    lower_level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT
                        + step as f32 * hallway::WELL_SHAFT_STEP_RISE,
                    false,
                    &arena,
                    &config,
                );
            }
            let rest = hallway::wellshaft_landing_rest(lower_level);
            drive_wellshaft_body_to(
                &mut body,
                Vec2::new(rest.0, rest.1),
                lower_level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT,
                false,
                &arena,
                &config,
            );
            for _ in 0..30 {
                step_body(
                    &mut body,
                    PlayerIntent::default(),
                    &arena,
                    &config,
                    FIXED_DT,
                );
            }
            let feet = body.position.y - config.half_height;
            let expected = lower_level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT;
            assert!(
                (feet - expected).abs() < 0.08,
                "level {lower_level}: {feet}"
            );
        }

        for lower_level in 0..hallway::WELL_SHAFT_LEVELS - 1 {
            for step in 0..hallway::WELL_SHAFT_STEPS_PER_FLIGHT {
                let point = hallway::wellshaft_stair_center(lower_level, step);
                drive_wellshaft_body_to(
                    &mut body,
                    Vec2::new(point.0, point.1),
                    lower_level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT
                        + step as f32 * hallway::WELL_SHAFT_STEP_RISE,
                    true,
                    &arena,
                    &config,
                );
            }
            let upper_level = lower_level + 1;
            let rest = hallway::wellshaft_landing_rest(upper_level);
            drive_wellshaft_body_to(
                &mut body,
                Vec2::new(rest.0, rest.1),
                upper_level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT,
                true,
                &arena,
                &config,
            );
        }
        let feet = body.position.y - config.half_height;
        assert!((feet - hallway::WELL_SHAFT_HEIGHT).abs() < 0.08);
    }

    #[test]
    fn vertical_role_never_replaces_a_gantry_or_locked_objective_edge() {
        let gantry = hallway::TEMPLATES
            .iter()
            .find(|template| template.flavor == hallway::HallwayFlavor::Gantry)
            .expect("gantry template");
        let endpoints = HallwayGeomEndpoints {
            from: RoomId(1),
            to: RoomId(2),
            from_room_slot: ThresholdSlotId(0),
            to_room_slot: ThresholdSlotId(0),
            exit_room: RoomId(EXIT_ROOM),
        };
        let kept_gantry = hallway_geom_with_slots_and_role(
            endpoints,
            gantry,
            5,
            false,
            Some(CorridorRole::Vertical),
        );
        assert!(!kept_gantry.is_wellshaft());

        let locked = hallway_geom_with_slots_and_role(
            HallwayGeomEndpoints {
                from: RoomId(7),
                to: RoomId(EXIT_ROOM),
                from_room_slot: ThresholdSlotId(0),
                to_room_slot: ThresholdSlotId(0),
                exit_room: RoomId(EXIT_ROOM),
            },
            hallway::template(0),
            5,
            true,
            Some(CorridorRole::Vertical),
        );
        assert!(!locked.is_wellshaft());
        assert!(
            locked
                .gaps
                .iter()
                .any(|gap| gap.kind == GapKind::LockedExit)
        );
    }

    // --- Phase 74 regression fixtures: the four "render / physics / graph cannot
    // disagree" invariants, asserted directly against the junction topology. ---

    use observed_core::{PlaceId as CorePlaceId, ThresholdId as CoreThresholdId};

    fn room_socket(room: RoomId, slot: u16) -> CoreThresholdId {
        CoreThresholdId::new(CorePlaceId::Room(room), slot)
    }

    /// Traversable ⇒ never walled: every active room-side attachment is a passage that
    /// leaves a real opening (a body steps out through it, not clamped back), and the
    /// corridor it partners into presents a matching passage back to the room.
    #[test]
    fn phase74_traversable_sockets_are_never_walled() {
        let nav = nav(&[1, 3], Some(1));
        let room = Place::Room(RoomId(0));
        let geom = geom_for(room, &nav);
        let topology = place_junction(room, &nav);
        let passages: Vec<_> = geom
            .gaps
            .iter()
            .filter(|gap| gap.kind.is_passage())
            .cloned()
            .collect();
        assert!(!passages.is_empty(), "the room has a crossable doorway");
        let radius = 0.4;
        for gap in &passages {
            // (1) The room socket is attached — its partner is a corridor socket.
            let socket = room_socket(RoomId(0), gap.threshold.room.slot.0);
            let partner = topology
                .partner(socket)
                .expect("a traversable socket has a topology partner");
            assert!(
                matches!(partner.place, CorePlaceId::Corridor(_)),
                "a room socket partners into a corridor"
            );

            // (2) The aperture is a real opening: stepping out through it is not clamped.
            let at_door = gap.center + gap.normal * 0.3;
            let after = contain(&geom, at_door, radius);
            assert!(
                (after - at_door).length() < 0.01,
                "no solid spans the traversable aperture"
            );

            // (3) The corridor it opens into presents the matching passage back.
            let (hall, _) = apply_crossing(room, gap, &nav);
            assert_eq!(
                hall.place_id(),
                CorePlaceId::Corridor(partner_corridor(partner))
            );
            let hgeom = geom_for(hall, &nav);
            assert!(
                hgeom
                    .gaps
                    .iter()
                    .any(|g| g.kind.is_passage() && g.target == RoomId(0)),
                "the corridor's entry socket is a passage back to the room"
            );
        }
    }

    fn partner_corridor(threshold: CoreThresholdId) -> observed_core::CorridorId {
        match threshold.place {
            CorePlaceId::Corridor(id) => id,
            CorePlaceId::Room(_) => panic!("expected a corridor partner"),
        }
    }

    /// Sealed ⇒ never crossable: a socket the collapse sealed produces no `partner` and no
    /// passable gap — even when the sealed side is the forward objective doorway. This is
    /// the aperture-sourcing invariant (step 4): revert `enforce_active_sockets` and the
    /// sealed forward doorway stays a `Forward` passage, failing the final assertion.
    #[test]
    fn phase74_sealed_sockets_are_never_crossable() {
        let mut nav = nav(&[1, 3], Some(1));
        // In `nav()`, connection 1 (the objective) is slot 0; connection 3 is slot 1.
        // Seal the forward/objective socket while keeping a second live socket so the
        // topology is non-degenerate.
        nav.sealed_slots = vec![ThresholdSlotId(0)];
        let room = Place::Room(RoomId(0));
        let topology = place_junction(room, &nav);

        // The sealed socket has no partner — un-crossable by construction.
        assert!(
            topology.partner(room_socket(RoomId(0), 0)).is_none(),
            "a sealed socket has no topology partner"
        );
        // The still-open side keeps its partner.
        assert!(
            topology.partner(room_socket(RoomId(0), 1)).is_some(),
            "the unsealed connection stays crossable"
        );

        // ...and the sealed forward doorway is not a passage in the rendered / physical
        // geometry. (Reverting step 4 leaves it a `Forward` passage → this fails.)
        let geom = geom_for(room, &nav);
        let forward_to_1 = geom
            .gaps
            .iter()
            .find(|gap| gap.target == RoomId(1))
            .expect("the sealed relation still owns a doorway slot");
        assert!(
            !forward_to_1.kind.is_passage(),
            "a sealed socket renders as a wall, never a passage"
        );
    }

    /// Reciprocity: crossing a room socket into the corridor and crossing back through the
    /// corridor's mirrored socket returns to the origin room, and the corridor identity is
    /// direction-independent.
    #[test]
    fn phase74_crossing_round_trips_through_the_same_corridor() {
        let nav = nav(&[1, 3], Some(1));
        let room = Place::Room(RoomId(0));
        let forward = *geom_for(room, &nav).forward_gap().unwrap();

        let (hall, entered) = apply_crossing(room, &forward, &nav);
        let cid = corridor_id_for(RoomId(0), RoomId(1));
        assert_eq!(hall.corridor_id(), Some(cid));
        assert_eq!(hall.place_id(), CorePlaceId::Corridor(cid));
        assert!(matches!(
            entered,
            Crossing::EnteredHallway { corridor, .. } if corridor == cid
        ));
        // Direction independence: the reverse pairing names the same corridor.
        assert_eq!(corridor_id_for(RoomId(1), RoomId(0)), cid);

        // Cross back through the corridor's entry socket → arrive in the origin room.
        let hgeom = geom_for(hall, &nav);
        let entry = *hgeom
            .gaps
            .iter()
            .find(|gap| gap.kind == GapKind::Entry && gap.target == RoomId(0))
            .expect("the corridor has an entry socket back to the origin");
        let (back, arrived) = apply_crossing(hall, &entry, &nav);
        assert_eq!(back, Place::Room(RoomId(0)));
        assert_eq!(arrived, Crossing::ArrivedRoom(RoomId(0)));
    }

    /// Atomic reroute: a reroute rebuilds the socket topology so that no socket is ever
    /// attached to two corridors and no attachment is one-sided — the reciprocal partner
    /// always round-trips, before and after the rewire, and the crate rejects a
    /// half-rewire outright.
    #[test]
    fn phase74_reroute_keeps_attachments_atomic_and_reciprocal() {
        let assert_reciprocal = |place: Place, nav: &Nav| {
            let topology = place_junction(place, nav);
            assert!(topology.threshold_count() > 0, "a live place has sockets");
            for connection in &nav.connections {
                let slot = nav.slot_for(*connection).unwrap();
                let socket = room_socket(RoomId(0), slot.0);
                let partner = topology
                    .partner(socket)
                    .expect("every live connection is attached");
                // Reciprocal: the corridor socket partners straight back to the room.
                assert_eq!(topology.partner(partner), Some(socket));
                // Single-valued: a socket never partners two corridors (map invariant).
                assert!(matches!(partner.place, CorePlaceId::Corridor(_)));
            }
        };

        // Before the reroute: room 0 connects to 1 and 3.
        let before = nav(&[1, 3], Some(1));
        assert_reciprocal(Place::Room(RoomId(0)), &before);

        // After a reroute the connection 0↔3 is rewired to 0↔4 as one operation.
        let after = nav(&[1, 4], Some(1));
        assert_reciprocal(Place::Room(RoomId(0)), &after);

        // The corridor that 3 used to reach is gone from the active set; the new one is
        // present — the rewire moved both sides together, never one.
        let before_topo = place_junction(Place::Room(RoomId(0)), &before);
        let after_topo = place_junction(Place::Room(RoomId(0)), &after);
        let cid_3 = corridor_id_for(RoomId(0), RoomId(3));
        let cid_4 = corridor_id_for(RoomId(0), RoomId(4));
        assert!(!before_topo.corridor_rooms(cid_4).contains(&RoomId(0)));
        assert!(before_topo.corridor_rooms(cid_3).contains(&RoomId(0)));
        assert!(after_topo.corridor_rooms(cid_4).contains(&RoomId(0)));
        assert!(!after_topo.corridor_rooms(cid_3).contains(&RoomId(0)));

        // The crate itself refuses a half-rewire: attaching a second room to an
        // already-attached corridor socket is rejected, so an attachment can never be
        // left one-sided.
        use observed_facility::junction::{CorridorSpec, JunctionTopology, ThresholdAttachment};
        let cid = observed_core::CorridorId(9);
        let first = ThresholdAttachment::new(
            room_socket(RoomId(0), 0),
            CoreThresholdId::new(CorePlaceId::Corridor(cid), 0),
        )
        .unwrap();
        let mut topo =
            JunctionTopology::new([CorridorSpec::with_slot_count(cid, 2)], [first]).unwrap();
        let half_rewire = ThresholdAttachment::new(
            room_socket(RoomId(7), 0),
            CoreThresholdId::new(CorePlaceId::Corridor(cid), 0),
        )
        .unwrap();
        assert!(
            topo.attach(half_rewire).is_err(),
            "a socket cannot be attached to two rooms — no one-sided rewire"
        );
    }

    fn count_rendered_walls(poly: &[Vec2], gaps: &[DoorGap]) -> usize {
        plan_boundary(poly, gaps, 3.4, 3.4)
            .expect("test room boundary must plan")
            .wall_panels
            .len()
    }

    fn count_wellshaft_hex_walls(poly: &[Vec2], gaps: &[DoorGap], total_height: f32) -> usize {
        plan_boundary(poly, gaps, total_height, 3.4)
            .expect("test wellshaft boundary must plan")
            .wall_panels
            .len()
    }

    fn count_deck_primitives(decks: &[DeckSeg]) -> usize {
        decks
            .iter()
            .filter(|d| (d.top_y - d.bottom_y) > 0.01)
            .count()
    }

    fn count_perimeter_walls(
        primitives: &[observed_traversal::rapier_controller::StructuralCollider],
        floor_y: f32,
        total_height: f32,
    ) -> usize {
        primitives
            .iter()
            .filter(|c| {
                // A perimeter wall is full-height (y_min ~ 0, height ~ total_height)
                // and has a wall half-thickness (0.4)
                let y_min = c.center.y - c.half.y - floor_y;
                let height = c.half.y * 2.0;
                let is_full_height = y_min.abs() < 0.05 && (height - total_height).abs() < 0.05;
                let is_wall_thickness =
                    (c.half.z - 0.4).abs() < 0.01 || (c.half.x - 0.4).abs() < 0.01;
                is_full_height && is_wall_thickness
            })
            .count()
    }

    #[test]
    fn phase75_corpus_parity_structural_agreement() {
        use crate::hallway;
        use observed_facility::map_spec::RoomRole;

        // (1) Rooms: check structural wall counts agree across every room role and seed
        for role in [
            None,
            Some(RoomRole::Start),
            Some(RoomRole::Exit),
            Some(RoomRole::Decision),
            Some(RoomRole::DecoherenceFork),
            Some(RoomRole::AnchorCheckpoint),
            Some(RoomRole::TeleportRelay),
            Some(RoomRole::Keystone),
            Some(RoomRole::DualStation),
            Some(RoomRole::GuardianControl),
            Some(RoomRole::Monitor),
            Some(RoomRole::Recovery),
        ] {
            for seed in 0..16 {
                let room = room_geom_with_slots_and_seals_for_role(
                    RoomId(0),
                    &[RoomId(1), RoomId(2)],
                    &[],
                    &[],
                    Some(RoomId(1)),
                    role,
                    seed,
                );
                let primitives = place_structural_primitives(&room, 0.0, 3.4);
                let scene = place_rapier_scene(&room, 0.0, 3.4);

                // Agree with Rapier scene size
                assert_eq!(scene.collider_count(), primitives.len() + 1);

                // Rendered walls count must match full-height primitives count
                let rendered_count = count_rendered_walls(room.poly.as_ref().unwrap(), &room.gaps);
                let prim_wall_count = count_perimeter_walls(&primitives, 0.0, 3.4);
                let explicit_closures = room
                    .gaps
                    .iter()
                    .filter(|gap| ThresholdClosure::for_kind(gap.kind).is_some())
                    .count();
                assert_eq!(
                    rendered_count,
                    prim_wall_count - explicit_closures,
                    "Room role {role:?} seed {seed}"
                );
            }
        }

        // (2) Hallways: check across all templates
        for template in hallway::TEMPLATES {
            let roles: &[Option<CorridorRole>] = if template.grid.is_some() {
                &[None, Some(CorridorRole::Mystery)]
            } else if template.flavor == hallway::HallwayFlavor::Straight {
                &[None, Some(CorridorRole::Vertical)]
            } else {
                &[None]
            };
            for &role in roles {
                for seed in 0..8 {
                    let geom = hallway_geom_with_slots_and_role(
                        HallwayGeomEndpoints {
                            from: RoomId(0),
                            to: RoomId(1),
                            from_room_slot: ThresholdSlotId(0),
                            to_room_slot: ThresholdSlotId(0),
                            exit_room: RoomId(EXIT_ROOM),
                        },
                        &template,
                        seed,
                        false,
                        role,
                    );
                    let primitives = place_structural_primitives(&geom, 0.0, 3.4);
                    let scene = place_rapier_scene(&geom, 0.0, 3.4);

                    // Agree with Rapier scene size
                    assert_eq!(scene.collider_count(), primitives.len() + 1);

                    if let Some(poly) = &geom.poly
                        && geom.is_wellshaft()
                    {
                        let total_height = structural_height(&geom, 3.4);
                        let rendered_hex_walls =
                            count_wellshaft_hex_walls(poly, &geom.gaps, total_height);
                        let explicit_closures = geom
                            .gaps
                            .iter()
                            .filter(|gap| ThresholdClosure::for_kind(gap.kind).is_some())
                            .count();
                        let prim_hex_walls = primitives.len()
                            - count_deck_primitives(&geom.decks)
                            - explicit_closures;
                        assert_eq!(rendered_hex_walls, prim_hex_walls, "Wellshaft seed {seed}");
                    }
                }
            }
        }
    }

    fn verify_bidirectional_crossing(
        from: RoomId,
        to: RoomId,
        from_room_slot: ThresholdSlotId,
        to_room_slot: ThresholdSlotId,
        template: &hallway::HallwayTemplate,
        seed: u64,
        role: Option<CorridorRole>,
    ) {
        let nav = Nav {
            connections: vec![from, to],
            connection_slots: vec![
                RoomConnectionSlot {
                    target: to,
                    slot: from_room_slot,
                },
                RoomConnectionSlot {
                    target: from,
                    slot: to_room_slot,
                },
            ],
            sealed_slots: Vec::new(),
            hallway_entry_room_slot: Some(from_room_slot),
            hallway_exit_room_slot: Some(to_room_slot),
            target_room: Some(to),
            room_role: None,
            corridor_roles: role.map(|r| vec![(to, r)]).unwrap_or_default(),
            seed,
            version: 0,
            exit_locked: false,
            exit_room: RoomId(EXIT_ROOM),
            pinned_corridors: Vec::new(),
            map_spec: None,
        };

        // (1) Cross from "from" room to Hallway
        let room_from_geom = room_geom_with_slots_and_seals_for_role(
            from,
            &[to],
            &[RoomConnectionSlot {
                target: to,
                slot: from_room_slot,
            }],
            &[],
            Some(to),
            None,
            seed,
        );
        let from_gap = room_from_geom
            .gaps
            .iter()
            .find(|g| g.target == to)
            .expect("from gap");
        let (hall_place, entered) = apply_crossing(Place::Room(from), from_gap, &nav);

        let cid = corridor_id_for(from, to);
        assert_eq!(hall_place.corridor_id(), Some(cid));
        assert!(matches!(
            entered,
            Crossing::EnteredHallway { corridor, .. } if corridor == cid
        ));

        // (2) In the Hallway, check that all active/passage gaps are crossable back
        let hall_geom = hallway_geom_with_slots_and_role(
            HallwayGeomEndpoints {
                from,
                to,
                from_room_slot,
                to_room_slot,
                exit_room: RoomId(EXIT_ROOM),
            },
            template,
            seed,
            false,
            role,
        );

        let mut checked_gaps = 0;
        for gap in &hall_geom.gaps {
            if gap.kind.is_passage() {
                checked_gaps += 1;
                let (dest_place, arrived) = apply_crossing(hall_place, gap, &nav);
                assert_eq!(dest_place, Place::Room(gap.target));
                assert_eq!(arrived, Crossing::ArrivedRoom(gap.target));
            }
        }
        assert!(
            checked_gaps > 0,
            "hallway must have at least one passage gap"
        );
    }

    #[test]
    fn phase75_corpus_parity_bidirectional_apertures() {
        use crate::hallway;
        let from = RoomId(0);
        let to = RoomId(1);
        let from_room_slot = ThresholdSlotId(0);
        let to_room_slot = ThresholdSlotId(1);

        for template in hallway::TEMPLATES {
            let roles: &[Option<CorridorRole>] = if template.grid.is_some() {
                &[None, Some(CorridorRole::Mystery)]
            } else if template.flavor == hallway::HallwayFlavor::Straight {
                &[None, Some(CorridorRole::Vertical)]
            } else {
                &[None]
            };
            for &role in roles {
                for seed in 0..4 {
                    verify_bidirectional_crossing(
                        from,
                        to,
                        from_room_slot,
                        to_room_slot,
                        &template,
                        seed,
                        role,
                    );
                }
            }
        }
    }

    #[test]
    fn phase75_corpus_parity_sealed_sockets() {
        use observed_facility::map_spec::RoomRole;

        for role in [
            None,
            Some(RoomRole::Start),
            Some(RoomRole::Exit),
            Some(RoomRole::Decision),
            Some(RoomRole::DecoherenceFork),
            Some(RoomRole::AnchorCheckpoint),
            Some(RoomRole::TeleportRelay),
            Some(RoomRole::Keystone),
            Some(RoomRole::DualStation),
            Some(RoomRole::GuardianControl),
            Some(RoomRole::Monitor),
            Some(RoomRole::Recovery),
        ] {
            for seed in 0..8 {
                let from = RoomId(0);
                let to = RoomId(1);
                let other = RoomId(2);
                let from_room_slot = ThresholdSlotId(0);
                let other_room_slot = ThresholdSlotId(1);

                let nav = Nav {
                    connections: vec![to, other],
                    connection_slots: vec![
                        RoomConnectionSlot {
                            target: to,
                            slot: from_room_slot,
                        },
                        RoomConnectionSlot {
                            target: other,
                            slot: other_room_slot,
                        },
                    ],
                    sealed_slots: vec![from_room_slot], // SEALED!
                    hallway_entry_room_slot: Some(from_room_slot),
                    hallway_exit_room_slot: None,
                    target_room: Some(to),
                    room_role: role,
                    corridor_roles: Vec::new(),
                    seed,
                    version: 0,
                    exit_locked: false,
                    exit_room: RoomId(EXIT_ROOM),
                    pinned_corridors: Vec::new(),
                    map_spec: None,
                };

                let room_place = Place::Room(from);
                let topology = place_junction(room_place, &nav);
                assert_eq!(topology.threshold_count(), 2);

                let socket = observed_core::ThresholdId::new(
                    observed_core::PlaceId::Room(from),
                    from_room_slot.0,
                );
                assert!(
                    topology.partner(socket).is_none(),
                    "sealed socket must have no topology partner"
                );

                let geom = geom_for(room_place, &nav);
                let gap = geom
                    .gaps
                    .iter()
                    .find(|g| g.target == to)
                    .expect("gap to target");
                assert!(
                    !gap.kind.is_passage(),
                    "sealed gap kind must be non-passage"
                );

                let (dest_place, arrived) = apply_crossing(room_place, gap, &nav);
                assert_eq!(dest_place, room_place, "sealed gap must not be crossable");
                assert_eq!(arrived, Crossing::ArrivedRoom(from));
            }
        }
    }

    fn is_graph_solvable(
        connections_map: &std::collections::HashMap<RoomId, Vec<RoomId>>,
        start: RoomId,
        exit: RoomId,
    ) -> bool {
        let mut queue = std::collections::VecDeque::new();
        let mut visited = std::collections::BTreeSet::new();
        queue.push_back(start);
        visited.insert(start);
        while let Some(room) = queue.pop_front() {
            if room == exit {
                return true;
            }
            if let Some(neighbors) = connections_map.get(&room) {
                for &next_room in neighbors {
                    if visited.insert(next_room) {
                        queue.push_back(next_room);
                    }
                }
            }
        }
        false
    }

    #[test]
    fn phase75_corpus_parity_reroute_solvability() {
        use crate::map_validation::nav_for_spec_room;
        use observed_facility::map_spec::sector_relay_v1;

        let spec = sector_relay_v1();
        let start = spec.start_room().expect("start room");
        let exit = spec.exit_room().expect("exit room");

        for seed in 0..8 {
            for version in 0..4 {
                let mut adjacency: std::collections::HashMap<RoomId, Vec<RoomId>> =
                    std::collections::HashMap::new();

                for room in &spec.rooms {
                    let nav_a = nav_for_spec_room(&spec, seed, version, room.id);

                    for &connection in &nav_a.connections {
                        let nav_b = nav_for_spec_room(&spec, seed, version, connection);

                        let slot_a = nav_a.slot_for(connection).unwrap();
                        let slot_b = nav_b.slot_for(room.id).unwrap();

                        let is_sealed_a = nav_a.sealed_slots.contains(&slot_a);
                        let is_sealed_b = nav_b.sealed_slots.contains(&slot_b);

                        if !is_sealed_a && !is_sealed_b {
                            adjacency.entry(room.id).or_default().push(connection);
                        }
                    }
                }

                let is_reachable = is_graph_solvable(&adjacency, start, exit);
                assert!(
                    is_reachable,
                    "Graph unsolvable via active slots: seed {seed} version {version}"
                );
            }
        }
    }

    #[test]
    fn phase76_multi_exit_crossing_integration_test() {
        use crate::hallway;
        use crate::map_validation::nav_for_spec_room;
        use crate::teleport::geom::{
            HallwayGeomEndpoints, hallway_geom_with_slots_and_role_and_spec,
            room_geom_with_slots_and_seals_for_role,
        };
        use crate::teleport::transition::apply_crossing;
        use crate::teleport::{Crossing, Place};
        use observed_core::Direction;
        use observed_facility::map_spec::multi_exit_fixture;

        let spec = multi_exit_fixture();
        let seed = 0;
        let version = 0;

        // (1) Cross from Room 1 (Decision) to Corridor 80 (Gantry)
        let room1 = RoomId(1);
        let nav1 = nav_for_spec_room(&spec, seed, version, room1);
        let room1_geom = room_geom_with_slots_and_seals_for_role(
            room1,
            &nav1.connections,
            &nav1.connection_slots,
            &nav1.sealed_slots,
            nav1.target_room,
            nav1.room_role,
            nav1.seed,
        );

        let gap_to_2 = room1_geom
            .gaps
            .iter()
            .find(|g| g.target == RoomId(2))
            .expect("gap to Room 2");

        let (hall_place, entered) = apply_crossing(Place::Room(room1), gap_to_2, &nav1);

        assert_eq!(hall_place.corridor_id(), Some(CorridorId(80)));
        if let Place::Hallway {
            corridor,
            entered_socket,
            from,
            to,
            ..
        } = hall_place
        {
            assert_eq!(corridor, CorridorId(80));
            assert_eq!(entered_socket, ThresholdSlotId(0));
            assert_eq!(from, room1);
            assert_eq!(to, RoomId(2));
        } else {
            panic!("Expected Place::Hallway");
        }

        assert!(matches!(
            entered,
            Crossing::EnteredHallway { corridor, .. } if corridor == CorridorId(80)
        ));

        // (2) Get the Gantry 80 geometry and cross back from hallway to the other exits
        let hall_geom_80 = hallway_geom_with_slots_and_role_and_spec(
            HallwayGeomEndpoints {
                from: RoomId(1),
                to: RoomId(2),
                from_room_slot: ThresholdSlotId(Direction::East.index() as u16),
                to_room_slot: ThresholdSlotId(Direction::West.index() as u16),
                exit_room: RoomId(EXIT_ROOM),
            },
            hallway::gantry_template(),
            seed,
            false,
            Some(CorridorRole::Gantry),
            Some(&spec),
        );

        // Find passage gaps in the hallway geometry
        let gap_back_to_1 = hall_geom_80
            .gaps
            .iter()
            .find(|g| g.target == RoomId(1))
            .expect("gap back to Room 1");
        let gap_to_2_hall = hall_geom_80
            .gaps
            .iter()
            .find(|g| g.target == RoomId(2))
            .expect("gap to Room 2");
        let gap_to_3_hall = hall_geom_80
            .gaps
            .iter()
            .find(|g| g.target == RoomId(3))
            .expect("gap to Room 3 (side exit)");

        // Let's verify crossing from hallway back to Room 1:
        let (dest1, arrived1) = apply_crossing(hall_place, gap_back_to_1, &nav1);
        assert_eq!(dest1, Place::Room(RoomId(1)));
        assert_eq!(arrived1, Crossing::ArrivedRoom(RoomId(1)));

        // Let's verify crossing from hallway to Room 2:
        let (dest2, arrived2) = apply_crossing(hall_place, gap_to_2_hall, &nav1);
        assert_eq!(dest2, Place::Room(RoomId(2)));
        assert_eq!(arrived2, Crossing::ArrivedRoom(RoomId(2)));

        // Let's verify crossing from hallway to Room 3:
        let (dest3, arrived3) = apply_crossing(hall_place, gap_to_3_hall, &nav1);
        assert_eq!(dest3, Place::Room(RoomId(3)));
        assert_eq!(arrived3, Crossing::ArrivedRoom(RoomId(3)));

        // --- PART 2: TEST WELLSHAFT 81 (3-EXIT) ---
        // Let's get the Nav for Room 2 (Keystone).
        let room2 = RoomId(2);
        let nav2 = nav_for_spec_room(&spec, seed, version, room2);
        let room2_geom = room_geom_with_slots_and_seals_for_role(
            room2,
            &nav2.connections,
            &nav2.connection_slots,
            &nav2.sealed_slots,
            nav2.target_room,
            nav2.room_role,
            nav2.seed,
        );

        let gap_to_4 = room2_geom
            .gaps
            .iter()
            .find(|g| g.target == RoomId(4))
            .expect("gap to Room 4");

        let (hall_place2, _entered2) = apply_crossing(Place::Room(room2), gap_to_4, &nav2);

        assert_eq!(hall_place2.corridor_id(), Some(CorridorId(81)));
        if let Place::Hallway {
            corridor,
            entered_socket,
            from,
            to,
            ..
        } = hall_place2
        {
            assert_eq!(corridor, CorridorId(81));
            assert_eq!(entered_socket, ThresholdSlotId(0));
            assert_eq!(from, room2);
            assert_eq!(to, RoomId(4));
        } else {
            panic!("Expected Place::Hallway");
        }

        let hall_geom_81 = hallway_geom_with_slots_and_role_and_spec(
            HallwayGeomEndpoints {
                from: RoomId(2),
                to: RoomId(4),
                from_room_slot: ThresholdSlotId(Direction::South.index() as u16),
                to_room_slot: ThresholdSlotId(Direction::North.index() as u16),
                exit_room: RoomId(EXIT_ROOM),
            },
            hallway::wellshaft_template(),
            seed,
            false,
            Some(CorridorRole::Vertical),
            Some(&spec),
        );

        let gap_back_to_2 = hall_geom_81
            .gaps
            .iter()
            .find(|g| g.target == RoomId(2))
            .expect("gap back to Room 2");
        let gap_to_4_hall = hall_geom_81
            .gaps
            .iter()
            .find(|g| g.target == RoomId(4))
            .expect("gap to Room 4");
        let gap_to_12_hall = hall_geom_81
            .gaps
            .iter()
            .find(|g| g.target == RoomId(12))
            .expect("gap to Room 12");

        let (dest2_back, arrived2_back) = apply_crossing(hall_place2, gap_back_to_2, &nav2);
        assert_eq!(dest2_back, Place::Room(RoomId(2)));
        assert_eq!(arrived2_back, Crossing::ArrivedRoom(RoomId(2)));

        let (dest4, arrived4) = apply_crossing(hall_place2, gap_to_4_hall, &nav2);
        assert_eq!(dest4, Place::Room(RoomId(4)));
        assert_eq!(arrived4, Crossing::ArrivedRoom(RoomId(4)));

        let (dest12, arrived12) = apply_crossing(hall_place2, gap_to_12_hall, &nav2);
        assert_eq!(dest12, Place::Room(RoomId(12)));
        assert_eq!(arrived12, Crossing::ArrivedRoom(RoomId(12)));
    }
}
