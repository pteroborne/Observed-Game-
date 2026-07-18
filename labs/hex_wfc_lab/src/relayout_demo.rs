//! Phase 93 plan-view relayout/decoherence demonstration.

use std::collections::BTreeSet;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_facility::hex_wfc::{
    HexObservationFrame, HexRelayoutCandidate, HexRelayoutProgress, HexRelayoutWork, HexSpace,
    HexWfcWorld, SolveStep,
};
use observed_hex::HexCoord;
use observed_style::MarkerRole;

use crate::facility_3d::LabViewMode;
use crate::{LabState, SCALE, screen_position};

#[derive(Component)]
struct RelayoutVisual;

#[derive(Component)]
struct RelayoutStatus;

#[derive(Resource, Default)]
pub(crate) struct RelayoutDemo {
    pinned_room_key: Option<u64>,
    work: Option<HexRelayoutWork>,
    candidate: Option<HexRelayoutCandidate>,
    reveal_order: Vec<HexCoord>,
    reveal_cursor: usize,
    last_changed: BTreeSet<HexCoord>,
    attempts: u32,
    pulse: u32,
    dirty: bool,
    message: String,
    base_seed: Option<u64>,
    active: bool,
}

impl RelayoutDemo {
    fn pinned_blueprint<'a>(
        &self,
        world: &'a HexWfcWorld,
    ) -> &'a observed_facility::hex_wfc::StampedBlueprint {
        self.pinned_room_key
            .and_then(|key| {
                world
                    .blueprints
                    .iter()
                    .find(|blueprint| blueprint.generation_key() == key)
            })
            .unwrap_or(&world.blueprints[0])
    }

    fn pinned_cell(&self, world: &HexWfcWorld) -> HexCoord {
        self.pinned_blueprint(world).cells[0]
    }

    fn observation(&self, world: &HexWfcWorld) -> HexObservationFrame {
        let mut frame = HexObservationFrame::default();
        frame.visible_cells.insert(self.pinned_cell(world));
        // Treat spawn as one remaining objective so the lab visibly exercises
        // the same route guard used by the simulation.
        frame.objective_cells.insert(world.config.spawn());
        frame
    }

    fn begin_pulse(&mut self, world: &HexWfcWorld) {
        if self.work.is_some() || self.candidate.is_some() {
            return;
        }
        self.work = Some(world.begin_relayout(&self.observation(world)));
        self.last_changed.clear();
        self.attempts = 0;
        self.message = "pulse started - one deterministic attempt per fixed tick".into();
        self.dirty = true;
    }

    fn cancel(&mut self) {
        self.work = None;
        self.candidate = None;
        self.reveal_order.clear();
        self.reveal_cursor = 0;
        self.last_changed.clear();
        self.attempts = 0;
        self.message = "relayout work cancelled".into();
        self.dirty = true;
    }
}

/// Cross-mode/reset hook: any system replacing `LabState.world` or leaving the
/// relayout view emits this before the next fixed relayout tick.
#[derive(Message, Clone, Copy, Debug, Default)]
pub(crate) struct CancelRelayout;

#[derive(Resource)]
struct RelayoutCapture {
    dir: String,
    timer: f32,
    frame: u32,
    final_wait: u32,
    warmup: u32,
    started: bool,
}

impl RelayoutCapture {
    fn new(dir: String) -> Self {
        Self {
            dir,
            timer: 0.0,
            frame: 0,
            final_wait: 0,
            warmup: 0,
            started: false,
        }
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<RelayoutDemo>()
        .add_message::<CancelRelayout>()
        .add_systems(Startup, setup_status)
        .add_systems(FixedUpdate, advance_relayout.run_if(relayout_mode_active))
        .add_systems(
            Update,
            (
                cancel_requested,
                handle_input,
                rebuild_overlay,
                update_status,
            )
                .chain(),
        );
    if let Ok(path) = std::env::var("OBSERVED2_RELAYOUT_CAPTURE") {
        app.insert_resource(RelayoutCapture::new(path))
            .add_systems(Update, capture_progress.after(rebuild_overlay));
    }
}

pub(crate) fn cancel_requested(
    mut requests: MessageReader<CancelRelayout>,
    mut demo: ResMut<RelayoutDemo>,
) {
    if requests.read().next().is_some() {
        demo.cancel();
        demo.active = false;
        demo.base_seed = None;
    }
}

fn relayout_mode_active(mode: Res<LabViewMode>) -> bool {
    *mode == LabViewMode::Relayout2d
}

fn setup_status(mut commands: Commands) {
    commands.spawn((
        RelayoutStatus,
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.93, 0.96, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(18.0),
            top: Val::Px(14.0),
            max_width: Val::Px(475.0),
            ..default()
        },
    ));
}

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<LabViewMode>,
    mut demo: ResMut<RelayoutDemo>,
    mut lab: ResMut<LabState>,
) {
    if keyboard.just_pressed(KeyCode::F4) && *mode != LabViewMode::Facility3d {
        *mode = if *mode == LabViewMode::Relayout2d {
            LabViewMode::Plan2d
        } else {
            LabViewMode::Relayout2d
        };
        if *mode == LabViewMode::Relayout2d {
            enter_demo(&mut demo, &mut lab);
        } else {
            demo.dirty = true;
        }
    }
    if *mode != LabViewMode::Relayout2d {
        if demo.active {
            demo.cancel();
            demo.active = false;
            demo.base_seed = None;
            lab.overlay = true;
            lab.dirty = true;
        }
        return;
    }
    if demo.base_seed != Some(lab.world.seed) {
        demo.cancel();
        demo.base_seed = Some(lab.world.seed);
    }
    demo.active = true;
    if keyboard.just_pressed(KeyCode::KeyO) && demo.work.is_none() && demo.candidate.is_none() {
        let current_key = demo.pinned_blueprint(&lab.world).generation_key();
        let current = lab
            .world
            .blueprints
            .iter()
            .position(|blueprint| blueprint.generation_key() == current_key)
            .unwrap_or(0);
        let next = (current + 1) % lab.world.blueprints.len();
        demo.pinned_room_key = Some(lab.world.blueprints[next].generation_key());
        demo.message = "observed room changed; whole footprint + thresholds pinned".into();
        demo.last_changed.clear();
        demo.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        demo.begin_pulse(&lab.world);
    }
}

fn enter_demo(demo: &mut RelayoutDemo, lab: &mut LabState) {
    lab.playing = false;
    lab.cursor = lab.trace.len();
    lab.overlay = false;
    // Bound the interactive wait without changing the generated layout.
    lab.world.config.retry_budget = lab.world.config.retry_budget.min(12);
    lab.dirty = true;
    demo.active = true;
    demo.base_seed = Some(lab.world.seed);
    if demo.pinned_room_key.is_none() {
        demo.pinned_room_key = Some(lab.world.blueprints[0].generation_key());
    }
    demo.message = "room observed - O changes pin, P emits decoherence pulse".into();
    demo.dirty = true;
}

fn advance_relayout(mut demo: ResMut<RelayoutDemo>, mut lab: ResMut<LabState>) {
    if let Some(work) = demo.work.take() {
        match lab.world.advance_relayout(work) {
            Ok(HexRelayoutProgress::Pending(next)) => {
                demo.attempts = next.next_attempt();
                demo.message = format!(
                    "collapse attempt {} rejected; resuming next tick",
                    demo.attempts
                );
                demo.work = Some(next);
            }
            Ok(HexRelayoutProgress::Ready(candidate)) => {
                demo.attempts = candidate.attempts;
                demo.reveal_order = candidate.changed_cells.iter().copied().collect();
                demo.reveal_cursor = 0;
                demo.message = if candidate.used_fallback {
                    "retry budget exhausted - port-identical register fallback".into()
                } else {
                    format!(
                        "proposal ready after {} attempts - revealing candidate changes in coordinate order",
                        candidate.attempts
                    )
                };
                demo.candidate = Some(candidate);
            }
            Err(error) => demo.message = format!("relayout rejected: {error:?}"),
        }
        demo.dirty = true;
        return;
    }

    if demo.candidate.is_some() {
        let step = demo.reveal_order.len().div_ceil(36).max(1);
        demo.reveal_cursor = (demo.reveal_cursor + step).min(demo.reveal_order.len());
        demo.dirty = true;
        if demo.reveal_cursor == demo.reveal_order.len() {
            let candidate = demo.candidate.take().expect("candidate exists");
            let changed = candidate.changed_cells.clone();
            let observation = demo.observation(&lab.world);
            match lab.world.commit_relayout(candidate, &observation) {
                Ok(()) => {
                    demo.last_changed = changed;
                    demo.pulse += 1;
                    demo.message = format!(
                        "pulse {} committed - {} cells changed, pinned room held",
                        demo.pulse,
                        demo.last_changed.len()
                    );
                    lab.trace = solved_trace(&lab.world);
                    lab.cursor = lab.trace.len();
                    lab.dirty = true;
                }
                Err(error) => demo.message = format!("commit rejected: {error:?}"),
            }
        }
    }
}

fn solved_trace(world: &HexWfcWorld) -> Vec<SolveStep> {
    let mut trace = vec![SolveStep::AttemptStart { attempt: 0 }];
    for blueprint in &world.blueprints {
        for &coord in &blueprint.cells {
            trace.push(SolveStep::BlueprintCell {
                coord,
                role: blueprint.role,
            });
        }
    }
    for placement in world.placements.values() {
        trace.push(SolveStep::Collapsed {
            coord: placement.coord,
            space: placement.space,
            archetype: placement.archetype,
            doors: placement.doors,
            up: placement.up,
            down: placement.down,
        });
    }
    trace.push(SolveStep::Completed {
        rooms: world.room_count() as u16,
        halls: world
            .placements
            .values()
            .filter(|placement| placement.space == HexSpace::Hall)
            .count() as u16,
    });
    trace
}

fn rebuild_overlay(
    mut commands: Commands,
    mode: Res<LabViewMode>,
    mut demo: ResMut<RelayoutDemo>,
    lab: Res<LabState>,
    visuals: Query<Entity, With<RelayoutVisual>>,
) {
    if !demo.dirty {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }
    if *mode != LabViewMode::Relayout2d {
        demo.dirty = false;
        return;
    }

    let observation = demo.observation(&lab.world);
    let pins = lab
        .world
        .begin_relayout(&observation)
        .pinned_cells()
        .clone();
    let pin_style = observed_style::marker(MarkerRole::Control);
    for coord in pins {
        if coord.level == lab.current_level {
            commands.spawn((
                RelayoutVisual,
                Text2d::new("[P] PIN"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(pin_style.base_color),
                Transform::from_translation(screen_position(coord).extend(4.0)),
            ));
        }
    }

    let revealed = demo
        .candidate
        .as_ref()
        .map(|_| &demo.reveal_order[..demo.reveal_cursor])
        .unwrap_or(&[]);
    let changed = revealed
        .iter()
        .copied()
        .chain(demo.last_changed.iter().copied());
    let change_style = observed_style::marker(MarkerRole::Collapse);
    for coord in changed {
        if coord.level != lab.current_level {
            continue;
        }
        let register = demo
            .candidate
            .as_ref()
            .map_or(lab.world.architecture[&coord], |candidate| {
                candidate.architecture[&coord]
            });
        let palette = observed_style::architecture(register);
        commands.spawn((
            RelayoutVisual,
            Sprite::from_color(
                palette.light_color.with_alpha(0.82),
                Vec2::splat(6.0 * SCALE),
            ),
            Transform::from_translation(screen_position(coord).extend(3.0))
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
            Name::new("changed candidate cell"),
        ));
        commands.spawn((
            RelayoutVisual,
            Text2d::new("[C]"),
            TextFont {
                font_size: 13.0,
                ..default()
            },
            TextColor(change_style.base_color),
            Transform::from_translation(screen_position(coord).extend(4.0)),
        ));
    }
    demo.dirty = false;
}

fn update_status(
    mode: Res<LabViewMode>,
    demo: Res<RelayoutDemo>,
    lab: Res<LabState>,
    mut status: Query<&mut Text, With<RelayoutStatus>>,
) {
    let Ok(mut text) = status.single_mut() else {
        return;
    };
    if *mode != LabViewMode::Relayout2d {
        **text = "F4 - relayout/decoherence demo".into();
        return;
    }
    let observation = demo.observation(&lab.world);
    let (pinned, free) = lab.world.decoherence_yield(&observation);
    **text = format!(
        "RELAYOUT / DECOHERENCE - Arc L Phase 93\nF4 exit | O cycle observed room | P pulse\ngeneration {} | attempt {} | pinned {} | free {}\n{}\n\nLegend: [P] PIN = immutable blueprint/threshold | [C] = changed candidate cell\nChanges reveal in coordinate order; topology commits atomically.",
        lab.world.generation, demo.attempts, pinned, free, demo.message
    );
}

fn capture_progress(
    time: Res<Time>,
    mut capture: ResMut<RelayoutCapture>,
    mut mode: ResMut<LabViewMode>,
    mut demo: ResMut<RelayoutDemo>,
    mut lab: ResMut<LabState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    capture.timer += time.delta_secs();
    if !capture.started {
        std::fs::create_dir_all(&capture.dir).expect("relayout capture dir");
        *mode = LabViewMode::Relayout2d;
        enter_demo(&mut demo, &mut lab);
        capture.started = true;
    }
    capture.warmup += 1;
    if capture.warmup == 30 {
        demo.begin_pulse(&lab.world);
    }
    if capture.frame < 12 && capture.warmup >= 10 && capture.timer >= 0.10 {
        let path = format!("{}/relayout_{:03}.png", capture.dir, capture.frame);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        capture.frame += 1;
        capture.timer = 0.0;
    }
    if demo.pulse > 0 {
        capture.final_wait += 1;
    }
    if capture.final_wait > 120 {
        let manifest = serde_json::json!({
            "lab": "hex_wfc_lab",
            "phase": 93,
            "seed": format!("{:#018x}", lab.world.seed),
            "generation": lab.world.generation,
            "observed_room_generation_key": format!("{:#018x}", demo.pinned_blueprint(&lab.world).generation_key()),
            "changed_cells": demo.last_changed.len(),
            "pinned_cells": lab.world.decoherence_yield(&demo.observation(&lab.world)).0,
            "free_cells": lab.world.decoherence_yield(&demo.observation(&lab.world)).1,
            "attempts": demo.attempts,
            "frames": capture.frame,
            "reveal_order": "deterministic coordinate order",
            "topology_commit": "atomic",
        });
        std::fs::write(
            format!("{}/manifest.json", capture.dir),
            serde_json::to_string_pretty(&manifest).expect("manifest"),
        )
        .expect("write manifest");
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_trace_replays_every_cell() {
        let world = HexWfcWorld::generate(0x9303, crate::LabState::new(0x9303).world.config)
            .expect("world");
        let trace = solved_trace(&world);
        assert_eq!(
            trace
                .iter()
                .filter(|step| matches!(step, SolveStep::Collapsed { .. }))
                .count(),
            world.placements.len()
        );
    }

    #[test]
    fn cancellation_drops_in_flight_work_and_candidate_state() {
        let lab = crate::LabState::new(0x9304);
        let mut demo = RelayoutDemo::default();
        demo.begin_pulse(&lab.world);
        assert!(demo.work.is_some());
        demo.cancel();
        assert!(demo.work.is_none());
        assert!(demo.candidate.is_none());
        assert!(demo.reveal_order.is_empty());
    }

    #[test]
    fn reselecting_the_active_seed_cancels_work_and_candidate_in_the_same_update() {
        let lab = crate::LabState::new(crate::PRESET_SEEDS[0]);
        let mut demo = RelayoutDemo::default();
        demo.begin_pulse(&lab.world);
        demo.candidate = Some(
            lab.world
                .propose_relayout(&demo.observation(&lab.world))
                .expect("candidate"),
        );
        assert!(demo.work.is_some());
        assert!(demo.candidate.is_some());

        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(lab)
            .insert_resource(demo)
            .add_message::<CancelRelayout>()
            .add_systems(
                Update,
                (crate::plan_input::handle, cancel_requested).chain(),
            );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit1);
        app.update();

        let lab = app.world().resource::<crate::LabState>();
        assert_eq!(lab.world.seed, crate::PRESET_SEEDS[0]);
        let demo = app.world().resource::<RelayoutDemo>();
        assert!(demo.work.is_none());
        assert!(demo.candidate.is_none());
        assert!(demo.reveal_order.is_empty());
        assert_eq!(demo.base_seed, None);
    }

    #[test]
    fn optional_room_selection_keeps_the_same_generation_key_across_pulse() {
        let mut lab = crate::LabState::new(0xA11C_E3D0_0000_0008);
        lab.world.config.retry_budget = 1;
        let selected = &lab.world.blueprints[3];
        let selected_key = selected.generation_key();
        let selected_cell = selected.cells[0];
        let demo = RelayoutDemo {
            pinned_room_key: Some(selected_key),
            ..default()
        };
        let observation = demo.observation(&lab.world);
        let candidate = lab.world.propose_relayout(&observation).expect("proposal");
        lab.world
            .commit_relayout(candidate, &observation)
            .expect("commit");
        assert_eq!(
            demo.pinned_blueprint(&lab.world).generation_key(),
            selected_key
        );
        assert_eq!(demo.pinned_cell(&lab.world), selected_cell);
    }
}
