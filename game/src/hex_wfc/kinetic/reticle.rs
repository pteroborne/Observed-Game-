//! Contextual aiming feedback, read from authoritative action rules.
use super::{KineticAssets, KineticPresentation, Reticle, charge};
use crate::{
    GameState,
    hex_wfc::{
        hud::{play::HudNotice, words::Tone},
        overlay::MatchOverlayState,
        sim::HexWfcRuntime,
    },
    settings::Settings,
};
use bevy::prelude::*;
use observed_match::{ascent::economy::KINETIC_SHOT_COST, hex_wfc::HexPlayerState};
use observed_style::kinetic::{ReticleState, reticle_color};

#[derive(Component)]
pub(in crate::hex_wfc) struct RangeLabel;

pub(super) fn setup(commands: &mut Commands) {
    commands
        .spawn((
            Reticle,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(50),
                width: px(18),
                height: px(18),
                margin: UiRect::new(px(-9), px(0), px(-9), px(0)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(reticle_color(ReticleState::Neutral)),
            Outline::new(
                px(1),
                px(0),
                observed_style::kinetic::treatment(observed_style::kinetic::Role::Panel).base_color,
            ),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(6),
                    top: px(6),
                    width: px(2),
                    height: px(2),
                    ..default()
                },
                BackgroundColor(reticle_color(ReticleState::Neutral)),
            ));
            parent.spawn((
                RangeLabel,
                Text::new(""),
                TextFont {
                    font_size: bevy::text::FontSize::Px(12.0),
                    ..default()
                },
                TextColor(reticle_color(ReticleState::Neutral)),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(23),
                    left: px(-100),
                    width: px(218),
                    ..default()
                },
                TextLayout::justify(Justify::Center),
            ));
        });
}

#[derive(Clone, Debug)]
struct Hint {
    state: ReticleState,
    label: String,
}
fn hint(runtime: &HexWfcRuntime) -> Hint {
    use observed_match::{
        ascent::facility::{AtFixture, FIXTURE_REACH},
        hex_wfc::{DOOR_REACH, KINETIC_REACH},
    };
    let physical = &runtime.match_state;
    let id = runtime.local_player;
    let mut candidates = Vec::new();
    if let Some(target) = physical.kinetic_aim_target(id, 30.0) {
        let dry = charge(runtime, id).is_some_and(|c| c < KINETIC_SHOT_COST);
        let (state, label) = if target.distance > KINETIC_REACH {
            (ReticleState::TooFar, "TOO FAR")
        } else if dry {
            (ReticleState::Unavailable, "RECHARGE")
        } else if physical.kinetic_cooldown(id) > 0 {
            (ReticleState::Unavailable, "RECOVERING")
        } else {
            (ReticleState::Ready, "PUSH / PULL")
        };
        candidates.push((
            target.distance,
            Hint {
                state,
                label: format!("{label}  {:.0} / {:.0} m", target.distance, KINETIC_REACH),
            },
        ));
    }
    if let Some(ascent) = &runtime.ascent {
        for fixture in ascent.fixtures() {
            let Some(aim) =
                physical.aim_candidate(id, fixture.floor + Vec3::Y * 1.0, FIXTURE_REACH, 0.8)
            else {
                continue;
            };
            let action = ascent
                .at_fixture(physical, id)
                .filter(|(f, _)| f.cell == fixture.cell && f.kind == fixture.kind)
                .map(|(_, at)| at);
            let (state, label) = if action.is_none() && !aim.in_reach {
                (ReticleState::TooFar, "TOO FAR")
            } else {
                match action {
                    Some(AtFixture::Generator { operable: true, .. }) => {
                        (ReticleState::Ready, "INTERACT")
                    }
                    Some(AtFixture::Station { powered: true, .. }) => {
                        (ReticleState::Ready, "RECHARGE")
                    }
                    Some(AtFixture::Station { powered: false, .. }) => {
                        (ReticleState::Unavailable, "NO POWER")
                    }
                    _ => (ReticleState::Unavailable, "UNAVAILABLE"),
                }
            };
            candidates.push((
                aim.distance,
                Hint {
                    state,
                    label: format!("{label}  {:.1} m", aim.distance),
                },
            ));
        }
        for door in physical.doors() {
            let (floor, _) = door.pose();
            let Some(aim) = physical.aim_candidate(id, floor + Vec3::Y * 1.2, DOOR_REACH, 1.4)
            else {
                continue;
            };
            let at = ascent
                .at_door(physical, id)
                .filter(|(d, _)| d.cell == door.cell && d.face == door.face)
                .map(|(_, at)| at);
            let (state, label) = if at.is_none() && !aim.in_reach {
                (ReticleState::TooFar, "TOO FAR")
            } else if at.is_some_and(|a| !a.powered) {
                (ReticleState::Unavailable, "NO POWER")
            } else if at.is_some_and(|a| a.operable) {
                (ReticleState::Ready, "INTERACT")
            } else {
                (ReticleState::Unavailable, "UNAVAILABLE")
            };
            candidates.push((
                aim.distance,
                Hint {
                    state,
                    label: format!("{label}  {:.1} m", aim.distance),
                },
            ));
        }
    }
    candidates
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, h)| h)
        .unwrap_or(Hint {
            state: ReticleState::Neutral,
            label: String::new(),
        })
}

/// Light the reticle for what a shot would do now, and answer a shot the pool cannot pay
/// for with an empty click and a notice.
#[allow(clippy::too_many_arguments)]
pub(in crate::hex_wfc) fn sync_reticle(
    mut commands: Commands,
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    settings: Res<Settings>,
    overlay: Res<MatchOverlayState>,
    buttons: Option<Res<ButtonInput<MouseButton>>>,
    gamepads: Query<&Gamepad>,
    architect: Option<Res<crate::hex_wfc::architect::ArchitectDesk>>,
    spectator: Option<Res<crate::sim::state::SpectatorBot>>,
    assets: Option<Res<KineticAssets>>,
    presentation: Option<ResMut<KineticPresentation>>,
    mut notice: ResMut<HudNotice>,
    mut label: Query<(&mut Text, &mut TextColor), With<RangeLabel>>,
    mut reticle: Query<(&mut BorderColor, &mut Visibility, &mut Node), With<Reticle>>,
) {
    let (Some(assets), Some(mut presentation)) = (assets, presentation) else {
        return;
    };
    let Ok((mut border, mut visibility, mut node)) = reticle.single_mut() else {
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
        crate::hex_wfc::audio::play(
            &mut commands,
            assets.empty.clone(),
            0.6 * settings.effective_sfx_volume(),
            "Kinetic empty",
            None,
        );
    }
    let flashing = presentation.empty_at.is_some_and(|at| now - at < 0.6);
    let mut hint = hint(&runtime);
    if flashing {
        hint.state = ReticleState::Unavailable;
        hint.label = "RECHARGE".into();
    }
    let color = reticle_color(hint.state);
    let [left, right, top, bottom] = observed_style::kinetic::reticle_edges(hint.state);
    let edges = UiRect::new(px(left), px(right), px(top), px(bottom));
    if node.border != edges {
        node.border = edges;
    }
    if let Ok((mut text, mut text_color)) = label.single_mut() {
        if **text != hint.label {
            **text = hint.label;
        }
        text_color.0 = color;
    }
    let wanted = BorderColor::all(color);
    if *border != wanted {
        *border = wanted;
    }
}
