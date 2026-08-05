use serde::{Deserialize, Serialize};

use super::ContractDiagnostic;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ModuleFamilyId(pub String);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssemblyScope {
    Cell,
    VerticalColumn,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyContract {
    pub family: ModuleFamilyId,
    pub scope: AssemblyScope,
    /// Selection weight applied once when choosing a family.
    ///
    /// Member weight remains the owning module's existing `weight`; accepted
    /// rotations remain its `rotations`; and register fallback remains its
    /// `register_scope`, where an exact scope wins and `"all"` is the only
    /// fallback. TR-8A validates those existing fields across every member so
    /// the contract does not serialize two disagreeing copies of each value.
    pub family_weight: u16,
}

impl AssemblyContract {
    pub fn validate(&self) -> Result<(), ContractDiagnostic> {
        if self.family.0.trim().is_empty() {
            return Err(ContractDiagnostic::whole(
                "empty_family",
                "module family ID must be nonempty",
            ));
        }
        if self.family_weight == 0 {
            return Err(ContractDiagnostic::whole(
                "zero_family_weight",
                format!("family {:?} has zero selection weight", self.family.0),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyVariantId {
    pub family: ModuleFamilyId,
    /// Clockwise 60-degree turn, always `0..=5`.
    pub rotation: u8,
}

impl AssemblyVariantId {
    pub fn validate(&self) -> Result<(), ContractDiagnostic> {
        if self.rotation > 5 {
            return Err(ContractDiagnostic::whole(
                "invalid_assembly_rotation",
                format!("assembly rotation {} is outside 0..=5", self.rotation),
            ));
        }
        Ok(())
    }
}
