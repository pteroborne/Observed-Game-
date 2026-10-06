//! The kinetic tool in the Observer's hand, in Architect Ascent.
//!
//! Every body in an Ascent match carries the Lance (`observed_tool`) in its main hand,
//! where the race holds the observation torch; the torch moves to the off hand. Left
//! click (the controller's right trigger) pushes the minor in the crosshair away, right
//! click (right stick press) pulls it back. The simulation is `hex_wfc::kinetic`; the
//! Ascent rules own the charge each shot spends.
//!
//! What this draws and plays:
//!
//! - The Lance, posed for what it last did: the prongs open on a push and close on a
//!   pull, and the signal burns with the local body's charge. Its gimbal hangs the plumb
//!   bob straight down, true gravity, until the plumb is armed, and then along the armed
//!   direction, turning with the body.
//! - A small reticle at the centre of the view, lit in the push colour when a minor is in
//!   reach and dim otherwise, amber once the pool cannot pay for a shot.
//! - The lab's kinetic sounds: push and pull at the tool, an empty click when the pool is
//!   dry, and the void when a minor the local body shoved goes over.
//! - Notices: the tool is empty, and a minor was sent into the void.
//!
//! The plumb, as `wfc_kinetic_lab` proved it: the arm key (Q) arms it along the look, and
//! held, the mouse dials it round the way the body faces instead of turning the view; the
//! plumb key (G, or the middle button) fires it at the minor in the crosshair, whose down
//! becomes the armed direction. The armed direction is the local player's own until they
//! fire: only the aim travels, with the shot. The gimbal on the Lance shows it.
//!
//! Presentation only: it reads the match and the rules and writes nothing back.

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use observed_core::PlayerId;
use observed_match::ascent::economy::{KINETIC_SHOT_COST, MAX_CHARGE};
use observed_match::hex_wfc::{HexMatchEventKind, HexPlayerState};
use observed_style::equipment::{Hardware, finish};
use observed_style::kinetic::{Role, treatment};
use observed_tool::{Beat, Design, Finish, ToolState};

use super::equipment::{Hand, HeldSway, held_transform, sway_for};
use super::hud::play::HudNotice;
use super::hud::words::Tone;
use super::overlay::MatchOverlayState;
use super::sim::HexWfcRuntime;
use crate::GameState;
use crate::view::theme::{DIM, WARNING};

pub(super) mod capture;
mod plumb;

pub(in crate::hex_wfc) use plumb::{ArmedPlumb, arm_and_fire};

/// The design every Observer carries.
const HELD: Design = Design::Lance;

/// The Lance rides in the main hand, where the race holds the torch, level with the view
/// so its muzzle runs along the crosshair. `kinetic_tool_lab` judged it at this size; it
/// sits four centimetres further back than the torch, so its prongs stay inside the body's
/// radius and never push through a wall the body stands against.
pub(in crate::hex_wfc) const HAND: Hand = Hand {
    offset: Vec3::new(0.13, -0.095, -0.16),
    roll: 0.16,
    tip: 0.0,
    scale: 0.40,
};

/// The corners of a box around the Lance in its own frame, for
/// `held_devices_stay_inside_the_body`.
#[cfg(test)]
pub(in crate::hex_wfc) const REACH: [Vec3; 2] = [
    Vec3::new(-0.066, -0.141, -0.341),
    Vec3::new(0.066, 0.136, 0.131),
];

/// Seconds a minor going over after a local shove is credited to it; after a plumb, these
/// count from when the plumb lets go.
const CREDIT_SECONDS: f32 = 3.0;

/// Whether the match plays Ascent, which is when bodies carry the tool.
pub(super) fn carried(runtime: &HexWfcRuntime) -> bool {
    runtime.ascent.is_some()
}

/// The local body's charge, 0 to [`MAX_CHARGE`], if it has an Observer.
fn charge(runtime: &HexWfcRuntime, player: PlayerId) -> Option<u32> {
    let ascent = runtime.ascent.as_ref()?;
    let observer = ascent.observer_for(player)?;
    Some(ascent.rules().economy.charge(observer))
}

/// What the tool presentation remembers between frames.
#[derive(Resource, Default)]
pub(super) struct KineticPresentation {
    /// What each body's tool last did, and when (app seconds).
    beats: BTreeMap<PlayerId, (Beat, f32)>,
    /// The last tick whose events were read.
    tick: u64,
    /// Until when (app seconds) a minor lost is credited to the local body's last shot:
    /// a shove's few seconds, or a plumb's hold and a few seconds after it lets go.
    credited_until: Option<f32>,
    /// When the reticle last flashed empty (app seconds).
    empty_at: Option<f32>,
    /// Which bodies' tools are spawned.
    held: Vec<PlayerId>,
}

/// The tool's finishes and sounds.
#[derive(Resource)]
pub(super) struct KineticAssets {
    meshes: Vec<Handle<Mesh>>,
    body: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    grip: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    /// The local body's signal, lit by its charge.
    signal: Handle<StandardMaterial>,
    /// Every other body's signal, dim.
    other_signal: Handle<StandardMaterial>,
    push: Handle<AudioSource>,
    pull: Handle<AudioSource>,
    empty: Handle<AudioSource>,
    void: Handle<AudioSource>,
}

impl KineticAssets {
    fn of(&self, finish: Finish, local: bool) -> Handle<StandardMaterial> {
        match finish {
            Finish::Body => self.body.clone(),
            Finish::Trim => self.trim.clone(),
            Finish::Grip | Finish::Pupil => self.grip.clone(),
            Finish::Glass | Finish::Eye => self.glass.clone(),
            Finish::Signal if local => self.signal.clone(),
            Finish::Signal => self.other_signal.clone(),
        }
    }
}

/// A body's held tool.
#[derive(Component)]
pub(super) struct HeldTool(PlayerId);

#[derive(Component)]
pub(super) struct ToolPart(usize);

/// The reticle at the centre of the view.
#[derive(Component)]
pub(super) struct Reticle;

pub(super) fn setup(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !carried(&runtime) {
        return;
    }
    let mut hardware = |part: Hardware| {
        let f = finish(part);
        materials.add(StandardMaterial {
            base_color: f.base_color,
            metallic: f.metallic,
            perceptual_roughness: f.roughness,
            alpha_mode: if part == Hardware::Glass {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            ..default()
        })
    };
    let (body, trim, grip, glass) = (
        hardware(Hardware::Body),
        hardware(Hardware::Trim),
        hardware(Hardware::Grip),
        hardware(Hardware::Glass),
    );
    let dark = |materials: &mut Assets<StandardMaterial>, glow: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.02, 0.03),
            emissive: LinearRgba::from(treatment(Role::Push).base_color) * glow,
            ..default()
        })
    };
    let signal = dark(&mut materials, 2.0);
    let other_signal = dark(&mut materials, 1.2);
    commands.insert_resource(KineticAssets {
        meshes: observed_tool::parts(HELD)
            .into_iter()
            .map(|part| meshes.add(observed_guardian::mesh::mesh(part.shape)))
            .collect(),
        body,
        trim,
        grip,
        glass,
        signal,
        other_signal,
        push: server.load("sounds/kinetic/push.ogg"),
        pull: server.load("sounds/kinetic/pull.ogg"),
        empty: server.load("sounds/kinetic/empty.ogg"),
        void: server.load("sounds/kinetic/void.ogg"),
    });
    commands.insert_resource(KineticPresentation::default());
    commands.insert_resource(ArmedPlumb::default());
    commands.spawn((
        Reticle,
        DespawnOnExit(GameState::HexWfc),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(10),
            height: px(10),
            margin: UiRect::new(px(-5), px(0), px(-5), px(0)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(DIM.with_alpha(0.5)),
        Visibility::Hidden,
    ));
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<KineticAssets>();
    commands.remove_resource::<KineticPresentation>();
    commands.remove_resource::<ArmedPlumb>();
}

/// Spawn a tool for every body walking the facility, and take away the tools of those
/// that are not.
pub(super) fn sync_held(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    assets: Option<Res<KineticAssets>>,
    presentation: Option<ResMut<KineticPresentation>>,
    tools: Query<(Entity, &HeldTool)>,
) {
    let (Some(assets), Some(mut presentation)) = (assets, presentation) else {
        return;
    };
    let holding: Vec<PlayerId> = runtime
        .match_state
        .players
        .values()
        .filter(|player| player.in_facility())
        .map(|player| player.id)
        .collect();
    if presentation.held == holding {
        return;
    }
    for (entity, tool) in &tools {
        if !holding.contains(&tool.0) {
            commands.entity(entity).despawn();
        }
    }
    for &player in holding.iter().filter(|p| !presentation.held.contains(p)) {
        let local = player == runtime.local_player;
        commands
            .spawn((
                HeldTool(player),
                DespawnOnExit(GameState::HexWfc),
                Transform::default(),
                Visibility::default(),
                Name::new("Kinetic tool"),
            ))
            .with_children(|tool| {
                for (index, part) in observed_tool::parts(HELD).into_iter().enumerate() {
                    tool.spawn((
                        ToolPart(index),
                        Mesh3d(assets.meshes[index].clone()),
                        MeshMaterial3d(assets.of(part.finish, local)),
                        Transform::IDENTITY,
                    ));
                }
            });
    }
    presentation.held = holding;
}

/// Read this tick's shots: which tool fired, the sounds, and the notices.
pub(super) fn read_shots(
    mut commands: Commands,
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    settings: Res<crate::settings::Settings>,
    assets: Option<Res<KineticAssets>>,
    presentation: Option<ResMut<KineticPresentation>>,
    mut notice: ResMut<HudNotice>,
) {
    let (Some(assets), Some(mut presentation)) = (assets, presentation) else {
        return;
    };
    let game = &runtime.match_state;
    if presentation.tick == game.tick {
        return;
    }
    presentation.tick = game.tick;
    let now = time.elapsed_secs();
    let volume = settings.effective_sfx_volume();
    for event in &game.recent_events {
        let beat = match event.kind {
            HexMatchEventKind::KineticPush => Beat::Push,
            HexMatchEventKind::KineticPull => Beat::Pull,
            HexMatchEventKind::KineticPlumb => Beat::Lash,
            HexMatchEventKind::GuardianLost => {
                // A minor the local body shoved a moment ago has gone over.
                if presentation.credited_until.is_some_and(|until| now < until) {
                    presentation.credited_until = None;
                    notice.show("Minor sent into the void", Tone::Good, now.into());
                    super::audio::play(
                        &mut commands,
                        assets.void.clone(),
                        0.7 * volume,
                        "Kinetic void",
                        None,
                    );
                }
                continue;
            }
            _ => continue,
        };
        let Some(player) = event.player else {
            continue;
        };
        presentation.beats.insert(player, (beat, now));
        let local = player == runtime.local_player;
        if local {
            let hold = if beat == Beat::Lash {
                observed_match::hex_wfc::PLUMB_TICKS as f32 / 60.0
            } else {
                0.0
            };
            presentation.credited_until = Some(now + hold + CREDIT_SECONDS);
            let connected = match beat {
                Beat::Push => "Minor pushed.",
                Beat::Pull => "Minor pulled.",
                Beat::Lash => "Minor reoriented.",
                _ => "Kinetic tool connected.",
            };
            notice.show(connected, Tone::Good, time.elapsed_secs_f64());
        }
        let sound = if beat == Beat::Pull {
            assets.pull.clone()
        } else {
            assets.push.clone()
        };
        // The local body's own shot is in its hand; anyone else's is where they stand.
        let at = (!local)
            .then(|| game.players.get(&player).map(|p| p.position))
            .flatten();
        super::audio::play(&mut commands, sound, 0.55 * volume, "Kinetic shot", at);
    }
}

type HeldTools<'w, 's> = Query<
    'w,
    's,
    (&'static HeldTool, &'static mut Transform, &'static Children),
    Without<ToolPart>,
>;
type ToolParts<'w, 's> =
    Query<'w, 's, (&'static ToolPart, &'static mut Transform), Without<HeldTool>>;

#[derive(SystemParam)]
pub(super) struct KineticPoseContext<'w, 's> {
    runtime: Res<'w, HexWfcRuntime>,
    time: Res<'w, Time>,
    sway: Res<'w, HeldSway>,
    assets: Option<Res<'w, KineticAssets>>,
    presentation: Option<Res<'w, KineticPresentation>>,
    armed: Option<Res<'w, ArmedPlumb>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    tools: HeldTools<'w, 's>,
    parts: ToolParts<'w, 's>,
}

/// Pose every held tool for what it last did, and light the local body's signal with
/// its charge.
pub(super) fn pose_held(context: KineticPoseContext) {
    let KineticPoseContext {
        runtime,
        time,
        sway,
        assets,
        presentation,
        armed,
        mut materials,
        mut tools,
        mut parts,
    } = context;
    let (Some(assets), Some(presentation)) = (assets, presentation) else {
        return;
    };
    let now = time.elapsed_secs();
    let mut local_signal = None;
    for (tool, mut transform, children) in &mut tools {
        let Some(player) = runtime.match_state.players.get(&tool.0) else {
            continue;
        };
        *transform = held_pose(&runtime, &sway, player);
        let (beat, since) = presentation
            .beats
            .get(&tool.0)
            .map_or((Beat::Idle, 10.0), |&(beat, at)| (beat, now - at));
        // The bob hangs true until the plumb is armed. Only the local player's arming is
        // known here: it travels only with a shot.
        let (armed_pitch, armed_yaw) = armed
            .as_ref()
            .filter(|_| tool.0 == runtime.local_player)
            .and_then(|armed| armed.armed)
            .unwrap_or((-FRAC_PI_2, 0.0));
        let state = ToolState {
            beat,
            since,
            armed_pitch,
            armed_yaw,
            charge: charge(&runtime, tool.0)
                .map_or(0.0, |charge| charge as f32 / MAX_CHARGE as f32),
        };
        let pose = observed_tool::pose(HELD, state, now);
        for child in children.iter() {
            if let Ok((part, mut at)) = parts.get_mut(child)
                && let Some(placed) = pose.parts.get(part.0)
            {
                *at = *placed;
            }
        }
        if tool.0 == runtime.local_player {
            local_signal = Some((pose.signal, beat));
        }
    }
    if let Some((signal, beat)) = local_signal
        && let Some(mut material) = materials.get_mut(&assets.signal)
    {
        let role = if beat == Beat::Pull {
            Role::Pull
        } else {
            Role::Push
        };
        material.emissive = LinearRgba::from(treatment(role).base_color) * (signal * 5.0);
    }
}

/// Where `player`'s hand holds the tool.
pub(super) fn held_pose(
    runtime: &HexWfcRuntime,
    sway: &HeldSway,
    player: &HexPlayerState,
) -> Transform {
    held_transform(player, sway_for(runtime, sway, player), &HAND)
}

/// Light the reticle for what a shot would do now, and answer a shot the pool cannot pay
/// for with an empty click and a notice.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_reticle(
    mut commands: Commands,
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    settings: Res<crate::settings::Settings>,
    overlay: Res<MatchOverlayState>,
    buttons: Option<Res<ButtonInput<MouseButton>>>,
    gamepads: Query<&Gamepad>,
    architect: Option<Res<super::architect::ArchitectDesk>>,
    spectator: Option<Res<crate::sim::state::SpectatorBot>>,
    assets: Option<Res<KineticAssets>>,
    presentation: Option<ResMut<KineticPresentation>>,
    mut notice: ResMut<HudNotice>,
    mut reticle: Query<(&mut BorderColor, &mut Visibility), With<Reticle>>,
) {
    let (Some(assets), Some(mut presentation)) = (assets, presentation) else {
        return;
    };
    let Ok((mut border, mut visibility)) = reticle.single_mut() else {
        return;
    };
    let local = runtime.local_player;
    let in_play = *overlay == MatchOverlayState::Playing
        && architect.is_none()
        && spectator.is_none()
        && runtime
            .match_state
            .players
            .get(&local)
            .is_some_and(HexPlayerState::in_facility);
    let wanted = if in_play {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != wanted {
        *visibility = wanted;
    }
    if !in_play {
        return;
    }
    let now = time.elapsed_secs();
    let dry = charge(&runtime, local).is_some_and(|charge| charge < KINETIC_SHOT_COST);
    let pressed = buttons
        .as_ref()
        .is_some_and(|b| b.just_pressed(MouseButton::Left) || b.just_pressed(MouseButton::Right))
        || gamepads.iter().any(|pad| {
            pad.just_pressed(GamepadButton::RightTrigger2)
                || pad.just_pressed(GamepadButton::RightThumb)
        });
    if dry && pressed {
        presentation.empty_at = Some(now);
        notice.show(
            "Kinetic tool empty. Recharge at a powered station",
            Tone::Against,
            now.into(),
        );
        super::audio::play(
            &mut commands,
            assets.empty.clone(),
            0.6 * settings.effective_sfx_volume(),
            "Kinetic empty",
            None,
        );
    }
    let flashing = presentation.empty_at.is_some_and(|at| now - at < 0.6);
    let color = if dry || flashing {
        WARNING
    } else if runtime.match_state.kinetic_target(local).is_some() {
        treatment(Role::Push).base_color
    } else {
        DIM.with_alpha(0.5)
    };
    let wanted = BorderColor::all(color);
    if *border != wanted {
        *border = wanted;
    }
}

#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;

    use super::*;

    /// Every vertex of the Lance, in every pose it takes, is inside [`REACH`], so the box
    /// `held_devices_stay_inside_the_body` checks is the tool's real size.
    #[test]
    fn the_lance_stays_inside_its_reach_box() {
        let [lo, hi] = REACH;
        let meshes: Vec<Mesh> = observed_tool::parts(HELD)
            .into_iter()
            .map(|part| observed_guardian::mesh::mesh(part.shape))
            .collect();
        for beat in [Beat::Idle, Beat::Push, Beat::Pull, Beat::Lash] {
            for since in [0.0, 0.05, 0.2, 1.0] {
                for armed_pitch in [-FRAC_PI_2, -0.6, 0.0, 0.8, FRAC_PI_2] {
                    for armed_yaw in [-2.8, -1.0, 0.0, 1.5, 3.1] {
                        let state = ToolState {
                            beat,
                            since,
                            armed_pitch,
                            armed_yaw,
                            charge: 0.5,
                        };
                        let pose = observed_tool::pose(HELD, state, 0.7);
                        for (mesh, at) in meshes.iter().zip(&pose.parts) {
                            let Some(VertexAttributeValues::Float32x3(positions)) =
                                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                            else {
                                panic!("positions");
                            };
                            for &p in positions {
                                let p = at.transform_point(Vec3::from(p));
                                assert!(
                                    p.cmpge(lo).all() && p.cmple(hi).all(),
                                    "{state:?}: a vertex at {p} is outside the reach box"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
