//! Presentation and input for the kinetic tool lab.
//!
//! Everything here reads [`KineticWorld`] and never decides a rule. The one
//! thing this layer adds to the proof is the **preview**: because
//! [`KineticWorld::resolve_shove`] is pure, the lab can show exactly where a
//! push would send its target, and what would kill it, before the trigger is
//! pulled. That is the lab's honest answer to "is a shove fair?" — the outcome
//! is visible in advance rather than discovered afterwards.

use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::PlayerId;
use observed_hex::{
    coords::HexCoord,
    faces::HexFace,
    metrics::{CORNERS, hex_origin_plan},
};

use crate::model::{
    CellKind, KineticEvent, KineticIntent, KineticWorld, MAX_CHARGE, MinorGuardianId, PUSH_COST,
    PUSH_IMPULSE, ShoveFate, TICKS_PER_SECOND, ToolRefusal,
};

/// Screen pixels per lattice meter. Sized so the authored board fills most of a
/// 1440x900 window without the debug panel overlapping it.
const CELL_SCALE: f32 = 5.0;
/// Most simulation ticks one frame may run. A quarter second of catch-up keeps
/// a hitching frame from teleporting a Guardian across the board.
const MAX_CATCHUP_TICKS: u32 = TICKS_PER_SECOND / 4;

// The Legibility Contract in one table: every critical state has its own hue
// *and* its own shape or outline weight, so none of it depends on colour alone.
const COLOR_SOLID: Color = Color::srgb(0.13, 0.17, 0.22);
const COLOR_LEDGE: Color = Color::srgb(0.32, 0.26, 0.10);
const COLOR_VOID: Color = Color::srgb(0.02, 0.03, 0.05);
const COLOR_WALL: Color = Color::srgb(0.30, 0.33, 0.38);
const COLOR_RETRACTING: Color = Color::srgb(0.40, 0.13, 0.10);
const COLOR_OBSERVER: Color = Color::srgb(0.45, 0.92, 1.0);
const COLOR_MINOR: Color = Color::srgb(1.0, 0.42, 0.30);
const COLOR_MAJOR_AWAKE: Color = Color::srgb(1.0, 0.16, 0.42);
const COLOR_MAJOR_FROZEN: Color = Color::srgb(0.38, 0.55, 0.72);
const COLOR_STATION: Color = Color::srgb(0.35, 1.0, 0.72);
const COLOR_STATION_DEAD: Color = Color::srgb(0.24, 0.34, 0.30);
const COLOR_GENERATOR: Color = Color::srgb(1.0, 0.86, 0.32);
const COLOR_LANE: Color = Color::srgba(0.45, 0.92, 1.0, 0.45);

fn cell_position(coord: HexCoord) -> Vec2 {
    let (x, z) = hex_origin_plan(coord);
    // Screen y grows upward; lattice r grows south, so z is negated.
    Vec2::new(x as f32 * CELL_SCALE, -(z as f32) * CELL_SCALE)
}

/// Board centre, so the lattice sits in the middle of the window.
fn board_offset(world: &KineticWorld) -> Vec2 {
    let last = HexCoord {
        q: world.grid.cols - 1,
        r: world.grid.rows - 1,
        level: 0,
    };
    -(cell_position(HexCoord::default()) + cell_position(last)) / 2.0
}

fn hex_outline(center: Vec2) -> [Vec2; 6] {
    CORNERS.map(|(x, z)| center + Vec2::new(x as f32 * CELL_SCALE, -(z as f32) * CELL_SCALE))
}

#[derive(Component)]
pub(crate) struct KineticOwned;

#[derive(Component)]
pub(crate) struct KineticUiRoot;

#[derive(Component)]
pub(crate) struct CellTile(pub HexCoord);

#[derive(Component)]
pub(crate) struct ObserverBody(pub PlayerId);

#[derive(Component)]
pub(crate) struct MinorBody(pub MinorGuardianId);

#[derive(Component)]
pub(crate) struct MajorBody;

#[derive(Component)]
pub(crate) struct DebugText;

/// Lab-local state: what the player is asking for, and what the lab is showing.
/// None of this is authoritative — the world owns every rule.
#[derive(Resource, Clone, Debug)]
pub struct KineticRuntime {
    pub pending: KineticIntent,
    pub paused: bool,
    pub step_requested: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    /// Fractional ticks carried between frames so a variable frame rate still
    /// drives a fixed-tick simulation.
    pub tick_accumulator: f32,
    /// The most recent thing worth reading, kept on screen for a moment.
    pub last_note: String,
}

impl Default for KineticRuntime {
    fn default() -> Self {
        Self {
            pending: KineticIntent::Idle,
            paused: false,
            step_requested: false,
            reset_requested: false,
            reset_count: 0,
            tick_accumulator: 0.0,
            last_note: "Face a minor Guardian and push it off something.".to_string(),
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<KineticWorld>) {
    let offset = board_offset(&world);

    commands
        .spawn((
            KineticOwned,
            Name::new("Kinetic Lab Board"),
            Transform::from_translation(offset.extend(0.0)),
            Visibility::default(),
        ))
        .with_children(|board| {
            for index in 0..world.grid.cell_count() {
                let coord = world.grid.coord(index);
                board.spawn((
                    CellTile(coord),
                    Sprite::from_color(COLOR_SOLID, Vec2::splat(12.0 * CELL_SCALE)),
                    Transform::from_translation(cell_position(coord).extend(0.0)),
                    Name::new(format!("Cell {},{}", coord.q, coord.r)),
                ));
            }

            for station in &world.stations {
                board.spawn((
                    Sprite::from_color(COLOR_STATION, Vec2::splat(5.0 * CELL_SCALE)),
                    Transform::from_translation(cell_position(station.cell).extend(1.0)),
                    Name::new("Recharge Station"),
                ));
            }

            board.spawn((
                Sprite::from_color(COLOR_GENERATOR, Vec2::splat(6.0 * CELL_SCALE)),
                Transform::from_translation(cell_position(world.generator).extend(1.0)),
                Name::new("Generator"),
            ));

            for minor in &world.minors {
                board.spawn((
                    MinorBody(minor.id),
                    Sprite::from_color(COLOR_MINOR, Vec2::splat(6.0 * CELL_SCALE)),
                    Transform::from_translation(cell_position(minor.cell).extend(3.0)),
                    Name::new(format!("Minor Guardian {}", minor.id.0)),
                ));
            }

            board.spawn((
                MajorBody,
                Sprite::from_color(COLOR_MAJOR_AWAKE, Vec2::splat(8.0 * CELL_SCALE)),
                Transform::from_translation(cell_position(world.major.cell).extend(3.0)),
                Name::new("Major Guardian"),
            ));

            for observer in &world.observers {
                board.spawn((
                    ObserverBody(observer.id),
                    Sprite::from_color(COLOR_OBSERVER, Vec2::splat(7.0 * CELL_SCALE)),
                    Transform::from_translation(cell_position(observer.cell).extend(4.0)),
                    Name::new(format!("Observer {}", observer.id.0)),
                ));
            }
        });

    commands
        .spawn((
            KineticOwned,
            KineticUiRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                top: Val::Px(16.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.025, 0.035, 0.94)),
            BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.55)),
        ))
        .with_children(|panel| {
            panel.spawn((
                DebugText,
                Text::new("Kinetic diagnostics starting..."),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.86, 0.95, 1.0)),
            ));
            panel.spawn((
                Text::new(help_text()),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.72, 0.80, 0.88)),
            ));
        });
}

/// The six lateral faces, laid out as a hex pad under one hand: `Q`/`E` are the
/// upper diagonals, `A`/`D` the flats, `Z`/`C` the lower diagonals.
///
/// There is deliberately no `W` or `S`: a flat-topped hex has no north or south
/// face. The on-screen help used to claim otherwise, which meant the first two
/// keys anyone presses did nothing at all.
pub(crate) const STEP_KEYS: [(KeyCode, HexFace); 6] = [
    (KeyCode::KeyQ, HexFace::NorthWest),
    (KeyCode::KeyE, HexFace::NorthEast),
    (KeyCode::KeyA, HexFace::West),
    (KeyCode::KeyD, HexFace::East),
    (KeyCode::KeyZ, HexFace::SouthWest),
    (KeyCode::KeyC, HexFace::SouthEast),
];

/// Turn in place, without spending a step.
///
/// `KineticIntent::Face` existed in the model from the beginning and no input
/// path ever produced it, so the only way to aim was to walk into the shot — on
/// a 14 metre lattice, at the cost of a plate of position every time.
pub(crate) const TURN_KEYS: [(KeyCode, i8); 2] =
    [(KeyCode::ArrowLeft, 1), (KeyCode::ArrowRight, -1)];

/// Rotate a lateral face by `steps` sixths. `HexFace::LATERAL` is ordered
/// counterclockwise, so a positive step turns left.
pub(crate) fn rotated(face: HexFace, steps: i8) -> HexFace {
    let index = (face.index() as i8 + steps).rem_euclid(6) as usize;
    HexFace::LATERAL[index]
}

fn key_label(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KeyQ => "Q",
        KeyCode::KeyE => "E",
        KeyCode::KeyA => "A",
        KeyCode::KeyD => "D",
        KeyCode::KeyZ => "Z",
        KeyCode::KeyC => "C",
        KeyCode::ArrowLeft => "LEFT",
        KeyCode::ArrowRight => "RIGHT",
        _ => "?",
    }
}

/// Build the help text *from the binding tables*, so the two cannot drift.
///
/// The previous help was a hand-written string that disagreed with the code on
/// half the gameplay keys. Generating it makes that class of bug impossible
/// rather than merely tested for.
pub(crate) fn help_text() -> String {
    let steps = STEP_KEYS
        .iter()
        .map(|(key, face)| format!("{} {face:?}", key_label(*key)))
        .collect::<Vec<_>>()
        .join("   ");
    let turns = TURN_KEYS
        .iter()
        .map(|(key, steps)| {
            format!(
                "{} turn {}",
                key_label(*key),
                if *steps > 0 { "left" } else { "right" }
            )
        })
        .collect::<Vec<_>>()
        .join("   ");
    format!(
        "step:  {steps}\n\
         {turns}\n\
         SPACE push   |   F pull   |   G operate generator\n\
         P pause   |   N single tick   |   R reset"
    )
}

/// Keyboard to abstract intent. This is the only place a key is read, and it
/// produces a [`KineticIntent`] rather than touching the world.
pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    world: Res<KineticWorld>,
    mut runtime: ResMut<KineticRuntime>,
) {
    let facing = world
        .observers
        .first()
        .map_or(HexFace::East, |observer| observer.facing);

    let mut intent = KineticIntent::Idle;
    for (key, face) in STEP_KEYS {
        if keyboard.just_pressed(key) {
            intent = KineticIntent::Step(face);
        }
    }
    for (key, steps) in TURN_KEYS {
        if keyboard.just_pressed(key) {
            intent = KineticIntent::Face(rotated(facing, steps));
        }
    }
    if keyboard.just_pressed(KeyCode::Space) {
        intent = KineticIntent::Push;
    } else if keyboard.just_pressed(KeyCode::KeyF) {
        intent = KineticIntent::Pull;
    } else if keyboard.just_pressed(KeyCode::KeyG) {
        intent = KineticIntent::ToggleGenerator;
    }
    if intent != KineticIntent::Idle {
        runtime.pending = intent;
    }

    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.paused = !runtime.paused;
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        runtime.step_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
}

pub(crate) fn perform_reset(mut runtime: ResMut<KineticRuntime>, mut world: ResMut<KineticWorld>) {
    if !runtime.reset_requested {
        return;
    }
    let reset_count = runtime.reset_count + 1;
    *runtime = KineticRuntime {
        reset_count,
        ..default()
    };
    *world = KineticWorld::authored();
}

/// Drive the fixed-tick model from real time, or one tick at a time when
/// paused. The model never sees a delta: it only ever advances by whole ticks.
pub(crate) fn simulate(
    time: Res<Time>,
    mut runtime: ResMut<KineticRuntime>,
    mut world: ResMut<KineticWorld>,
) {
    let mut ticks = 0;
    if runtime.paused {
        if runtime.step_requested {
            runtime.step_requested = false;
            ticks = 1;
        }
    } else {
        runtime.step_requested = false;
        let accumulated = runtime.tick_accumulator + time.delta_secs() * TICKS_PER_SECOND as f32;
        let whole = accumulated.floor().max(0.0);
        // Cap catch-up so a stalled frame cannot fast-forward the board, and
        // carry only the fraction: a backlog beyond the cap is dropped rather
        // than replayed next frame, which would defeat the cap entirely.
        ticks = (whole as u32).min(MAX_CATCHUP_TICKS);
        runtime.tick_accumulator = accumulated - whole;
    }

    for tick in 0..ticks {
        // The staged intent belongs to exactly one tick; later ticks in the
        // same frame are idle so a keypress can never fire the tool twice.
        let intent = if tick == 0 {
            std::mem::take(&mut runtime.pending)
        } else {
            KineticIntent::Idle
        };
        let observer = world.observers.first().map(|observer| observer.id);
        let intents: Vec<(PlayerId, KineticIntent)> =
            observer.map(|id| vec![(id, intent)]).unwrap_or_default();
        world.step(&intents);

        if let Some(note) = describe(&world) {
            runtime.last_note = note;
        }
    }
}

/// Turn this tick's events into one line a person can read.
fn describe(world: &KineticWorld) -> Option<String> {
    world.events.iter().find_map(|event| match event {
        KineticEvent::Shoved(resolution) => Some(match resolution.fate {
            ShoveFate::Void => format!(
                "Shoved {} cells into void - the edge killed it, not the tool.",
                resolution.cells_travelled
            ),
            ShoveFate::Doomed => {
                "Shoved onto a retracting tile - it dies when the tile commits.".to_string()
            }
            ShoveFate::Rest => format!(
                "Shoved {} cells onto solid floor. Still alive, staggered.",
                resolution.cells_travelled
            ),
            ShoveFate::Blocked => "Blocked by structure - nothing moved.".to_string(),
        }),
        KineticEvent::GuardianDestroyed { by_retraction, .. } => Some(if *by_retraction {
            "A retracting tile committed and took its passenger.".to_string()
        } else {
            "Minor Guardian committed to void.".to_string()
        }),
        KineticEvent::ToolRefused { refusal, .. } => Some(match refusal {
            ToolRefusal::NoTargetInLane => "Nothing in the lane.".to_string(),
            ToolRefusal::NotEnoughCharge => "Not enough charge.".to_string(),
            ToolRefusal::NotOnGenerator => "Stand on the generator to operate it.".to_string(),
        }),
        KineticEvent::ChargeRestored { charge, .. } => Some(format!(
            "Station restored a charge ({charge}/{MAX_CHARGE})."
        )),
        KineticEvent::GeneratorToggled { powered, .. } => Some(if *powered {
            "Power restored. Stations live, sight returns.".to_string()
        } else {
            "Power cut. Stations dead, sight is your own cell only.".to_string()
        }),
        KineticEvent::ObserverCaptured { by_major, .. } => Some(if *by_major {
            "Captured by the major Guardian.".to_string()
        } else {
            "Captured by a minor Guardian.".to_string()
        }),
        KineticEvent::TileRetracted { .. } => None,
    })
}

// The disjointness filters are what let one system hold all four at once; the
// aliases keep that readable.
type CellQuery<'w, 's> = Query<
    'w,
    's,
    (&'static CellTile, &'static mut Sprite),
    (
        Without<MinorBody>,
        Without<MajorBody>,
        Without<ObserverBody>,
    ),
>;
type MinorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static MinorBody,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (Without<MajorBody>, Without<ObserverBody>),
>;
type MajorQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Sprite),
    (With<MajorBody>, Without<ObserverBody>),
>;
type ObserverQuery<'w, 's> =
    Query<'w, 's, (&'static ObserverBody, &'static mut Transform), With<ObserverBody>>;

#[derive(SystemParam)]
pub(crate) struct BodyQueries<'w, 's> {
    cells: CellQuery<'w, 's>,
    minors: MinorQuery<'w, 's>,
    major: MajorQuery<'w, 's>,
    observers: ObserverQuery<'w, 's>,
}

pub(crate) fn present(world: Res<KineticWorld>, mut bodies: BodyQueries) {
    for (tile, mut sprite) in &mut bodies.cells {
        sprite.color = match world.cell(tile.0) {
            CellKind::Solid => COLOR_SOLID,
            CellKind::Ledge => COLOR_LEDGE,
            CellKind::Void => COLOR_VOID,
            CellKind::Wall => COLOR_WALL,
            // A retracting tile pulses toward void as its countdown runs, so the
            // deadline is legible without reading a number.
            CellKind::Retracting { ticks_remaining } => {
                let remaining = (ticks_remaining as f32 / 180.0).clamp(0.0, 1.0);
                COLOR_VOID.mix(&COLOR_RETRACTING, remaining)
            }
        };
    }

    for (body, mut transform, mut visibility) in &mut bodies.minors {
        let Some(minor) = world.minor(body.0) else {
            continue;
        };
        transform.translation = cell_position(minor.cell).extend(3.0);
        // A destroyed Guardian leaves play; its entity stays pooled so reset
        // never has to respawn the board.
        *visibility = if minor.alive {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    if let Ok((mut transform, mut sprite)) = bodies.major.single_mut() {
        transform.translation = cell_position(world.major.cell).extend(3.0);
        sprite.color = if world.major.frozen {
            COLOR_MAJOR_FROZEN
        } else {
            COLOR_MAJOR_AWAKE
        };
    }

    for (body, mut transform) in &mut bodies.observers {
        if let Some(observer) = world.observer(body.0) {
            transform.translation = cell_position(observer.cell).extend(4.0);
        }
    }
}

/// Outlines, the facing lane, and the live shove preview.
pub(crate) fn draw_debug(world: Res<KineticWorld>, mut gizmos: Gizmos) {
    let offset = board_offset(&world);

    for index in 0..world.grid.cell_count() {
        let coord = world.grid.coord(index);
        let center = offset + cell_position(coord);
        let outline = hex_outline(center);
        let kind = world.cell(coord);
        // Void gets no outline at all: an edge you can be shoved over should
        // read as absence, not as a drawn cell.
        if kind == CellKind::Void {
            continue;
        }
        let color = match kind {
            CellKind::Ledge => Color::srgba(1.0, 0.72, 0.22, 0.85),
            CellKind::Retracting { .. } => Color::srgba(1.0, 0.30, 0.22, 0.9),
            CellKind::Wall => Color::srgba(0.70, 0.76, 0.84, 0.8),
            _ => Color::srgba(0.30, 0.38, 0.46, 0.55),
        };
        for i in 0..6 {
            gizmos.line_2d(outline[i], outline[(i + 1) % 6], color);
        }
    }

    for station in &world.stations {
        gizmos.circle_2d(
            offset + cell_position(station.cell),
            5.0 * CELL_SCALE,
            if world.powered {
                COLOR_STATION
            } else {
                COLOR_STATION_DEAD
            },
        );
    }

    let Some(observer) = world.observers.first() else {
        return;
    };
    let origin = offset + cell_position(observer.cell);

    // The facing lane: what the tool can reach, drawn whether or not something
    // is in it, so an empty lane is visibly empty rather than silently so.
    let mut cursor = observer.cell;
    for _ in 0..crate::model::TOOL_RANGE {
        let Some(next) = world.grid.neighbor(cursor, observer.facing) else {
            break;
        };
        gizmos.line_2d(
            offset + cell_position(cursor),
            offset + cell_position(next),
            COLOR_LANE,
        );
        cursor = next;
    }

    // The preview. `resolve_shove` is pure, so this is the same computation the
    // tick would run — not an estimate of it.
    if let Some(target) = world.target_in_lane(observer)
        && let Some(preview) = world.resolve_shove(target, observer.facing, PUSH_IMPULSE)
    {
        let affordable = observer.charge >= PUSH_COST;
        let color = match preview.fate {
            ShoveFate::Void => Color::srgb(0.30, 1.0, 0.55),
            ShoveFate::Doomed => Color::srgb(1.0, 0.72, 0.22),
            ShoveFate::Rest => Color::srgb(0.55, 0.62, 0.70),
            ShoveFate::Blocked => Color::srgb(1.0, 0.28, 0.24),
        };
        let color = if affordable {
            color
        } else {
            color.with_alpha(0.3)
        };
        let from = offset + cell_position(preview.from);
        let to = offset + cell_position(preview.to);
        gizmos.line_2d(from, to, color);
        gizmos.circle_2d(to, 6.5 * CELL_SCALE, color);
        // A lethal destination gets a second ring: the outcome is never carried
        // by hue alone.
        if matches!(preview.fate, ShoveFate::Void | ShoveFate::Doomed) {
            gizmos.circle_2d(to, 8.0 * CELL_SCALE, color);
        }
    }

    // Facing indicator, so the lane's direction is unambiguous at a glance.
    if let Some(ahead) = world.grid.neighbor(observer.cell, observer.facing) {
        gizmos.line_2d(origin, offset + cell_position(ahead), COLOR_OBSERVER);
    }
}

pub(crate) fn update_debug_text(
    world: Res<KineticWorld>,
    runtime: Res<KineticRuntime>,
    mut text: Query<&mut Text, With<DebugText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let Some(observer) = world.observers.first() else {
        return;
    };
    let target = world
        .target_in_lane(observer)
        .and_then(|id| world.resolve_shove(id, observer.facing, PUSH_IMPULSE));
    let preview = match target {
        Some(resolution) => format!(
            "{:?} after {} cells",
            resolution.fate, resolution.cells_travelled
        ),
        None => "no target in lane".to_string(),
    };

    **text = format!(
        "tick {}   |   {}\n\
         charge {}/{}   |   power {}   |   facing {:?}\n\
         minors alive {}   |   major {}\n\
         lane: {}\n\
         resets {}   |   {}\n\
         {}",
        world.tick,
        if runtime.paused { "PAUSED" } else { "running" },
        observer.charge,
        MAX_CHARGE,
        if world.powered { "ON" } else { "OUT" },
        observer.facing,
        world.living_minors(),
        if world.major.frozen {
            "FROZEN (observed)"
        } else {
            "awake"
        },
        preview,
        runtime.reset_count,
        if observer.jailed { "JAILED" } else { "free" },
        runtime.last_note,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every bound key is distinct and every lateral face is reachable.
    #[test]
    fn the_step_bindings_cover_all_six_faces_exactly_once() {
        let mut faces: Vec<usize> = STEP_KEYS.iter().map(|(_, face)| face.index()).collect();
        faces.sort_unstable();
        faces.dedup();
        assert_eq!(faces.len(), 6, "a face is bound twice or not at all");

        let mut keys: Vec<KeyCode> = STEP_KEYS
            .iter()
            .map(|(key, _)| *key)
            .chain(TURN_KEYS.iter().map(|(key, _)| *key))
            .collect();
        let before = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(before, keys.len(), "a key is bound to two actions");
    }

    /// Regression for the defect behind "none of the keys worked": the
    /// on-screen help advertised `W` and `S` as step keys and called `Z`/`C`
    /// "turn in place", while the code bound neither that way. The help is now
    /// generated from the tables, so this guards the generator.
    #[test]
    fn the_help_text_describes_exactly_the_keys_that_are_bound() {
        let help = help_text();
        for (key, face) in STEP_KEYS {
            let label = key_label(key);
            assert!(
                help.contains(label),
                "help never mentions bound step key {label}"
            );
            assert!(
                help.contains(&format!("{face:?}")),
                "help never mentions face {face:?}"
            );
        }
        for (key, _) in TURN_KEYS {
            assert!(
                help.contains(key_label(key)),
                "help never mentions bound turn key {}",
                key_label(key)
            );
        }
    }

    /// A flat-topped hex has no north or south face, so these two are
    /// deliberately unbound — and the help must not promise them.
    #[test]
    fn w_and_s_are_unbound_and_unadvertised() {
        for key in [KeyCode::KeyW, KeyCode::KeyS] {
            assert!(
                !STEP_KEYS.iter().any(|(bound, _)| *bound == key),
                "{key:?} is bound after all"
            );
        }
        let steps_line = help_text()
            .lines()
            .next()
            .expect("help has a step line")
            .to_string();
        assert!(
            !steps_line.contains(" W ") && !steps_line.contains(" S "),
            "the step line advertises an unbound key: {steps_line}"
        );
    }

    #[test]
    fn turning_six_times_returns_to_the_same_face() {
        for face in HexFace::LATERAL {
            let mut left = face;
            for _ in 0..6 {
                left = rotated(left, 1);
            }
            assert_eq!(left, face);
            assert_eq!(rotated(rotated(face, 1), -1), face);
            assert_ne!(rotated(face, 1), face);
        }
    }
}
