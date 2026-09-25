//! `OBSERVED2_CAPTURE_HEX_WFC_VISTA=<dir>`: stand in the production facility's open
//! edges and look out.
//!
//! Evidence only. The runner is placed, not walked (the spectator bot that admits a
//! capture into the facility steps aside once it is inside): the point is what an open edge
//! shows, and a bot would spend minutes reaching one. Each pose is an open hall of the
//! solved facility — a railed loggia, a bare one above the unsafe height, a walkway if
//! the solve made one, and the highest open edge of all — each chosen for the deepest
//! drop beyond its open face, and facing through it, and each is held long enough for streaming and the atmosphere to settle
//! before the still is taken. The last two are inside: a sealed room, the evidence that
//! the moonlight stays out of it, and a room whose window faces the moon.
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_facility::hex_wfc::{HexCoord, HexFace, HexWfcWorld};
use observed_hex::{FLOOR_SLAB_TOP, face_edge, hex_origin};
use observed_match::hex_wfc::{OpenEdges, RAILED_BELOW_LEVEL, open_edges};

use super::sim::HexWfcRuntime;

/// Frames before the first pose: the facility enters and the first cells stream in.
const WARM_UP: u16 = 150;
/// Frames each pose is held before its still, then after it.
const SETTLE: u16 = 150;
const AFTER: u16 = 20;

/// One place to stand and look out from.
#[derive(Clone, Debug)]
pub(super) struct VistaPose {
    pub(super) name: &'static str,
    pub(super) cell: HexCoord,
    pub(super) feet: Vec3,
    pub(super) yaw: f32,
    pub(super) pitch: f32,
    pub(super) stage: Stage,
}

/// What the equipment capture puts in the runner's hands and on the floor for a pose,
/// held there every frame of it. Vista poses stage nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage {
    Nothing,
    /// A lantern and two plates in hand.
    InHand,
    /// And a linked pair of the runner's plates on the floor ahead.
    LinkedPads,
    /// And one of the runner's plates alone, beside a rival team's.
    LoneAndRival,
    /// And a lantern cache standing in the middle of the cell, with a linked pair.
    SetDown,
    /// The major Guardian stood `metres` ahead, where the runner is looking at it.
    Guardian {
        metres: u8,
    },
}

fn face_dir(face: HexFace) -> Vec2 {
    let [a, b] = face_edge(face);
    #[allow(clippy::cast_precision_loss)]
    Vec2::new((a.0 + b.0) as f32, (a.1 + b.1) as f32).normalize()
}

/// Standing `back` metres from an open face, looking through it.
fn pose(name: &'static str, cell: HexCoord, face: HexFace, back: f32, pitch: f32) -> VistaPose {
    let dir = face_dir(face);
    let o = Vec3::from_array(hex_origin(cell));
    // `back` is measured from the open face: near the lip, the drop is in the frame.
    let from_centre = 6.9 - back;
    VistaPose {
        name,
        cell,
        feet: o + Vec3::new(dir.x * from_centre, FLOOR_SLAB_TOP, dir.y * from_centre),
        yaw: dir.x.atan2(-dir.y),
        pitch,
        stage: Stage::Nothing,
    }
}

/// `OBSERVED2_CAPTURE_HEX_WFC_EQUIPMENT`: the carried and placed equipment, staged in
/// the vista's railed loggia under the moon and in its moonlit room.
#[must_use]
pub(super) fn equipment_poses(world: &HexWfcWorld) -> Vec<VistaPose> {
    let vistas = poses(world);
    let find = |name: &str| vistas.iter().find(|pose| pose.name == name).cloned();
    let mut out = Vec::new();
    if let Some(loggia) = find("railed_loggia") {
        let staged = |name, stage, back: f32, pitch: f32, inward: bool| {
            let dir = Vec2::new(loggia.yaw.sin(), -loggia.yaw.cos());
            let o = Vec3::from_array(hex_origin(loggia.cell));
            let from_centre = 6.9 - back;
            VistaPose {
                name,
                cell: loggia.cell,
                feet: o + Vec3::new(dir.x * from_centre, FLOOR_SLAB_TOP, dir.y * from_centre),
                yaw: if inward {
                    loggia.yaw + std::f32::consts::PI
                } else {
                    loggia.yaw
                },
                pitch,
                stage,
            }
        };
        out.push(staged("in_hand", Stage::InHand, 1.4, -0.08, false));
        out.push(staged("linked_pads", Stage::LinkedPads, 5.2, -0.42, false));
        out.push(staged(
            "lone_and_rival",
            Stage::LoneAndRival,
            5.2,
            -0.42,
            false,
        ));
        out.push(staged("set_down", Stage::SetDown, 1.0, -0.3, true));
    }
    if let Some(room) = find("moonlit_room") {
        out.push(VistaPose {
            name: "in_hand_indoors",
            stage: Stage::InHand,
            ..room
        });
    }
    out
}

/// `OBSERVED2_CAPTURE_HEX_WFC_GUARDIAN`: the major Guardian in the facility, frozen by
/// the runner looking at it, in the moonlit loggia and in the windowed room.
#[must_use]
pub(super) fn guardian_poses(world: &HexWfcWorld) -> Vec<VistaPose> {
    let vistas = poses(world);
    let find = |name: &str| vistas.iter().find(|pose| pose.name == name).cloned();
    let mut out = Vec::new();
    if let Some(loggia) = find("railed_loggia") {
        let dir = Vec2::new(loggia.yaw.sin(), -loggia.yaw.cos());
        let o = Vec3::from_array(hex_origin(loggia.cell));
        out.push(VistaPose {
            name: "guardian_loggia",
            feet: o + Vec3::new(dir.x * 5.9, FLOOR_SLAB_TOP, dir.y * 5.9),
            yaw: loggia.yaw + std::f32::consts::PI,
            pitch: 0.12,
            stage: Stage::Guardian { metres: 7 },
            ..loggia
        });
    }
    if let Some(room) = find("moonlit_room") {
        out.push(VistaPose {
            name: "guardian_room",
            pitch: 0.05,
            stage: Stage::Guardian { metres: 5 },
            ..room
        });
    }
    out
}

/// Hold `pose`'s staging in the simulation: what the runner carries, and which plates
/// and lantern caches lie where. Evidence only, like the pose itself.
fn stage(runtime: &mut HexWfcRuntime, pose: &VistaPose) {
    use observed_core::{EquipmentId, TeamId};
    use observed_match::hex_wfc::HexDeployedPad;
    if pose.stage == Stage::Nothing {
        return;
    }
    if let Stage::Guardian { metres } = pose.stage {
        // Where the runner is looking: the simulation freezes it there itself.
        let ahead = Vec3::new(pose.yaw.sin(), 0.0, -pose.yaw.cos());
        let guardian = &mut runtime.match_state.guardian;
        guardian.cell = pose.cell;
        guardian.position = pose.feet + ahead * f32::from(metres) + Vec3::Y * 0.4;
        return;
    }
    let id = runtime.local_player;
    let state = &mut runtime.match_state;
    let team = state.players[&id].team;
    state.pads.carried.insert(id, 2);
    state.lanterns.carried.insert(id, 1);
    state.pads.deployed.clear();
    state.lanterns.caches.clear();
    let ahead = Vec3::new(pose.yaw.sin(), 0.0, -pose.yaw.cos());
    let right = Vec3::new(-ahead.z, 0.0, ahead.x);
    let mut pad = |n: u32, team: TeamId, at: Vec3| {
        state.pads.deployed.insert(
            EquipmentId(9_000 + n),
            HexDeployedPad {
                id: EquipmentId(9_000 + n),
                owner: id,
                team,
                cell: pose.cell,
                position: at,
            },
        );
    };
    let floor = pose.feet;
    match pose.stage {
        Stage::Nothing | Stage::InHand | Stage::Guardian { .. } => {}
        Stage::LinkedPads => {
            pad(0, team, floor + ahead * 2.6 - right * 1.0);
            pad(1, team, floor + ahead * 3.4 + right * 1.2);
        }
        Stage::LoneAndRival => {
            pad(0, team, floor + ahead * 2.6 - right * 1.0);
            pad(
                1,
                TeamId(team.0.wrapping_add(1)),
                floor + ahead * 3.4 + right * 1.2,
            );
        }
        Stage::SetDown => {
            let centre = Vec3::from_array(hex_origin(pose.cell)) + Vec3::Y * FLOOR_SLAB_TOP;
            pad(0, team, centre - right * 2.2 - ahead * 1.2);
            pad(1, team, centre + right * 2.0 + ahead * 1.0);
            state.lanterns.caches.insert(
                EquipmentId(9_100),
                observed_match::hex_wfc::HexLanternCache {
                    id: EquipmentId(9_100),
                    cell: pose.cell,
                    amount: 1,
                    collected: false,
                },
            );
        }
    }
}

/// How far a body stepping through `face` would fall: open cells straight down from
/// the neighbour beyond it, and a whole lattice's worth more if nothing ever catches it.
fn drop_beyond(world: &HexWfcWorld, at: HexCoord, face: HexFace) -> u32 {
    let grid = world.config.grid();
    let open = |cell: HexCoord| {
        world
            .placements
            .get(&cell)
            .is_none_or(|p| p.space.unbuilt())
    };
    let Some(mut cell) = grid.neighbor(at, face) else {
        // Off the edge of the lattice: straight down to true void.
        return u32::from(at.level) + 1 + u32::from(world.config.levels);
    };
    let mut levels = 0;
    while open(cell) {
        levels += 1;
        match grid.neighbor(cell, HexFace::Down) {
            Some(below) => cell = below,
            None => return levels + u32::from(world.config.levels),
        }
    }
    levels
}

/// The open face with the deepest drop beyond it. Most of the facility is built, so
/// most open edges overlook the roof of the storey below; a vista is where they don't.
///
/// `inside` keeps to faces with a neighbour on the lattice: a drop between two towers
/// rather than off the rim, so there is building on the far side of it.
fn open_face(
    world: &HexWfcWorld,
    at: HexCoord,
    open: &OpenEdges,
    inside: bool,
) -> Option<(HexFace, u32)> {
    let grid = world.config.grid();
    HexFace::LATERAL
        .into_iter()
        .filter(|&face| open.opens(face) && (!inside || grid.neighbor(at, face).is_some()))
        .map(|face| (face, drop_beyond(world, at, face)))
        .max_by_key(|&(face, drop)| (drop, std::cmp::Reverse(face)))
}

/// The open face that looks most toward the middle of the facility.
fn inward_face(world: &HexWfcWorld, at: HexCoord, open: &OpenEdges) -> Option<HexFace> {
    let far = hex_origin(HexCoord {
        q: world.config.cols - 1,
        r: world.config.rows - 1,
        level: 0,
    });
    let here = hex_origin(at);
    let inward = Vec2::new(far[0] * 0.5 - here[0], far[2] * 0.5 - here[2]).normalize_or_zero();
    HexFace::LATERAL
        .into_iter()
        .filter(|&face| open.opens(face))
        .max_by(|&a, &b| face_dir(a).dot(inward).total_cmp(&face_dir(b).dot(inward)))
}

/// The poses a solved facility offers, in shooting order. Deterministic: the first
/// qualifying cell in coordinate order, preferring the most open faces.
#[must_use]
pub(super) fn poses(world: &HexWfcWorld) -> Vec<VistaPose> {
    let open: Vec<(HexCoord, OpenEdges)> = world
        .placements
        .keys()
        .filter_map(|&at| open_edges(world, at).map(|edges| (at, edges)))
        .collect();
    // For each kind of pose, the open edge with the deepest drop, then the most open.
    let best = |filter: &dyn Fn(&HexCoord, &OpenEdges) -> bool, inside: bool| {
        open.iter()
            .filter(|(at, edges)| edges.span.is_none() && filter(at, edges))
            .filter_map(|&(at, edges)| {
                open_face(world, at, &edges, inside).map(|(face, drop)| (at, edges, face, drop))
            })
            .max_by_key(|&(at, edges, _, drop)| {
                (drop, edges.faces.count_ones(), std::cmp::Reverse(at))
            })
    };
    let mut poses = Vec::new();
    let top = world.config.levels.saturating_sub(1);
    // Off the rim, over the cloud sea.
    if let Some((at, _, face, _)) =
        best(&|at, _| (2..RAILED_BELOW_LEVEL).contains(&at.level), false)
    {
        poses.push(pose("railed_loggia", at, face, 1.4, -0.3));
    }
    // Between towers: the deepest drop with building on the far side of it.
    if let Some((at, _, face, _)) =
        best(&|at, _| (RAILED_BELOW_LEVEL..top).contains(&at.level), true)
    {
        poses.push(pose("bare_edge", at, face, 1.2, -0.4));
    }
    if let Some(&(at, edges)) = open.iter().find(|(_, edges)| edges.span.is_some())
        && let Some(axis) = edges.span
    {
        poses.push(pose("walkway", at, axis, 9.0, -0.18));
    }
    // Toward the moon: the high open face that looks most straight at it, eyes raised.
    let moon = observed_style::open_air::toward_moon();
    let moon_plan = Vec2::new(moon[0], moon[2]).normalize_or_zero();
    if let Some((at, face)) = open
        .iter()
        .filter(|(at, edges)| at.level + 2 >= top && edges.span.is_none())
        .flat_map(|&(at, edges)| {
            HexFace::LATERAL
                .into_iter()
                .filter(move |&face| edges.opens(face))
                .map(move |face| (at, face))
        })
        .max_by(|a, b| {
            face_dir(a.1)
                .dot(moon_plan)
                .total_cmp(&face_dir(b.1).dot(moon_plan))
                .then(b.0.cmp(&a.0))
        })
    {
        poses.push(pose("moon", at, face, 1.5, 0.28));
    }
    // From the top, across the building: the open edge nearest the middle.
    let far = hex_origin(HexCoord {
        q: world.config.cols - 1,
        r: world.config.rows - 1,
        level: 0,
    });
    let middle = Vec2::new(far[0] * 0.5, far[2] * 0.5);
    let off_centre = |at: &HexCoord| {
        let o = hex_origin(*at);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let metres = (Vec2::new(o[0], o[2]) - middle).length() as u32;
        metres
    };
    if let Some(&(at, edges)) = open
        .iter()
        .filter(|(at, edges)| at.level == top && edges.span.is_none())
        .min_by_key(|(at, _)| (off_centre(at), *at))
        && let Some(face) = inward_face(world, at, &edges)
    {
        poses.push(pose("summit", at, face, 3.0, -0.3));
    }
    // Inside, sealed: a room with building on every side and above, facing the moon.
    // The moonlight must not reach it; the still is the evidence that walls keep it out.
    let grid = world.config.grid();
    let built = |cell: Option<HexCoord>| {
        cell.and_then(|cell| world.placements.get(&cell))
            .is_some_and(|placement| placement.space.built())
    };
    if let Some(at) = world
        .placements
        .values()
        .filter(|placement| placement.space == observed_facility::hex_wfc::HexSpace::Room)
        .map(|placement| placement.coord)
        .filter(|&at| at.level >= 2 && open_edges(world, at).is_none())
        .find(|&at| {
            built(grid.neighbor(at, HexFace::Up))
                && HexFace::LATERAL
                    .into_iter()
                    .all(|face| built(grid.neighbor(at, face)))
        })
    {
        let face = HexFace::LATERAL
            .into_iter()
            .max_by(|&a, &b| {
                face_dir(a)
                    .dot(moon_plan)
                    .total_cmp(&face_dir(b).dot(moon_plan))
            })
            .unwrap_or(HexFace::LATERAL[0]);
        poses.push(pose("sealed_room", at, face, 6.9, 0.1));
    }
    // Inside, windowed: the room wall that looks out most toward the moon, seen from
    // across the room, with the floor in frame where the moonlight lands.
    let looks_out = |room: &[HexCoord], at: HexCoord, face: HexFace| {
        grid.neighbor(at, face)
            .is_none_or(|next| !room.contains(&next) && !built(Some(next)))
    };
    if let Some((at, face)) = world
        .blueprints
        .iter()
        .flat_map(|room| {
            room.cells.iter().flat_map(move |&at| {
                HexFace::LATERAL
                    .into_iter()
                    .filter(move |&face| looks_out(&room.cells, at, face))
                    .map(move |face| (at, face))
            })
        })
        .max_by(|a, b| {
            face_dir(a.1)
                .dot(moon_plan)
                .total_cmp(&face_dir(b.1).dot(moon_plan))
                .then(a.0.level.cmp(&b.0.level))
                .then(b.0.cmp(&a.0))
        })
    {
        poses.push(pose("moonlit_room", at, face, 6.0, -0.22));
    }
    poses
}

/// Place the runner at each pose in turn and take its still.
pub(super) fn progress(
    frame: u16,
    path: &str,
    runtime: Option<&mut HexWfcRuntime>,
    poses: &mut Option<Vec<VistaPose>>,
    which: fn(&HexWfcWorld) -> Vec<VistaPose>,
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if poses.is_none() {
        // The spectator bot is what lets a capture enter the facility directly; once
        // inside, the runner is placed rather than driven, so the bot steps aside.
        commands.remove_resource::<crate::sim::state::SpectatorBot>();
    }
    let poses = poses.get_or_insert_with(|| poses_for(runtime, which));
    if frame < WARM_UP {
        return;
    }
    let slot = (frame - WARM_UP) / (SETTLE + AFTER);
    let within = (frame - WARM_UP) % (SETTLE + AFTER);
    let Some(pose) = poses.get(usize::from(slot)) else {
        exit.write(AppExit::Success);
        return;
    };
    let id = runtime.local_player;
    // Held every frame of the pose, so nothing walks or falls away from it.
    if within < SETTLE {
        stage(runtime, pose);
    }
    if within < SETTLE
        && let Some(player) = runtime.match_state.players.get_mut(&id)
    {
        player.cell = pose.cell;
        player.position = pose.feet + Vec3::Y * 0.9;
        player.yaw = pose.yaw;
        player.pitch = pose.pitch;
    }
    if within == SETTLE - 1 {
        let file =
            std::path::Path::new(path).join(format!("vista_{:02}_{}.png", slot + 1, pose.name));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(file));
    }
}

fn poses_for(runtime: &HexWfcRuntime, which: fn(&HexWfcWorld) -> Vec<VistaPose>) -> Vec<VistaPose> {
    let found = which(&runtime.match_state.facility);
    for pose in &found {
        println!(
            "hex vista capture: {} at q{} r{} L{}",
            pose.name, pose.cell.q, pose.cell.r, pose.cell.level
        );
    }
    found
}
