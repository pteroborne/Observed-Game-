//! Active map selection for the assembled game.
//!
//! Phase 44 keeps generation out of the game and adds the plumbing future map builders
//! will use: callers ask for the active [`MapSpec`] by seed, and the catalog enforces
//! that every registered builder returns a fully validated map.

use observed_facility::map_spec::{MapSpec, sector_relay_v1};

pub const MAP_ENV_VAR: &str = "OBSERVED2_MAP";
pub const SECTOR_RELAY_V1: &str = "sector_relay_v1";
pub const DEFAULT_MAP: &str = SECTOR_RELAY_V1;

pub type MapSpecBuilder = fn(u64) -> MapSpec;

#[derive(Clone, Copy)]
pub struct MapSpecEntry {
    pub key: &'static str,
    builder: MapSpecBuilder,
}

impl MapSpecEntry {
    pub fn build(self, seed: u64) -> MapSpec {
        let spec = (self.builder)(seed);
        validate_builder_contract(self.key, &spec);
        spec
    }
}

const CATALOG: [MapSpecEntry; 1] = [MapSpecEntry {
    key: SECTOR_RELAY_V1,
    builder: build_sector_relay_v1,
}];

fn build_sector_relay_v1(_seed: u64) -> MapSpec {
    sector_relay_v1()
}

/// Return the active map for `seed`, selected by `OBSERVED2_MAP`.
///
/// The default is `sector_relay_v1`. Unknown names panic at startup/test time instead
/// of silently running a different course than the operator requested.
pub fn active_map_spec(seed: u64) -> MapSpec {
    let requested = std::env::var(MAP_ENV_VAR)
        .ok()
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .unwrap_or_else(|| DEFAULT_MAP.to_string());
    map_spec_for_selection(Some(&requested), seed)
}

pub fn default_map_spec(seed: u64) -> MapSpec {
    map_spec_for_selection(None, seed)
}

pub fn map_spec_for_selection(selection: Option<&str>, seed: u64) -> MapSpec {
    let requested = selection.unwrap_or(DEFAULT_MAP);
    let Some(entry) = catalog_entry(requested) else {
        panic!(
            "unknown {MAP_ENV_VAR} value `{requested}`; available maps: {}",
            available_map_names().join(", ")
        );
    };
    entry.build(seed)
}

pub fn available_map_names() -> Vec<&'static str> {
    CATALOG.iter().map(|entry| entry.key).collect()
}

pub fn catalog_entry(name: &str) -> Option<MapSpecEntry> {
    match normalize_map_name(name).as_str() {
        SECTOR_RELAY_V1 | "default" | "dev" => Some(CATALOG[0]),
        _ => None,
    }
}

fn normalize_map_name(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            '-' | ' ' => '_',
            _ => ch,
        })
        .collect()
}

fn validate_builder_contract(key: &str, spec: &MapSpec) {
    if let Err(errors) = spec.validate() {
        panic!(
            "map builder `{key}` produced invalid MapSpec `{}`: {errors:?}",
            spec.name
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_core::RoomId;

    #[test]
    fn default_selection_returns_valid_sector_relay() {
        let spec = default_map_spec(1);
        assert_eq!(spec.name, "Sector Relay V1");
        assert_eq!(spec.start_room(), Some(RoomId(0)));
        assert_eq!(spec.exit_room(), Some(RoomId(11)));
        spec.validate().expect("catalog maps are validated");
    }

    #[test]
    fn sector_relay_can_be_selected_by_stable_name_and_aliases() {
        for name in [
            "sector_relay_v1",
            "Sector Relay V1",
            "sector-relay-v1",
            "default",
            "dev",
        ] {
            let spec = map_spec_for_selection(Some(name), 7);
            assert_eq!(spec.name, "Sector Relay V1");
        }
    }

    #[test]
    fn unknown_names_are_rejected() {
        assert!(catalog_entry("missing_map").is_none());
    }
}
