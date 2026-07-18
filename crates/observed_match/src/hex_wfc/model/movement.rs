//! Per-player physical integration: plain walking (including up ramps) through
//! the Rapier scene, shaft climbing as a scripted vertical traversal, and the
//! world-position → `HexCoord` sync that keeps each player's logical cell in
//! step with the walking surface.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexArchetype, HexCoord, HexFace, HexPlacement, HexSpace, HexWfcConfig, PortClass,
};
use observed_hex::{TILE_LEVEL_HEIGHT, hex_origin};
use observed_traversal::rapier_controller::step_character;
use player_input::PlayerIntent;

use super::{CLIMB_SPEED, FIXED_DT, HexMatchEvent, HexMatchEventKind, HexWfcMatch};

/// Radius (metres) around a cell centre within which a shaft climb may be
/// initiated. Hex cells span 14 m across the flats and the shaft-landing tiles
/// wrap the central well with steps and rails, so the bot cannot always reach
/// dead centre; this admits the scripted climb from anywhere on the landing.
const SHAFT_CLIMB_RADIUS: f32 = 5.5;

/// How far (metres, toward an open door) a completed climb lands the body off
/// the shaft column's centre, clearing the vertical opening it climbed through.
const SHAFT_LANDING_OFFSET: f32 = 4.5;

/// Plan-view distance (metres) within which a shaft-transit body is treated as
/// standing on the column centre and may commit its lateral exit.
const SHAFT_CENTER_EPS: f32 = 1.0;

/// Minimum heading alignment (dot of the look heading with a lateral door's
/// plan direction) required before a centred shaft body commits its scripted
/// exit through that door. Below this the body holds centre and keeps rotating.
const SHAFT_EXIT_COMMIT_DOT: f32 = 0.5;

/// Deterministic glide speed (metres/second) of a scripted shaft transit, both
/// the centre-seek and the lateral step off the landing.
const SHAFT_TRANSIT_SPEED: f32 = 6.0;

/// Height (metres) of the walkable floor-slab top above a cell's level base.
/// Every authored tile lays a 0.5 m floor slab (`FLOOR_TOP` = 8 TB units at 16
/// units/m), so a body resting on a cell floor sits `FLOOR_SLAB_TOP +
/// half_height` above `hex_origin(cell).y`. Scripted glides must land here, not
/// on the bare level base — a ramp entrance's sloped slab and door frame jam a
/// body dropped 0.5 m low rather than popping it up as a flat floor would.
const FLOOR_SLAB_TOP: f32 = 0.5;

/// Net displacement (metres) from its progress anchor a body must make to count
/// as progressing; below this it is judged wedged (jitter-immune). A walker
/// clears it in a handful of ticks; a jammed body never does.
const STUCK_PROGRESS_EPS: f32 = 0.4;

/// Plan-view deadband (metres) a body must gain toward a new same-level cell's
/// centre before its logical cell switches, so boundary jitter cannot flip it.
const CELL_SWITCH_HYSTERESIS: f32 = 1.5;

impl HexWfcMatch {
    /// Advance one player by one fixed step. Ramps are plain walking under
    /// `step_character`; only shafts use the scripted vertical climb.
    pub(super) fn move_player(&mut self, id: PlayerId, intent: PlayerIntent) {
        if self.players[&id].escaped {
            return;
        }

        // An in-progress shaft climb owns the whole step: glide vertically to
        // the destination level, then land at the target cell centre.
        if let Some(target) = self.players[&id].climb_target {
            self.advance_climb(id, target);
            return;
        }

        // An in-progress scripted lateral exit off a shaft landing owns the
        // step: glide horizontally across the landing and through the door.
        if let Some(target) = self.players[&id].transit_target {
            self.advance_shaft_exit(id, target);
            return;
        }

        // Shaft climb initiation: pressing jump near a shaft column's centre
        // begins an upward climb, holding interact a downward one.
        let cell = self.players[&id].cell;
        let climb_face = if intent.jump_pressed {
            Some(HexFace::Up)
        } else if intent.interact_held {
            Some(HexFace::Down)
        } else {
            None
        };
        if let Some(face) = climb_face {
            let center = Vec3::from_array(hex_origin(cell));
            let position = self.players[&id].position;
            let near_shaft = Vec2::new(position.x - center.x, position.z - center.z)
                .length_squared()
                <= SHAFT_CLIMB_RADIUS * SHAFT_CLIMB_RADIUS;
            if near_shaft
                && let Some(target) =
                    shaft_target(self.facility.config, &self.facility.placements, cell, face)
            {
                self.players.get_mut(&id).expect("player").climb_target = Some(target);
                return;
            }
        }

        // Shaft cells are scripted transit nodes for lateral locomotion too. The
        // interior climbing ledges and the open central well tangle naive
        // center-seeking physics (the body climbs a ledge and jams at the door
        // sill, or falls through the well), so a lateral move inside a shaft is
        // scripted: seek the column centre at floor height, then — once centred
        // and facing an open door — commit a glide out through it. Ramps stay
        // plain walking; only shaft cells take this path.
        if climb_face.is_none()
            && intent.movement.length_squared() > 1e-6
            && self
                .facility
                .placements
                .get(&cell)
                .is_some_and(|placement| placement.archetype == HexArchetype::Shaft)
        {
            self.advance_shaft_locomotion(id, cell, intent);
            return;
        }

        step_character(
            &self.physics,
            self.bodies.get_mut(&id).expect("body"),
            intent,
            &self.traversal_config,
            FIXED_DT,
        );
        self.sync_player_from_body(id);
    }

    fn advance_climb(&mut self, id: PlayerId, target: HexCoord) {
        let target_y = hex_origin(target)[1] + FLOOR_SLAB_TOP + self.traversal_config.half_height;
        let body = self.bodies.get_mut(&id).expect("body");
        let delta = target_y - body.position.y;
        let step = CLIMB_SPEED * FIXED_DT;
        if delta.abs() <= step {
            let angle = f32::from(id.0) * (TAU / 8.0);
            let perturb = Vec3::new(angle.cos() * 0.5, 0.0, angle.sin() * 0.5);
            // Land on the landing floor beside the shaft well, not over the
            // central opening (which the body would immediately fall back
            // through). Offset toward the first open door of the target cell.
            let landing = self
                .facility
                .placements
                .get(&target)
                .and_then(|placement| HexFace::LATERAL.into_iter().find(|&f| placement.is_open(f)))
                .map_or(Vec3::ZERO, |face| {
                    let dir = face_plan_dir(face);
                    Vec3::new(dir.x, 0.0, dir.y) * SHAFT_LANDING_OFFSET
                });
            body.position = Vec3::from_array(hex_origin(target))
                + Vec3::Y * self.traversal_config.half_height
                + landing
                + perturb;
            body.velocity = Vec3::ZERO;
            let player = self.players.get_mut(&id).expect("player");
            player.cell = target;
            player.climb_target = None;
        } else {
            body.position.y += delta.signum() * step;
            body.velocity = Vec3::ZERO;
        }
        self.sync_player_from_body(id);
    }

    /// Scripted shaft locomotion, entered while standing in a shaft cell with a
    /// lateral movement intent and no climb press. Glides to the column centre
    /// at floor height (crossing the open well kinematically), rotating toward
    /// the intent heading; once centred and aligned with an open door it commits
    /// a [`Self::advance_shaft_exit`] glide out through that door.
    fn advance_shaft_locomotion(&mut self, id: PlayerId, cell: HexCoord, intent: PlayerIntent) {
        let center = Vec3::from_array(hex_origin(cell));
        let floor_y = center.y + FLOOR_SLAB_TOP + self.traversal_config.half_height;
        let look_step = self.traversal_config.look_step;
        let body = self.bodies.get_mut(&id).expect("body");
        // Keep rotating toward the heading so the exit door is picked correctly.
        body.yaw = (body.yaw + intent.look.x * look_step).rem_euclid(TAU);
        let plan = Vec2::new(body.position.x - center.x, body.position.z - center.z);
        let step = SHAFT_TRANSIT_SPEED * FIXED_DT;
        if plan.length() > SHAFT_CENTER_EPS {
            // Seek the column centre at floor height.
            let inward = (-plan).normalize_or_zero() * step.min(plan.length());
            body.position = Vec3::new(
                body.position.x + inward.x,
                floor_y,
                body.position.z + inward.y,
            );
            body.velocity = Vec3::ZERO;
            self.sync_player_from_body(id);
            return;
        }
        // Centred: pick the open lateral door best matching the look heading and
        // commit the scripted exit if the alignment is decisive.
        body.position = Vec3::new(center.x, floor_y, center.z);
        body.velocity = Vec3::ZERO;
        let heading = Vec2::new(body.yaw.sin(), -body.yaw.cos());
        let placement = self.facility.placements[&cell];
        let exit = HexFace::LATERAL
            .into_iter()
            .filter(|&face| placement.is_open(face))
            .map(|face| (face, heading.dot(face_plan_dir(face))))
            .filter(|&(_, dot)| dot >= SHAFT_EXIT_COMMIT_DOT)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(face, _)| face)
            .and_then(|face| self.facility.config.grid().neighbor(cell, face))
            .filter(|next| {
                self.facility
                    .placements
                    .get(next)
                    .is_some_and(|placement| placement.space != HexSpace::Void)
            });
        if let Some(target) = exit {
            self.players.get_mut(&id).expect("player").transit_target = Some(target);
        }
        self.sync_player_from_body(id);
    }

    /// Advance a committed scripted lateral shaft exit: glide along the exit
    /// heading at floor height until the body crosses the door into the target
    /// cell, then hand back to plain walking with forward momentum. Central
    /// pylons in a target junction are handled downstream by the bot's steering
    /// (which rounds them), so this only has to clear the landing and door.
    fn advance_shaft_exit(&mut self, id: PlayerId, target: HexCoord) {
        let target_center = Vec3::from_array(hex_origin(target));
        let floor_y = target_center.y + FLOOR_SLAB_TOP + self.traversal_config.half_height;
        let run_speed = self.traversal_config.run_speed;
        let body = self.bodies.get_mut(&id).expect("body");
        let plan = Vec2::new(
            target_center.x - body.position.x,
            target_center.z - body.position.z,
        );
        let dir = plan.normalize_or_zero();
        let step = SHAFT_TRANSIT_SPEED * FIXED_DT;
        let advance = dir * step.min(plan.length());
        body.position = Vec3::new(
            body.position.x + advance.x,
            floor_y,
            body.position.z + advance.y,
        );
        body.velocity = Vec3::ZERO;
        // Face the direction of travel so plain walking resumes cleanly.
        if dir.length_squared() > 1e-6 {
            body.yaw = dir.x.atan2(-dir.y);
        }
        // The glide ends the moment the body resolves into the target cell (it
        // has cleared the door), or on reaching the target centre. Hand off with
        // forward momentum so the body behaves like a natural walk-through.
        let resolved = self.players[&id].cell == target;
        if resolved || plan.length() <= step {
            self.bodies.get_mut(&id).expect("body").velocity =
                Vec3::new(dir.x, 0.0, dir.y) * run_speed;
            self.players.get_mut(&id).expect("player").transit_target = None;
        }
        self.sync_player_from_body(id);
    }

    /// Project the Rapier body back onto the lattice. The logical level follows
    /// the walking surface — the body's actual height — so a player mid-ramp
    /// resolves to the ramp head above once past the half-rise, never to the
    /// floor cell they physically stand over.
    pub(super) fn sync_player_from_body(&mut self, id: PlayerId) {
        let body = self.bodies[&id];
        let top_level = f32::from(self.facility.config.levels.saturating_sub(1));
        let level = (body.position.y / TILE_LEVEL_HEIGHT)
            .round()
            .clamp(0.0, top_level) as u8;
        let candidate =
            horizontal_cell(self.facility.config, body.position, level).filter(|cell| {
                self.facility
                    .placements
                    .get(cell)
                    .is_some_and(|placement| placement.space != HexSpace::Void)
            });
        let current = self.players[&id].cell;
        let current_valid = self
            .facility
            .placements
            .get(&current)
            .is_some_and(|placement| placement.space != HexSpace::Void);
        let player = self.players.get_mut(&id).expect("player");
        if let Some(cell) = candidate {
            // Axial rounding is ambiguous at a cell boundary, so a body jittering
            // on a corner would otherwise flip its logical cell every tick and
            // whipsaw the bot's route. Accept a vertical change immediately (a
            // climb or ramp genuinely changes level); damp a same-level flip with
            // hysteresis so the cell only switches once the body is clearly
            // inside the new hex.
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

    /// Deterministic fall recovery. Ordinary drops — including a full 8 m
    /// shaft/ramp fall between levels — land on a lower floor at `y >= 0` and
    /// are survivable-by-design: nothing here touches them. Only a body that has
    /// truly left the world volume (a non-finite position, or one below the
    /// arena floor) is reset to its logical cell anchor and reported as a
    /// [`HexMatchEventKind::PlayerRecovered`], so a physics blow-up can never
    /// silently strand a bot mid-route.
    pub(super) fn recover_fallen_bodies(&mut self) {
        let floor_y = self.geometry.arena.floor_y;
        let half_height = self.traversal_config.half_height;
        let tick = self.tick;
        let mut recovered = Vec::new();
        for player in self.players.values() {
            if player.escaped {
                continue;
            }
            let body = self.bodies[&player.id];
            let out_of_world = !body.position.is_finite() || body.position.y < floor_y - 4.0;
            if out_of_world {
                recovered.push((player.id, player.cell));
            }
        }
        for (id, cell) in recovered {
            let anchor =
                Vec3::from_array(hex_origin(cell)) + Vec3::Y * (FLOOR_SLAB_TOP + half_height);
            *self.bodies.get_mut(&id).expect("body") =
                observed_traversal::FpsBody::spawned(anchor, self.players[&id].yaw);
            let player = self.players.get_mut(&id).expect("player");
            player.position = anchor;
            player.climb_target = None;
            player.transit_target = None;
            self.recent_events.push(HexMatchEvent {
                tick,
                kind: HexMatchEventKind::PlayerRecovered,
                player: Some(id),
                cell: Some(cell),
            });
        }
    }

    /// Update each player's wedged-tick counter from net progress. A body that
    /// has moved clear of its progress anchor (in any axis, so vertical climbs
    /// count) re-anchors and resets; one that only jitters in place while not
    /// escaped or scripted accumulates, so the bot can detect a jam and sweep
    /// sideways out of it. Measuring against an anchor (not the previous tick)
    /// makes the detector immune to a body buzzing against a wall.
    pub(super) fn update_stuck_ticks(&mut self) {
        for id in self.players.keys().copied().collect::<Vec<_>>() {
            let player = &self.players[&id];
            let position = player.position;
            let scripted = player.climb_target.is_some() || player.transit_target.is_some();
            if player.escaped || scripted {
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

    /// Reconcile any bodies whose player state was teleported out from under
    /// them (spawn placement, escape resolution) back to a fresh body anchor.
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

/// The vertical neighbour reachable by a shaft climb across `face`, if the
/// source and destination both present a matching `ShaftOpen` port. Ramp
/// (`RampOpen`) bonds are excluded — those are walked, never climbed.
pub(super) fn shaft_target(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    cell: HexCoord,
    face: HexFace,
) -> Option<HexCoord> {
    let placement = placements.get(&cell)?;
    let source_class = if face == HexFace::Up {
        placement.up
    } else {
        placement.down
    };
    if source_class != PortClass::ShaftOpen {
        return None;
    }
    let next = config.grid().neighbor(cell, face)?;
    let neighbor = placements.get(&next)?;
    let dest_class = if face == HexFace::Up {
        neighbor.down
    } else {
        neighbor.up
    };
    (dest_class == PortClass::ShaftOpen).then_some(next)
}

/// Plan-view (x, z) distance from a world position to a cell-centre array.
fn plan_distance_xz(position: Vec3, center: [f32; 3]) -> f32 {
    Vec2::new(position.x - center[0], position.z - center[2]).length()
}

/// Inverse of [`observed_hex::hex_origin`] in plan view: recover `(q, r)` from a
/// world position at a known level. `x = 14q + 7r`, `z = 12r`.
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

/// Plan-view (x, z) unit direction of a lateral face, from the hex pitch. The
/// vertical faces have no lateral direction and return zero.
pub(super) fn face_plan_dir(face: HexFace) -> Vec2 {
    let (dq, dr, _) = face.delta();
    let x = dq * 14 + dr * 7;
    let z = dr * 12;
    Vec2::new(x as f32, z as f32).normalize_or_zero()
}

/// Coarse "which face is this player looking at" for threshold pinning: pitch
/// past the vertical thresholds selects `Up`/`Down`, otherwise the lateral face
/// whose plan direction best matches the yaw heading.
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
            let da = heading.dot(face_plan_dir(a));
            let db = heading.dot(face_plan_dir(b));
            da.total_cmp(&db)
        })
        .expect("lateral faces are non-empty")
}
