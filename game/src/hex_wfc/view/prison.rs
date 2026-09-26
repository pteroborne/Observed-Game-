//! The prison maze, drawn while the local body is in it.
//!
//! In the simulation each team's maze is a space of its own. Presentation draws it far
//! below the facility at its team's offset (`ascent::prison_offset`), where the camera
//! and the jailed bodies are drawn too, so nothing of the facility is in view and no two
//! teams' mazes overlap. Only the local team's maze is ever drawn, and only while the
//! local body is in it: a maze is sixty-odd halls, spawned whole when the body arrives.

use bevy::prelude::*;
use observed_core::TeamId;
use observed_match::hex_wfc::HexBodyPlace;

use super::assets::HexWfcVisualAssets;
use super::shell::{HexGeometryCatalog, spawn_cells};
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

#[derive(Component)]
pub(in crate::hex_wfc) struct PrisonMaze;

/// Which maze is drawn: its team, and the seed it was carved from.
#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct PrisonView {
    shown: Option<(TeamId, u64)>,
}

pub(in crate::hex_wfc) fn sync_prison_view(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    mut view: ResMut<PrisonView>,
    mut assets: ResMut<HexWfcVisualAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    gate: Option<Res<crate::hex_wfc::prison_gate::PrisonGateAssets>>,
    drawn: Query<Entity, With<PrisonMaze>>,
) {
    let local = runtime.viewed();
    let prison = runtime.match_state.prison.as_ref();
    let wanted = (local.place == HexBodyPlace::Prison)
        .then(|| prison?.mazes.get(&local.team))
        .flatten()
        .map(|maze| (local.team, maze.world.seed));
    // Leaving play despawns the maze without this system seeing it, so a maze that should
    // be shown and has no entity is shown again.
    if wanted == view.shown && (wanted.is_none() || !drawn.is_empty()) {
        return;
    }
    for maze in &drawn {
        commands.entity(maze).despawn();
    }
    view.shown = wanted;
    let (Some((team, _)), Some(prison)) = (wanted, prison) else {
        return;
    };
    let maze = &prison.mazes[&team];
    let catalog = HexGeometryCatalog::build(&maze.world, &maze.geometry);
    let every_cell = catalog.cells.keys().copied().collect();
    let root = commands
        .spawn((
            PrisonMaze,
            DespawnOnExit(GameState::HexWfc),
            Transform::from_translation(crate::hex_wfc::ascent::prison_offset(team)),
            Visibility::default(),
            Name::new("Prison maze"),
        ))
        .id();
    let cells = spawn_cells(
        &mut commands,
        &mut assets,
        &mut meshes,
        (&maze.world, &maze.geometry),
        &catalog,
        &every_cell,
    );
    for cell in cells {
        commands.entity(root).add_child(cell.entity);
    }
    // The way out, marked with the gate the lobby wears.
    if let Some(gate) = gate {
        crate::hex_wfc::prison_gate::spawn_gate(
            &mut commands,
            &gate,
            maze.exit(),
            Some(root),
            false,
        );
    }
}
