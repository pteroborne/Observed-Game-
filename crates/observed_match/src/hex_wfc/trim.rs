//! Derived seam-trim geometry: a non-authoritative "transition grammar" pass
//! over an already-projected [`HexWfcGeometrySnapshot`]. It never changes
//! structure or collision — it only proposes decorative descriptors (railings,
//! buttresses, ...) that a later, separate renderer pass can turn into meshes.
//!
//! This module is intentionally pure: no RNG, no time, no globals, no Bevy
//! types. `derive_trim` is a plain function of its snapshot input, so the same
//! snapshot always yields the same trim set.
//!
//! ## Known data gap: no lintel rule yet
//!
//! The design note this module implements calls for a **lintel** at `Door`
//! port-class faces. [`HexWfcGeometrySnapshot`] does not carry that
//! information: [`HexStructurePiece`] records a `role`, a `source_cell`, and
//! an optional [`observed_authoring::TileKey`], but never the per-face
//! [`observed_hex::PortClass`] the solver assigned (that lives on
//! `HexWfcWorld::placements[coord].ports()` / `TilePrototype::signature`,
//! neither of which reach the projected snapshot). Guessing "this shared face
//! is probably a Door" from role/tile adjacency alone would be fabricating a
//! field this module does not actually have, so `HexTrimKind::Lintel` is
//! defined (matching the design note's vocabulary) but never emitted here.
//! Emitting it needs either an extended snapshot (out of scope: geometry.rs is
//! owned by another stream) or a follow-up pass fed the solved `HexWfcWorld`
//! directly.

use std::collections::BTreeMap;

use glam::{Quat, Vec2, Vec3};
use observed_hex::{HexCoord, HexFace, TILE_LEVEL_HEIGHT, face_edge, hex_origin};

use super::geometry::{HexStructurePiece, HexStructureRole, HexWfcGeometrySnapshot};

/// Waist height for a railing descriptor above its owning cell's floor, in
/// meters. Purely a placement hint for the later renderer pass.
const RAILING_HEIGHT: f32 = 1.0;

/// One kind of derived seam trim. Matches the design note's vocabulary;
/// `Lintel` is reserved for a future pass (see module docs).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HexTrimKind {
    /// A guard rail at an open ledge: a walkable cell's lateral face borders
    /// no occupied neighbor cell (solid <-> void edge).
    Railing,
    /// A doorway header at a `Door` port-class face. Not yet derivable from
    /// [`HexWfcGeometrySnapshot`]; see module docs.
    Lintel,
    /// A structural seam marker where two adjacent occupied cells differ in
    /// role (e.g. room <-> hall) or in resolved tile register. "Register"
    /// here is [`observed_authoring::TileKey::register`] on the piece that
    /// was actually selected for each cell — the projected snapshot does not
    /// carry `HexWfcWorld::architecture` (the procedural style register)
    /// directly, so a facility whose whole occupied catalogue resolves to
    /// one fallback register (e.g. an all-"generic" compatibility kit) will
    /// only ever trigger this rule through the role half of the check.
    Buttress,
}

/// One derived, non-colliding trim descriptor. Pure data — no mesh, no
/// collider, no Bevy component. `cell` (and `face`) are the piece's owning
/// coordinates so a changed-cell halo rebuild can regenerate only the trim
/// touching affected cells, mirroring
/// [`observed_facility::hex_wfc::relayout::changed_cells`] granularity.
#[derive(Clone, Debug, PartialEq)]
pub struct HexTrimPiece {
    pub kind: HexTrimKind,
    /// The cell that owns this trim piece and would be invalidated by a
    /// changed-cell rebuild.
    pub cell: HexCoord,
    /// The face the trim sits on, in `cell`'s local frame.
    pub face: HexFace,
    /// World-space placement, derived from [`hex_origin`] plus the face's
    /// edge midpoint (via [`face_edge`]) — the same building blocks
    /// `geometry.rs` uses for boundary-shell placement.
    pub position: Vec3,
    /// World-space yaw quaternion facing outward along the face normal.
    pub rotation: [f32; 4],
}

/// One occupied cell's role/register, summarized from its structure pieces.
/// `Boundary`-role pieces (the arena shell) are excluded: they all share the
/// spawn cell as a bookkeeping anchor rather than describing per-cell
/// coverage, so they carry no useful adjacency signal here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CellSummary<'a> {
    role: HexStructureRole,
    register: Option<&'a str>,
}

/// Derive decorative seam trim from a projected snapshot. Pure and
/// deterministic: iterates cells in coordinate order and never reads any
/// state outside `snapshot`.
#[must_use]
pub fn derive_trim(snapshot: &HexWfcGeometrySnapshot) -> Vec<HexTrimPiece> {
    let cells = summarize_cells(&snapshot.pieces);
    let mut trim = Vec::new();
    for (&coord, summary) in &cells {
        for face in HexFace::LATERAL {
            let Some(neighbor_coord) = step(coord, face) else {
                continue;
            };
            match cells.get(&neighbor_coord) {
                None => {
                    // Solid <-> void: an open ledge. `cells` already excludes
                    // the Boundary-role shell (see `summarize_cells`), so
                    // every remaining role (Hall, Room, Ramp, Shaft) is a
                    // walkable surface that can have an open lateral edge.
                    trim.push(trim_piece(
                        HexTrimKind::Railing,
                        coord,
                        face,
                        RAILING_HEIGHT,
                    ));
                }
                Some(neighbor) => {
                    // Only emit once per unordered pair, on the lower-ordered cell,
                    // so the two cells sharing a seam do not each emit a duplicate.
                    if coord < neighbor_coord
                        && (summary.role != neighbor.role || summary.register != neighbor.register)
                    {
                        trim.push(trim_piece(
                            HexTrimKind::Buttress,
                            coord,
                            face,
                            TILE_LEVEL_HEIGHT * 0.5,
                        ));
                    }
                }
            }
        }
    }
    trim
}

/// Reduce every projected piece down to one role/register summary per
/// `source_cell`, skipping the boundary shell (see [`CellSummary`] docs).
fn summarize_cells(pieces: &[HexStructurePiece]) -> BTreeMap<HexCoord, CellSummary<'_>> {
    let mut cells = BTreeMap::new();
    for piece in pieces {
        if piece.role == HexStructureRole::Boundary {
            continue;
        }
        cells.entry(piece.source_cell).or_insert(CellSummary {
            role: piece.role,
            register: piece.tile.as_ref().map(|key| key.register.as_str()),
        });
    }
    cells
}

/// Pure axial+level step, independent of any particular `HexGridSize` bound
/// (the snapshot does not carry the world's grid dimensions, only which
/// cells were actually occupied).
fn step(coord: HexCoord, face: HexFace) -> Option<HexCoord> {
    let (dq, dr, dl) = face.delta();
    let q = i32::from(coord.q) + dq;
    let r = i32::from(coord.r) + dr;
    let level = i32::from(coord.level) + dl;
    Some(HexCoord {
        q: u16::try_from(q).ok()?,
        r: u16::try_from(r).ok()?,
        level: u8::try_from(level).ok()?,
    })
}

/// World placement/orientation for a face-anchored trim piece: the face's
/// edge midpoint, translated by the cell's origin, at `height` above the
/// cell floor, facing outward along the face normal — the same
/// origin+edge+yaw construction `geometry.rs::push_boundary_shell` uses for
/// the arena shell.
fn trim_piece(kind: HexTrimKind, cell: HexCoord, face: HexFace, height: f32) -> HexTrimPiece {
    let origin = Vec3::from_array(hex_origin(cell));
    let [a, b] = face_edge(face);
    let midpoint = Vec2::new((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5);
    let position = origin + Vec3::new(midpoint.x, height, midpoint.y);
    let edge = Vec2::new((b.0 - a.0) as f32, (b.1 - a.1) as f32);
    let yaw = (-edge.y).atan2(edge.x);
    HexTrimPiece {
        kind,
        cell,
        face,
        position,
        rotation: Quat::from_rotation_y(yaw).to_array(),
    }
}

#[cfg(test)]
mod tests;
