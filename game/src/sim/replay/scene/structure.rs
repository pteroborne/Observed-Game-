//! Owned replay geometry patched by changed owners. History retains immutable
//! Arcs; copying the pointer vector and index buffers avoids inspecting every hull.
use super::ReplayStructure;
use observed_facility::hex_wfc::HexWfcWorld;
use observed_hex::HexCoord;
use observed_match::hex_wfc::{HexStructurePiece, HexWfcGeometrySnapshot};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

pub(super) fn retain_structure(
    world: &HexWfcWorld,
    geometry: &HexWfcGeometrySnapshot,
    previous: Option<&ReplayStructure>,
    changed: Option<&BTreeSet<HexCoord>>,
) -> ReplayStructure {
    let (pieces, piece_indices, cell_piece_ids) =
        if let Some((previous, changed)) = previous.zip(changed) {
            let mut pieces = previous.pieces.clone();
            let mut indices = previous.piece_indices.clone();
            let mut cells = previous.cell_piece_ids.clone();
            let removed: BTreeSet<_> = changed
                .iter()
                .flat_map(|cell| cells.get(cell).into_iter().flatten().copied())
                .collect();
            let mut upserted: BTreeMap<_, _> = changed
                .iter()
                .flat_map(|&cell| geometry.pieces_in_cell(cell))
                .map(|piece| {
                    let retained = indices
                        .get(&piece.id)
                        .map(|&index| &pieces[index])
                        .filter(|old| old.as_ref() == piece)
                        .map_or_else(|| Arc::new(piece.clone()), Arc::clone);
                    (piece.id, retained)
                })
                .collect();
            for id in removed {
                if let Some(piece) = upserted.remove(&id) {
                    pieces[indices[&id]] = piece;
                } else {
                    let index = indices.remove(&id).expect("owned piece has an index");
                    pieces.swap_remove(index);
                    if index < pieces.len() {
                        indices.insert(pieces[index].id, index);
                    }
                }
            }
            for (id, piece) in upserted {
                if let Some(&index) = indices.get(&id) {
                    pieces[index] = piece;
                } else {
                    indices.insert(id, pieces.len());
                    pieces.push(piece);
                }
            }
            for &cell in changed {
                let ids: Vec<_> = geometry
                    .pieces_in_cell(cell)
                    .map(|piece| piece.id)
                    .collect();
                if ids.is_empty() {
                    cells.remove(&cell);
                } else {
                    cells.insert(cell, ids);
                }
            }
            (pieces, indices, cells)
        } else {
            let pieces = retain_pieces(&geometry.pieces, previous);
            let indices = pieces
                .iter()
                .enumerate()
                .map(|(index, piece)| (piece.id, index))
                .collect();
            let mut cells: BTreeMap<_, Vec<_>> = BTreeMap::new();
            for piece in &pieces {
                cells.entry(piece.source_cell).or_default().push(piece.id);
            }
            (pieces, indices, cells)
        };
    ReplayStructure {
        seed: world.seed,
        generation: world.generation,
        pieces,
        registers: world.architecture.clone(),
        exit: world.config.exit(),
        piece_indices,
        cell_piece_ids,
    }
}

pub(super) fn retain_pieces(
    pieces: &[HexStructurePiece],
    previous: Option<&ReplayStructure>,
) -> Vec<Arc<HexStructurePiece>> {
    let old: HashMap<_, _> = previous
        .into_iter()
        .flat_map(|p| &p.pieces)
        .map(|p| (p.id, p))
        .collect();
    pieces
        .iter()
        .map(|piece| {
            old.get(&piece.id)
                .filter(|old| old.as_ref() == piece)
                .map_or_else(|| Arc::new(piece.clone()), |old| Arc::clone(old))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::hex_wfc::{HexArchetype, HexSpace};
    use observed_match::hex_wfc::{HexMatchConfig, HexMatchContent, HexWfcMatch};

    #[test]
    fn patched_history_matches_live_packed_geometry_through_removal_and_restore() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/tiles");
        let registers: Vec<_> = observed_content::ArchitectureRegister::ALL
            .iter()
            .map(|register| register.slug())
            .collect();
        let content = Arc::new(HexMatchContent::load(&base, &registers).unwrap());
        let mut game = HexWfcMatch::new_with_content(
            44,
            HexMatchConfig {
                guardian: false,
                ..Default::default()
            },
            content,
        )
        .unwrap();
        let target = game
            .facility
            .placements
            .values()
            .find(|p| {
                p.space == HexSpace::Hall
                    && !game
                        .facility
                        .blueprints
                        .iter()
                        .any(|room| room.cells.contains(&p.coord))
            })
            .copied()
            .unwrap();
        let mut empty = target;
        empty.space = HexSpace::Void;
        empty.archetype = HexArchetype::Void;
        empty.doors = 0;
        empty.low_doors = 0;
        empty.up = observed_hex::PortClass::Sealed;
        empty.down = observed_hex::PortClass::Sealed;
        let original = retain_structure(&game.facility, &game.geometry, None, None);
        let mut previous = original.clone();
        for placement in [empty, target, empty, target] {
            game.last_geometry_cells.clear();
            game.apply_directed_change(BTreeMap::from([(target.coord, placement)]))
                .unwrap();
            let patched = retain_structure(
                &game.facility,
                &game.geometry,
                Some(&previous),
                Some(&game.last_geometry_cells),
            );
            assert_eq!(
                patched.pieces.iter().map(Arc::as_ref).collect::<Vec<_>>(),
                game.geometry.pieces.iter().collect::<Vec<_>>()
            );
            for (index, piece) in patched.pieces.iter().enumerate() {
                assert_eq!(patched.piece_indices[&piece.id], index);
            }
            previous = patched;
        }
        assert!(
            original
                .pieces
                .iter()
                .any(|piece| piece.source_cell == target.coord)
        );
        assert_eq!(original.generation, 0);
    }
}
