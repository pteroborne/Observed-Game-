//! `architect_lab` — the first Rogue Architect playable proof.
//!
//! A human uses the same simulation-owned command boundary as the optional
//! Architect behavior tree. Every other role is autonomous and deterministic.

use crate::{sim, view};

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};

use sim::{ACTOR_BEAT_TICKS, ArchitectLab, ArchitectMode, CommandRefusal, MatchOutcome};

const DEFAULT_MODE: ArchitectMode = ArchitectMode::Pocket;

#[derive(Resource)]
pub struct LabSession {
    pub sim: ArchitectLab,
    pub selected_target: Option<observed_hex::HexCoord>,
    pub hovered_target: Option<observed_hex::HexCoord>,
    pub selected_card: usize,
    pub rotation: u8,
    pub paused: bool,
    pub debug_overlay: bool,
    pub reset_count: u32,
    pub last_message: String,
    pub(crate) dirty: bool,
}

impl Default for LabSession {
    fn default() -> Self {
        let mode = if let Ok(mode_str) = std::env::var("OBSERVED2_MODE") {
            match mode_str.to_lowercase().as_str() {
                "deep_stack" | "deepstack" | "deep" => ArchitectMode::DeepStack,
                "quick_climb" | "quickclimb" | "quick" => ArchitectMode::QuickClimb,
                "full_ascent" | "fullascent" | "full" => ArchitectMode::FullAscent,
                _ => DEFAULT_MODE,
            }
        } else {
            DEFAULT_MODE
        };
        let debug_overlay = std::env::var("OBSERVED2_OVERLAY").is_ok();
        Self {
            sim: ArchitectLab::for_mode(mode).expect("the pinned architect lab mode solves"),
            selected_target: None,
            hovered_target: None,
            selected_card: 0,
            rotation: 0,
            paused: true,
            debug_overlay,
            reset_count: 0,
            last_message: format!(
                "{} ready. Pick a card and help the Guardian hunt.",
                mode.label()
            ),
            dirty: true,
        }
    }
}

impl LabSession {
    pub(crate) fn target(&self) -> Option<observed_hex::HexCoord> {
        self.selected_target
    }

    pub(crate) fn select_target(&mut self, target: observed_hex::HexCoord) -> bool {
        let targets = self.sim.mutable_targets();
        if !targets.contains(&target) {
            return false;
        }
        if self.selected_target != Some(target) {
            self.selected_target = Some(target);
            self.last_message = format!(
                "Target locked: floor {}, cell {}, {}.",
                target.level + 1,
                target.q,
                target.r
            );
            self.dirty = true;
        }
        true
    }

    pub(crate) fn apply_action(&mut self, action: ArchitectAction) {
        match action {
            ArchitectAction::SelectCard(index) => {
                if index < self.sim.deck.hand.len() {
                    self.selected_card = index;
                    self.last_message =
                        format!("Card {} armed. Choose a target and orientation.", index + 1);
                }
            }
            ArchitectAction::Rotate(delta) => {
                self.rotation = (i16::from(self.rotation) + i16::from(delta)).rem_euclid(6) as u8;
                self.last_message = format!("Preview rotated to face {}.", self.rotation + 1);
            }
            ArchitectAction::Submit => self.submit_selected(),
            ArchitectAction::ToggleBot => {
                self.sim.bot_architect = !self.sim.bot_architect;
                self.last_message = if self.sim.bot_architect {
                    "Bot Architect enabled; it submits through this same command surface."
                        .to_string()
                } else {
                    "Human Architect enabled; the mutation console is yours.".to_string()
                };
            }
            ArchitectAction::TogglePause => {
                self.paused = !self.paused;
                self.last_message = if self.paused {
                    "Timeline held. Step advances one full behavior-tree beat.".to_string()
                } else {
                    "Timeline live at 60 fixed ticks per second.".to_string()
                };
            }
            ArchitectAction::StepBeat => {
                self.sim.step_beat();
                self.last_message = "Advanced one behavior-tree beat.".to_string();
            }
            ArchitectAction::Reset => self.reset(),
            ArchitectAction::CycleMode(direction) => self.cycle_mode(direction),
            ArchitectAction::ToggleOverlay => {
                self.debug_overlay = !self.debug_overlay;
                self.last_message = if self.debug_overlay {
                    "Debug overlay active: telegraph countdown, fall safety, and floor power visible."
                        .to_string()
                } else {
                    "Debug overlay hidden.".to_string()
                };
            }
        }
        self.dirty = true;
    }

    fn submit_selected(&mut self) {
        if self.sim.bot_architect {
            self.last_message =
                "Autopilot owns the command boundary. Disable BOT to play this card.".to_string();
            return;
        }
        let result = self
            .target()
            .and_then(|target| {
                self.sim
                    .selected_command(self.selected_card, target, self.rotation)
            })
            .ok_or(CommandRefusal::UnknownTarget)
            .and_then(|command| self.sim.submit(command));
        self.last_message = match result {
            Ok(()) => "Mutation committed. Card refilled; cooldown cycling.".to_string(),
            Err(refusal) => format!("Command held: {}.", refusal.label()),
        };
        self.selected_card = self
            .selected_card
            .min(self.sim.deck.hand.len().saturating_sub(1));
    }

    pub(crate) fn reset(&mut self) {
        let bot_architect = self.sim.bot_architect;
        let mode = self.sim.mode;
        let loyal_team_size = self.sim.loyal_team_size;
        self.sim = ArchitectLab::for_mode_with_team_size(mode, loyal_team_size)
            .expect("the pinned architect lab mode solves");
        self.sim.bot_architect = bot_architect;
        self.selected_target = None;
        self.hovered_target = None;
        self.selected_card = 0;
        self.rotation = 0;
        self.paused = true;
        self.reset_count += 1;
        self.last_message = format!(
            "Reset: {} restored its seed, hand, actors, and facility.",
            mode.label()
        );
        self.dirty = true;
    }

    fn cycle_mode(&mut self, direction: i8) {
        let bot_architect = self.sim.bot_architect;
        let loyal_team_size = self.sim.loyal_team_size;
        let mode = if direction < 0 {
            self.sim.mode.previous()
        } else {
            self.sim.mode.next()
        };
        self.sim = ArchitectLab::for_mode_with_team_size(mode, loyal_team_size)
            .expect("the pinned architect lab mode solves");
        self.sim.bot_architect = bot_architect;
        self.selected_target = None;
        self.hovered_target = None;
        self.selected_card = 0;
        self.rotation = 0;
        self.paused = true;
        self.last_message = format!("{} loaded. {}", mode.label(), mode.description());
        self.dirty = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArchitectAction {
    SelectCard(usize),
    Rotate(i8),
    Submit,
    ToggleBot,
    TogglePause,
    StepBeat,
    Reset,
    CycleMode(i8),
    ToggleOverlay,
}

pub struct ArchitectLabPlugin;

impl Plugin for ArchitectLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LabSession>()
            .init_resource::<view::MapCameraState>()
            .insert_resource(Time::<Fixed>::from_hz(f64::from(sim::FIXED_HZ)))
            .add_systems(Startup, view::setup)
            .add_systems(FixedUpdate, fixed_tick)
            .add_systems(
                Update,
                (
                    handle_input,
                    view::handle_ui_actions,
                    view::camera_controls,
                    view::map_pointer_input,
                    view::focus_selected_tile,
                    view::sync_camera_viewport,
                    view::sync_layout,
                    view::sync_dynamic_text,
                    view::sync_card_text,
                    view::sync_card_buttons,
                    view::sync_previews,
                    view::sync_charge_pips,
                    view::sync_action_buttons,
                    view::rebuild_board,
                )
                    .chain()
                    .after(InputSystems),
            );
    }
}

fn fixed_tick(mut session: ResMut<LabSession>) {
    if session.paused || session.sim.outcome != MatchOutcome::Running {
        return;
    }
    let before = session.sim.tick;
    session.sim.tick();
    if before / u64::from(ACTOR_BEAT_TICKS) != session.sim.tick / u64::from(ACTOR_BEAT_TICKS) {
        session.dirty = true;
    }
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<LabSession>,
    mut camera: ResMut<view::MapCameraState>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        session.apply_action(ArchitectAction::Reset);
        camera.reset_for_mode(session.sim.mode);
        return;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        session.apply_action(ArchitectAction::ToggleBot);
    }
    if keys.just_pressed(KeyCode::KeyP) {
        session.apply_action(ArchitectAction::TogglePause);
    }
    if keys.just_pressed(KeyCode::KeyN) {
        session.apply_action(ArchitectAction::StepBeat);
    }
    if keys.just_pressed(KeyCode::KeyO) {
        session.apply_action(ArchitectAction::ToggleOverlay);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        session.apply_action(ArchitectAction::CycleMode(-1));
        camera.reset_for_mode(session.sim.mode);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        session.apply_action(ArchitectAction::CycleMode(1));
        camera.reset_for_mode(session.sim.mode);
    }
    if keys.just_pressed(KeyCode::Tab) {
        let targets: Vec<_> = session
            .sim
            .mutable_targets()
            .into_iter()
            .filter(|c| c.level == camera.floor)
            .collect();
        let next = session
            .target()
            .and_then(|c| targets.iter().position(|t| *t == c))
            .map_or(0, |i| (i + 1) % targets.len().max(1));
        if let Some(&target) = targets.get(next) {
            session.select_target(target);
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        if camera.lab_controls || camera.details {
            camera.lab_controls = false;
            camera.details = false;
        } else {
            session.selected_target = None;
            session.dirty = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyL) {
        camera.lab_controls = !camera.lab_controls;
        camera.details = false;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        camera.details = !camera.details;
        camera.lab_controls = false;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        camera.overview = !camera.overview;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        camera.change_floor(1, session.sim.world.config.levels);
    }
    if keys.just_pressed(KeyCode::PageDown) {
        camera.change_floor(-1, session.sim.world.config.levels);
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        session.apply_action(ArchitectAction::Rotate(-1));
    }
    if keys.just_pressed(KeyCode::KeyE) {
        session.apply_action(ArchitectAction::Rotate(1));
    }
    for (key, index) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
    ] {
        if keys.just_pressed(key) {
            session.apply_action(ArchitectAction::SelectCard(index));
        }
    }
    if keys.just_pressed(KeyCode::Space) && view::ui::can_submit(&session, &camera) {
        session.apply_action(ArchitectAction::Submit);
    }
    if keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::Home) {
        camera.center();
    }
    if keys.just_pressed(KeyCode::Equal) {
        camera.zoom_centered(0.86);
    }
    if keys.just_pressed(KeyCode::Minus) {
        camera.zoom_centered(1.16);
    }
}

pub fn run() {
    let (width, height) = std::env::var("OBSERVED2_CAPTURE_SIZE")
        .ok()
        .and_then(|size| {
            let (w, h) = size.split_once('x')?;
            Some((
                w.parse::<u32>().ok()?.max(1200),
                h.parse::<u32>().ok()?.max(800),
            ))
        })
        .unwrap_or((1600, 1000));
    let mut app = App::new();
    app.insert_resource(ClearColor(observed_style::architect::color(
        observed_style::architect::Role::Background,
    )))
    .add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Observed 2 — Rogue Architect Lab".to_string(),
            resolution: WindowResolution::new(width, height),
            present_mode: PresentMode::AutoVsync,
            resizable: true,
            resize_constraints: WindowResizeConstraints {
                min_width: 1200.0,
                min_height: 800.0,
                ..default()
            },
            ..default()
        }),
        ..default()
    }))
    .add_plugins(ArchitectLabPlugin);
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest {
            path,
            phase: 0,
            frames: 0,
        })
        .add_systems(Update, capture_progress.after(view::rebuild_board));
    }
    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
    frames: u32,
}

fn capture_progress(
    mut request: ResMut<CaptureRequest>,
    mut session: ResMut<LabSession>,
    mut camera: ResMut<view::MapCameraState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    request.frames += 1;
    if request.phase == 0 {
        if let Ok(mode_str) = std::env::var("OBSERVED2_MODE") {
            let target_mode = match mode_str.to_lowercase().as_str() {
                "deep_stack" | "deepstack" | "deep" => Some(ArchitectMode::DeepStack),
                "quick_climb" | "quickclimb" | "quick" => Some(ArchitectMode::QuickClimb),
                "full_ascent" | "fullascent" | "full" => Some(ArchitectMode::FullAscent),
                "pocket" => Some(ArchitectMode::Pocket),
                _ => None,
            };
            if let Some(mode) = target_mode {
                session.sim = ArchitectLab::for_mode(mode).expect("capture mode solves");
                camera.reset_for_mode(mode);
            }
        }
        if std::env::var("OBSERVED2_OVERLAY").is_ok() {
            session.debug_overlay = true;
        }

        // Capture an actionable opening, before autonomous play can resolve it.
        session.paused = true;
        if std::env::var("OBSERVED2_CAPTURE_IDLE").is_err()
            && let Some(command) = session.sim.legal_commands().into_iter().next()
        {
            let sim::ArchitectCommand::Play {
                card,
                target,
                rotation,
            } = command
            else {
                return;
            };
            if let Some(index) = session
                .sim
                .deck
                .hand
                .iter()
                .position(|held| held.id == card)
            {
                session.selected_card = index;
            }
            let _ = session.select_target(target);
            camera.floor = target.level;
            session.rotation = rotation;
        }
        if let Ok(floor) = std::env::var("OBSERVED2_FLOOR")
            && let Ok(floor) = floor.parse::<u8>()
        {
            camera.floor = floor.min(session.sim.world.config.levels - 1);
        }
        camera.details = std::env::var("OBSERVED2_DETAILS").is_ok();
        camera.overview = std::env::var("OBSERVED2_CONTEXT").is_ok();
        camera.lab_controls = std::env::var("OBSERVED2_LAB_CONTROLS").is_ok();
        session.paused = true;
        session.last_message = "Inspect the amber preview, then play the card.".to_string();
        session.dirty = true;
        request.phase = 1;
    } else if request.phase == 1 && request.frames >= 90 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && request.frames >= 120 {
        exit.write(AppExit::Success);
        request.phase = 3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, input::InputPlugin};

    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), InputPlugin))
            .init_asset::<Mesh>()
            .init_asset::<ColorMaterial>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .add_plugins(ArchitectLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn live_entity_count(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query::<Entity>();
        query.iter(world).count()
    }

    fn named_entity(app: &mut App, expected: &str) -> Entity {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &Name)>();
        query
            .iter(world)
            .find(|(_, name)| name.as_str() == expected)
            .map(|(entity, _)| entity)
            .unwrap_or_else(|| panic!("headless interface should contain {expected}"))
    }

    #[test]
    fn reset_restores_the_lab_without_changing_control_mode() {
        let mut session = LabSession::default();
        session.sim.bot_architect = true;
        session.sim.step_beat();
        session.selected_target = Some(observed_hex::HexCoord {
            q: 2,
            r: 2,
            level: 0,
        });
        session.reset();
        assert_eq!(session.sim.tick, 0);
        assert!(session.sim.bot_architect);
        assert_eq!(session.selected_target, None);
        assert_eq!(session.reset_count, 1);
        assert_eq!(
            session.sim.deck.hand,
            ArchitectLab::for_mode(DEFAULT_MODE).unwrap().deck.hand
        );
    }

    #[test]
    fn headless_bevy_boot_builds_the_complete_interface_without_a_window() {
        let mut app = headless_app();
        assert_eq!(count::<Window>(&mut app), 0);
        assert_eq!(count::<Camera>(&mut app), 7);
        assert_eq!(count::<view::BoardCamera>(&mut app), 1);
        assert_eq!(count::<view::ui::InterfaceRoot>(&mut app), 1);
        assert_eq!(count::<view::ui::CardButton>(&mut app), sim::HAND_SIZE);
        assert_eq!(count::<view::ui::ChargePip>(&mut app), sim::HAND_SIZE);
        assert_eq!(count::<ImageNode>(&mut app), sim::HAND_SIZE);
        let solid_cells = app
            .world()
            .resource::<LabSession>()
            .sim
            .world
            .placements
            .values()
            .filter(|placement| placement.space != observed_facility::hex_wfc::HexSpace::Void)
            .count();
        assert!(count::<view::BoardVisual>(&mut app) > solid_cells);
    }

    #[test]
    fn placement_controls_are_contextual_and_details_are_exclusive() {
        let mut app = headless_app();
        fn display<T: Component>(app: &mut App) -> Display {
            let world = app.world_mut();
            world
                .query_filtered::<&Node, With<T>>()
                .single(world)
                .unwrap()
                .display
        }
        assert_eq!(display::<view::ui::Inspector>(&mut app), Display::None);
        assert_eq!(display::<view::ui::Sidebar>(&mut app), Display::None);
        assert_eq!(display::<view::ui::LabControls>(&mut app), Display::None);
        let target = app.world().resource::<LabSession>().sim.mutable_targets()[0];
        app.world_mut()
            .resource_mut::<LabSession>()
            .select_target(target);
        app.world_mut().resource_mut::<view::MapCameraState>().floor = target.level;
        app.update();
        assert_eq!(display::<view::ui::Inspector>(&mut app), Display::Flex);
        let details = named_entity(&mut app, "Architect control DETAILS");
        app.world_mut()
            .entity_mut(details)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(display::<view::ui::Sidebar>(&mut app), Display::Flex);
        assert_eq!(display::<view::ui::Inspector>(&mut app), Display::None);
        let lab = named_entity(&mut app, "Architect control LAB");
        app.world_mut()
            .entity_mut(details)
            .insert(Interaction::None);
        app.world_mut().entity_mut(lab).insert(Interaction::Pressed);
        app.update();
        assert_eq!(display::<view::ui::LabControls>(&mut app), Display::Flex);
        assert_eq!(display::<view::ui::Sidebar>(&mut app), Display::None);
        assert_eq!(display::<view::ui::Inspector>(&mut app), Display::None);
    }

    #[test]
    fn inspector_play_commits_once_and_refuses_cooldown_and_hidden_floor() {
        let mut app = headless_app();
        let command = app.world().resource::<LabSession>().sim.legal_commands()[0];
        let sim::ArchitectCommand::Play {
            card,
            target,
            rotation,
        } = command
        else {
            panic!("play expected")
        };
        {
            let mut session = app.world_mut().resource_mut::<LabSession>();
            session.selected_card = session
                .sim
                .deck
                .hand
                .iter()
                .position(|c| c.id == card)
                .unwrap();
            session.rotation = rotation;
            session.select_target(target);
        }
        app.world_mut().resource_mut::<view::MapCameraState>().floor = target.level + 1;
        let play = named_entity(&mut app, "Execute selected card");
        app.world_mut()
            .entity_mut(play)
            .insert(Interaction::Pressed);
        app.update();
        assert!(
            app.world()
                .resource::<LabSession>()
                .sim
                .command_log
                .is_empty()
        );
        app.world_mut().entity_mut(play).insert(Interaction::None);
        app.update();
        app.world_mut().resource_mut::<view::MapCameraState>().floor = target.level;
        app.world_mut()
            .entity_mut(play)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world().resource::<LabSession>().sim.command_log.len(),
            1
        );
        app.world_mut().entity_mut(play).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(play)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world().resource::<LabSession>().sim.command_log.len(),
            1
        );
        assert_eq!(
            app.world().resource::<LabSession>().sim.deck.hand.len(),
            sim::HAND_SIZE
        );
    }

    #[test]
    fn headless_ui_selection_and_repeated_reset_are_leak_free() {
        let mut app = headless_app();
        let baseline_board = count::<view::BoardVisual>(&mut app);
        let baseline_entities = live_entity_count(&mut app);

        let card_four = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &view::ui::CardButton)>();
            query
                .iter(world)
                .find(|(_, card)| card.0 == 3)
                .map(|(entity, _)| entity)
                .expect("the fourth physical card exists")
        };
        app.world_mut()
            .entity_mut(card_four)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(app.world().resource::<LabSession>().selected_card, 3);

        let reset = named_entity(&mut app, "Architect control RESET [R]");
        for expected_reset in 1..=4 {
            app.world_mut().entity_mut(reset).insert(Interaction::None);
            app.update();
            app.world_mut()
                .entity_mut(reset)
                .insert(Interaction::Pressed);
            app.update();

            let session = app.world().resource::<LabSession>();
            assert_eq!(session.reset_count, expected_reset);
            assert_eq!(session.sim.tick, 0);
            assert_eq!(session.selected_card, 0);
            assert_eq!(count::<view::BoardVisual>(&mut app), baseline_board);
            assert_eq!(count::<view::ui::CardButton>(&mut app), sim::HAND_SIZE);
            assert_eq!(count::<view::ui::InterfaceRoot>(&mut app), 1);
            assert_eq!(count::<view::BoardCamera>(&mut app), 1);
            assert_eq!(live_entity_count(&mut app), baseline_entities);
        }

        let next_mode = named_entity(&mut app, "Architect control ] >");
        for expected_mode in [
            ArchitectMode::QuickClimb,
            ArchitectMode::FullAscent,
            ArchitectMode::DeepStack,
            ArchitectMode::Pocket,
        ] {
            app.world_mut()
                .entity_mut(next_mode)
                .insert(Interaction::None);
            app.update();
            app.world_mut()
                .entity_mut(next_mode)
                .insert(Interaction::Pressed);
            app.update();
            assert_eq!(app.world().resource::<LabSession>().sim.mode, expected_mode);
        }
        assert_eq!(live_entity_count(&mut app), baseline_entities);
    }
}
