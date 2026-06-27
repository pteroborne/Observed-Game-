use bevy::{ecs::system::SystemParam, prelude::*};
use competitive_facility::model::CompetitiveFacility;
use observed_core::TeamId;

use crate::model::{Profile, Slot, catalog, cosmetic, level_progress};

const LOCAL_TEAM: TeamId = TeamId(0);

fn slot_color(slot: Slot) -> Color {
    match slot {
        Slot::Color => Color::srgb(0.30, 0.78, 0.82),
        Slot::Trail => Color::srgb(1.0, 0.62, 0.25),
        Slot::Badge => Color::srgb(0.72, 0.48, 1.0),
    }
}

const CELL: f32 = 84.0;
const COL_X: f32 = 150.0;
const ROW_Y: f32 = 120.0;
const BAR_W: f32 = 560.0;

#[derive(Component)]
pub(crate) struct ProgOwned;

#[derive(Component)]
pub(crate) struct ProgUiRoot;

#[derive(Component)]
pub(crate) struct CosmeticCell {
    id: u16,
    slot_index: usize,
    in_slot: usize,
}

#[derive(Component)]
pub(crate) struct XpFill;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Default)]
pub struct SaveSlot(pub Option<String>);

#[derive(Resource)]
pub struct ProgRuntime {
    pub selected: usize,
    pub debug_visible: bool,
    pub reset_count: u32,
}

impl Default for ProgRuntime {
    fn default() -> Self {
        Self {
            selected: 0,
            debug_visible: true,
            reset_count: 0,
        }
    }
}

/// Run the proven competitive match to a deterministic finish; return the local
/// team's placement. Note: it takes no profile — progression cannot reach the sim.
pub fn play_local_match() -> Option<u8> {
    let mut m = CompetitiveFacility::authored();
    for _ in 0..10_000 {
        if m.finished {
            break;
        }
        m.advance_round(&[]);
    }
    m.team(LOCAL_TEAM).and_then(|t| t.placement)
}

fn cell_position(slot_index: usize, in_slot: usize) -> Vec3 {
    let x = (in_slot as f32 - 1.5) * COL_X;
    let y = (1.0 - slot_index as f32) * ROW_Y - 30.0;
    Vec3::new(x, y, 1.0)
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            ProgOwned,
            Name::new("Progression Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            let slots = [Slot::Color, Slot::Trail, Slot::Badge];
            for (slot_index, slot) in slots.into_iter().enumerate() {
                for (in_slot, c) in catalog().into_iter().filter(|c| c.slot == slot).enumerate() {
                    parent.spawn((
                        CosmeticCell {
                            id: c.id,
                            slot_index,
                            in_slot,
                        },
                        Name::new(c.name),
                        Sprite::from_color(slot_color(slot), Vec2::splat(CELL)),
                        Transform::from_translation(cell_position(slot_index, in_slot)),
                    ));
                }
            }
            // XP bar: a fixed background and a fill that scales with progress.
            parent.spawn((
                Name::new("XP Bar BG"),
                Sprite::from_color(Color::srgb(0.10, 0.12, 0.16), Vec2::new(BAR_W, 26.0)),
                Transform::from_translation(Vec3::new(0.0, 240.0, 0.5)),
            ));
            parent.spawn((
                XpFill,
                Name::new("XP Bar Fill"),
                Sprite::from_color(Color::srgb(0.4, 1.0, 0.6), Vec2::new(BAR_W, 22.0)),
                Transform::from_translation(Vec3::new(0.0, 240.0, 0.6)),
            ));
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            ProgOwned,
            ProgUiRoot,
            Name::new("Progression UI Root"),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                DebugPanel,
                Node {
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.94)),
                BorderColor::all(Color::srgba(0.5, 1.0, 0.7, 0.6)),
                children![(
                    DebugText,
                    Text::new("Profile starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 1.0, 0.94)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.94)),
                BorderColor::all(Color::srgba(0.5, 1.0, 0.7, 0.6)),
                children![(
                    Text::new(
                        "PROGRESSION & COSMETICS (Phase 18)\n\
                         Enter   Play a match (earn XP from your placement)\n\
                         ← / →   Select a cosmetic\n\
                         E       Equip the selected cosmetic (if unlocked)\n\
                         S / L   Save / load the profile\n\
                         R reset · F1 debug\n\n\
                         A persistence layer for unlocks and cosmetics that NEVER\n\
                         touches the simulation: the match is the proven competitive\n\
                         brain, which takes no profile, so what you unlock or equip\n\
                         cannot change a result or a replay. XP, levels, unlocks and\n\
                         equipped slots serialize to a save string and round-trip.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.95, 0.9)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut profile: ResMut<Profile>,
    mut runtime: ResMut<ProgRuntime>,
    mut save: ResMut<SaveSlot>,
) {
    let count = catalog().len();
    if keyboard.just_pressed(KeyCode::Enter) {
        let placement = play_local_match();
        profile.award_match(placement);
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        runtime.selected = (runtime.selected + 1) % count;
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        runtime.selected = (runtime.selected + count - 1) % count;
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        let id = catalog()[runtime.selected].id;
        profile.equip(id);
    }
    if keyboard.just_pressed(KeyCode::KeyS) {
        save.0 = Some(profile.serialize());
        profile.last_event = "Saved profile.".to_string();
    }
    if keyboard.just_pressed(KeyCode::KeyL) {
        if let Some(text) = &save.0 {
            if let Some(loaded) = Profile::parse(text) {
                *profile = loaded;
                profile.last_event = "Loaded profile.".to_string();
            }
        } else {
            profile.last_event = "No save to load.".to_string();
        }
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        *profile = Profile::new();
        runtime.selected = 0;
        runtime.reset_count += 1;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn present_cells(
    profile: Res<Profile>,
    mut cells: Query<(&CosmeticCell, &mut Sprite, &mut Transform)>,
) {
    for (cell, mut sprite, mut transform) in &mut cells {
        let Some(c) = cosmetic(cell.id) else { continue };
        let base = slot_color(c.slot);
        sprite.color = if profile.is_equipped(cell.id) {
            base.mix(&Color::WHITE, 0.55)
        } else if profile.is_unlocked(cell.id) {
            base
        } else {
            base.mix(&Color::srgb(0.05, 0.05, 0.06), 0.78)
        };
        let equipped = profile.is_equipped(cell.id);
        let size = if equipped { CELL + 8.0 } else { CELL };
        sprite.custom_size = Some(Vec2::splat(size));
        transform.translation = cell_position(cell.slot_index, cell.in_slot);
    }
}

pub(crate) fn present_xp_bar(profile: Res<Profile>, mut fill: Single<&mut Sprite, With<XpFill>>) {
    let (have, need) = level_progress(profile.xp);
    let frac = (have as f32 / need.max(1) as f32).clamp(0.0, 1.0);
    // Scale the fill from the left edge by adjusting size + recentre.
    let width = BAR_W * frac;
    fill.custom_size = Some(Vec2::new(width.max(1.0), 22.0));
}

pub(crate) fn align_xp_bar(profile: Res<Profile>, mut fill: Single<&mut Transform, With<XpFill>>) {
    let (have, need) = level_progress(profile.xp);
    let frac = (have as f32 / need.max(1) as f32).clamp(0.0, 1.0);
    let width = (BAR_W * frac).max(1.0);
    fill.translation.x = -BAR_W * 0.5 + width * 0.5;
}

pub(crate) fn draw_selection(runtime: Res<ProgRuntime>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }
    let cat = catalog();
    let Some(selected) = cat.get(runtime.selected) else {
        return;
    };
    let slot_index = match selected.slot {
        Slot::Color => 0,
        Slot::Trail => 1,
        Slot::Badge => 2,
    };
    let in_slot = cat
        .iter()
        .filter(|c| c.slot == selected.slot)
        .position(|c| c.id == selected.id)
        .unwrap_or(0);
    let pos = cell_position(slot_index, in_slot);
    gizmos.rect_2d(
        Vec2::new(pos.x, pos.y),
        Vec2::splat(CELL + 18.0),
        Color::srgb(1.0, 0.85, 0.3),
    );
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    profile: Res<'w, Profile>,
    runtime: Res<'w, ProgRuntime>,
    save: Res<'w, SaveSlot>,
    cells: Query<'w, 's, (), With<CosmeticCell>>,
    ui_roots: Query<'w, 's, (), With<ProgUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.runtime.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let profile = &*context.profile;
    let (have, need) = level_progress(profile.xp);

    // The save round-trips: serialize, parse, re-serialize must match.
    let round_trips = Profile::parse(&profile.serialize())
        .is_some_and(|loaded| loaded.serialize() == profile.serialize());

    let cells = context.cells.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = cells == catalog().len() && ui_roots == 1 && round_trips;

    let mut equipped = String::new();
    for slot in [Slot::Color, Slot::Trail, Slot::Badge] {
        let name = profile
            .equipped
            .get(&slot)
            .and_then(|id| cosmetic(*id))
            .map(|c| c.name)
            .unwrap_or("—");
        equipped.push_str(&format!("  {:<6} {}\n", slot.label(), name));
    }

    let selected = catalog()[context.runtime.selected];

    let mut text = context.text.into_inner();
    **text = format!(
        "PROGRESSION  {}\n\
         level           {}   xp {}\n\
         next level      {} / {}\n\
         matches {}   wins {}\n\
         unlocked        {} / {}\n\
         equipped:\n{}\
         selected        {} ({})\n\
         save present    {}   round-trips {}\n\
         cells {cells}  UI {ui_roots}   resets {}\n\n\
         save: {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        profile.level(),
        profile.xp,
        have,
        need,
        profile.matches_played,
        profile.wins,
        profile.unlocked.len(),
        catalog().len(),
        equipped,
        selected.name,
        selected.slot.label(),
        if context.save.0.is_some() {
            "yes"
        } else {
            "no"
        },
        if round_trips { "yes" } else { "NO" },
        context.runtime.reset_count,
        context.save.0.as_deref().unwrap_or("(none)"),
        profile.last_event,
    );
}
