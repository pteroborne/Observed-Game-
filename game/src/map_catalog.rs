//! Active map selection for the assembled game.
//!
//! Phase 44 kept generation out of the game and added the plumbing map builders use:
//! callers ask for the active [`MapSpec`] by seed, and the catalog enforces that every
//! registered builder returns a fully validated map. Phase 46 flips the default to the
//! procedurally generated `liminal_wfc_v1` map (Arc D's liminal-scale facility);
//! `sector_relay_v1` remains registered and selectable (via `OBSERVED2_MAP=dev` or
//! `=sector_relay_v1`) for regression testing and audits that intentionally pin the
//! authored fixture.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use observed_facility::map_spec::{MapSpec, sector_relay_v1};
use observed_facility::wfc::{WfcMapConfig, generate_liminal_map};

pub const MAP_ENV_VAR: &str = "OBSERVED2_MAP";
pub const SECTOR_RELAY_V1: &str = "sector_relay_v1";
pub const LIMINAL_WFC_V1: &str = "liminal_wfc_v1";
pub const DEFAULT_MAP: &str = LIMINAL_WFC_V1;

pub type MapSpecBuilder = fn(u64) -> MapSpec;

#[derive(Clone, Copy)]
pub struct MapSpecEntry {
    pub key: &'static str,
    builder: MapSpecBuilder,
}

impl MapSpecEntry {
    pub fn build(self, seed: u64) -> MapSpec {
        if let Some(spec) = cache_get(self.key, seed) {
            return spec;
        }
        let spec = (self.builder)(seed);
        validate_builder_contract(self.key, &spec);
        cache_put(self.key, seed, &spec);
        spec
    }
}

const CATALOG: [MapSpecEntry; 2] = [
    MapSpecEntry {
        key: SECTOR_RELAY_V1,
        builder: build_sector_relay_v1,
    },
    MapSpecEntry {
        key: LIMINAL_WFC_V1,
        builder: build_liminal_wfc_v1,
    },
];

fn build_sector_relay_v1(_seed: u64) -> MapSpec {
    sector_relay_v1()
}

fn build_liminal_wfc_v1(seed: u64) -> MapSpec {
    generate_liminal_map(seed, &WfcMapConfig::default()).unwrap_or_else(|error| {
        panic!("liminal_wfc_v1 failed to generate for seed {seed}: {error:?}")
    })
}

/// In-process memoization of built (and validated) [`MapSpec`]s by `(catalog key,
/// seed)`. The game's test suite enters the Match on the order of ~150 times; each
/// entry now generates a 24-32 room WFC map (grid collapse + role spread + repair
/// passes + full `MapSpec::validate()`), which is far more expensive than the old
/// fixed `sector_relay_v1()` literal. Caching the validated result keeps repeated test
/// runs in the same order of magnitude as before. Only validated specs are ever
/// inserted (`build` validates before caching), so a cache hit never needs to
/// re-validate.
static CACHE: OnceLock<Mutex<HashMap<(String, u64), MapSpec>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<(String, u64), MapSpec>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_get(key: &str, seed: u64) -> Option<MapSpec> {
    cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(&(key.to_string(), seed))
        .cloned()
}

fn cache_put(key: &str, seed: u64, spec: &MapSpec) {
    cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert((key.to_string(), seed), spec.clone());
}

/// Return the active map for `seed`, selected by `OBSERVED2_MAP`.
///
/// The default is `liminal_wfc_v1` (the generated liminal-scale facility). Unknown
/// names panic at startup/test time instead of silently running a different course
/// than the operator requested.
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
        LIMINAL_WFC_V1 | "default" | "liminal" => Some(CATALOG[1]),
        SECTOR_RELAY_V1 | "dev" => Some(CATALOG[0]),
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
    use observed_facility::map_spec::CorridorRole;

    #[test]
    fn default_selection_returns_a_valid_generated_map() {
        let spec = default_map_spec(1);
        assert_eq!(spec.name, "WFC Liminal Map");
        assert!(
            (24..=32).contains(&spec.room_count()),
            "default map should be liminal-scale, got {} rooms",
            spec.room_count()
        );
        assert!(spec.start_room().is_some(), "default map has a start room");
        assert!(spec.exit_room().is_some(), "default map has an exit room");
        spec.validate().expect("catalog maps are validated");
    }

    #[test]
    fn liminal_wfc_v1_can_be_selected_by_stable_name_and_aliases() {
        for name in [
            "liminal_wfc_v1",
            "Liminal WFC V1",
            "liminal-wfc-v1",
            "default",
            "liminal",
        ] {
            let spec = map_spec_for_selection(Some(name), 7);
            assert_eq!(spec.name, "WFC Liminal Map");
        }
    }

    #[test]
    fn sector_relay_can_be_selected_by_stable_name_and_aliases() {
        for name in [
            "sector_relay_v1",
            "Sector Relay V1",
            "sector-relay-v1",
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

    #[test]
    fn repeated_builds_for_the_same_key_and_seed_are_memoized_and_identical() {
        let a = default_map_spec(42);
        let b = default_map_spec(42);
        assert_eq!(a, b, "memoized builds must return byte-identical specs");
    }

    #[test]
    fn generated_wfc_vertical_roles_project_the_hex_pillar_wellshaft() {
        let (edge, seed) = (0..16_u64)
            .find_map(|seed| {
                let spec = default_map_spec(seed);
                spec.edges
                    .into_iter()
                    .find(|edge| edge.role == CorridorRole::Vertical)
                    .map(|edge| (edge, seed))
            })
            .expect("the WFC seed corpus emits a vertical edge");
        let geom = crate::teleport::hallway_geom_with_slots_and_role(
            crate::teleport::HallwayGeomEndpoints {
                from: edge.a.room,
                to: edge.b.room,
                from_room_slot: crate::teleport::ThresholdSlotId(0),
                to_room_slot: crate::teleport::ThresholdSlotId(0),
                exit_room: observed_core::RoomId(observed_match::mutable::EXIT_ROOM),
            },
            crate::hallway::template(0),
            seed,
            false,
            Some(edge.role),
        );
        assert!(geom.is_wellshaft());
        assert_eq!(geom.poly.as_ref().map(Vec::len), Some(6));
        assert_eq!(
            geom.gaps.len(),
            2,
            "only top and bottom are live graph exits"
        );
    }
}
