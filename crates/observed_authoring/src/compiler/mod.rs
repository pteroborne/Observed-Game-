//! The version-3 contract compiler.
//!
//! [`crate::contract`] froze *what* a module contract is. This module is *how*
//! one is derived from an authored `.map`: exact logical footprint, geometry
//! envelope and clearance volumes ([`spatial`]), quantized landing/aperture/
//! guide interfaces on lateral **and** vertical faces ([`interface`]), and the
//! family/register/rotation coverage a catalog must prove before selection may
//! trust it ([`family`]).
//!
//! Version-1 and version-2 sources never reach here. They keep compiling into
//! catalog v3 with no contract at all, which is what keeps the committed
//! corpus hash unchanged while the schema moves.

pub mod family;
pub mod interface;
pub mod spatial;

pub use family::{AssemblyFamilyIndex, FamilyDiagnostic, FamilyEntry, FamilyMember};
pub use interface::{
    CompiledGuide, INTERFACE_CLEARANCE_DEPTH_LOCAL, VERTICAL_LOCAL_SCALE, compile_guide,
    compile_interfaces,
};
pub use spatial::{
    CLEARANCE_HEIGHT_UNITS, LATERAL_CLEARANCE_HALF_UNITS, UNITS_PER_LEVEL,
    VERTICAL_CLEARANCE_DEPTH_UNITS, VERTICAL_CLEARANCE_HALF_UNITS, compile_spatial_contract,
    quantize_world, validate_geometry,
};

use crate::contract::{AssemblyContract, ContractDiagnostic, ModuleContract};
use crate::source::AuthoredModule;

/// Compile one authored module into its complete, canonical contract.
///
/// Deliberately all-or-nothing: a module either declares a full spatial,
/// assembly, traversal, and interface contract, or it stays a compatibility
/// source. Partial metadata is never treated as a valid version-3 module.
pub fn compile_module_contract(
    module: &AuthoredModule,
    assembly: AssemblyContract,
) -> Result<ModuleContract, ContractDiagnostic> {
    assembly.validate()?;
    let spatial = spatial::compile_spatial_contract(module)?;
    let guide = interface::compile_guide(module)?;
    let interfaces = interface::compile_interfaces(module, &guide.contract)?;
    let contract = ModuleContract {
        spatial,
        assembly,
        traversal: guide.contract,
        interfaces,
    }
    .canonicalized();
    contract.validate()?;
    spatial::validate_geometry(module, &contract)?;
    Ok(contract)
}

/// Synthetic version-3 sources. Kept beside the compiler rather than in the
/// crate's shared test module so the coverage cases read next to their rules.
#[cfg(test)]
pub(crate) mod fixtures {
    use crate::ModuleKind;

    /// The minimal contracted cell: one solid hex slab, one named east door,
    /// one practical.
    pub(crate) fn contracted_cell(id: &str, family: &str) -> String {
        crate::catalog::new_module_template(id, ModuleKind::Cell).replace(
            "\"authoring_version\" \"2\"",
            &format!(
                "\"authoring_version\" \"3\"\n\"family\" \"{family}\"\n\"family_weight\" \"3\""
            ),
        )
    }

    /// A one-level shaft column: an open floor, a west wall, a vertical climb,
    /// and both vertical handoffs. `climb_top` is the editor Z the authored
    /// spine actually reaches, which is what a stacked pair has to agree on.
    pub(crate) fn contracted_tower(id: &str, family: &str, climb_top: i32) -> String {
        format!(
            "{{\n\"classname\" \"worldspawn\"\n{}\n}}\n\
             {{\n\"classname\" \"tile_meta\"\n\"authoring_version\" \"3\"\n\"id\" \"{id}\"\n\
             \"kind\" \"cell\"\n\"archetype\" \"stair_tower\"\n\"register\" \"generic\"\n\
             \"register_scope\" \"all\"\n\"variant\" \"0\"\n\"levels\" \"1\"\n\
             \"rotation_policy\" \"none\"\n\"weight\" \"1\"\n\"family\" \"{family}\"\n\
             \"assembly_scope\" \"vertical_column\"\n\"family_weight\" \"2\"\n}}\n\
             {{\n\"classname\" \"tile_cell\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\
             \"levels\" \"1\"\n\"floor\" \"open\"\n}}\n\
             {{\n\"classname\" \"tile_port\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\
             \"face\" \"up\"\n\"class\" \"shaft_open\"\n\"name\" \"up_shaft\"\n\
             \"origin\" \"0 0 128\"\n}}\n\
             {{\n\"classname\" \"tile_port\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\
             \"face\" \"down\"\n\"class\" \"shaft_open\"\n\"name\" \"down_shaft\"\n\
             \"origin\" \"0 0 0\"\n}}\n\
             {{\n\"classname\" \"tile_stair_node\"\n\"index\" \"0\"\n\"origin\" \"0 0 8\"\n}}\n\
             {{\n\"classname\" \"tile_stair_node\"\n\"index\" \"1\"\n\
             \"origin\" \"0 0 {climb_top}\"\n}}\n\
             {{\n\"classname\" \"tile_light\"\n\"kind\" \"practical\"\n\"origin\" \"0 0 96\"\n}}\n",
            crate::tile_source::box_brush_text([-112, -64, 0], [-96, 64, 128]),
        )
    }
}

#[cfg(test)]
mod tests {
    use observed_hex::{HexFace, PortClass};

    use super::*;
    use crate::ModuleKind;
    use crate::contract::{AssemblyScope, DiagnosticLocation};
    use crate::source::{FloorPolicy, ModuleCellRef, SourceError, parse_authored_module};

    #[test]
    fn a_version_3_source_compiles_one_complete_canonical_contract() {
        let module = parse_authored_module(&fixtures::contracted_cell("test/flat", "test/hall"))
            .expect("contracted cell parses");
        let contract = module.contract.as_ref().expect("v3 carries a contract");

        assert_eq!(contract.assembly.family.0, "test/hall");
        assert_eq!(contract.assembly.scope, AssemblyScope::Cell);
        assert_eq!(contract.assembly.family_weight, 3);
        assert_eq!(contract.spatial.logical_footprint.cells.len(), 1);
        assert_eq!(
            contract.spatial.logical_footprint.cells[0].floor,
            FloorPolicy::Solid
        );
        assert_eq!(contract.spatial.clearance_volumes.len(), 1);
        assert_eq!(
            contract.spatial.clearance_volumes[0].id,
            "port/east_threshold"
        );
        assert_eq!(contract.interfaces.len(), 1);
        assert_eq!(contract.interfaces[0].profile.face, HexFace::East);
        assert_eq!(contract.interfaces[0].profile.class, PortClass::Door);
        assert!(
            contract
                .traversal
                .port_bindings
                .contains_key("east_threshold")
        );
        assert_eq!(contract.clone().canonicalized(), *contract);
        contract.validate().expect("compiled contract validates");
    }

    #[test]
    fn compilation_is_deterministic_and_round_trips_through_catalog_v4() {
        let text = fixtures::contracted_cell("test/determinism", "test/hall");
        let first = parse_authored_module(&text).expect("parse").contract;
        let second = parse_authored_module(&text).expect("reparse").contract;
        assert_eq!(first, second);
        let serialized = ron::to_string(&first.expect("contract")).expect("serialize");
        let parsed: crate::contract::ModuleContract =
            ron::from_str(&serialized).expect("deserialize");
        assert_eq!(Some(parsed), second);
    }

    #[test]
    fn a_lateral_interface_reports_landing_aperture_clearance_and_terminal() {
        let module = parse_authored_module(&fixtures::contracted_cell("test/flat", "test/hall"))
            .expect("parse");
        let profile = &module.contract.as_ref().unwrap().interfaces[0].profile;
        // The authored slab top is 8 editor units above the level plane.
        assert_eq!(profile.landing.position.u, 0);
        assert_eq!(profile.landing.position.inward, 0);
        assert_eq!(profile.landing.position.v, 8 * VERTICAL_LOCAL_SCALE);
        // The channel is one sixteenth of the hex edge either side of centre.
        assert_eq!(profile.aperture.min_u, -profile.aperture.max_u);
        assert!(profile.aperture.max_u > 0);
        assert_eq!(profile.aperture.min_v, profile.landing.position.v);
        assert_eq!(
            profile.clearance.max.v - profile.clearance.min.v,
            i64::from(CLEARANCE_HEIGHT_UNITS) * VERTICAL_LOCAL_SCALE
        );
        assert_eq!(profile.clearance.max.inward, INTERFACE_CLEARANCE_DEPTH_LOCAL);
        assert_eq!(profile.guide_terminal.position.v, profile.landing.position.v);
    }

    #[test]
    fn a_vertical_interface_carries_the_same_quantized_profile_as_a_lateral_one() {
        let module = parse_authored_module(&fixtures::contracted_tower("test/tower", "test/t", 124))
            .expect("contracted tower parses");
        let contract = module.contract.as_ref().expect("contract");
        let interface = |face| {
            contract
                .interfaces
                .iter()
                .find(|interface| interface.profile.face == face)
                .unwrap_or_else(|| panic!("{face:?} interface"))
                .profile
                .clone()
        };
        let up = interface(HexFace::Up);
        let down = interface(HexFace::Down);
        assert_eq!(up.class, PortClass::ShaftOpen);
        // The shared plane is the landing by construction; the climb's real
        // reach is what the guide terminal records.
        assert_eq!(up.landing.position.inward, 0);
        assert_eq!(up.guide_terminal.position.inward, 4 * VERTICAL_LOCAL_SCALE);
        assert_eq!(down.guide_terminal.position.inward, 8 * VERTICAL_LOCAL_SCALE);
        assert_ne!(up.fingerprint(), down.fingerprint());

        let shorter =
            parse_authored_module(&fixtures::contracted_tower("test/short", "test/t", 120))
                .expect("shorter tower parses");
        let shorter_up = shorter
            .contract
            .as_ref()
            .unwrap()
            .interfaces
            .iter()
            .find(|interface| interface.profile.face == HexFace::Up)
            .expect("up interface");
        assert_ne!(
            up.fingerprint(),
            shorter_up.profile.fingerprint(),
            "a climb that stops short must not share a vertical fingerprint"
        );
    }

    #[test]
    fn a_capped_shaft_is_rejected_at_compile_time() {
        // A ceiling slab across the handoff plane is exactly the fault that
        // shipped a staircase into a ceiling.
        let wall = crate::tile_source::box_brush_text([-112, -64, 0], [-96, 64, 128]);
        let cap = crate::tile_source::box_brush_text([-64, -64, 120], [64, 64, 128]);
        let capped = fixtures::contracted_tower("test/capped", "test/t", 124)
            .replace(&wall, &format!("{wall}\n{cap}"));
        let error = parse_authored_module(&capped).expect_err("a capped shaft must not compile");
        let SourceError::Contract(diagnostic) = error else {
            panic!("expected a contract diagnostic, got {error:?}");
        };
        assert_eq!(diagnostic.code, "clearance_obstructed");
        assert!(
            diagnostic.message.contains("port/up_shaft"),
            "{diagnostic:?}"
        );
    }

    #[test]
    fn an_unnamed_port_cannot_be_bound_and_is_named_in_the_diagnostic() {
        let unnamed = fixtures::contracted_cell("test/unnamed", "test/hall")
            .replace("\"name\" \"east_threshold\"\n", "");
        let error = parse_authored_module(&unnamed).expect_err("an unnamed port must not compile");
        let SourceError::Contract(diagnostic) = error else {
            panic!("expected a contract diagnostic, got {error:?}");
        };
        assert_eq!(diagnostic.code, "unnamed_contract_port");
        assert_eq!(
            diagnostic.location,
            DiagnosticLocation::Face(
                ModuleCellRef {
                    q: 0,
                    r: 0,
                    level: 0
                },
                HexFace::East
            )
        );
    }

    #[test]
    fn contract_metadata_is_refused_on_a_compatibility_source() {
        let smuggled = crate::catalog::new_module_template("test/smuggle", ModuleKind::Cell)
            .replace(
                "\"weight\" \"1\"",
                "\"weight\" \"1\"\n\"family\" \"test/hall\"",
            );
        assert!(matches!(
            parse_authored_module(&smuggled),
            Err(SourceError::InvalidProperty {
                entity: "tile_meta",
                ..
            })
        ));
    }

    #[test]
    fn version_3_without_a_family_fails_closed() {
        let anonymous = crate::catalog::new_module_template("test/anon", ModuleKind::Cell)
            .replace("\"authoring_version\" \"2\"", "\"authoring_version\" \"3\"");
        assert!(matches!(
            parse_authored_module(&anonymous),
            Err(SourceError::InvalidProperty {
                entity: "tile_meta",
                ..
            })
        ));
    }
}
