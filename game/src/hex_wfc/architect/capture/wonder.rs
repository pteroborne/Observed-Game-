//! Eye-level stills and real-controller traversal of a committed wonder.
use super::super::ArchitectDesk;
use crate::hex_wfc::{HexWfcCapture, sim::HexWfcRuntime};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_match::ascent::sim::{ArchitectCommand, CardKind};
/// Evidence-only traversal: the match is held still after a real card commit,
/// but this body walks its live collision snapshot with the production controller.
pub(in crate::hex_wfc) struct WonderWalk {
    scene: observed_traversal::rapier_controller::RapierTraversalScene,
    body: observed_traversal::FpsBody,
    route: Vec<Vec3>,
    cells: [observed_hex::HexCoord; 3],
    waypoint: usize,
    frame: u16,
}

pub(super) struct Inspection<'a> {
    pub(super) reservoir: Option<(observed_hex::HexCoord, u8)>,
    pub(super) portrait_start: Option<u16>,
    pub(super) walk: &'a mut Option<WonderWalk>,
    pub(super) factory: bool,
}

pub(super) fn inspect(
    commands: &mut Commands,
    request: &mut HexWfcCapture,
    runtime: &mut HexWfcRuntime,
    desk: &ArchitectDesk,
    exit: &mut MessageWriter<AppExit>,
    state: Inspection<'_>,
) {
    let Inspection {
        reservoir,
        portrait_start,
        walk,
        factory,
    } = state;
    let path = std::path::PathBuf::from(&request.path);
    let shoot = |commands: &mut Commands, name: &str| {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(
                path.join(format!("architect-{name}-1280x800.png")),
            ));
    };
    match request.stills {
        20 => {
            // Evidence poses: hold a real committed room still, then inspect it from
            // its doorway, its pool and the opposite bay. Normal play never enters here.
            let (Some((anchor, rotation)), Some(start), Some(eye)) =
                (reservoir, portrait_start, desk.eyes)
            else {
                error!("Wonder portraits require a committed room and a team eye");
                exit.write(AppExit::error());
                return;
            };
            let expected = runtime
                .ascent
                .as_ref()
                .expect("ascent capture")
                .rules()
                .played_wonder(
                    if factory {
                        CardKind::Chargeworks
                    } else {
                        CardKind::Cistern
                    },
                    anchor,
                    rotation,
                );
            if expected
                .iter()
                .any(|p| runtime.match_state.facility.placements.get(&p.coord) != Some(p))
            {
                error!(
                    "Wonder capture requires the complete physically committed card composition"
                );
                exit.write(AppExit::error());
                return;
            }
            let elapsed = request.frame.saturating_sub(start);
            let slot = elapsed / 180;
            if slot >= 4 {
                let cells = runtime
                    .ascent
                    .as_ref()
                    .expect("ascent capture")
                    .rules()
                    .played_wonder(
                        if factory {
                            CardKind::Chargeworks
                        } else {
                            CardKind::Cistern
                        },
                        anchor,
                        rotation,
                    )
                    .map(|p| p.coord);
                let centers = cells
                    .map(|cell| Vec3::from_array(observed_hex::hex_origin(cell)) + Vec3::Y * 0.75);
                let turn =
                    Quat::from_rotation_y(-f32::from(rotation % 6) * std::f32::consts::TAU / 6.0);
                let entry = centers[0] + turn * Vec3::new(-3.0, 0.0, -4.4);
                let config = observed_traversal::FpsConfig::default();
                *walk = Some(WonderWalk {
                    scene: runtime.match_state.geometry.rapier_scene(),
                    body: observed_traversal::FpsBody::spawned(
                        entry + Vec3::Y * config.half_height,
                        0.0,
                    ),
                    route: if factory {
                        let origin = Vec3::from_array(observed_hex::hex_origin(cells[1]));
                        let h = Quat::from_rotation_y(
                            -f32::from((rotation + 2) % 6) * std::f32::consts::TAU / 6.0,
                        );
                        let ramp = |p| origin + h * p;
                        vec![
                            entry,
                            centers[0],
                            centers[1],
                            ramp(Vec3::new(0.7, 0.5, -4.08)),
                            ramp(Vec3::new(-5.0, 0.5, -4.08)),
                            ramp(Vec3::new(-5.0, 3.0, 2.5)),
                            ramp(Vec3::new(-2.0, 3.0, 2.5)),
                            ramp(Vec3::new(-5.0, 3.0, 2.5)),
                            ramp(Vec3::new(-5.0, 0.5, -4.08)),
                            ramp(Vec3::new(0.7, 0.5, -4.08)),
                            centers[1],
                            centers[2],
                            centers[0],
                            entry,
                        ]
                    } else {
                        vec![entry, centers[0], centers[1], centers[2], centers[0], entry]
                    },
                    cells,
                    waypoint: 1,
                    frame: 0,
                });
                request.stills = 21;
                return;
            }
            let turn =
                Quat::from_rotation_y(-f32::from(rotation % 6) * std::f32::consts::TAU / 6.0);
            let origin = Vec3::from_array(observed_hex::hex_origin(anchor));
            let (offset, focus, name) = match slot {
                0 => (
                    Vec3::new(-4.2, 0.5, -7.2),
                    Vec3::new(6.0, 1.0, 6.0),
                    "cistern-approach",
                ),
                1 => (
                    Vec3::new(-3.0, 0.75, -4.4),
                    Vec3::new(10.0, 1.0, 5.0),
                    "cistern-entry",
                ),
                2 => (
                    Vec3::new(5.2, 0.5, 2.9),
                    Vec3::new(11.8, 0.7, 5.3),
                    "cistern-water",
                ),
                _ => (
                    Vec3::new(10.0, 0.75, 0.0),
                    Vec3::new(2.0, 2.4, 4.8),
                    "cistern-colonnade",
                ),
            };
            let (feet, target, name) = if factory {
                let cells = runtime
                    .ascent
                    .as_ref()
                    .expect("ascent capture")
                    .rules()
                    .played_wonder(CardKind::Chargeworks, anchor, rotation)
                    .map(|p| p.coord);
                let (sector, offset, focus, name) = match slot {
                    0 => (
                        0,
                        Vec3::new(-3.0, 0.75, -4.4),
                        Vec3::new(8.0, 2.2, 4.0),
                        "chargeworks-arrival",
                    ),
                    1 => (
                        0,
                        Vec3::new(1.0, 0.75, -3.2),
                        Vec3::new(-4.3, 3.5, 0.0),
                        "chargeworks-fabricator",
                    ),
                    2 => (
                        2,
                        Vec3::new(2.0, 0.75, 2.0),
                        Vec3::new(-5.2, 3.7, 0.0),
                        "chargeworks-receiver",
                    ),
                    _ => (
                        1,
                        Vec3::new(-1.8, 3.0, 2.5),
                        Vec3::new(2.0, 1.2, -2.0),
                        "chargeworks-gantry",
                    ),
                };
                let at = Vec3::from_array(observed_hex::hex_origin(cells[sector]));
                let h = Quat::from_rotation_y(
                    -f32::from((rotation + (sector as u8) * 2) % 6) * std::f32::consts::TAU / 6.0,
                );
                (at + h * offset, at + h * focus, name)
            } else {
                (origin + turn * offset, origin + turn * focus, name)
            };
            let camera = feet + Vec3::Y * 1.7;
            let look = (target - camera).normalize();
            let cell = runtime
                .match_state
                .facility
                .config
                .grid()
                .neighbor(
                    anchor,
                    observed_hex::HexFace::LATERAL
                        [usize::from((rotation + if slot == 0 { 4 } else { 0 }) % 6)],
                )
                .filter(|_| slot == 0 || slot == 3)
                .unwrap_or(anchor);
            let cell = if factory {
                let cells = runtime
                    .ascent
                    .as_ref()
                    .expect("ascent capture")
                    .rules()
                    .played_wonder(CardKind::Chargeworks, anchor, rotation)
                    .map(|p| p.coord);
                cells[if slot == 2 {
                    2
                } else if slot == 3 {
                    1
                } else {
                    0
                }]
            } else {
                cell
            };
            if let Some(player) = runtime.match_state.players.get_mut(&eye) {
                player.cell = cell;
                player.position = feet + Vec3::Y * 0.9;
                player.yaw = look.x.atan2(-look.z);
                player.pitch = look.y.asin();
            }
            if elapsed % 180 == 160 {
                shoot(commands, name);
            }
        }
        21 => {
            let (Some(walk), Some(eye)) = (walk.as_mut(), desk.eyes) else {
                exit.write(AppExit::error());
                return;
            };
            if walk.frame >= 1_200 {
                error!(
                    "Wonder controller walkthrough stalled at {:?}, waypoint {} toward {:?}",
                    walk.body.position,
                    walk.waypoint,
                    walk.route.get(walk.waypoint)
                );
                exit.write(AppExit::error());
                return;
            }
            let config = observed_traversal::FpsConfig::default();
            for _ in 0..2 {
                let feet = walk.body.position - Vec3::Y * config.half_height;
                let Some(target) = walk.route.get(walk.waypoint) else {
                    info!(
                        "Wonder controller walkthrough complete: {} frames",
                        walk.frame
                    );
                    exit.write(AppExit::Success);
                    return;
                };
                let toward = (*target - feet).with_y(0.0);
                if toward.length() < 0.25 && (feet.y - target.y).abs() < 0.35 {
                    walk.waypoint += 1;
                    continue;
                }
                walk.body.yaw = toward.x.atan2(-toward.z);
                observed_traversal::rapier_controller::step_character(
                    &walk.scene,
                    &mut walk.body,
                    player_input::PlayerIntent {
                        movement: Vec2::Y,
                        ..default()
                    },
                    &config,
                    1.0 / 60.0,
                );
            }
            if let Some(player) = runtime.match_state.players.get_mut(&eye) {
                player.position = walk.body.position;
                player.yaw = walk.body.yaw;
                player.pitch = -0.1;
                player.cell =
                    *walk
                        .cells
                        .iter()
                        .min_by(|a, b| {
                            walk.body
                                .position
                                .distance_squared(Vec3::from_array(observed_hex::hex_origin(**a)))
                                .total_cmp(&walk.body.position.distance_squared(Vec3::from_array(
                                    observed_hex::hex_origin(**b),
                                )))
                        })
                        .expect("three reservoir cells");
            }
            let name = format!(
                "{}-walk-{:03}.png",
                if factory { "chargeworks" } else { "cistern" },
                walk.frame
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.join(name)));
            walk.frame += 1;
        }
        _ => {}
    }
}

/// Evidence poses begin inside a real, buildable Reactor hall to discover it,
/// then move into its existing neighbour. No knowledge or geometry is injected.
pub(in crate::hex_wfc) struct FactoryStart {
    departure: observed_hex::HexCoord,
    release_tick: u64,
    departed: bool,
}
impl FactoryStart {
    pub(super) fn depart(&mut self, runtime: &mut HexWfcRuntime) {
        if self.departed || runtime.match_state.tick < self.release_tick {
            return;
        }
        place_bodies(runtime, self.departure);
        self.departed = true;
    }
}
fn place_bodies(runtime: &mut HexWfcRuntime, cell: observed_hex::HexCoord) {
    let at = Vec3::from_array(observed_hex::hex_origin(cell)) + Vec3::Y * 1.4;
    for (i, player) in runtime.match_state.players.values_mut().enumerate() {
        player.cell = cell;
        player.position = at;
        player.yaw = (i as f32) * 1.5;
        player.pitch = 0.0;
    }
}
pub(super) fn stage_reactor(
    runtime: &mut HexWfcRuntime,
    desk: &ArchitectDesk,
) -> Option<FactoryStart> {
    use observed_facility::hex_wfc::HexArchetype;
    use observed_hex::HexFace;
    let ascent = runtime.ascent.as_mut()?;
    let card = ascent.stage_card(desk.seat, CardKind::Chargeworks)?;
    let mut probe = ascent.rules().clone();
    probe.deck = desk.hand(ascent.session())?.deck.clone();
    probe.known = probe.world.placements.keys().copied().collect();
    probe.cooldown = 0;
    let grid = probe.world.config.grid();
    let site = probe.world.placements.iter().find_map(|(&cell, p)| {
        if !matches!(
            p.archetype,
            HexArchetype::Straight | HexArchetype::Corner | HexArchetype::Junction
        ) {
            return None;
        }
        for rotation in 0..6 {
            if probe
                .refusal(ArchitectCommand::Play {
                    card,
                    target: cell,
                    rotation,
                })
                .is_some()
            {
                continue;
            }
            let neighbour = HexFace::LATERAL.into_iter().find_map(|face| {
                let next = grid.neighbor(cell, face)?;
                let other = probe.world.placements.get(&next)?;
                (p.is_open(face)
                    && other.is_open(face.opposite())
                    && other.space.built()
                    && !matches!(other.archetype, HexArchetype::Climb { .. }))
                .then_some(next)
            });
            if let Some(next) = neighbour {
                return Some((cell, next));
            }
        }
        None
    })?;
    place_bodies(runtime, site.0);
    info!(
        "Chargeworks evidence: physical discovery begins at Reactor {:?}",
        site.0
    );
    Some(FactoryStart {
        departure: site.1,
        release_tick: runtime.match_state.tick + 120,
        departed: false,
    })
}
