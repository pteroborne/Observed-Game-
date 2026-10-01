//! Floor power in Architect Ascent: each floor's generator and the stations
//! Architects deploy, drawn where the rules place them.
//!
//! The rules own power and charge (`observed_match::ascent::facility::power`); this reads
//! them and writes nothing back. A body works a fixture by standing at it: interact at the
//! generator switches the floor, and a powered station fills the kinetic tool of whoever
//! stands in its cradle.
//!
//! - **The generator** is a squat hexagonal turbine in the equipment's language: a dark
//!   plinth and cap, bronze rotor rings that turn while the floor has power, and a core in
//!   the powered colour ([`Role::Powered`]) under a column of its light. Cut, the rotor
//!   stops and the core burns the collapse red: dark, but never unreadable, because the
//!   generator is the thing a dark floor sends you looking for.
//! - **The station** is a cradle of three bronze posts round a charge cell in the tool's
//!   own push colour, under a ring in the powered colour. Dead, both go the unpowered grey,
//!   still self-lit enough to be found.
//! - **A dark floor's practicals** fall to a fraction of their light, and their diffusers
//!   go out. The district key over the runner stays: darkness costs observation range in
//!   the rules, never legibility here.
//! - **Sounds and notices**: the power going on or off is heard at the generator, and said
//!   when it is the local body's floor; charge ticks in as the station fills the tool.
//! - **The prompt**: at the generator, what interact would do; at the station, the fill.

use std::collections::{BTreeMap, BTreeSet};

use bevy::ecs::system::SystemParam;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use observed_match::ascent::economy::MAX_CHARGE;
use observed_match::ascent::facility::{Fixture, FixtureKind};
use observed_style::equipment::{Hardware, finish};
use observed_style::kinetic::{Role, treatment};
use observed_style::{MarkerRole, marker};

use super::equipment::{hex_prism, hex_ring, light_tube};
use super::hud::play::HudNotice;
use super::hud::words::Tone;
use super::sim::HexWfcRuntime;
use super::view::HexPractical;
use crate::GameState;
use crate::settings::Settings;

pub(super) mod capture;
mod words;
pub(super) use words::{local_floor_powered, prompt};

/// How much of its light a practical keeps on a floor without power.
const DARK_PRACTICAL: f32 = 0.12;
/// Radians a second the generator's rotor turns while powered.
const ROTOR_SPEED: f32 = 1.4;
/// Height of the column of light over a powered fixture, metres.
const COLUMN: f32 = 2.6;

#[derive(Resource)]
pub(super) struct PowerAssets {
    plinth: Handle<Mesh>,
    cap: Handle<Mesh>,
    rotor: Handle<Mesh>,
    core: Handle<Mesh>,
    post: Handle<Mesh>,
    ring: Handle<Mesh>,
    cell: Handle<Mesh>,
    column: Handle<Mesh>,
    body: Handle<StandardMaterial>,
    bronze: Handle<StandardMaterial>,
    /// Lit: the powered colour, the cut red, the tool's push colour, the dead grey.
    powered: Handle<StandardMaterial>,
    cut: Handle<StandardMaterial>,
    charge: Handle<StandardMaterial>,
    dead: Handle<StandardMaterial>,
    haze: Handle<StandardMaterial>,
    /// What a dark floor's diffusers wear.
    unlit_diffuser: Handle<StandardMaterial>,
    power_on: Handle<AudioSource>,
    power_off: Handle<AudioSource>,
    charge_tick: Handle<AudioSource>,
    recharged: Handle<AudioSource>,
}

impl FromWorld for PowerAssets {
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        let (power_on, power_off, charge_tick, recharged) = (
            server.load("sounds/kinetic/power_on.ogg"),
            server.load("sounds/kinetic/power_off.ogg"),
            server.load("sounds/kinetic/charge_tick.ogg"),
            server.load("sounds/kinetic/recharge.ogg"),
        );
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let plinth = meshes.add(hex_prism(0.95, 0.85, 0.0, 0.28));
        let cap = meshes.add(hex_prism(0.8, 0.6, 1.62, 1.84));
        let rotor = meshes.add(hex_ring(0.62, 0.46, -0.05, 0.05));
        let core = meshes.add(hex_prism(0.3, 0.3, 0.28, 1.62));
        let post = meshes.add(hex_prism(0.06, 0.06, 0.2, 1.25));
        let ring = meshes.add(hex_ring(0.62, 0.5, 1.18, 1.28));
        let cell = meshes.add(hex_prism(0.2, 0.2, 0.3, 1.05));
        let column = meshes.add(light_tube(0.5, COLUMN));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let mut hardware = |part: Hardware| {
            let f = finish(part);
            materials.add(StandardMaterial {
                base_color: f.base_color,
                metallic: f.metallic,
                perceptual_roughness: f.roughness,
                ..default()
            })
        };
        let (body, bronze) = (hardware(Hardware::Body), hardware(Hardware::Trim));
        let mut lit = |base: Color, emissive: LinearRgba| {
            materials.add(StandardMaterial {
                base_color: base,
                emissive,
                ..default()
            })
        };
        let on = treatment(Role::Powered);
        let red = marker(MarkerRole::Collapse);
        let push = treatment(Role::Push);
        let grey = treatment(Role::Unpowered);
        let powered = lit(on.base_color, on.emissive);
        // Cut and dead are dark, not black: each keeps a self-lit minimum.
        let cut = lit(red.base_color, red.emissive * 0.35);
        let charge = lit(push.base_color, push.emissive);
        let dead = lit(grey.base_color, LinearRgba::from(grey.base_color) * 0.6);
        let haze = materials.add(StandardMaterial {
            base_color: Color::LinearRgba(on.emissive * 0.04),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        let unlit_diffuser = materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.05, 0.06),
            perceptual_roughness: 0.6,
            ..default()
        });
        Self {
            plinth,
            cap,
            rotor,
            core,
            post,
            ring,
            cell,
            column,
            body,
            bronze,
            powered,
            cut,
            charge,
            dead,
            haze,
            unlit_diffuser,
            power_on,
            power_off,
            charge_tick,
            recharged,
        }
    }
}

/// A fixture in the facility, drawn: which floor powers it.
#[derive(Component)]
pub(super) struct PowerFixture {
    level: u8,
    kind: FixtureKind,
    cell: observed_hex::HexCoord,
}

/// A part of a fixture lit by its floor's power: what it wears lit, and dark.
#[derive(Component)]
pub(super) struct PowerLit {
    on: Handle<StandardMaterial>,
    off: Handle<StandardMaterial>,
}

/// A part shown only while its floor has power.
#[derive(Component)]
pub(super) struct PoweredOnly;

/// The generator's rotor rings, which turn while the floor has power.
#[derive(Component)]
pub(super) struct Rotor;

/// A practical's own light and diffuser, as built, to dim and restore.
#[derive(Component)]
pub(super) struct PracticalAtFullPower {
    intensity: Option<f32>,
    diffuser: Option<Handle<StandardMaterial>>,
}

/// What the presentation remembers between frames.
#[derive(Resource, Default)]
pub(super) struct PowerPresentation {
    /// Each floor's power as last drawn.
    power: BTreeMap<u8, bool>,
    /// The local body's charge as last heard.
    charge: Option<u32>,
}

fn spawn_fixture(commands: &mut Commands, assets: &PowerAssets, fixture: Fixture) {
    let level = fixture.cell.level;
    let lit = |on: &Handle<StandardMaterial>, off: &Handle<StandardMaterial>| PowerLit {
        on: on.clone(),
        off: off.clone(),
    };
    let name = match fixture.kind {
        FixtureKind::Generator => "Generator",
        FixtureKind::Station => "Recharge station",
    };
    commands
        .spawn((
            PowerFixture {
                level,
                kind: fixture.kind,
                cell: fixture.cell,
            },
            DespawnOnExit(GameState::HexWfc),
            Transform::from_translation(fixture.floor),
            Visibility::default(),
            Name::new(name),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(assets.plinth.clone()),
                MeshMaterial3d(assets.body.clone()),
            ));
            parent.spawn((
                PoweredOnly,
                Mesh3d(assets.column.clone()),
                MeshMaterial3d(assets.haze.clone()),
                Transform::from_xyz(0.0, 0.3, 0.0),
                NotShadowCaster,
                NotShadowReceiver,
            ));
            match fixture.kind {
                FixtureKind::Generator => {
                    parent.spawn((
                        Mesh3d(assets.cap.clone()),
                        MeshMaterial3d(assets.body.clone()),
                    ));
                    parent.spawn((
                        lit(&assets.powered, &assets.cut),
                        Mesh3d(assets.core.clone()),
                        MeshMaterial3d(assets.powered.clone()),
                    ));
                    for height in [0.55, 0.95, 1.35] {
                        parent.spawn((
                            Rotor,
                            Mesh3d(assets.rotor.clone()),
                            MeshMaterial3d(assets.bronze.clone()),
                            Transform::from_xyz(0.0, height, 0.0),
                        ));
                    }
                }
                FixtureKind::Station => {
                    parent.spawn((
                        lit(&assets.charge, &assets.dead),
                        Mesh3d(assets.cell.clone()),
                        MeshMaterial3d(assets.charge.clone()),
                    ));
                    parent.spawn((
                        lit(&assets.powered, &assets.dead),
                        Mesh3d(assets.ring.clone()),
                        MeshMaterial3d(assets.powered.clone()),
                    ));
                    for corner in 0..3u8 {
                        let angle = (f32::from(corner) * 120.0 + 30.0).to_radians();
                        parent.spawn((
                            Mesh3d(assets.post.clone()),
                            MeshMaterial3d(assets.bronze.clone()),
                            Transform::from_xyz(0.56 * angle.cos(), 0.0, 0.56 * angle.sin()),
                        ));
                    }
                }
            }
        });
}

#[derive(SystemParam)]
pub(super) struct FixtureParts<'w, 's> {
    fixtures: Query<'w, 's, (Entity, &'static PowerFixture, &'static Children)>,
    lit: Query<
        'w,
        's,
        (
            &'static PowerLit,
            &'static mut MeshMaterial3d<StandardMaterial>,
        ),
    >,
    shown: Query<'w, 's, &'static mut Visibility, With<PoweredOnly>>,
    rotors: Query<'w, 's, &'static mut Transform, With<Rotor>>,
}

/// Keep drawn fixtures in step with station-card plays and tile retractions.
pub(super) fn sync_fixtures(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    time: Res<Time>,
    assets: Option<Res<PowerAssets>>,
    presentation: Option<ResMut<PowerPresentation>>,
    parts: FixtureParts,
) {
    let FixtureParts {
        fixtures,
        mut lit,
        mut shown,
        mut rotors,
    } = parts;
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    // Built on first need, so a race never makes them.
    let (Some(assets), Some(_presentation)) = (assets, presentation) else {
        commands.init_resource::<PowerAssets>();
        commands.init_resource::<PowerPresentation>();
        return;
    };
    let wanted: BTreeSet<_> = ascent
        .fixtures()
        .iter()
        .map(|fixture| (fixture.kind, fixture.cell))
        .collect();
    let drawn: BTreeSet<_> = fixtures
        .iter()
        .map(|(_, fixture, _)| (fixture.kind, fixture.cell))
        .collect();
    for (entity, fixture, _) in &fixtures {
        if !wanted.contains(&(fixture.kind, fixture.cell)) {
            commands.entity(entity).despawn();
        }
    }
    for &fixture in ascent.fixtures() {
        if !drawn.contains(&(fixture.kind, fixture.cell)) {
            spawn_fixture(&mut commands, &assets, fixture);
        }
    }
    let economy = &ascent.rules().economy;
    let spin = time.delta_secs() * ROTOR_SPEED;
    for (_, fixture, children) in &fixtures {
        let powered = economy.is_powered(fixture.level);
        for child in children.iter() {
            if let Ok((lit, mut material)) = lit.get_mut(child) {
                let wanted = if powered { &lit.on } else { &lit.off };
                if material.0 != *wanted {
                    material.0 = wanted.clone();
                }
            }
            if let Ok(mut visibility) = shown.get_mut(child) {
                let wanted = if powered {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *visibility != wanted {
                    *visibility = wanted;
                }
            }
            if powered && let Ok(mut transform) = rotors.get_mut(child) {
                // Alternate rings turn against each other.
                let direction = if (transform.translation.y - 0.95).abs() < 0.1 {
                    -1.0
                } else {
                    1.0
                };
                transform.rotate_y(spin * direction);
            }
        }
    }
}

type Practicals<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static HexPractical,
        &'static GlobalTransform,
        Option<&'static mut PointLight>,
        Option<&'static mut MeshMaterial3d<StandardMaterial>>,
        Option<&'static PracticalAtFullPower>,
    ),
>;

/// Dim the practicals of every floor without power, and bring them back with it.
pub(super) fn sync_practicals(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    assets: Option<Res<PowerAssets>>,
    mut practicals: Practicals,
) {
    let (Some(ascent), Some(assets)) = (runtime.ascent.as_ref(), assets) else {
        return;
    };
    let economy = &ascent.rules().economy;
    for (entity, practical, placed, light, material, full) in &mut practicals {
        // A prison maze's cells are practicals too, numbered in the maze's own frame and
        // drawn far below: the facility's floors do not power them.
        if placed.translation().y < -super::ascent::PRISON_DEPTH / 2.0 {
            continue;
        }
        let Some(full) = full else {
            // First seen: remember it as built, at full power.
            commands.entity(entity).insert(PracticalAtFullPower {
                intensity: light.as_ref().map(|light| light.intensity),
                diffuser: material.as_ref().map(|material| material.0.clone()),
            });
            continue;
        };
        let powered = economy.is_powered(practical.0.level);
        if let (Some(mut light), Some(intensity)) = (light, full.intensity) {
            let wanted = if powered {
                intensity
            } else {
                intensity * DARK_PRACTICAL
            };
            if light.intensity != wanted {
                light.intensity = wanted;
            }
        }
        if let (Some(mut material), Some(diffuser)) = (material, full.diffuser.as_ref()) {
            let wanted = if powered {
                diffuser
            } else {
                &assets.unlit_diffuser
            };
            if material.0 != *wanted {
                material.0 = wanted.clone();
            }
        }
    }
}

/// Hear and say what changed: a floor's power, and the local tool filling at a station.
pub(super) fn read_changes(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    time: Res<Time>,
    settings: Res<Settings>,
    assets: Option<Res<PowerAssets>>,
    presentation: Option<ResMut<PowerPresentation>>,
    mut notice: ResMut<HudNotice>,
) {
    let (Some(ascent), Some(assets), Some(mut presentation)) =
        (runtime.ascent.as_ref(), assets, presentation)
    else {
        return;
    };
    let volume = settings.effective_sfx_volume();
    let now = time.elapsed_secs_f64();
    let economy = &ascent.rules().economy;
    let local_level = runtime.local().cell.level;
    let first = presentation.power.is_empty();
    for (&level, &powered) in &economy.power {
        let before = presentation.power.insert(level, powered);
        if first || before == Some(powered) {
            continue;
        }
        let at = ascent
            .fixtures()
            .iter()
            .find(|f| f.kind == FixtureKind::Generator && f.cell.level == level)
            .map(|f| f.floor + Vec3::Y);
        let sound = if powered {
            assets.power_on.clone()
        } else {
            assets.power_off.clone()
        };
        super::audio::play(&mut commands, sound, 0.8 * volume, "Floor power", at);
        if level == local_level && runtime.local().in_facility() {
            if powered {
                notice.show("Power restored on this floor", Tone::Good, now);
            } else {
                notice.show(
                    "This floor has lost its power. Find its generator",
                    Tone::Against,
                    now,
                );
            }
        }
    }
    let charge = ascent
        .observer_for(runtime.local_player)
        .map(|observer| economy.charge(observer));
    let before = std::mem::replace(&mut presentation.charge, charge);
    if let (Some(before), Some(charge)) = (before, charge)
        && charge > before
    {
        let (sound, name) = if charge >= MAX_CHARGE {
            (assets.recharged.clone(), "Kinetic recharged")
        } else {
            (assets.charge_tick.clone(), "Kinetic charge")
        };
        super::audio::play(&mut commands, sound, 0.55 * volume, name, None);
    }
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<PowerAssets>();
    commands.remove_resource::<PowerPresentation>();
}
