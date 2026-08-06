//! Exact-integer checks run before any body is simulated.
//!
//! These are the cheap half of certification and the half that fails loudly:
//! every value is an exact quantized authoring unit, so a corrupted landing,
//! aperture, clearance, or guide terminal is a mismatch rather than a
//! tolerance argument. They are deliberately not advisory — a module that
//! fails here is never handed to the Rapier runner, because the physical
//! result of walking a contract that already contradicts itself is not
//! evidence about the geometry.

use std::collections::BTreeMap;

use observed_hex::HexFace;

use super::model::{CertificationFailure, CertificationFailureKind};
use crate::QUANTIZED_UNITS_PER_METER;
use crate::contract::{
    FaceLocalBox, FaceLocalPoint, LateralFaceFrame, ModuleContract, ModuleInterface, QuantizedPoint,
    VerticalFaceFrame,
};
use crate::source::ModuleCellRef;

/// Editor units spanned by one lattice level, i.e. `TILE_LEVEL_HEIGHT` metres.
const LEVEL_UNITS: i32 = 8 * QUANTIZED_UNITS_PER_METER;

/// The exact editor-space origin of one logical cell inside a module.
///
/// A module's geometry, guide, and interfaces are authored around the anchor
/// cell's centre, so every face frame is cell-local. The plan lattice is the
/// same exact integer relation `observed_hex::hex_origin_plan` states, spelled
/// out here because a module cell is signed while a facility coordinate is
/// not. Editor axes are Z-up, so a world `+z` metre is an editor `-y` unit.
fn cell_editor_origin(cell: ModuleCellRef) -> QuantizedPoint {
    let x_metres = i32::from(cell.q) * 14 + i32::from(cell.r) * 7;
    let z_metres = i32::from(cell.r) * 12;
    QuantizedPoint {
        x: x_metres * QUANTIZED_UNITS_PER_METER,
        y: -z_metres * QUANTIZED_UNITS_PER_METER,
        z: i32::from(cell.level) * LEVEL_UNITS,
    }
}

/// Project one module-local guide node into the face-local connection frame
/// that the interface's landing, aperture, clearance, and terminal all use.
fn project_into_face(
    interface: &ModuleInterface,
    point: QuantizedPoint,
) -> Result<FaceLocalPoint, String> {
    let origin = cell_editor_origin(interface.cell);
    let local = QuantizedPoint {
        x: point.x - origin.x,
        y: point.y - origin.y,
        z: point.z,
    };
    let face = interface.profile.face;
    if face.is_vertical() {
        let boundary_z = match face {
            HexFace::Up => origin.z + LEVEL_UNITS,
            _ => origin.z,
        };
        VerticalFaceFrame::try_new(face, boundary_z)
            .and_then(|frame| frame.project_point(local))
            .map_err(|diagnostic| diagnostic.message)
    } else {
        LateralFaceFrame::try_new(face, origin.z)
            .and_then(|frame| frame.project_point(local))
            .map_err(|diagnostic| diagnostic.message)
    }
}

fn contains(bounds: FaceLocalBox, point: FaceLocalPoint) -> bool {
    (bounds.min.u..=bounds.max.u).contains(&point.u)
        && (bounds.min.v..=bounds.max.v).contains(&point.v)
        && (bounds.min.inward..=bounds.max.inward).contains(&point.inward)
}

fn failure(
    module_id: &str,
    port: &str,
    kind: CertificationFailureKind,
    message: impl Into<String>,
) -> CertificationFailure {
    let mut failure = CertificationFailure::module(module_id, kind, message);
    failure.entry = Some(port.to_string());
    failure.stage = "preflight".to_string();
    failure
}

/// Every exact-integer contradiction inside one complete module contract.
///
/// Returns every fault rather than the first, because an author correcting a
/// corrupted module wants the whole list once.
pub(crate) fn preflight(module_id: &str, contract: &ModuleContract) -> Vec<CertificationFailure> {
    let mut failures = Vec::new();
    if let Err(diagnostic) = contract.validate() {
        failures.push(CertificationFailure::module(
            module_id,
            CertificationFailureKind::InvalidContract,
            format!("{}: {}", diagnostic.code, diagnostic.message),
        ));
        return failures;
    }

    let nodes = contract
        .traversal
        .nodes
        .iter()
        .map(|node| (node.id, node.position))
        .collect::<BTreeMap<_, _>>();

    for interface in &contract.interfaces {
        let profile = &interface.profile;
        let port = interface.port.as_str();

        let landing = profile.landing.position;
        if !(profile.aperture.min_u..=profile.aperture.max_u).contains(&landing.u)
            || !(profile.aperture.min_v..=profile.aperture.max_v).contains(&landing.v)
        {
            failures.push(failure(
                module_id,
                port,
                CertificationFailureKind::LandingOutsideAperture,
                format!(
                    "landing {:?} is outside aperture {:?}",
                    landing, profile.aperture
                ),
            ));
        }
        if !contains(profile.clearance, landing) {
            failures.push(failure(
                module_id,
                port,
                CertificationFailureKind::LandingOutsideClearance,
                format!(
                    "landing {:?} is outside clearance {:?}",
                    landing, profile.clearance
                ),
            ));
        }
        if !contains(profile.clearance, profile.guide_terminal.position) {
            failures.push(failure(
                module_id,
                port,
                CertificationFailureKind::TerminalOutsideClearance,
                format!(
                    "guide terminal {:?} is outside clearance {:?}",
                    profile.guide_terminal.position, profile.clearance
                ),
            ));
        }

        let Some(bound) = contract.traversal.port_bindings.get(port) else {
            failures.push(failure(
                module_id,
                port,
                CertificationFailureKind::TerminalBindingMismatch,
                "interface port has no traversal binding",
            ));
            continue;
        };
        let Some(&position) = nodes.get(bound) else {
            failures.push(failure(
                module_id,
                port,
                CertificationFailureKind::TerminalBindingMismatch,
                format!("port binding names missing node {bound:?}"),
            ));
            continue;
        };
        match project_into_face(interface, position) {
            Ok(projected) if projected == profile.guide_terminal.position => {}
            Ok(projected) => failures.push(failure(
                module_id,
                port,
                CertificationFailureKind::TerminalBindingMismatch,
                format!(
                    "bound node {bound:?} projects to {projected:?}, but the interface declares terminal {:?}",
                    profile.guide_terminal.position
                ),
            )),
            Err(message) => failures.push(failure(
                module_id,
                port,
                CertificationFailureKind::InvalidContract,
                format!("cannot project bound node {bound:?}: {message}"),
            )),
        }
    }
    failures
}
