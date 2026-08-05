use serde::{Deserialize, Serialize};

use super::{ContractDiagnostic, DiagnosticLocation};
use crate::source::{FloorPolicy, ModuleCellRef};

/// Exact integer TrenchBroom coordinate, Z-up.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuantizedPoint {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuantizedBox {
    pub min: QuantizedPoint,
    pub max: QuantizedPoint,
}

impl QuantizedBox {
    pub fn validate(self) -> Result<(), ContractDiagnostic> {
        if self.min.x >= self.max.x || self.min.y >= self.max.y || self.min.z >= self.max.z {
            return Err(ContractDiagnostic::whole(
                "invalid_quantized_bounds",
                format!("box requires min < max on every axis: {self:?}"),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalCell {
    pub cell: ModuleCellRef,
    pub floor: FloorPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalFootprint {
    pub cells: Vec<LogicalCell>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryEnvelope {
    pub bounds: QuantizedBox,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClearanceVolume {
    pub id: String,
    pub bounds: QuantizedBox,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleSpatialContract {
    pub logical_footprint: LogicalFootprint,
    pub geometry_envelope: GeometryEnvelope,
    pub clearance_volumes: Vec<ClearanceVolume>,
}

impl ModuleSpatialContract {
    pub fn validate(&self) -> Result<(), ContractDiagnostic> {
        self.geometry_envelope.bounds.validate()?;
        if self.logical_footprint.cells.is_empty() {
            return Err(ContractDiagnostic::whole(
                "empty_logical_footprint",
                "module contract must own at least one logical cell",
            ));
        }
        let mut cells = std::collections::BTreeSet::new();
        for cell in &self.logical_footprint.cells {
            if !cells.insert(cell.cell) {
                return Err(ContractDiagnostic {
                    code: "duplicate_logical_cell".to_string(),
                    location: DiagnosticLocation::Cell(cell.cell),
                    message: format!("logical footprint repeats {:?}", cell.cell),
                });
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for clearance in &self.clearance_volumes {
            if clearance.id.trim().is_empty() || !ids.insert(clearance.id.as_str()) {
                return Err(ContractDiagnostic::whole(
                    "invalid_clearance_id",
                    format!(
                        "clearance IDs must be unique and nonempty: {:?}",
                        clearance.id
                    ),
                ));
            }
            clearance.bounds.validate()?;
        }
        Ok(())
    }
}
