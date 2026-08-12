//! Shared authored cutaway rendering, re-exported under the studio's original
//! module path so its established callers keep the same interface.

pub use observed_cutaway::{
    DetailMode, DetailReport, HullBatch, TileMeshCache, build, focus_set, measure, mesh_from,
    survives,
};
