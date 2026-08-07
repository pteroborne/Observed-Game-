//! Exact spatial compilation: logical footprint, geometry envelope, and the
//! clearance volumes a body has to be able to occupy at every declared port.
//!
//! Everything here works in integer TrenchBroom units (Z-up), which is the
//! point: a contract compared as integers cannot drift the way formatted floats
//! do.
//!
//! What the integers are *not* is a claim that the authored geometry is on the
//! grid. Chamfers and splayed reveals are irrational against the quantized
//! hexagon and appear off-grid throughout the corpus. Quantization here rounds
//! deterministically rather than refusing, and bounds round outward. See
//! [`quantize_world`].

use glam::Vec3;
use observed_hex::{HexFace, face_edge};

use crate::QUANTIZED_UNITS_PER_METER;
use crate::contract::{
    ClearanceVolume, ContractDiagnostic, DiagnosticLocation, GeometryEnvelope, LogicalCell,
    LogicalFootprint, ModuleContract, ModuleSpatialContract, QuantizedBox, QuantizedPoint,
};
use crate::source::{AuthoredModule, ModuleCellRef, ModulePort};

/// Editor units spanned by one lattice level.
pub const UNITS_PER_LEVEL: i32 = 8 * QUANTIZED_UNITS_PER_METER;

/// Half-width of the walkable channel a lateral interface must keep clear,
/// in editor units. The deliberate controller capsule is 0.38 m in radius, so
/// half a metre is the smallest honest requirement.
pub const LATERAL_CLEARANCE_HALF_UNITS: i32 = 8;
/// Height of that channel. `MIN_HEADROOM_METERS` is 2.2 m; 36 units is the
/// next whole editor unit above it.
pub const CLEARANCE_HEIGHT_UNITS: i32 = 36;
/// Plan half-extent of the column a vertical interface must keep clear.
pub const VERTICAL_CLEARANCE_HALF_UNITS: i32 = 8;
/// Half-depth of that column either side of the shared level plane.
pub const VERTICAL_CLEARANCE_DEPTH_UNITS: i32 = 8;
/// How far below a cell's own level plane a walkable surface may be sampled
/// before it stops being that cell's floor.
const FLOOR_SAMPLE_BELOW_UNITS: i32 = 1;
/// How far above it the same sample may reach: half a level, so a ramp's high
/// end never reads as the doorway's floor.
const FLOOR_SAMPLE_ABOVE_UNITS: i32 = UNITS_PER_LEVEL / 2;
/// Plan tolerance for treating an authored vertex as lying on a face's
/// boundary line, in editor units.
const BOUNDARY_UNITS: i32 = 2;

/// An axis-aligned integer extent. Kept separate from [`QuantizedBox`] because
/// a single hull may legitimately be degenerate on one axis while a declared
/// contract volume may not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Extent {
    pub min: QuantizedPoint,
    pub max: QuantizedPoint,
}

impl Extent {
    fn around(point: QuantizedPoint) -> Self {
        Self {
            min: point,
            max: point,
        }
    }

    fn include(&mut self, point: QuantizedPoint) {
        self.min = QuantizedPoint {
            x: self.min.x.min(point.x),
            y: self.min.y.min(point.y),
            z: self.min.z.min(point.z),
        };
        self.max = QuantizedPoint {
            x: self.max.x.max(point.x),
            y: self.max.y.max(point.y),
            z: self.max.z.max(point.z),
        };
    }

    fn spans_z(self, low: i32, high: i32) -> bool {
        self.min.z < high && self.max.z > low
    }
}

/// One authored convex hull, quantized once and paired with its plan-view
/// convex outline so a clearance axis can be tested exactly.
pub(crate) struct QuantizedHull {
    pub index: usize,
    pub extent: Extent,
    plan: Vec<(i32, i32)>,
}

impl QuantizedHull {
    /// Whether the vertical line through plan point `point` passes through this
    /// hull's plan outline. Exact integer arithmetic, boundary inclusive.
    pub(crate) fn covers_plan_point(&self, point: (i32, i32)) -> bool {
        match self.plan.len() {
            0..=2 => false,
            _ => self
                .plan
                .iter()
                .copied()
                .zip(self.plan.iter().copied().cycle().skip(1))
                .take(self.plan.len())
                .all(|(a, b)| cross(a, b, point) >= 0),
        }
    }
}

fn cross(origin: (i32, i32), a: (i32, i32), b: (i32, i32)) -> i64 {
    let ax = i64::from(a.0) - i64::from(origin.0);
    let ay = i64::from(a.1) - i64::from(origin.1);
    let bx = i64::from(b.0) - i64::from(origin.0);
    let by = i64::from(b.1) - i64::from(origin.1);
    ax * by - ay * bx
}

/// Counter-clockwise plan convex hull of integer points (monotone chain).
fn plan_hull(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut sorted = points.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() < 3 {
        return sorted;
    }
    let mut lower: Vec<(i32, i32)> = Vec::new();
    for &candidate in &sorted {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], candidate) <= 0
        {
            lower.pop();
        }
        lower.push(candidate);
    }
    let mut upper: Vec<(i32, i32)> = Vec::new();
    for &candidate in sorted.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], candidate) <= 0
        {
            upper.pop();
        }
        upper.push(candidate);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// World-space Y-up metres to editor Z-up units, rounded to the nearest.
///
/// **This used to refuse anything off the 1/16 m grid**, on the stated reasoning
/// that authored geometry only ever reaches world space by dividing integer
/// editor units by 16, so an off-grid value is a real authoring fault rather
/// than float noise.
///
/// That is false for this corpus, and not marginally. Chamfers and splayed door
/// reveals go through `offset_inward`, which normalises by `hypot` — irrational
/// against the quantized hexagon's 7:4 corners. `hall_cap`, one of the plainest
/// tiles in the kit, carries plane points like `(54.5116, -93.3953)`. Fractional
/// geometry is not a fault here; it is the chamfer that gives the kit its shape,
/// and refusing it would have meant re-authoring the visual language rather than
/// migrating it.
///
/// Nothing that consumes this needs exactness. Every caller computes a bound or
/// a measurement over vertices — an envelope, a clearance hull, a landing
/// height, a headroom — and none compares two values for equality. Even the
/// interface fingerprints do not need exact *input*: they need to be
/// *comparable*, and rounding to nearest is already deterministic, so identical
/// authored geometry yields identical fingerprints whether or not its vertices
/// were integral.
///
/// What is genuinely given up: a brush nudged slightly off-grid by mistake is
/// now absorbed instead of reported. That check could not have caught it anyway
/// while the corpus is legitimately off-grid everywhere, and footprint
/// validation still rejects geometry that leaves its cell.
///
/// For a bound that must *contain* its input rather than approximate it, use
/// [`quantize_world_outward`].
#[must_use]
pub fn quantize_world(point: Vec3) -> QuantizedPoint {
    let axis = |value: f32| -> i32 {
        let scaled = f64::from(value) * f64::from(QUANTIZED_UNITS_PER_METER);
        scaled
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    };
    QuantizedPoint {
        x: axis(point.x),
        y: axis(-point.z),
        z: axis(point.y),
    }
}

/// The smallest integer box containing `point`, as its low and high corners.
///
/// An envelope is a bound, so rounding its input to nearest is wrong in the one
/// direction that matters: it can exclude by up to half a unit the very vertex
/// it was asked to contain. Including both corners costs at most 1/16 m of slack
/// and cannot under-report.
#[must_use]
pub fn quantize_world_outward(point: Vec3) -> (QuantizedPoint, QuantizedPoint) {
    let axis = |value: f32| -> (i32, i32) {
        let scaled = f64::from(value) * f64::from(QUANTIZED_UNITS_PER_METER);
        let low = scaled
            .floor()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX));
        let high = scaled
            .ceil()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX));
        (low as i32, high as i32)
    };
    let (x0, x1) = axis(point.x);
    let (y0, y1) = axis(-point.z);
    let (z0, z1) = axis(point.y);
    (
        QuantizedPoint {
            x: x0,
            y: y0,
            z: z0,
        },
        QuantizedPoint {
            x: x1,
            y: y1,
            z: z1,
        },
    )
}

/// Plan origin of one module cell in editor units.
#[must_use]
pub fn cell_plan_origin(cell: ModuleCellRef) -> (i32, i32) {
    let x = (i32::from(cell.q) * 14 + i32::from(cell.r) * 7) * QUANTIZED_UNITS_PER_METER;
    let y = -(i32::from(cell.r) * 12) * QUANTIZED_UNITS_PER_METER;
    (x, y)
}

/// Editor Z of one cell's own level plane.
#[must_use]
pub fn cell_level_z(cell: ModuleCellRef) -> i32 {
    i32::from(cell.level) * UNITS_PER_LEVEL
}

/// The boundary plane a vertical face connects across.
#[must_use]
pub fn vertical_plane_z(cell: ModuleCellRef, face: HexFace) -> i32 {
    match face {
        HexFace::Up => cell_level_z(cell) + UNITS_PER_LEVEL,
        _ => cell_level_z(cell),
    }
}

/// Both editor-unit endpoints of one lateral face's hex edge, cell-local.
#[must_use]
pub fn face_edge_units(face: HexFace) -> [(i32, i32); 2] {
    face_edge(face).map(|(x, z)| {
        (
            x * QUANTIZED_UNITS_PER_METER,
            -z * QUANTIZED_UNITS_PER_METER,
        )
    })
}

/// Midpoint of that edge. Exact: both corner sums are even multiples of the
/// editor scale.
#[must_use]
pub fn face_midpoint_units(face: HexFace) -> (i32, i32) {
    let [a, b] = face_edge_units(face);
    ((a.0 + b.0) / 2, (a.1 + b.1) / 2)
}

/// One sixteenth of the face's edge vector: the half-width of the walkable
/// channel, and exactly integral for every quantized hex edge.
#[must_use]
pub fn face_tangent_step(face: HexFace) -> (i32, i32) {
    let [a, b] = face_edge_units(face);
    ((b.0 - a.0) / 16, (b.1 - a.1) / 16)
}

/// A short step from the face plane toward the cell centre. Derived from the
/// midpoint so it stays integral on the quantized hexagon's oblique faces.
#[must_use]
pub fn face_inward_step(face: HexFace) -> (i32, i32) {
    let mid = face_midpoint_units(face);
    (-mid.0 / 8, -mid.1 / 8)
}

/// Everything the interface compiler needs about one declared port, derived
/// once from authored geometry.
pub(crate) struct PortGeometry {
    pub cell_origin: (i32, i32),
    /// Reference plane the face frame is built against.
    pub frame_z: i32,
    /// Where a body stands to cross, in module coordinates.
    pub landing: QuantizedPoint,
    /// Where the guide terminal is anchored, in module coordinates.
    pub anchor: QuantizedPoint,
    /// Editor units of clear height above the landing.
    pub headroom: i32,
    pub clearance: ClearanceVolume,
}

pub(crate) fn quantized_hulls(
    module: &AuthoredModule,
) -> Result<Vec<QuantizedHull>, ContractDiagnostic> {
    module
        .prototype
        .hulls
        .iter()
        .enumerate()
        .map(|(index, hull)| {
            let points = hull
                .iter()
                .map(|&point| quantize_world(point))
                .collect::<Vec<_>>();
            let mut iter = points.iter().copied();
            let first = iter.next().ok_or_else(|| {
                ContractDiagnostic::whole(
                    "empty_authored_hull",
                    format!("hull {index} has no vertices"),
                )
            })?;
            let mut extent = Extent::around(first);
            for point in iter {
                extent.include(point);
            }
            Ok(QuantizedHull {
                index,
                extent,
                plan: plan_hull(
                    &points
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect::<Vec<_>>(),
                ),
            })
        })
        .collect()
}

fn near_edge(point: QuantizedPoint, a: (i32, i32), b: (i32, i32)) -> bool {
    let (dx, dy) = (i64::from(b.0 - a.0), i64::from(b.1 - a.1));
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0 {
        return false;
    }
    let px = i64::from(point.x) - i64::from(a.0);
    let py = i64::from(point.y) - i64::from(a.1);
    let along = px * dx + py * dy;
    if along < 0 || along > length_squared {
        return false;
    }
    let perpendicular = px * dy - py * dx;
    perpendicular * perpendicular
        <= i64::from(BOUNDARY_UNITS) * i64::from(BOUNDARY_UNITS) * length_squared
}

/// Top of the walkable surface a lateral port opens onto, sampled from the
/// authored vertices that actually lie on that boundary line.
fn lateral_landing_z(
    module: &AuthoredModule,
    port: &ModulePort,
) -> Result<i32, ContractDiagnostic> {
    let origin = cell_plan_origin(port.cell);
    let level = cell_level_z(port.cell);
    let [a, b] = face_edge_units(port.face);
    let a = (a.0 + origin.0, a.1 + origin.1);
    let b = (b.0 + origin.0, b.1 + origin.1);
    let mut best: Option<i32> = None;
    for hull in &module.prototype.hulls {
        for &vertex in hull {
            let point = quantize_world(vertex);
            if !near_edge(point, a, b) {
                continue;
            }
            if point.z < level - FLOOR_SAMPLE_BELOW_UNITS
                || point.z > level + FLOOR_SAMPLE_ABOVE_UNITS
            {
                continue;
            }
            best = Some(best.map_or(point.z, |current: i32| current.max(point.z)));
        }
    }
    best.ok_or(ContractDiagnostic {
        code: "no_floor_at_interface".to_string(),
        location: DiagnosticLocation::Face(port.cell, port.face),
        message: format!(
            "port {:?} has no authored walkable surface on its boundary line",
            port.name
        ),
    })
}

fn lateral_headroom(
    module: &AuthoredModule,
    port: &ModulePort,
    landing: i32,
) -> Result<i32, ContractDiagnostic> {
    let origin = cell_plan_origin(port.cell);
    let level = cell_level_z(port.cell);
    let [a, b] = face_edge_units(port.face);
    let a = (a.0 + origin.0, a.1 + origin.1);
    let b = (b.0 + origin.0, b.1 + origin.1);
    let ceiling = level + UNITS_PER_LEVEL;
    let mut lowest: Option<i32> = None;
    for hull in &module.prototype.hulls {
        for &vertex in hull {
            let point = quantize_world(vertex);
            if !near_edge(point, a, b) || point.z <= landing + 1 || point.z > ceiling {
                continue;
            }
            lowest = Some(lowest.map_or(point.z, |current: i32| current.min(point.z)));
        }
    }
    Ok(lowest.unwrap_or(ceiling) - landing)
}

pub(crate) fn port_geometry(
    module: &AuthoredModule,
    port: &ModulePort,
) -> Result<PortGeometry, ContractDiagnostic> {
    let cell_origin = cell_plan_origin(port.cell);
    let id = format!("port/{}", port.name);
    if port.face.is_lateral() {
        let landing_z = lateral_landing_z(module, port)?;
        let headroom = lateral_headroom(module, port, landing_z)?;
        let mid = face_midpoint_units(port.face);
        let step = face_tangent_step(port.face);
        let inward = face_inward_step(port.face);
        let landing = QuantizedPoint {
            x: cell_origin.0 + mid.0,
            y: cell_origin.1 + mid.1,
            z: landing_z,
        };
        let mut extent = Extent::around(landing);
        for (dx, dy) in [
            (step.0, step.1),
            (-step.0, -step.1),
            (step.0 + inward.0, step.1 + inward.1),
            (-step.0 + inward.0, -step.1 + inward.1),
        ] {
            extent.include(QuantizedPoint {
                x: landing.x + dx,
                y: landing.y + dy,
                z: landing_z + CLEARANCE_HEIGHT_UNITS,
            });
        }
        Ok(PortGeometry {
            cell_origin,
            frame_z: cell_level_z(port.cell),
            landing,
            anchor: landing,
            headroom,
            clearance: ClearanceVolume {
                id,
                bounds: QuantizedBox {
                    min: extent.min,
                    max: extent.max,
                },
            },
        })
    } else {
        let plane = vertical_plane_z(port.cell, port.face);
        let landing = QuantizedPoint {
            x: cell_origin.0,
            y: cell_origin.1,
            z: plane,
        };
        Ok(PortGeometry {
            cell_origin,
            frame_z: plane,
            landing,
            anchor: landing,
            headroom: UNITS_PER_LEVEL,
            clearance: ClearanceVolume {
                id,
                bounds: QuantizedBox {
                    min: QuantizedPoint {
                        x: cell_origin.0 - VERTICAL_CLEARANCE_HALF_UNITS,
                        y: cell_origin.1 - VERTICAL_CLEARANCE_HALF_UNITS,
                        z: plane - VERTICAL_CLEARANCE_DEPTH_UNITS,
                    },
                    max: QuantizedPoint {
                        x: cell_origin.0 + VERTICAL_CLEARANCE_HALF_UNITS,
                        y: cell_origin.1 + VERTICAL_CLEARANCE_HALF_UNITS,
                        z: plane + VERTICAL_CLEARANCE_DEPTH_UNITS,
                    },
                },
            },
        })
    }
}

/// Compile the explicit spatial meanings `levels` used to conflate: which
/// lattice cells the module owns, where its geometry may live, and which
/// volumes collision geometry must leave empty.
pub fn compile_spatial_contract(
    module: &AuthoredModule,
) -> Result<ModuleSpatialContract, ContractDiagnostic> {
    let mut cells = Vec::new();
    for cell in &module.footprint {
        for offset in 0..cell.levels {
            cells.push(LogicalCell {
                cell: ModuleCellRef {
                    q: cell.q,
                    r: cell.r,
                    level: cell.level.saturating_add_unsigned(offset),
                },
                floor: cell.floor,
            });
        }
    }

    let mut envelope: Option<Extent> = None;
    let mut include = |point: QuantizedPoint| match envelope.as_mut() {
        Some(extent) => extent.include(point),
        None => envelope = Some(Extent::around(point)),
    };
    // Outward, because this is the one place the value has to *contain* what it
    // measures rather than approximate it.
    let mut include_containing = |point: Vec3| {
        let (low, high) = quantize_world_outward(point);
        include(low);
        include(high);
    };
    for hull in &module.prototype.hulls {
        for &vertex in hull {
            include_containing(vertex);
        }
    }
    for light in &module.prototype.lights {
        include_containing(light.position);
    }
    for node in module
        .prototype
        .spine
        .nodes
        .iter()
        .chain(&module.prototype.deck.nodes)
    {
        include_containing(*node);
    }
    let envelope = envelope.ok_or_else(|| {
        ContractDiagnostic::whole(
            "empty_geometry_envelope",
            "a module contract needs authored geometry to bound",
        )
    })?;

    let mut clearance_volumes = Vec::with_capacity(module.ports.len());
    for port in &module.ports {
        clearance_volumes.push(port_geometry(module, port)?.clearance);
    }

    Ok(ModuleSpatialContract {
        logical_footprint: LogicalFootprint { cells },
        geometry_envelope: GeometryEnvelope {
            bounds: QuantizedBox {
                min: envelope.min,
                max: envelope.max,
            },
        },
        clearance_volumes,
    })
}

/// Prove the compiled contract against the geometry it claims to describe:
/// nothing outside the envelope, and nothing inside a clearance volume.
pub fn validate_geometry(
    module: &AuthoredModule,
    contract: &ModuleContract,
) -> Result<(), ContractDiagnostic> {
    let hulls = quantized_hulls(module)?;
    let envelope = contract.spatial.geometry_envelope.bounds;
    for hull in &hulls {
        let outside = hull.extent.min.x < envelope.min.x
            || hull.extent.min.y < envelope.min.y
            || hull.extent.min.z < envelope.min.z
            || hull.extent.max.x > envelope.max.x
            || hull.extent.max.y > envelope.max.y
            || hull.extent.max.z > envelope.max.z;
        if outside {
            return Err(ContractDiagnostic::whole(
                "geometry_outside_envelope",
                format!(
                    "hull {} escapes the declared geometry envelope {envelope:?}",
                    hull.index
                ),
            ));
        }
    }

    let footprint_levels = contract
        .spatial
        .logical_footprint
        .cells
        .iter()
        .map(|cell| cell.cell.level)
        .collect::<Vec<_>>();
    let lowest = footprint_levels.iter().copied().min().unwrap_or(0);
    let highest = footprint_levels.iter().copied().max().unwrap_or(0);
    let floor = i32::from(lowest) * UNITS_PER_LEVEL;
    let ceiling = (i32::from(highest) + 1) * UNITS_PER_LEVEL;
    if envelope.min.z < floor || envelope.max.z > ceiling {
        return Err(ContractDiagnostic::whole(
            "envelope_outside_footprint",
            format!(
                "geometry envelope z {}..{} escapes the logical footprint's {floor}..{ceiling}",
                envelope.min.z, envelope.max.z
            ),
        ));
    }

    for clearance in &contract.spatial.clearance_volumes {
        let bounds = clearance.bounds;
        let axis = (
            (bounds.min.x + bounds.max.x) / 2,
            (bounds.min.y + bounds.max.y) / 2,
        );
        for hull in &hulls {
            if hull.extent.spans_z(bounds.min.z, bounds.max.z) && hull.covers_plan_point(axis) {
                return Err(ContractDiagnostic::whole(
                    "clearance_obstructed",
                    format!(
                        "hull {} blocks the axis of clearance volume {:?}",
                        hull.index, clearance.id
                    ),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_hex::HexFace;

    #[test]
    fn one_level_is_exactly_the_shared_editor_constant() {
        assert_eq!(
            UNITS_PER_LEVEL,
            (observed_hex::TILE_LEVEL_HEIGHT as i32) * QUANTIZED_UNITS_PER_METER
        );
    }

    /// The cell editor-space origin and the frame reference plane are a
    /// cross-packet convention, not this module's private choice: certification
    /// projects a bound guide node into the face frame the same way, and a
    /// second convention would fail every contracted module for a reason that
    /// has nothing to do with its geometry. Spelled out here as the exact
    /// integer relation both sides must keep.
    #[test]
    fn the_cell_origin_and_frame_planes_match_the_shared_projection_convention() {
        for cell in [
            ModuleCellRef {
                q: 0,
                r: 0,
                level: 0,
            },
            ModuleCellRef {
                q: 2,
                r: -3,
                level: 1,
            },
            ModuleCellRef {
                q: -1,
                r: 4,
                level: -2,
            },
        ] {
            let (x, y) = cell_plan_origin(cell);
            assert_eq!(
                x,
                (i32::from(cell.q) * 14 + i32::from(cell.r) * 7) * QUANTIZED_UNITS_PER_METER
            );
            assert_eq!(y, -(i32::from(cell.r) * 12) * QUANTIZED_UNITS_PER_METER);
            // The same exact integer relation as `observed_hex::hex_origin_plan`.
            // It is spelled out rather than delegated because a module cell is
            // signed and a facility coordinate is not, so only the
            // non-negative cells can be cross-checked against it directly.
            if let (Ok(q), Ok(r)) = (u16::try_from(cell.q), u16::try_from(cell.r)) {
                let plan = observed_hex::hex_origin_plan(observed_hex::HexCoord { q, r, level: 0 });
                assert_eq!(x, plan.0 * QUANTIZED_UNITS_PER_METER);
                assert_eq!(y, -plan.1 * QUANTIZED_UNITS_PER_METER);
            }

            let origin_z = cell_level_z(cell);
            assert_eq!(origin_z, i32::from(cell.level) * UNITS_PER_LEVEL);
            assert_eq!(
                vertical_plane_z(cell, HexFace::Up),
                origin_z + UNITS_PER_LEVEL
            );
            assert_eq!(vertical_plane_z(cell, HexFace::Down), origin_z);
        }
    }

    #[test]
    fn every_face_yields_integral_tangent_and_inward_steps() {
        for face in HexFace::LATERAL {
            let mid = face_midpoint_units(face);
            let [a, b] = face_edge_units(face);
            assert_eq!(((a.0 + b.0) / 2, (a.1 + b.1) / 2), mid);
            assert_eq!(
                face_tangent_step(face),
                ((b.0 - a.0) / 16, (b.1 - a.1) / 16),
                "{face:?} tangent must divide exactly"
            );
            assert_eq!((b.0 - a.0) % 16, 0, "{face:?} tangent x remainder");
            assert_eq!((b.1 - a.1) % 16, 0, "{face:?} tangent y remainder");
            assert_eq!(mid.0 % 8, 0, "{face:?} inward x remainder");
            assert_eq!(mid.1 % 8, 0, "{face:?} inward y remainder");
        }
    }

    #[test]
    fn the_editor_grid_round_trips_exactly_and_rounds_the_chamfers_it_must_accept() {
        // On-grid geometry is still exact, which is most of the corpus.
        assert_eq!(
            quantize_world(Vec3::new(7.0, 0.5, -4.0)),
            QuantizedPoint {
                x: 112,
                y: 64,
                z: 8
            }
        );

        // Off-grid geometry is accepted and rounded, because chamfers produce
        // it everywhere. `hall_cap`'s own plane point, in world metres.
        let chamfer = Vec3::new(54.5116 / 16.0, 0.5, -(-93.3953) / 16.0);
        let quantized = quantize_world(chamfer);
        assert_eq!(quantized.x, 55);
        assert_eq!(quantized.y, -93);

        // Deterministic, which is the property fingerprints actually rely on:
        // the same authored geometry must always quantize the same way.
        assert_eq!(quantize_world(chamfer), quantized);

        // And a bound must contain what it bounds, so the outward form brackets
        // the point instead of approximating it.
        let (low, high) = quantize_world_outward(chamfer);
        assert!(low.x <= 54 && high.x >= 55, "{low:?}..{high:?}");
        assert!(low.y <= -94 && high.y >= -93, "{low:?}..{high:?}");
    }

    #[test]
    fn a_plan_outline_answers_containment_without_tolerance() {
        let square = plan_hull(&[(-16, -16), (16, -16), (16, 16), (-16, 16)]);
        let hull = QuantizedHull {
            index: 0,
            extent: Extent {
                min: QuantizedPoint {
                    x: -16,
                    y: -16,
                    z: 0,
                },
                max: QuantizedPoint { x: 16, y: 16, z: 8 },
            },
            plan: square,
        };
        assert!(hull.covers_plan_point((0, 0)));
        assert!(hull.covers_plan_point((16, 0)));
        assert!(!hull.covers_plan_point((17, 0)));
    }
}
