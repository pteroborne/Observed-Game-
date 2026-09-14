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
    pub selected_target: usize,
    pub hovered_target: Option<observed_hex::HexCoord>,
    pub selected_card: usize,
    pub rotation: u8,
    pub paused: bool,
    pub reset_count: u32,
    pub last_message: String,
    pub(crate) dirty: bool,
}

impl Default for LabSession {
    fn default() -> Self {
        Self {
            sim: ArchitectLab::for_mode(DEFAULT_MODE)
                .expect("the pinned architect lab mode solves"),
            selected_target: 0,
            hovered_target: None,
            selected_card: 0,
            rotation: 0,
            paused: false,
            reset_count: 0,
            last_message: "Pocket Pursuit ready. Pick a card and help the Guardian hunt."
                .to_string(),
            dirty: true,
        }
    }
}

impl LabSession {
    pub(crate) fn target(&self) -> Option<observed_hex::HexCoord> {
        let targets = self.sim.mutable_targets();
        targets
            .get(self.selected_target % targets.len().max(1))
            .copied()
    }

    pub(crate) fn select_target(&mut self, target: observed_hex::HexCoord) -> bool {
        let targets = self.sim.mutable_targets();
        let Some(index) = targets.iter().position(|candidate| *candidate == target) else {
            return false;
        };
        if self.selected_target != index {
            self.selected_target = index;
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
            ArchitectAction::NextTarget => {
                let count = self.sim.mutable_targets().len().max(1);
                self.selected_target = (self.selected_target + 1) % count;
                self.last_message = "Advanced to the next mutable target.".to_string();
            }
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

    fn reset(&mut self) {
        let bot_architect = self.sim.bot_architect;
        let mode = self.sim.mode;
        self.sim = ArchitectLab::for_mode(mode).expect("the pinned architect lab mode solves");
        self.sim.bot_architect = bot_architect;
        self.selected_target = 0;
        self.hovered_target = None;
        self.selected_card = 0;
        self.rotation = 0;
        self.paused = false;
        self.reset_count += 1;
        self.last_message = format!(
            "Reset: {} restored its seed, hand, actors, and facility.",
            mode.label()
        );
        self.dirty = true;
    }

    fn cycle_mode(&mut self, direction: i8) {
        let bot_architect = self.sim.bot_architect;
        let mode = if direction < 0 {
            self.sim.mode.previous()
        } else {
            self.sim.mode.next()
        };
        self.sim = ArchitectLab::for_mode(mode).expect("the pinned architect lab mode solves");
        self.sim.bot_architect = bot_architect;
        self.selected_target = 0;
        self.hovered_target = None;
        self.selected_card = 0;
        self.rotation = 0;
        self.paused = false;
        self.last_message = format!("{} loaded. {}", mode.label(), mode.description());
        self.dirty = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArchitectAction {
    NextTarget,
    SelectCard(usize),
    Rotate(i8),
    Submit,
    ToggleBot,
    TogglePause,
    StepBeat,
    Reset,
    CycleMode(i8),
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
                    view::sync_camera_viewport,
                    handle_input,
                    view::handle_ui_actions,
                    view::camera_controls,
                    view::map_pointer_input,
                    view::sync_layout,
                    view::sync_dynamic_text,
                    view::sync_card_text,
                    view::sync_card_buttons,
                    view::sync_card_accents,
                    view::sync_card_art,
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
    if keys.just_pressed(KeyCode::BracketLeft) {
        session.apply_action(ArchitectAction::CycleMode(-1));
        camera.reset_for_mode(session.sim.mode);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        session.apply_action(ArchitectAction::CycleMode(1));
        camera.reset_for_mode(session.sim.mode);
    }
    if keys.just_pressed(KeyCode::Tab) {
        session.apply_action(ArchitectAction::NextTarget);
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
    if keys.just_pressed(KeyCode::Space) {
        session.apply_action(ArchitectAction::Submit);
    }
    if keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::Home) {
        camera.reset_for_mode(session.sim.mode);
    }
    if keys.just_pressed(KeyCode::Equal) {
        camera.zoom_centered(0.86);
    }
    if keys.just_pressed(KeyCode::Minus) {
        camera.zoom_centered(1.16);
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(observed_style::schematic_screen()))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Rogue Architect Lab".to_string(),
                resolution: WindowResolution::new(1600, 1000),
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
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress.after(view::rebuild_board));
    }
    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut session: ResMut<LabSession>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if request.phase == 0 {
        // Let every autonomous role establish a readable trace, then return the
        // console to the human with a legal card/target preview ready to commit.
        session.sim.bot_architect = true;
        session.sim.step_beat();
        session.sim.bot_architect = false;
        for _ in 0..5 {
            session.sim.step_beat();
        }
        if let Some(command) = session.sim.legal_commands().into_iter().next() {
            let sim::ArchitectCommand::Play {
                card,
                target,
                rotation,
            } = command;
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
            session.rotation = rotation;
        }
        session.paused = true;
        session.last_message =
            "Target solution ready. Press EXECUTE to commit the highlighted topology.".to_string();
        session.dirty = true;
        request.phase = 1;
    } else if request.phase == 1 && time.elapsed_secs() >= 0.75 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && time.elapsed_secs() >= 1.5 {
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
        session.selected_target = 9;
        session.reset();
        assert_eq!(session.sim.tick, 0);
        assert!(session.sim.bot_architect);
        assert_eq!(session.selected_target, 0);
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
        assert_eq!(count::<Camera>(&mut app), 2);
        assert_eq!(count::<view::BoardCamera>(&mut app), 1);
        assert_eq!(count::<view::ui::InterfaceRoot>(&mut app), 1);
        assert_eq!(count::<view::ui::CardButton>(&mut app), sim::HAND_SIZE);
        assert_eq!(count::<view::ui::ChargePip>(&mut app), sim::HAND_SIZE);
        assert_eq!(count::<view::ui::CardAccent>(&mut app), sim::HAND_SIZE * 2);
        assert_eq!(
            count::<view::ui::CardArtFrame>(&mut app),
            sim::HAND_SIZE * 6
        );
        assert_eq!(
            count::<view::ui::CardArtMotif>(&mut app),
            sim::HAND_SIZE * 12
        );
        let hand = app.world().resource::<LabSession>().sim.deck.hand.clone();
        let visible_motifs = {
            let world = app.world_mut();
            let mut query = world.query::<(&view::ui::CardArtMotif, &Visibility)>();
            let mut counts = [0; sim::HAND_SIZE];
            for (motif, visibility) in query.iter(world) {
                if *visibility != Visibility::Hidden {
                    counts[motif.index] += 1;
                }
            }
            counts
        };
        for (index, card) in hand.into_iter().enumerate() {
            let expected = match (card.kind, card.district) {
                (sim::CardKind::Door, _) => 3,
                (_, Some(sim::District::Institutional)) => 3,
                (_, Some(sim::District::LiminalGrid)) => 6,
                _ => 0,
            };
            assert_eq!(visible_motifs[index], expected);
        }
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
