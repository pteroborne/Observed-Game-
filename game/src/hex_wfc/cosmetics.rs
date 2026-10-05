//! Frozen visual loadouts and bounded decorative trails. No simulation writes.
use super::{entities::ActorVisual, sim::HexWfcRuntime};
use crate::{GameState, flow::Career, lan::LanRuntime, sim::replay::ReplayTape};
use bevy::{light::NotShadowCaster, prelude::*};
use observed_core::{PlayerId, cosmetics::CosmeticLook};
use std::collections::{BTreeMap, VecDeque};

#[derive(Resource, Default)]
pub(crate) struct MatchCosmetics {
    pub(crate) looks: BTreeMap<PlayerId, CosmeticLook>,
    histories: BTreeMap<PlayerId, VecDeque<Vec3>>,
}
impl MatchCosmetics {
    pub(crate) fn look(&self, player: PlayerId) -> CosmeticLook {
        self.looks.get(&player).copied().unwrap_or_default()
    }
}
#[derive(Component)]
pub(super) struct TrailPoint {
    player: PlayerId,
    index: usize,
}

pub(super) fn setup(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    career: Res<Career>,
    lan: Res<LanRuntime>,
    mut tape: ResMut<ReplayTape>,
) {
    let local = career.profile.cosmetic_look();
    let looks = runtime
        .match_state
        .players
        .keys()
        .map(|&player| {
            let look = if runtime.networked {
                lan.client
                    .as_ref()
                    .and_then(|c| c.appearances.get(player.index()))
                    .copied()
                    .unwrap_or_default()
            } else if player == runtime.local_player {
                local
            } else {
                CosmeticLook::default()
            };
            (player, look)
        })
        .collect();
    let cosmetics = MatchCosmetics { looks, ..default() };
    tape.cosmetics = cosmetics.looks.clone();
    commands.insert_resource(cosmetics);
}

/// Badge geometry is shared with the recorded world viewer. IDs select shapes, never roles.
pub(crate) fn badge_parts(look: CosmeticLook) -> Vec<Transform> {
    let bars = if look.badge == 7 { 1 } else { 3 };
    (0..bars)
        .map(|i| {
            let height = if look.badge == 9 && i == 1 {
                0.19
            } else if look.badge == 9 {
                0.13
            } else {
                0.09
            };
            Transform::from_xyz(
                0.29 + i as f32 * 0.055,
                observed_observer::form::EYE_RISE - 0.25 + height / 2.0,
                -0.20,
            )
            .with_scale(Vec3::new(0.035, height, 0.04))
        })
        .collect()
}
pub(crate) fn trail_count(look: CosmeticLook) -> usize {
    match look.trail {
        5 => 3,
        6 => 6,
        _ => 0,
    }
}
pub(crate) fn trail_scale(look: CosmeticLook, index: usize) -> Vec3 {
    let radius = if look.trail == 6 { 0.075 } else { 0.04 };
    Vec3::splat(radius * (1.0 - index as f32 * 0.10))
}
/// Keep trails within the current space and discard teleport/prison transitions.
pub(crate) fn remember(history: &mut VecDeque<Vec3>, at: Vec3) {
    if history
        .front()
        .is_some_and(|previous| previous.distance(at) > 3.0)
    {
        history.clear();
    }
    if history
        .front()
        .is_none_or(|previous| previous.distance(at) >= 0.15)
    {
        history.push_front(at);
        history.truncate(7);
    }
}

pub(super) fn decorate(
    commands: &mut Commands,
    root: Entity,
    player: PlayerId,
    look: CosmeticLook,
    art: (&Handle<Mesh>, &Handle<Mesh>, &Handle<StandardMaterial>),
) {
    let (cube, sphere, material) = art;
    commands.entity(root).with_children(|body| {
        for transform in badge_parts(look) {
            body.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(material.clone()),
                transform,
                NotShadowCaster,
            ));
        }
    });
    for index in 0..trail_count(look) {
        commands.spawn((
            TrailPoint { player, index },
            Mesh3d(sphere.clone()),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            DespawnOnExit(GameState::HexWfc),
        ));
    }
}

pub(super) fn sync(
    runtime: Res<HexWfcRuntime>,
    mut cosmetics: ResMut<MatchCosmetics>,
    actors: Query<(&ActorVisual, &Visibility)>,
    mut points: Query<(&TrailPoint, &mut Transform, &mut Visibility), Without<ActorVisual>>,
) {
    for player in runtime.match_state.players.values() {
        let at = super::ascent::presented_position(player)
            + Vec3::Y * (observed_observer::form::EYE_RISE - 0.35);
        remember(cosmetics.histories.entry(player.id).or_default(), at);
    }
    for (point, mut transform, mut visibility) in &mut points {
        let visible = actors.iter().any(|(actor, visibility)| {
            actor.player() == point.player && *visibility != Visibility::Hidden
        });
        let history = cosmetics.histories.get(&point.player);
        // Skip the newest point inside the eye, and never draw the body being viewed through.
        let at = history.and_then(|history| history.get(point.index + 1));
        *visibility = if visible && at.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Some(at) = at {
            transform.translation = *at;
            transform.scale = trail_scale(cosmetics.look(point.player), point.index);
        }
    }
}
pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<MatchCosmetics>();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_freezes_cosmetics_for_live_views_and_replay_then_cleans_up() {
        let mut app = crate::tests::test_app();
        {
            let mut career = app.world_mut().resource_mut::<Career>();
            for _ in 0..15 {
                career.profile.award_match(Some(1));
            }
            for id in [2, 6, 9] {
                assert!(career.profile.equip(id));
            }
        }
        let mut setup = crate::play_setup::PlaySetupDraft::default();
        setup.select_preset(crate::play_setup::PlayPreset::CoOp);
        app.insert_resource(setup);
        crate::tests::go(&mut app, GameState::HexWfc);
        let local = app.world().resource::<HexWfcRuntime>().local_player;
        let expected = CosmeticLook {
            color: 2,
            trail: 6,
            badge: 9,
        };
        assert_eq!(
            app.world().resource::<MatchCosmetics>().look(local),
            expected
        );
        assert_eq!(
            app.world().resource::<ReplayTape>().cosmetics[&local],
            expected
        );
        assert_eq!(
            app.world_mut()
                .query::<&TrailPoint>()
                .iter(app.world())
                .count(),
            6
        );
        assert!(app.world_mut().resource_mut::<Career>().profile.equip(1));
        app.update();
        assert_eq!(
            app.world().resource::<MatchCosmetics>().look(local),
            expected
        );
        assert_eq!(
            app.world().resource::<ReplayTape>().cosmetics[&local],
            expected
        );
        crate::tests::go(&mut app, GameState::MainMenu);
        assert!(!app.world().contains_resource::<MatchCosmetics>());
        assert_eq!(
            app.world_mut()
                .query::<&TrailPoint>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(
            app.world().resource::<ReplayTape>().cosmetics[&local],
            expected
        );
        crate::tests::go(&mut app, GameState::HexWfc);
        assert_eq!(
            app.world().resource::<MatchCosmetics>().look(local).color,
            1
        );
    }

    #[test]
    fn trail_history_is_bounded_and_does_not_bridge_teleports() {
        let mut history = VecDeque::new();
        for i in 0..100 {
            remember(&mut history, Vec3::X * i as f32 * 0.2);
        }
        assert_eq!(history.len(), 7);
        remember(&mut history, Vec3::Y * 100.0);
        assert_eq!(history.len(), 1);
        let stationary = history.clone();
        remember(&mut history, Vec3::Y * 100.0);
        assert_eq!(history, stationary);
    }
}
