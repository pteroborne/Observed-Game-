//! The play being made, over the board: a ring on every cell the card in hand can be
//! played on at this rotation, and on the cell the play is about - aimed at, or under the
//! cursor - an amber ring if the rules would take it and a red one if not, with the lab's
//! amber ghost of the actual tile standing there, turned as it would be played. A cursor
//! that has left the aim wears a plain ring of its own. Rebuilt when the card, the
//! rotation, the aim or the cell changes.
//!
//! Nothing here decides legality: the rings are the rules' own answers
//! (`AscentSession::architect_refusal`) for this seat.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use observed_facility::hex_wfc::authored_hall;
use observed_hex::{HexCoord, hex_origin};
use observed_match::ascent::sim::{ArchitectCommand, CardKind};
use observed_match::hex_wfc::project_hypothetical_cell;
use observed_style::architect::{Role, color};

use super::ArchitectDesk;
use super::board::{BOARD_LAYER, Board, signal};
use super::building;
use super::feedback::Pulse;
use super::pick::{self, BOARD_ORIGIN};
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

#[derive(Component)]
pub(super) struct BoardOverlay;

/// Redraw the play being made: legal targets, and the ghost under the cursor.
pub(super) fn draw(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut board: ResMut<Board>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    drawn: Query<Entity, With<BoardOverlay>>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let mut hasher = std::hash::DefaultHasher::new();
    (
        desk.selected,
        desk.rotation,
        desk.hovered,
        desk.aimed,
        desk.floor,
        desk.pad_cursor.is_some(),
    )
        .hash(&mut hasher);
    board.marks_signature.hash(&mut hasher);
    runtime.match_state.geometry.generation.hash(&mut hasher);
    let hand = ascent.session().hands.get(&desk.team);
    hand.map(|hand| (hand.cooldown == 0, hand.deck.hand.len()))
        .hash(&mut hasher);
    let signature = hasher.finish();
    if signature == board.play_signature {
        return;
    }
    board.play_signature = signature;
    for entity in &drawn {
        commands.entity(entity).despawn();
    }
    let layer = RenderLayers::layer(BOARD_LAYER);
    let on_deck = |cell: HexCoord, rise: f32| {
        let at = hex_origin(cell);
        BOARD_ORIGIN + Vec3::new(at[0], pick::deck(cell.level) + rise, at[2])
    };
    let Some(card) = desk
        .selected
        .and_then(|index| hand?.deck.hand.get(index))
        .copied()
    else {
        // No card in hand: a controller's cursor still needs to be seen to be steered.
        if desk.pad_cursor.is_some()
            && let Some(hovered) = desk.hovered
        {
            commands.spawn((
                BoardOverlay,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(board.thin_ring.clone()),
                MeshMaterial3d(materials.add(signal(Role::Muted))),
                Transform::from_translation(on_deck(hovered, 0.18)),
                layer.clone(),
            ));
        }
        return;
    };
    let rules = ascent.rules();
    let Some(knowledge) = rules.team_knowledge.get(&desk.team) else {
        return;
    };
    let legal = legal_targets(
        ascent,
        &desk,
        card.id,
        knowledge
            .cells
            .keys()
            .copied()
            .filter(|cell| cell.level == desk.floor),
    );
    let valid = materials.add(signal(Role::Valid));
    for &cell in &legal {
        commands.spawn((
            BoardOverlay,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(board.thin_ring.clone()),
            MeshMaterial3d(valid.clone()),
            Transform::from_translation(on_deck(cell, 0.15)),
            layer.clone(),
        ));
    }
    // The cursor, when it has left the aim: a plain ring, the aim still standing.
    if let (Some(aimed), Some(hovered)) = (desk.aimed, desk.hovered)
        && aimed != hovered
    {
        let pointer = materials.add(signal(Role::Muted));
        commands.spawn((
            BoardOverlay,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(board.thin_ring.clone()),
            MeshMaterial3d(pointer),
            Transform::from_translation(on_deck(hovered, 0.18)),
            layer.clone(),
        ));
    }
    let Some(focus) = desk.focus() else {
        return;
    };
    let takes_it = legal.contains(&focus);
    let verdict = if takes_it {
        Role::Selected
    } else {
        Role::Guardian
    };
    let mut edge = commands.spawn((
        BoardOverlay,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(board.ring.clone()),
        MeshMaterial3d(materials.add(signal(verdict))),
        Transform::from_translation(on_deck(focus, 0.2)),
        layer.clone(),
    ));
    // An aim breathes while it waits to be confirmed.
    if desk.aimed == Some(focus) {
        edge.insert(Pulse(verdict));
    }
    // The tile itself, as it would be played: the lab's amber ghost of its real hulls,
    // floors and walls, turned as the desk has it.
    let CardKind::Tile(shape) = card.kind else {
        return;
    };
    // The corpus's hall for these doorways, as a play builds it (`played_placement`, which
    // may assume legality this cell has not been granted).
    let Some(placement) = authored_hall(focus, shape.doors(desk.rotation)) else {
        return;
    };
    let physical = &runtime.match_state;
    let Ok(pieces) = project_hypothetical_cell(
        &physical.facility,
        focus,
        placement,
        physical.content().cells(),
    ) else {
        return;
    };
    let ghost = materials.add(StandardMaterial {
        base_color: color(Role::Selected).with_alpha(0.45),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for floors in [true, false] {
        if let Some(mesh) = building::cutaway_mesh(&pieces, floors, building::bearing()) {
            commands.spawn((
                BoardOverlay,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(ghost.clone()),
                // Just proud of whatever stands there now, so it is seen over it.
                Transform::from_translation(BOARD_ORIGIN + Vec3::Y * 0.08),
                layer.clone(),
            ));
        }
    }
}

/// The cells among `cells` where the rules would take card `card` at the desk's rotation.
fn legal_targets(
    ascent: &observed_match::ascent::facility::AscentRules,
    desk: &ArchitectDesk,
    card: observed_match::ascent::sim::CardId,
    cells: impl Iterator<Item = HexCoord>,
) -> BTreeSet<HexCoord> {
    cells
        .filter(|&target| {
            ascent
                .session()
                .architect_refusal(
                    desk.seat,
                    ArchitectCommand::Play {
                        card,
                        target,
                        rotation: desk.rotation,
                    },
                )
                .is_none()
        })
        .collect()
}
