//! Collision-resolved per-player movement and world-position to lattice sync.
//! Stairs, ramps, halls, and rooms all use the same physical controller; tile
//! traversal never takes ownership of a body or rewrites its pose.

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexCoord, HexFace, HexWfcConfig};
use observed_hex::{TILE_LEVEL_HEIGHT, hex_origin};
use observed_traversal::rapier_controller::step_character_with_settings;
use player_input::PlayerIntent;

use super::{FIXED_DT, FLOOR_SLAB_TOP, HexMatchEvent, HexMatchEventKind, HexWfcMatch};

/// Net displacement required before the body establishes a new progress
/// anchor. Small collision jitter does not count as useful movement.
const STUCK_PROGRESS_EPS: f32 = 0.4;
/// Three seconds on a roof: long enough to read where you landed, short enough that a
/// fall is a setback rather than a wait.
pub(super) const STRANDED_RECOVERY_TICKS: u16 = 180;

/// Plan-view deadband a body must gain toward a new same-level cell's centre
/// before its logical cell switches, preventing boundary jitter.
const CELL_SWITCH_HYSTERESIS: f32 = 1.5;

/// Height a body's feet must clear past a level's floor plane before the
/// logical level follows.
///
/// Rounding the body's *centre* height put the changeover exactly halfway up a
/// level, which is precisely where a body stands while climbing a stair or ramp
/// — so a few centimetres of collision jitter flipped the logical level every
/// tick. Because the bot re-plans from its logical cell each tick and steers at
/// the next hop, a flipping level reversed its heading: bots ping-ponged up and
/// down a shaft for thousands of ticks (measured: five round trips, ~7,900
/// ticks, to climb two levels) without ever registering as stuck, since they
/// were moving the whole time.
///
/// Resolving from the feet against the floor plane instead puts the changeover
/// at the deck a body actually stands on, and these two margins keep it there.
///
/// They are deliberately asymmetric: climbing must nearly *reach* the deck above
/// before the level follows, while descending only needs to clearly *leave* this
/// deck. A single shared margin would put both thresholds at the same height,
/// which is no hysteresis at all.
const LEVEL_ARRIVE_MARGIN: f32 = 0.6;
/// See [`LEVEL_ARRIVE_MARGIN`]; must be the larger of the two.
const LEVEL_DEPART_MARGIN: f32 = 1.2;

impl HexWfcMatch {
    /// Advance one player by one fixed physical step.
    pub(super) fn move_player(&mut self, id: PlayerId, intent: PlayerIntent) {
        let player = &self.players[&id];
        if player.escaped {
            return;
        }
        match player.place {
            super::HexBodyPlace::Facility => {}
            super::HexBodyPlace::Prison => {
                self.move_jailed(id, intent);
                return;
            }
            super::HexBodyPlace::Void => return,
        }
        let profile = self.content.traversal_profile();
        let config = profile.controller();
        let step = step_character_with_settings(
            &self.physics,
            self.bodies.get_mut(&id).expect("body"),
            intent,
            &config,
            profile.rapier(),
            FIXED_DT,
        );
        self.sync_player_from_body(id);
        if step.recovered {
            self.recent_events.push(HexMatchEvent {
                tick: self.tick,
                kind: HexMatchEventKind::PlayerRecovered,
                player: Some(id),
                cell: Some(self.players[&id].cell),
            });
        }
    }

    /// Project the Rapier body back onto the lattice. Logical level follows the
    /// deck the feet are standing on, damped by [`LEVEL_SWITCH_HYSTERESIS`], so
    /// a body partway up a ramp or stair resolves to one level and stays there.
    pub(super) fn sync_player_from_body(&mut self, id: PlayerId) {
        let body = self.bodies[&id];
        let half_height = self
            .content
            .traversal_profile()
            .requirements()
            .capsule_half_height;
        let top_level = f32::from(self.facility.config.levels.saturating_sub(1));
        let current_level = self.players[&id].cell.level;
        let level = resolve_level(body.position.y - half_height, current_level, top_level);
        let candidate =
            horizontal_cell(self.facility.config, body.position, level).filter(|cell| {
                self.facility
                    .placements
                    .get(cell)
                    .is_some_and(|placement| placement.space.built())
            });
        let current = self.players[&id].cell;
        let current_valid = self
            .facility
            .placements
            .get(&current)
            .is_some_and(|placement| placement.space.built());
        let player = self.players.get_mut(&id).expect("player");
        if let Some(cell) = candidate {
            let switch = !current_valid
                || cell.level != current.level
                || plan_distance_xz(body.position, hex_origin(cell)) + CELL_SWITCH_HYSTERESIS
                    < plan_distance_xz(body.position, hex_origin(current));
            if switch {
                player.cell = cell;
            }
        }
        player.position = body.position;
        player.yaw = body.yaw;
        player.pitch = body.pitch;
    }

    /// Recover bodies that genuinely leave the arena, and bodies stranded outside
    /// every built cell. Ordinary physical falls between levels remain gameplay and
    /// are never converted to motion.
    ///
    /// Stranding is what open edges make possible: a body that steps off a bare
    /// high edge can land on the roof of a lower hall, where the railings that keep
    /// people from falling out of the loggias around it also keep it from getting
    /// back in. A fall is meant to cost time and ground, not the match, so a body
    /// that stands on a roof for [`STRANDED_RECOVERY_TICKS`] is returned to the last
    /// cell it stood in.
    pub(super) fn recover_fallen_bodies(&mut self) {
        let floor_y = self.geometry.arena.floor_y;
        let half_height = self
            .content
            .traversal_profile()
            .requirements()
            .capsule_half_height;
        let tick = self.tick;
        let mut recovered = Vec::new();
        let mut lost = Vec::new();
        for player in self.players.values() {
            // A jailed body is kept in its maze by `move_jailed`; a lost one is gone.
            if !player.in_facility() {
                continue;
            }
            let body = self.bodies[&player.id];
            let out_of_world = !body.position.is_finite() || body.position.y < floor_y - 4.0;
            let stranded =
                body.grounded && !self.stands_in_built_cell(body.position - Vec3::Y * half_height);
            let ticks = self.stranded_ticks.entry(player.id).or_insert(0);
            *ticks = if stranded { ticks.saturating_add(1) } else { 0 };
            // Where catches go to prison, a fall through the whole facility is true void:
            // the body does not come back. A roof you cannot climb down from is still only
            // a fall, and is still recovered.
            if out_of_world && self.prison.is_some() {
                lost.push(player.id);
            } else if out_of_world || *ticks >= STRANDED_RECOVERY_TICKS {
                recovered.push((player.id, player.cell));
            }
        }
        for id in lost {
            self.lose(id);
        }
        for (id, cell) in recovered {
            self.stranded_ticks.insert(id, 0);
            let anchor =
                Vec3::from_array(hex_origin(cell)) + Vec3::Y * (FLOOR_SLAB_TOP + half_height);
            *self.bodies.get_mut(&id).expect("body") =
                observed_traversal::FpsBody::spawned(anchor, self.players[&id].yaw);
            self.players.get_mut(&id).expect("player").position = anchor;
            self.recent_events.push(HexMatchEvent {
                tick,
                kind: HexMatchEventKind::PlayerRecovered,
                player: Some(id),
                cell: Some(cell),
            });
        }
    }

    /// Whether feet at `feet` stand inside a built cell: the deck nearest them, at the
    /// plan cell under them. A roof resolves to the unbuilt cell above it.
    fn stands_in_built_cell(&self, feet: Vec3) -> bool {
        let top_level = f32::from(self.facility.config.levels.saturating_sub(1));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let level = ((feet.y - FLOOR_SLAB_TOP) / TILE_LEVEL_HEIGHT)
            .round()
            .clamp(0.0, top_level) as u8;
        containing_cell(self.facility.config, feet, level)
            .and_then(|cell| self.facility.placements.get(&cell))
            .is_some_and(|placement| placement.space.built())
    }

    /// Update each player's no-progress counter for the objective bot's
    /// collision recovery steering.
    pub(super) fn update_stuck_ticks(&mut self) {
        for id in self.players.keys().copied().collect::<Vec<_>>() {
            let player = &self.players[&id];
            let position = player.position;
            // A jailed body walks too, and can wedge in its maze like anywhere else.
            let walking = player.in_facility() || player.place == super::HexBodyPlace::Prison;
            if !walking {
                self.progress_anchor.insert(id, position);
                self.stuck_ticks.insert(id, 0);
                continue;
            }
            let anchor = *self.progress_anchor.entry(id).or_insert(position);
            if position.distance(anchor) > STUCK_PROGRESS_EPS {
                self.progress_anchor.insert(id, position);
                self.stuck_ticks.insert(id, 0);
            } else {
                let counter = self.stuck_ticks.entry(id).or_insert(0);
                *counter = counter.saturating_add(1);
            }
        }
    }

    /// Reconcile simulation-authorized teleports (spawn, setbacks, escape) to
    /// fresh physical bodies. Tile traversal never calls this itself.
    pub(super) fn sync_teleports_to_bodies(&mut self) {
        for player in self.players.values() {
            let body = self.bodies.get_mut(&player.id).expect("body");
            if body.position.distance_squared(player.position) > 0.000_001 {
                *body = observed_traversal::FpsBody::spawned(player.position, player.yaw);
                body.pitch = player.pitch;
            }
        }
    }
}

/// The logical level for a body whose feet are at `feet`, holding `current`
/// unless the feet have clearly committed to another deck.
///
/// A level's walking deck sits [`FLOOR_SLAB_TOP`] above its base, so the level
/// nearest the feet is measured against that plane rather than by rounding the
/// body's centre. The two margins then form a genuine hysteresis band: within it
/// the level is bistable and simply holds, so jitter cannot flip it. A body that
/// falls several levels still resolves in one step, because the nearest level is
/// taken directly once the feet have clearly left this deck.
///
/// Pure and total — no state beyond the arguments, so it stays deterministic.
fn resolve_level(feet: f32, current: u8, top_level: f32) -> u8 {
    let deck_of = |level: f32| level * TILE_LEVEL_HEIGHT + FLOOR_SLAB_TOP;
    let nearest = ((feet - FLOOR_SLAB_TOP) / TILE_LEVEL_HEIGHT)
        .round()
        .clamp(0.0, top_level) as u8;
    if nearest == current {
        return current;
    }
    let current_deck = deck_of(f32::from(current));
    let committed = if nearest > current {
        // Climbing: hold this level until the deck above is nearly underfoot.
        feet >= deck_of(f32::from(current) + 1.0) - LEVEL_ARRIVE_MARGIN
    } else {
        // Descending: hold this level until the feet are clearly below its deck.
        feet <= current_deck - LEVEL_DEPART_MARGIN
    };
    if committed { nearest } else { current }
}

fn plan_distance_xz(position: Vec3, center: [f32; 3]) -> f32 {
    Vec2::new(position.x - center[0], position.z - center[2]).length()
}

/// Recover `(q, r)` from a plan-view world position at a known level.
pub(super) fn horizontal_cell(config: HexWfcConfig, position: Vec3, level: u8) -> Option<HexCoord> {
    let r = (position.z / 12.0).round();
    let q = ((position.x - r * 7.0) / 14.0).round();
    let (qi, ri) = (q as i32, r as i32);
    (qi >= 0 && ri >= 0 && qi < i32::from(config.cols) && ri < i32::from(config.rows)).then_some(
        HexCoord {
            q: qi as u16,
            r: ri as u16,
            level,
        },
    )
}

/// The cell whose footprint contains a plan position, exactly: the nearest cell
/// centre among the candidates around the rounded guess. [`horizontal_cell`] rounds
/// each axial coordinate on its own, which is close but can name a neighbour near a
/// corner; that is harmless for tracking and not for deciding somebody is on a roof.
pub(super) fn containing_cell(config: HexWfcConfig, position: Vec3, level: u8) -> Option<HexCoord> {
    let guess = horizontal_cell(config, position, level)?;
    let mut best: Option<(f32, HexCoord)> = None;
    for dr in -1..=1 {
        for dq in -1..=1 {
            let (q, r) = (i32::from(guess.q) + dq, i32::from(guess.r) + dr);
            if q < 0 || r < 0 || q >= i32::from(config.cols) || r >= i32::from(config.rows) {
                continue;
            }
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let cell = HexCoord {
                q: q as u16,
                r: r as u16,
                level,
            };
            let distance = plan_distance_xz(position, hex_origin(cell));
            if best.is_none_or(|(nearest, _)| distance < nearest) {
                best = Some((distance, cell));
            }
        }
    }
    best.map(|(_, cell)| cell)
}

/// Plan-view unit direction of a lateral face.
pub(super) fn face_plan_dir(face: HexFace) -> Vec2 {
    let (dq, dr, _) = face.delta();
    let x = dq * 14 + dr * 7;
    let z = dr * 12;
    Vec2::new(x as f32, z as f32).normalize_or_zero()
}

/// Coarse looked-at face for threshold observation.
pub(super) fn look_face(yaw: f32, pitch: f32) -> HexFace {
    if pitch > 0.72 {
        return HexFace::Up;
    }
    if pitch < -0.72 {
        return HexFace::Down;
    }
    let heading = Vec2::new(yaw.sin(), -yaw.cos());
    HexFace::LATERAL
        .into_iter()
        .max_by(|&a, &b| {
            heading
                .dot(face_plan_dir(a))
                .total_cmp(&heading.dot(face_plan_dir(b)))
        })
        .expect("lateral faces are non-empty")
}

#[cfg(test)]
mod level_tests {
    use super::{LEVEL_ARRIVE_MARGIN, LEVEL_DEPART_MARGIN, resolve_level};
    use observed_hex::TILE_LEVEL_HEIGHT;

    const TOP: f32 = 3.0;

    /// The invariant that matters. The measured stall was a bot ping-ponging
    /// between level 0 and level 1 — five round trips, ~7,900 ticks, to climb
    /// two levels — because rounding the body's centre height flipped the
    /// logical level on a few centimetres of jitter, reversing the bot's
    /// heading each tick.
    ///
    /// No height may resolve *upward* from the lower level and *downward* from
    /// the upper one; that combination is what oscillates. Holding different
    /// levels at the same height (bistability) is exactly the intent.
    #[test]
    fn no_height_flips_in_both_directions() {
        for step in 0..=(TILE_LEVEL_HEIGHT as i32 * 40) {
            let feet = f32::from(step as i16) * 0.1;
            let from_lower = resolve_level(feet, 0, TOP);
            let from_upper = resolve_level(feet, 1, TOP);
            assert!(
                !(from_lower > 0 && from_upper < 1),
                "feet={feet} flips both ways: 0->{from_lower} and 1->{from_upper}"
            );
        }
    }

    /// Specifically the height the stalling bots sat at: feet ~3.1 m, halfway
    /// between the level-0 deck (0.5 m) and the level-1 deck (8.5 m). Climbing
    /// from below must hold level 0 there — under the old centre-rounding it
    /// resolved to 1, and the next tick's jitter sent it back to 0.
    #[test]
    fn the_measured_oscillation_height_holds_while_climbing() {
        for feet in [3.0_f32, 3.1, 3.2, 3.9, 4.0, 4.1] {
            assert_eq!(resolve_level(feet, 0, TOP), 0, "feet={feet} should hold 0");
        }
    }

    /// Resolving is idempotent at every height: whatever level a body settles
    /// on, re-resolving from that level returns it unchanged. This is the
    /// property that makes the logical level usable as a control input.
    #[test]
    fn resolving_is_idempotent() {
        for step in 0..=(TILE_LEVEL_HEIGHT as i32 * 40) {
            let feet = f32::from(step as i16) * 0.1;
            for start in 0..=3u8 {
                let once = resolve_level(feet, start, TOP);
                let twice = resolve_level(feet, once, TOP);
                assert_eq!(once, twice, "feet={feet} start={start} not idempotent");
            }
        }
    }

    #[test]
    fn arriving_at_a_deck_switches_level() {
        // Standing on the level-1 deck is unambiguously level 1...
        let deck1 = TILE_LEVEL_HEIGHT + 0.5;
        assert_eq!(resolve_level(deck1, 0, TOP), 1);
        // ...and it commits a touch before arrival, by the arrive margin.
        assert_eq!(resolve_level(deck1 - LEVEL_ARRIVE_MARGIN, 0, TOP), 1);
        assert_eq!(resolve_level(deck1 - LEVEL_ARRIVE_MARGIN - 0.1, 0, TOP), 0);
    }

    #[test]
    fn descending_needs_to_clear_the_depart_margin() {
        let deck1 = TILE_LEVEL_HEIGHT + 0.5;
        assert_eq!(resolve_level(deck1 - LEVEL_DEPART_MARGIN + 0.1, 1, TOP), 1);
        assert_eq!(resolve_level(0.5, 1, TOP), 0);
    }

    /// A body that falls several levels resolves in one step, not one per tick.
    #[test]
    fn a_long_fall_resolves_directly() {
        assert_eq!(resolve_level(0.5, 3, TOP), 0);
    }

    #[test]
    fn level_stays_within_the_grid() {
        assert_eq!(resolve_level(-50.0, 0, TOP), 0);
        assert_eq!(resolve_level(9_999.0, 3, TOP), 3);
    }
}
