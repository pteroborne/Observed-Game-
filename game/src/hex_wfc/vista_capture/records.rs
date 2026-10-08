//! Provenance for staged construction views; no simulation mutation.
use super::VistaPose;
use crate::hex_wfc::sim::HexWfcRuntime;

pub(super) fn save(runtime: &HexWfcRuntime, pose: &VistaPose, path: &str, slot: u16) {
    let state = &runtime.match_state;
    let hash = state
        .simulation_content_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let report = serde_json::json!({
        "mode": "staged production construction reference, environment held",
        "seed": state.seed,
        "input_version": observed_match::hex_wfc::HEX_INPUT_VERSION,
        "simulation_content_hash": hash,
        "tick": state.tick,
        "grid": [state.facility.config.cols, state.facility.config.rows, u16::from(state.facility.config.levels)],
        "name": pose.name,
        "cell": [pose.cell.q, pose.cell.r, u16::from(pose.cell.level)],
        "feet": pose.feet.to_array(),
        "yaw": pose.yaw,
        "pitch": pose.pitch,
        "initial_module_variant": state.facility.initial_module_variant(pose.cell),
    });
    let file = std::path::Path::new(path).join(format!("vista_{:02}_{}.json", slot + 1, pose.name));
    std::fs::write(
        file,
        serde_json::to_string_pretty(&report).expect("reference report"),
    )
    .expect("save reference report");
}
