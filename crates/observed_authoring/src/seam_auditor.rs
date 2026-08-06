//! Height continuity and seam alignment auditor for 3D hex blueprints and compositions.
//!
//! WFC face compatibility (`observed_hex::ports_compatible`) is equal-class
//! matching only: any two faces declaring the same [`PortClass`] are legal
//! neighbors to the solver, regardless of what elevation the authored
//! geometry actually puts at that boundary. That is a real gap — a `Door`
//! face authored one level too high would still bond happily with any other
//! `Door` face and only manifest as a broken seam in play.
//!
//! This auditor closes that gap for real: it builds the authoring catalog,
//! samples the authored brush geometry at every declared port's boundary
//! plane to derive a [`FaceSignature`] (port class + floor height +
//! headroom), then checks every pair of same-class boundaries the solver
//! would consider compatible for elevation agreement.
//!
//! Sampled *elevation* stays a lateral-only measure. A vertical face's
//! elevation is pinned by the shared level grid rather than independently
//! authored, so comparing sampled heights across un-placed sources flags every
//! correctly authored pair. What is comparable there is the compiled interface
//! fingerprint — landing, aperture, clearance, and guide terminal in the
//! normalized connection frame. A version-3 module carries one on every face,
//! vertical included, so this auditor now compares vertical interfaces inside a
//! family instead of declining to look at them. Compatibility sources with no
//! contract are still reported as uncompared, with the reason.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use observed_hex::{HexFace, PortClass, TILE_LEVEL_HEIGHT, face_edge, ports_compatible};

use crate::catalog::{CompiledModule, CompiledPort};
use crate::contract::{InterfaceFingerprint, ModuleFamilyId};
use crate::source::ModuleCellRef;
use crate::tile::{class_from_name, face_from_name, face_name};

pub struct SeamAuditReport {
    pub valid_seams: usize,
    pub mismatched_seams: usize,
    pub report: String,
}

/// How far apart two elevations (meters) may be and still count as
/// continuous. Authored planes are integer TrenchBroom units, so real
/// agreement lands far under this.
const ELEVATION_EPSILON: f32 = 0.05;
/// Horizontal tolerance (meters, plan view) for treating a hull vertex as
/// lying on a face's boundary plane/line.
const BOUNDARY_EPSILON: f32 = 0.05;

/// A boundary's physical signature: what class of opening it declares, plus
/// the floor height and headroom the authored geometry actually provides
/// there. Two faces the solver considers compatible (equal `class`) must
/// also agree on `floor_height` and `headroom` to be a real, walkable seam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceSignature {
    pub class: PortClass,
    /// World-space height (meters) of the boundary's floor.
    pub floor_height: f32,
    /// Vertical clearance (meters) authored at the boundary.
    pub headroom: f32,
    /// The compiled interface fingerprint, when the owning module carries a
    /// version-3 contract. This is the only comparable measure on a vertical
    /// face, and a strictly stronger one on a lateral face: it also covers the
    /// aperture, the clearance, and the guide terminal, none of which a
    /// floor/headroom sample can see.
    pub fingerprint: Option<InterfaceFingerprint>,
}

/// Whether two boundary signatures may seamlessly meet. The WFC solver joins
/// any two faces of equal [`PortClass`]; this additionally demands the
/// authored geometry agrees on floor height and headroom, since class
/// matching alone cannot see a physical discontinuity.
#[must_use]
pub fn faces_compatible(a: &FaceSignature, b: &FaceSignature) -> bool {
    ports_compatible(a.class, b.class)
        && (a.floor_height - b.floor_height).abs() <= ELEVATION_EPSILON
        && (a.headroom - b.headroom).abs() <= ELEVATION_EPSILON
}

/// Whether two vertical interfaces of the same face may stand in for one
/// another. Only the compiled fingerprint answers this: two `Up` shafts in one
/// family have to present the same landing, aperture, clearance, and guide
/// terminal, or a column that selected either could not stack on the other.
#[must_use]
pub fn vertical_faces_interchangeable(a: &FaceSignature, b: &FaceSignature) -> bool {
    match (a.fingerprint, b.fingerprint) {
        (Some(left), Some(right)) => ports_compatible(a.class, b.class) && left == right,
        _ => false,
    }
}

/// One located boundary: which module/face it came from, and its signature.
#[derive(Clone, Debug)]
struct LocatedSignature {
    module_id: String,
    family: Option<ModuleFamilyId>,
    face: HexFace,
    signature: FaceSignature,
}

/// Cell-local plan origin `(x, y, z)` in world meters, matching the
/// `hex_origin_plan`/level-height arithmetic every authored cell is placed
/// by (see `observed_hex::metrics` and `source::plan_origin`).
fn cell_origin(cell: ModuleCellRef) -> [f32; 3] {
    [
        f32::from(cell.q) * 14.0 + f32::from(cell.r) * 7.0,
        f32::from(cell.level) * TILE_LEVEL_HEIGHT,
        f32::from(cell.r) * 12.0,
    ]
}

/// Whether plan point `(x, z)` lies within [`BOUNDARY_EPSILON`] of the
/// segment `a`-`b` (not just its infinite extension).
fn near_face_edge(x: f32, z: f32, a: (f32, f32), b: (f32, f32)) -> bool {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len_sq = dx * dx + dz * dz;
    if len_sq < 1.0e-6 {
        return ((x - a.0).powi(2) + (z - a.1).powi(2)).sqrt() <= BOUNDARY_EPSILON;
    }
    // Segment-clamped projection parameter, with a small margin so vertices
    // authored exactly at the corners still count.
    let t = ((x - a.0) * dx + (z - a.1) * dz) / len_sq;
    if !(-0.05..=1.05).contains(&t) {
        return false;
    }
    let proj = (a.0 + dx * t, a.1 + dz * t);
    ((x - proj.0).powi(2) + (z - proj.1).powi(2)).sqrt() <= BOUNDARY_EPSILON
}

/// Sample authored hull geometry at one declared port's boundary and derive
/// its physical signature.
///
/// Lateral faces sample every hull vertex that lies on the vertical plane
/// running along the cell's hex edge for that face: the lowest vertex found
/// there is the boundary's floor. Headroom is the span up to the highest
/// vertex, but bounded to one [`TILE_LEVEL_HEIGHT`] — a door is a single-level
/// opening, and on a multi-level tile the full-height jamb wall above the
/// lintel would otherwise be read as extra headroom (see the inline note).
/// Vertical faces
/// (`Up`/`Down`, used only by `RampOpen`/`ShaftOpen`) are the horizontal
/// level-boundary plane itself; the compiled catalog keeps geometry per
/// hull set but not per declared port, so there is no independent way to
/// recover a sloped landing's true clearance at that plane — this treats
/// vertical headroom as one authored level height, which is a known
/// limitation (see module docs / report notes).
fn sample_face_signature(
    hulls: &[Vec<[f32; 3]>],
    class: PortClass,
    cell: ModuleCellRef,
    face: HexFace,
    fingerprint: Option<InterfaceFingerprint>,
) -> Option<FaceSignature> {
    let origin = cell_origin(cell);
    if face.is_lateral() {
        let [ca, cb] = face_edge(face);
        let a = (origin[0] + ca.0 as f32, origin[2] + ca.1 as f32);
        let b = (origin[0] + cb.0 as f32, origin[2] + cb.1 as f32);
        let mut ys: Vec<f32> = hulls
            .iter()
            .flatten()
            .copied()
            .filter(|point| near_face_edge(point[0], point[2], a, b))
            .map(|point| point[1])
            .collect();
        if ys.is_empty() {
            return None;
        }
        ys.sort_by(|x, y| x.total_cmp(y));
        let floor = ys[0];
        let ceiling = *ys.last().expect("non-empty");
        // A door/opening is a single-level aperture. On a multi-level tile
        // (e.g. a two-level ramp) the jamb brushes beside and above the
        // doorway run the tile's *full* height, so sampling min/max Y over the
        // whole boundary plane reports the wall's height, not the walkable
        // opening's — a normal lower-level door in a 16 m ramp then reads 16 m
        // of "headroom" and falsely mismatches an ordinary 8 m hall door. The
        // wall mass above the lintel is not part of the seam, so bound the
        // sampled clearance to one level height: a lower-level door reads the
        // same headroom whether the structure above it is one storey or two.
        // A genuine cross-level misplacement still surfaces through
        // `floor_height`, which is compared independently.
        let headroom = (ceiling - floor).min(TILE_LEVEL_HEIGHT);
        Some(FaceSignature {
            class,
            floor_height: floor,
            headroom,
            fingerprint,
        })
    } else {
        let plane_y = match face {
            HexFace::Up => origin[1] + TILE_LEVEL_HEIGHT,
            HexFace::Down => origin[1],
            _ => unreachable!("HexFace::is_vertical only admits Up/Down"),
        };
        Some(FaceSignature {
            class,
            floor_height: plane_y,
            headroom: TILE_LEVEL_HEIGHT,
            fingerprint,
        })
    }
}

/// The compiled interface fingerprint for one declared port, when the module
/// carries a version-3 contract. Looked up by the port's stable name, which is
/// the same key the contract binds its traversal terminal with.
fn port_fingerprint(module: &CompiledModule, port: &CompiledPort) -> Option<InterfaceFingerprint> {
    module
        .contract
        .as_ref()?
        .interfaces
        .iter()
        .find(|interface| interface.port == port.name)
        .map(|interface| interface.profile.fingerprint())
}

/// Resolve one compiled port's face/class strings into the typed vocabulary,
/// returning a diagnostic string on failure instead of erroring the whole
/// audit — an unresolvable port is itself worth reporting, not hiding.
fn resolve_port(module_id: &str, port: &CompiledPort) -> Result<(HexFace, PortClass), String> {
    let face = face_from_name(&port.face)
        .map_err(|_| format!("{module_id}: unknown port face {:?}", port.face))?;
    let class = class_from_name(&port.class)
        .map_err(|_| format!("{module_id}: unknown port class {:?}", port.class))?;
    Ok((face, class))
}

/// Audit height/headroom continuity across every declared-compatible seam in
/// the catalog. Two boundaries are "declared compatible" exactly when the
/// WFC solver would treat them as such: equal `PortClass` on the faces that
/// may meet across a placed seam. Anything not `Sealed` is compared, since
/// `Sealed` faces never meet another tile's opening and agreement there
/// proves nothing about traversal.
pub fn audit_seams(root: &Path) -> Result<SeamAuditReport, String> {
    let built = crate::build_catalog(root).map_err(|err| err.to_string())?;

    let hull_sets: BTreeMap<&str, &Vec<Vec<[f32; 3]>>> = built
        .catalog
        .hull_sets
        .iter()
        .map(|set| (set.structural_hash.as_str(), &set.hulls))
        .collect();

    let mut located: Vec<LocatedSignature> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();

    for module in &built.catalog.modules {
        let Some(&hulls) = hull_sets.get(module.structural_hash.as_str()) else {
            unresolved.push(format!(
                "{}: no hull set for structural hash {}",
                module.id, module.structural_hash
            ));
            continue;
        };
        for port in &module.ports {
            let (face, class) = match resolve_port(&module.id, port) {
                Ok(pair) => pair,
                Err(message) => {
                    unresolved.push(message);
                    continue;
                }
            };
            let fingerprint = port_fingerprint(module, port);
            match sample_face_signature(hulls, class, port.cell, face, fingerprint) {
                Some(signature) => located.push(LocatedSignature {
                    module_id: module.id.clone(),
                    family: module
                        .contract
                        .as_ref()
                        .map(|contract| contract.assembly.family.clone()),
                    face,
                    signature,
                }),
                None => unresolved.push(format!(
                    "{}: no authored geometry found on the {} boundary plane",
                    module.id,
                    face_name(face)
                )),
            }
        }
    }

    let mut by_class: BTreeMap<PortClass, Vec<&LocatedSignature>> = BTreeMap::new();
    for entry in &located {
        by_class
            .entry(entry.signature.class)
            .or_default()
            .push(entry);
    }

    let mut report = String::new();
    report.push_str("=== OBSERVED 2 TILE HEIGHT CONTINUITY & SEAM AUDIT ===\n\n");
    report.push_str(&format!(
        "Catalog: {} authored sources ({} strict, {} legacy) | {} declared ports sampled | {} unresolved\n\n",
        built.audit.sources,
        built.audit.strict_sources,
        built.audit.legacy_sources,
        located.len(),
        unresolved.len(),
    ));

    let mut valid_seams = 0usize;
    let mut mismatched_seams = 0usize;

    for (class, entries) in &by_class {
        // Sealed faces never meet another tile's opening; comparing them
        // proves nothing about traversal continuity.
        if *class == PortClass::Sealed {
            continue;
        }
        report.push_str(&format!(
            "--- Port class {class:?}: {} declared boundaries, {} declared-compatible pairs ---\n",
            entries.len(),
            entries.len() * entries.len().saturating_sub(1) / 2
        ));
        // Vertical faces (`Up`/`Down`, the only faces `RampOpen`/`ShaftOpen`
        // may sit on) are excluded from *elevation* comparison. Their
        // signature's `floor_height` is a formula over the port's authored
        // cell level, not its solve-time world level: a `Down` port sits
        // exactly one `TILE_LEVEL_HEIGHT` below a matching `Up` port by
        // construction, since that is precisely the contract the WFC grid
        // already guarantees. Comparing those heights manufactures a
        // guaranteed false mismatch on every correctly authored pair.
        //
        // Their compiled interface fingerprints are another matter. Two
        // members of one family have to present the same vertical interface,
        // or a column that picked either could not stack on the other, and
        // that is a real fault this auditor used to decline to look for.
        if entries.iter().all(|entry| entry.face.is_vertical()) {
            let (agreed, faults) = audit_vertical_family_interfaces(entries, &mut report);
            valid_seams += agreed;
            mismatched_seams += faults;
            continue;
        }
        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                let a = entries[i];
                let b = entries[j];
                if faces_compatible(&a.signature, &b.signature) {
                    valid_seams += 1;
                } else {
                    mismatched_seams += 1;
                    report.push_str(&format!(
                        "  MISMATCH: {} {:?} (floor {:.2}m, headroom {:.2}m) <-> {} {:?} (floor {:.2}m, headroom {:.2}m)\n",
                        a.module_id,
                        a.face,
                        a.signature.floor_height,
                        a.signature.headroom,
                        b.module_id,
                        b.face,
                        b.signature.floor_height,
                        b.signature.headroom,
                    ));
                }
            }
        }
    }

    if !unresolved.is_empty() {
        report.push_str("\n--- Unresolved ports (excluded from comparison) ---\n");
        for line in &unresolved {
            report.push_str(&format!("  {line}\n"));
        }
    }

    report.push_str(&format!(
        "\nSeam Audit Summary: {} sources checked | {} valid seamless boundaries | {} height mismatches\nStatus: {}\n",
        built.audit.sources,
        valid_seams,
        mismatched_seams,
        if mismatched_seams == 0 {
            "GREEN PASSED"
        } else {
            "RED - MISMATCHES FOUND"
        }
    ));

    Ok(SeamAuditReport {
        valid_seams,
        mismatched_seams,
        report,
    })
}

/// Compare vertical interfaces family by family. Cross-family variation is
/// legitimate - two tower families are allowed to be different shapes - so only
/// disagreement *inside* one family is a fault, and a module with no compiled
/// contract is reported as uncompared rather than silently passed.
fn audit_vertical_family_interfaces(
    entries: &[&LocatedSignature],
    report: &mut String,
) -> (usize, usize) {
    let mut by_family: BTreeMap<(ModuleFamilyId, HexFace), Vec<&LocatedSignature>> = BTreeMap::new();
    let mut uncontracted = BTreeSet::new();
    for entry in entries {
        match entry.family.clone() {
            Some(family) => by_family
                .entry((family, entry.face))
                .or_default()
                .push(entry),
            None => {
                uncontracted.insert(entry.module_id.clone());
            }
        }
    }
    if !uncontracted.is_empty() {
        report.push_str(&format!(
            "  (vertical: {} compatibility source(s) carry no compiled interface contract, not compared: {:?})\n",
            uncontracted.len(),
            uncontracted
        ));
    }

    let mut agreed = 0usize;
    let mut faults = 0usize;
    for ((family, face), members) in &by_family {
        for pair in members.windows(2) {
            if vertical_faces_interchangeable(&pair[0].signature, &pair[1].signature) {
                agreed += 1;
            } else {
                faults += 1;
                report.push_str(&format!(
                    "  MISMATCH: family {:?} {face:?}: {} and {} present different vertical interfaces\n",
                    family.0, pair[0].module_id, pair[1].module_id,
                ));
            }
        }
    }
    (agreed, faults)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(class: PortClass, floor_height: f32, headroom: f32) -> FaceSignature {
        FaceSignature {
            class,
            floor_height,
            headroom,
            fingerprint: None,
        }
    }

    #[test]
    fn identical_door_signatures_are_compatible() {
        let a = signature(PortClass::Door, 0.0, 4.0);
        let b = signature(PortClass::Door, 0.0, 4.0);
        assert!(faces_compatible(&a, &b));
    }

    #[test]
    fn negligible_noise_within_epsilon_still_passes() {
        let a = signature(PortClass::Door, 8.0, 8.0);
        let b = signature(PortClass::Door, 8.02, 7.98);
        assert!(faces_compatible(&a, &b));
    }

    #[test]
    fn floor_height_mismatch_is_caught() {
        // Same class, same headroom, but one boundary's floor sits a full
        // meter above the other: a real trip/fall hazard the solver's
        // class-only matching would happily bond.
        let a = signature(PortClass::Door, 0.0, 4.0);
        let b = signature(PortClass::Door, 1.0, 4.0);
        assert!(!faces_compatible(&a, &b));
    }

    #[test]
    fn headroom_mismatch_is_caught() {
        let a = signature(PortClass::Door, 0.0, 4.0);
        let b = signature(PortClass::Door, 0.0, 2.5);
        assert!(!faces_compatible(&a, &b));
    }

    #[test]
    fn incompatible_port_classes_never_bond_even_with_matching_geometry() {
        let a = signature(PortClass::Door, 0.0, 4.0);
        let b = signature(PortClass::RampOpen, 0.0, 4.0);
        assert!(!faces_compatible(&a, &b));
    }

    fn write_module(dir: &Path, name: &str, text: &str) {
        std::fs::write(dir.join(name), text).expect("write synthetic module");
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "observed2_seam_auditor_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp dir");
        path
    }

    /// A minimal legacy (`authoring_version` 1) module: a flat hex slab from
    /// `z0` to `z1` (TrenchBroom units) with one `Door` port declared on
    /// `face`. Legacy sources skip the strict floor/headroom/port-origin
    /// contract, so this is free to author a deliberately mismatched
    /// boundary for the negative test below.
    fn door_slab_map(archetype: &str, variant: u16, face: &str, z0: f64, z1: f64) -> String {
        format!(
            "{{\n\"classname\" \"worldspawn\"\n{}\n}}\n{{\n\"classname\" \"tile_meta\"\n\"archetype\" \"{archetype}\"\n\"register\" \"institutional\"\n\"variant\" \"{variant}\"\n\"levels\" \"2\"\n}}\n{{\n\"classname\" \"tile_port\"\n\"face\" \"{face}\"\n\"class\" \"door\"\n}}\n",
            crate::tile_source::hex_slab_brush(z0, z1),
        )
    }

    #[test]
    fn audit_seams_passes_two_doors_authored_at_the_same_elevation() {
        let root = temp_dir("good_pair");
        write_module(
            &root,
            "a.map",
            &door_slab_map("test_hall_a", 0, "east", 0.0, 8.0),
        );
        write_module(
            &root,
            "b.map",
            &door_slab_map("test_hall_b", 0, "west", 0.0, 8.0),
        );
        let report = audit_seams(&root).expect("audit runs");
        assert_eq!(report.mismatched_seams, 0, "{}", report.report);
        assert!(report.valid_seams >= 1, "{}", report.report);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn audit_seams_does_not_flag_a_multi_level_door_against_a_single_level_door() {
        // A two-level-tall tile (a ramp) has full-height wall mass on its door
        // face, but the walkable opening is a single-level door at the same
        // floor as an ordinary one-level hall. The extra wall height above the
        // lintel must not be read as a headroom mismatch. One level is
        // `TILE_LEVEL_HEIGHT * UNITS_PER_METER` = 128 authored units.
        let root = temp_dir("multi_level_door");
        write_module(
            &root,
            "tall.map",
            &door_slab_map("test_ramp_tall", 0, "east", 0.0, 256.0),
        );
        write_module(
            &root,
            "short.map",
            &door_slab_map("test_hall_short", 0, "west", 0.0, 128.0),
        );
        let report = audit_seams(&root).expect("audit runs");
        assert_eq!(report.mismatched_seams, 0, "{}", report.report);
        assert!(report.valid_seams >= 1, "{}", report.report);
        let _ = std::fs::remove_dir_all(root);
    }

    /// A legacy shaft source: one `Up` port, no contract, so the auditor has
    /// nothing but the level grid to go on.
    fn shaft_slab_map(archetype: &str, variant: u16) -> String {
        format!(
            "{{\n\"classname\" \"worldspawn\"\n{}\n}}\n{{\n\"classname\" \"tile_meta\"\n\"archetype\" \"{archetype}\"\n\"register\" \"institutional\"\n\"variant\" \"{variant}\"\n\"levels\" \"1\"\n}}\n{{\n\"classname\" \"tile_port\"\n\"face\" \"up\"\n\"class\" \"shaft_open\"\n}}\n",
            crate::tile_source::hex_slab_brush(0.0, 8.0),
        )
    }

    /// The vertical half of the audit. Before the contract compiler these were
    /// printed and skipped; now two shaft columns in one family are actually
    /// compared, through the compiled interface fingerprint rather than
    /// through an elevation the level grid already pins.
    #[test]
    fn audit_seams_compares_vertical_interfaces_once_a_family_declares_them() {
        let root = temp_dir("vertical_family");
        write_module(
            &root,
            "one.map",
            &crate::compiler::fixtures::contracted_tower("test/one", "test/tower", 124),
        );
        write_module(
            &root,
            "two.map",
            &crate::compiler::fixtures::contracted_tower("test/two", "test/tower", 124),
        );
        let report = audit_seams(&root).expect("audit runs");
        assert_eq!(report.mismatched_seams, 0, "{}", report.report);
        assert!(report.valid_seams >= 2, "{}", report.report);
        assert!(
            !report.report.contains("not compared"),
            "a contracted vertical interface must not be skipped: {}",
            report.report
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// A compatibility source has no compiled interface, so the auditor says
    /// so instead of counting an unchecked boundary as a passing one.
    #[test]
    fn audit_seams_reports_uncontracted_vertical_faces_as_uncompared() {
        let root = temp_dir("vertical_legacy");
        write_module(
            &root,
            "a.map",
            &shaft_slab_map("test_shaft_a", 0),
        );
        write_module(
            &root,
            "b.map",
            &shaft_slab_map("test_shaft_b", 1),
        );
        let report = audit_seams(&root).expect("audit runs");
        assert_eq!(report.mismatched_seams, 0, "{}", report.report);
        assert!(
            report.report.contains("no compiled interface contract"),
            "{}",
            report.report
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn audit_seams_catches_a_door_authored_a_level_too_high() {
        let root = temp_dir("bad_pair");
        write_module(
            &root,
            "a.map",
            &door_slab_map("test_hall_a", 0, "east", 0.0, 8.0),
        );
        // A full-level-up authoring slip: same declared class, same
        // headroom, but the slab (and its door) sits one level higher.
        write_module(
            &root,
            "b.map",
            &door_slab_map("test_hall_b", 0, "west", 128.0, 136.0),
        );
        let report = audit_seams(&root).expect("audit runs");
        assert_eq!(report.valid_seams, 0, "{}", report.report);
        assert_eq!(report.mismatched_seams, 1, "{}", report.report);
        assert!(report.report.contains("MISMATCH"), "{}", report.report);
        let _ = std::fs::remove_dir_all(root);
    }
}
