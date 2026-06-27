use bevy::prelude::*;

use crate::LifecycleDiagnostics;

#[derive(Component, Debug)]
pub struct GameOwned;

#[derive(Component, Debug)]
pub struct SessionRoot;

#[derive(Component, Debug, Clone, Copy, Eq, PartialEq)]
pub struct LabPlayerId(pub u16);

#[derive(Resource, Debug, Clone, Copy)]
pub struct GameplaySession {
    pub generation: u32,
    pub owned_entities: usize,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct SessionResetRequested(pub bool);

const PLAYER_COLORS: [Color; 4] = [
    Color::srgb(0.25, 0.85, 1.0),
    Color::srgb(1.0, 0.45, 0.35),
    Color::srgb(0.65, 1.0, 0.35),
    Color::srgb(0.85, 0.45, 1.0),
];

pub(crate) fn ensure_session(world: &mut World) {
    if world.contains_resource::<GameplaySession>() {
        return;
    }

    spawn_session(world);
}

pub(crate) fn cleanup_session(world: &mut World) {
    cleanup_session_internal(world, false);
}

pub(crate) fn perform_requested_reset(world: &mut World) {
    let requested = world
        .get_resource::<SessionResetRequested>()
        .is_some_and(|request| request.0);
    if !requested {
        return;
    }

    world.resource_mut::<SessionResetRequested>().0 = false;

    if !world.contains_resource::<GameplaySession>() {
        return;
    }

    cleanup_session_internal(world, true);
    spawn_session(world);
}

fn cleanup_session_internal(world: &mut World, is_reset: bool) {
    let before = count_owned(world);
    let had_session = world.contains_resource::<GameplaySession>();

    let entities = {
        let mut query = world.query_filtered::<Entity, With<GameOwned>>();
        query.iter(world).collect::<Vec<_>>()
    };

    for entity in entities {
        let _ = world.despawn(entity);
    }

    world.remove_resource::<GameplaySession>();
    let after = count_owned(world);

    if had_session || before > 0 {
        let mut diagnostics = world.resource_mut::<LifecycleDiagnostics>();
        diagnostics.last_cleanup = Some((before, after));
        diagnostics.cleanup_attempts += 1;
        if is_reset {
            diagnostics.resets += 1;
        }
    }
}

fn spawn_session(world: &mut World) {
    let generation = {
        let mut diagnostics = world.resource_mut::<LifecycleDiagnostics>();
        diagnostics.generations += 1;
        diagnostics.generations
    };

    world.spawn((
        GameOwned,
        SessionRoot,
        Name::new(format!("Gameplay Session {generation}")),
    ));

    let room_color = Color::srgb(0.10, 0.14, 0.21);
    let line_color = Color::srgb(0.20, 0.32, 0.48);

    for (name, size, position) in [
        ("Floor", Vec2::new(860.0, 28.0), Vec3::new(0.0, -245.0, 0.0)),
        (
            "Ceiling",
            Vec2::new(860.0, 12.0),
            Vec3::new(0.0, 245.0, 0.0),
        ),
        (
            "Left Wall",
            Vec2::new(12.0, 500.0),
            Vec3::new(-430.0, 0.0, 0.0),
        ),
        (
            "Right Wall",
            Vec2::new(12.0, 500.0),
            Vec3::new(430.0, 0.0, 0.0),
        ),
        (
            "Center Platform",
            Vec2::new(300.0, 18.0),
            Vec3::new(0.0, -55.0, 0.0),
        ),
        (
            "Upper Platform",
            Vec2::new(220.0, 18.0),
            Vec3::new(230.0, 95.0, 0.0),
        ),
    ] {
        world.spawn((
            GameOwned,
            Name::new(name),
            Sprite::from_color(
                if name.contains("Platform") {
                    line_color
                } else {
                    room_color
                },
                size,
            ),
            Transform::from_translation(position),
        ));
    }

    for index in 0..4 {
        let x = -255.0 + index as f32 * 170.0;
        world.spawn((
            GameOwned,
            LabPlayerId(index),
            Name::new(format!("Lab Player {}", index + 1)),
            Sprite::from_color(PLAYER_COLORS[index as usize], Vec2::new(48.0, 76.0)),
            Transform::from_xyz(x, -190.0, 1.0),
        ));
    }

    world.spawn((
        GameOwned,
        Name::new("Session Core"),
        Sprite::from_color(Color::srgb(1.0, 0.78, 0.22), Vec2::splat(42.0)),
        Transform::from_xyz(0.0, -12.0, 1.0),
    ));

    let owned_entities = count_owned(world);
    world.insert_resource(GameplaySession {
        generation,
        owned_entities,
    });
}

fn count_owned(world: &mut World) -> usize {
    let mut query = world.query_filtered::<Entity, With<GameOwned>>();
    query.iter(world).count()
}

pub(crate) fn animate_players(
    time: Res<Time>,
    session: Option<Res<GameplaySession>>,
    mut players: Query<(&LabPlayerId, &mut Transform)>,
) {
    let Some(session) = session else {
        return;
    };

    let elapsed = time.elapsed_secs();
    for (player, mut transform) in &mut players {
        let phase = player.0 as f32 * 0.85 + session.generation as f32 * 0.15;
        transform.translation.y = -190.0 + (elapsed * 2.0 + phase).sin() * 8.0;
        transform.rotation = Quat::from_rotation_z((elapsed + phase).sin() * 0.025);
    }
}
