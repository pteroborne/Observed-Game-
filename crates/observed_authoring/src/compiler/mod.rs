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
