//! ldtk_schematic_lab -- Phase A2 of the asset-integration roadmap.
//!
//! Proves whether an authored LDtk project is useful for tactical maps and
//! room-graph sketches that do not need full 3D geometry. `bevy_ecs_ldtk 0.14`
//! is used as the Bevy 0.18-compatible LDtk importer/schema, but its tile renderer
//! is not adopted. The lab projects LDtk layers/entities into stable
//! [`observed_core`] IDs and a small schematic model, then renders that model
//! through `observed_style`.

pub mod lab;
pub mod ldtk_source;
pub mod project;

pub use lab::run;
