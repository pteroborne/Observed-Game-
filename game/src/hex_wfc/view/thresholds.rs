//! The doorway model stood in each named threshold.
//!
//! Deprecated: Kenney gate model and uncollided doorframe buttress jambs are removed
//! so doorway crossings remain clean, uncluttered, and free of walked-through pillars.

use bevy::prelude::*;

use super::assets::HexWfcVisualAssets;
use crate::hex_wfc::sim::HexWfcRuntime;

/// A threshold's frame: deprecated.
#[derive(Component)]
#[allow(dead_code)]
pub(in crate::hex_wfc) struct ThresholdFrame;

/// Deprecated no-op: previously spawned the Kenney gate or fallback jambs.
#[allow(dead_code)]
pub(in crate::hex_wfc::view) fn spawn_thresholds(
    _commands: &mut Commands,
    _assets: &mut HexWfcVisualAssets,
    _meshes: &mut Assets<Mesh>,
    _runtime: &HexWfcRuntime,
) {
}
