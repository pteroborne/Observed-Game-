//! World-space diegetic event feedback for the hex match.
//!
//! This used to be a screen-space `EventBanner` — text pinned near the top of
//! the screen reading e.g. "X ROOMS MUTATED", regardless of where "X" was. A
//! banner like that answers "something happened"; it cannot answer *where*,
//! and the project's standing rule is that a player should be able to see and
//! hear any event through diegetic means (`agents.md`; the Phase 50 immersion
//! ruling; the 2026-06-27 comfort pass that replaced decohere shake/flash with
//! diegetic cues). Every event that carries a place now answers with one: a
//! marker-role-tinted ring blooms at the cell where the event happened, then
//! fades. `audio.rs` answers the same question with a spatialized sound from
//! the same place.
//!
//! No new lights are spawned here — a production facility already spawns
//! ~109k mesh entities (backlog #10) and per-event lights are exactly the kind
//! of cost that does not stay bounded. A beacon is one small transient mesh
//! entity, sharing one mesh and one material per marker role, animated by
//! scale alone.

use bevy::prelude::*;
use observed_facility::hex_wfc::HexRelayoutDelta;
use observed_hex::{HexCoord, hex_origin};
use observed_match::hex_wfc::{HexMatchEvent, HexMatchEventKind};
use observed_style::MarkerRole;

use super::cues::cue_for;
use super::sim::HexWfcRuntime;
use crate::GameState;

/// How long a beacon lives, in real seconds. Long enough to notice and orient
/// toward, short enough that a burst of nearby events (e.g. a team holding a
/// station) stays legible rather than smearing into one glow.
const BEACON_LIFETIME: f32 = 1.1;
/// The floor and peak of the pulse's uniform scale.
const BEACON_MIN_SCALE: f32 = 0.05;
const BEACON_PEAK_SCALE: f32 = 2.2;
/// A committed relayout can touch many cells at once (`HexRelayoutDelta::changed_cells`);
/// this bounds how many beacons one event spawns so a large mutation cannot spike the
/// transient-entity count. The mutation is still perceptible — just at up to this many of
/// its cells, not all of them.
const MAX_BEACON_CELLS: usize = 6;

#[derive(Component)]
pub(super) struct EventBeacon {
    age: f32,
}

#[derive(Resource)]
pub(super) struct FeedbackAssets {
    mesh: Handle<Mesh>,
    next_room: Handle<StandardMaterial>,
    exit: Handle<StandardMaterial>,
    control: Handle<StandardMaterial>,
    collapse: Handle<StandardMaterial>,
    you: Handle<StandardMaterial>,
    teammate: Handle<StandardMaterial>,
    rival: Handle<StandardMaterial>,
    director: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(super) struct FeedbackState {
    last_tick: u64,
}

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Torus::new(0.45, 0.6));
    commands.insert_resource(FeedbackAssets {
        mesh,
        next_room: beacon_material(&mut materials, MarkerRole::NextRoom),
        exit: beacon_material(&mut materials, MarkerRole::Exit),
        control: beacon_material(&mut materials, MarkerRole::Control),
        collapse: beacon_material(&mut materials, MarkerRole::Collapse),
        you: beacon_material(&mut materials, MarkerRole::You),
        teammate: beacon_material(&mut materials, MarkerRole::Teammate),
        rival: beacon_material(&mut materials, MarkerRole::Rival),
        director: beacon_material(&mut materials, MarkerRole::Director),
    });
    commands.insert_resource(FeedbackState::default());
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<FeedbackAssets>();
    commands.remove_resource::<FeedbackState>();
}

/// Spawn a beacon for every event this tick that has a place to spawn one at.
/// Beacon entities carry `DespawnOnExit`, so leaving the match cleans them up
/// even mid-pulse; `animate` below is what makes an individual one disappear
/// on schedule.
pub(super) fn sync(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    assets: Res<FeedbackAssets>,
    mut state: ResMut<FeedbackState>,
) {
    let tick = runtime.match_state.tick;
    if state.last_tick == tick {
        return;
    }
    state.last_tick = tick;
    let delta = runtime.match_state.last_relayout_delta.as_ref();
    for event in &runtime.match_state.recent_events {
        let definition = cue_for(event.kind);
        let material = material_for(&assets, definition.marker);
        for cell in event_cells(event, delta) {
            spawn_beacon(&mut commands, &assets, material.clone(), cell);
        }
    }
}

pub(super) fn animate(
    mut commands: Commands,
    time: Res<Time>,
    mut beacons: Query<(Entity, &mut EventBeacon, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut beacon, mut transform) in &mut beacons {
        beacon.age += dt;
        if beacon.age >= BEACON_LIFETIME {
            commands.entity(entity).despawn();
            continue;
        }
        let t = (beacon.age / BEACON_LIFETIME).clamp(0.0, 1.0);
        // One smooth grow-then-fade arc: zero at spawn, peak at the midpoint, zero again
        // just before despawn. No alpha blending needed, so the material stays opaque and
        // cheap.
        let pulse = (t * std::f32::consts::PI).sin().max(0.0);
        transform.scale = Vec3::splat(BEACON_MIN_SCALE + pulse * BEACON_PEAK_SCALE);
    }
}

/// Where a beat should be perceived, if the simulation knows yet.
///
/// Most events already carry `event.cell` (`crates/observed_match/src/hex_wfc/model.rs`).
/// `MutationCommitted` is the one exception with a knowable place: the relayout state
/// machine does not have final cells until the commit has already happened, so the event
/// itself carries none — but `HexWfcMatch::last_relayout_delta` is set in the same tick,
/// and its `changed_cells` is exactly the set this event needs. `MutationWarning` and
/// `MutationCancelled` fire before or without a commit, when no delta exists yet: there is
/// genuinely no location to report yet, not merely one this module failed to plumb
/// through. `MatchFinished` has no single place either — it is the whole match ending, not
/// something that happened somewhere — and is left as audio-only by the same logic in
/// `audio.rs`.
pub(super) fn event_cells(
    event: &HexMatchEvent,
    delta: Option<&HexRelayoutDelta>,
) -> Vec<HexCoord> {
    if let Some(cell) = event.cell {
        return vec![cell];
    }
    if event.kind == HexMatchEventKind::MutationCommitted
        && let Some(delta) = delta
    {
        return delta
            .changed_cells
            .iter()
            .copied()
            .take(MAX_BEACON_CELLS)
            .collect();
    }
    Vec::new()
}

fn spawn_beacon(
    commands: &mut Commands,
    assets: &FeedbackAssets,
    material: Handle<StandardMaterial>,
    cell: HexCoord,
) {
    let origin = Vec3::from_array(hex_origin(cell)) + Vec3::Y * 0.08;
    commands.spawn((
        EventBeacon { age: 0.0 },
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(origin).with_scale(Vec3::splat(BEACON_MIN_SCALE)),
        Visibility::Visible,
        Name::new("Hex WFC event beacon"),
    ));
}

fn material_for(assets: &FeedbackAssets, role: MarkerRole) -> Handle<StandardMaterial> {
    match role {
        MarkerRole::NextRoom => assets.next_room.clone(),
        MarkerRole::Exit => assets.exit.clone(),
        MarkerRole::Control => assets.control.clone(),
        MarkerRole::Collapse => assets.collapse.clone(),
        MarkerRole::You => assets.you.clone(),
        MarkerRole::Teammate => assets.teammate.clone(),
        MarkerRole::Rival => assets.rival.clone(),
        MarkerRole::Director => assets.director.clone(),
    }
}

fn beacon_material(
    materials: &mut Assets<StandardMaterial>,
    role: MarkerRole,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::marker(role);
    materials.add(StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        metallic: 0.2,
        perceptual_roughness: 0.25,
        ..default()
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use observed_facility::hex_wfc::HexMutationRegion;

    use super::*;

    fn cell(q: u16, r: u16, level: u8) -> HexCoord {
        HexCoord { q, r, level }
    }

    fn event(kind: HexMatchEventKind, cell: Option<HexCoord>) -> HexMatchEvent {
        HexMatchEvent {
            tick: 0,
            kind,
            player: None,
            cell,
        }
    }

    fn delta_touching(cells: BTreeSet<HexCoord>) -> HexRelayoutDelta {
        HexRelayoutDelta {
            previous_generation: 0,
            generation: 1,
            previous_attempts: 0,
            region: HexMutationRegion {
                cells: BTreeSet::new(),
                boundary_cells: BTreeSet::new(),
                protected_cells: BTreeSet::new(),
            },
            changed_cells: cells,
            placements: BTreeMap::new(),
            architecture: BTreeMap::new(),
            cell_revisions: BTreeMap::new(),
            previous_placements: BTreeMap::new(),
            previous_architecture: BTreeMap::new(),
            previous_cell_revisions: BTreeMap::new(),
            previous_blueprints: Vec::new(),
            removed_blueprints: Vec::new(),
            upserted_blueprints: Vec::new(),
        }
    }

    #[test]
    fn a_located_event_reports_its_own_cell() {
        let at = cell(3, 2, 1);
        let e = event(HexMatchEventKind::KeystoneCollected, Some(at));
        assert_eq!(event_cells(&e, None), vec![at]);
    }

    #[test]
    fn mutation_committed_falls_back_to_the_relayout_delta() {
        let touched: BTreeSet<HexCoord> = [cell(1, 1, 0), cell(2, 1, 0)].into_iter().collect();
        let delta = delta_touching(touched.clone());
        let e = event(HexMatchEventKind::MutationCommitted, None);
        let cells: BTreeSet<HexCoord> = event_cells(&e, Some(&delta)).into_iter().collect();
        assert_eq!(cells, touched);
    }

    #[test]
    fn mutation_committed_caps_the_beacon_count() {
        let many: BTreeSet<HexCoord> = (0..20).map(|q| cell(q, 0, 0)).collect();
        let delta = delta_touching(many);
        let e = event(HexMatchEventKind::MutationCommitted, None);
        assert_eq!(event_cells(&e, Some(&delta)).len(), MAX_BEACON_CELLS);
    }

    #[test]
    fn mutation_warning_and_cancelled_have_no_location_yet() {
        assert!(event_cells(&event(HexMatchEventKind::MutationWarning, None), None).is_empty());
        assert!(event_cells(&event(HexMatchEventKind::MutationCancelled, None), None).is_empty());
    }

    #[test]
    fn match_finished_has_no_single_location() {
        assert!(event_cells(&event(HexMatchEventKind::MatchFinished, None), None).is_empty());
    }
}
