//! Tile placement-distribution reporting (`tilec audit-distribution`).
//!
//! Reports how the authored tile alphabet is distributed across archetypes,
//! registers (districts), variants, and hex rotations -- so content authors
//! can see which tiles/rotations never get a chance to appear and which
//! families dominate the mix.
//!
//! Two complementary views are reported:
//!
//! 1. **Catalog alphabet** ([`tally_placement_distribution`] over
//!    [`samples_from_modules`]): archetype x register x variant x rotation,
//!    weighted by each module's authored `weight`. A `weight` of zero is a
//!    genuine catalog bug (the tile is authored but the weighted selector will
//!    never choose it), so this surfaces "this will never place" cases and
//!    unused rotations from the declared alphabet alone.
//! 2. **Live per-seed placements** ([`tally_live_placements`] over
//!    [`architecture_rows`]): how often each [`HexArchetype`] actually places,
//!    and how live cells distribute across registers (districts), from a real
//!    `HexWfcWorld::generate` solve. This answers "what does a solve actually
//!    produce?", which the catalog alphabet cannot.
//!
//! The live view stops at the **architecture level**: the WFC solver places
//! `HexArchetype`s + port signatures, and the concrete authored tile + rotation
//! for each cell is only chosen later, at projection time, in `observed_match`
//! (which `observed_authoring` cannot depend on without a dependency cycle). So
//! catalog variant/rotation frequency stays the catalog tally's job; true
//! per-tile placement frequency would need a projection-side producer.
//!
//! All three tallies are pure functions of plain rows, so their producers can
//! be swapped without touching the reporting logic.

use std::collections::{BTreeMap, BTreeSet};

use observed_facility::hex_wfc::{HexArchetype, HexWfcWorld};

use crate::catalog::CompiledModule;

/// One archetype/register/variant's placement alphabet: counts keyed by hex
/// turn (`0..=5`). A turn absent from the map (or present with count `0`) is
/// a rotation this tile is never placed under.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CatalogSample {
    pub archetype: String,
    pub register: String,
    pub variant: u16,
    pub rotation_counts: BTreeMap<u8, u32>,
}

/// Build catalog-derived samples: one row per (module, register-in-scope),
/// using the module's authored `weight` as the per-rotation instance count.
/// Pure data mapping over an already-loaded catalog -- no IO, no solve.
pub fn samples_from_modules(modules: &[CompiledModule]) -> Vec<CatalogSample> {
    let mut samples = Vec::new();
    for module in modules {
        let mut rotation_counts = BTreeMap::new();
        for &turn in &module.rotations {
            rotation_counts.insert(turn, module.weight as u32);
        }
        if module.register_scope.is_empty() {
            samples.push(CatalogSample {
                archetype: module.archetype.clone(),
                register: "(unscoped)".to_string(),
                variant: module.variant,
                rotation_counts: rotation_counts.clone(),
            });
            continue;
        }
        for register in &module.register_scope {
            samples.push(CatalogSample {
                archetype: module.archetype.clone(),
                register: register.clone(),
                variant: module.variant,
                rotation_counts: rotation_counts.clone(),
            });
        }
    }
    samples
}

/// Stable report name for a solver archetype (independent of `Debug`).
fn archetype_name(archetype: HexArchetype) -> &'static str {
    match archetype {
        HexArchetype::Void => "void",
        HexArchetype::Room => "room",
        HexArchetype::Straight => "straight",
        HexArchetype::Corner => "corner",
        HexArchetype::Junction => "junction",
        HexArchetype::RampUp => "ramp_up",
        HexArchetype::RampHead => "ramp_head",
        HexArchetype::Shaft => "shaft",
    }
}

/// Extract one `(archetype, register)` row per live (non-`Void`) placed cell of
/// a solved world — the input rows for [`tally_live_placements`]. Reads the
/// world only; no IO, no solve. `Void` cells are empty space, not placed tiles,
/// so they are excluded from the placement distribution.
#[must_use]
pub fn architecture_rows(world: &HexWfcWorld) -> Vec<(String, String)> {
    world
        .placements
        .values()
        .filter(|placement| placement.archetype != HexArchetype::Void)
        .map(|placement| {
            let register = world
                .architecture
                .get(&placement.coord)
                .map_or("(none)", |register| register.slug());
            (
                archetype_name(placement.archetype).to_string(),
                register.to_string(),
            )
        })
        .collect()
}

/// A live solve's placement distribution: per-archetype and per-register cell
/// counts from an actual `HexWfcWorld`. Distinct from the catalog alphabet
/// (see module docs) — the solver has no catalog variant/rotation, so this
/// reports the architecture level it actually produces.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LivePlacementReport {
    /// Live (non-`Void`) cells placed.
    pub total_cells: u32,
    /// Solver attempts the accepted solve consumed (1-based).
    pub attempts: u32,
    pub archetype_instances: BTreeMap<String, u32>,
    pub register_instances: BTreeMap<String, u32>,
    pub report: String,
}

/// Pure tally over `(archetype, register)` rows — one per live placed cell (see
/// [`architecture_rows`]). IO/solve-free.
#[must_use]
pub fn tally_live_placements(rows: &[(String, String)], attempts: u32) -> LivePlacementReport {
    let mut archetype_instances: BTreeMap<String, u32> = BTreeMap::new();
    let mut register_instances: BTreeMap<String, u32> = BTreeMap::new();
    for (archetype, register) in rows {
        *archetype_instances.entry(archetype.clone()).or_insert(0) += 1;
        *register_instances.entry(register.clone()).or_insert(0) += 1;
    }
    let total_cells = rows.len() as u32;

    let mut report = String::new();
    report.push_str(&format!(
        "LIVE PLACEMENT DISTRIBUTION (per-seed solve)\n{total_cells} live cells across {} archetypes, {} registers (solve took {attempts} attempt(s)).\n\n",
        archetype_instances.len(),
        register_instances.len(),
    ));
    report.push_str("Per-archetype placements:\n");
    for (archetype, count) in &archetype_instances {
        let share = if total_cells == 0 {
            0.0
        } else {
            100.0 * *count as f32 / total_cells as f32
        };
        report.push_str(&format!("  - {archetype}: {count} ({share:.1}%)\n"));
    }
    report.push_str("\nPer-register placements:\n");
    for (register, count) in &register_instances {
        report.push_str(&format!("  - {register}: {count}\n"));
    }

    LivePlacementReport {
        total_cells,
        attempts,
        archetype_instances,
        register_instances,
        report,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArchetypeStats {
    pub variant_count: usize,
    pub total_instances: u32,
}

/// (archetype, register, variant)
pub type VariantKey = (String, String, u16);
/// (archetype, register, variant, rotation turn 0..=5)
pub type RotationKey = (String, String, u16, u8);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DistributionReport {
    pub total_samples: usize,
    pub total_instances: u32,
    pub archetype_stats: BTreeMap<String, ArchetypeStats>,
    pub register_instances: BTreeMap<String, u32>,
    /// Variants with zero placement/instance weight across every rotation.
    pub unused_variants: Vec<VariantKey>,
    /// Specific (variant, rotation) combinations that never place.
    pub unused_rotations: Vec<RotationKey>,
    /// Archetypes whose instance share is at least 2x the per-archetype fair share.
    pub over_represented: Vec<String>,
    /// Archetypes whose instance share is at most half the per-archetype fair share.
    pub under_represented: Vec<String>,
    pub report: String,
}

/// Pure tally over already-extracted placement/weight samples. Does not read
/// files, run a solve, or otherwise perform IO -- callers gather
/// [`CatalogSample`]s (e.g. via [`samples_from_modules`], or from a live
/// solve's placement stream) and pass them in.
pub fn tally_placement_distribution(samples: &[CatalogSample]) -> DistributionReport {
    let total_samples = samples.len();

    // Merge duplicate (archetype, register, variant) rows defensively.
    let mut merged: BTreeMap<VariantKey, BTreeMap<u8, u32>> = BTreeMap::new();
    for sample in samples {
        let entry = merged
            .entry((
                sample.archetype.clone(),
                sample.register.clone(),
                sample.variant,
            ))
            .or_default();
        for (&turn, &count) in &sample.rotation_counts {
            *entry.entry(turn).or_insert(0) += count;
        }
    }

    let mut archetype_variants: BTreeMap<String, BTreeSet<u16>> = BTreeMap::new();
    let mut archetype_instances: BTreeMap<String, u32> = BTreeMap::new();
    let mut register_instances: BTreeMap<String, u32> = BTreeMap::new();
    let mut unused_variants = Vec::new();
    let mut unused_rotations = Vec::new();
    let mut total_instances: u32 = 0;

    for ((archetype, register, variant), rotation_counts) in &merged {
        let variant_total: u32 = rotation_counts.values().sum();
        total_instances += variant_total;

        archetype_variants
            .entry(archetype.clone())
            .or_default()
            .insert(*variant);
        *archetype_instances.entry(archetype.clone()).or_insert(0) += variant_total;
        *register_instances.entry(register.clone()).or_insert(0) += variant_total;

        if variant_total == 0 {
            unused_variants.push((archetype.clone(), register.clone(), *variant));
        }
        for turn in 0..=5u8 {
            let count = rotation_counts.get(&turn).copied().unwrap_or(0);
            if count == 0 {
                unused_rotations.push((archetype.clone(), register.clone(), *variant, turn));
            }
        }
    }

    let archetype_stats: BTreeMap<String, ArchetypeStats> = archetype_variants
        .iter()
        .map(|(archetype, variants)| {
            (
                archetype.clone(),
                ArchetypeStats {
                    variant_count: variants.len(),
                    total_instances: archetype_instances.get(archetype).copied().unwrap_or(0),
                },
            )
        })
        .collect();

    let archetype_count = archetype_stats.len();
    let fair_share = if archetype_count == 0 {
        0.0
    } else {
        total_instances as f32 / archetype_count as f32
    };

    let mut over_represented = Vec::new();
    let mut under_represented = Vec::new();
    if fair_share > 0.0 {
        for (archetype, stats) in &archetype_stats {
            let ratio = stats.total_instances as f32 / fair_share;
            if ratio >= 2.0 {
                over_represented.push(archetype.clone());
            } else if ratio <= 0.5 {
                under_represented.push(archetype.clone());
            }
        }
    }

    let report = render_report(
        total_samples,
        total_instances,
        &archetype_stats,
        &register_instances,
        &unused_variants,
        &unused_rotations,
        &over_represented,
        &under_represented,
    );

    DistributionReport {
        total_samples,
        total_instances,
        archetype_stats,
        register_instances,
        unused_variants,
        unused_rotations,
        over_represented,
        under_represented,
        report,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_report(
    total_samples: usize,
    total_instances: u32,
    archetype_stats: &BTreeMap<String, ArchetypeStats>,
    register_instances: &BTreeMap<String, u32>,
    unused_variants: &[VariantKey],
    unused_rotations: &[RotationKey],
    over_represented: &[String],
    under_represented: &[String],
) -> String {
    let mut report = String::new();
    report.push_str(&format!(
        "TILE PLACEMENT-DISTRIBUTION AUDIT\n{} samples ({}, {} total instances) across {} archetypes, {} registers.\n\n",
        total_samples,
        if total_samples == 1 { "row" } else { "rows" },
        total_instances,
        archetype_stats.len(),
        register_instances.len(),
    ));

    report.push_str("Per-archetype instance share:\n");
    for (archetype, stats) in archetype_stats {
        report.push_str(&format!(
            "  - {archetype}: {} variant(s), {} instances\n",
            stats.variant_count, stats.total_instances
        ));
    }
    report.push('\n');

    report.push_str("Per-register instances:\n");
    for (register, count) in register_instances {
        report.push_str(&format!("  - {register}: {count}\n"));
    }
    report.push('\n');

    if unused_variants.is_empty() {
        report.push_str("Unused variants: none.\n");
    } else {
        report.push_str(&format!(
            "Unused variants ({}) -- authored but zero placement/weight, will never be selected:\n",
            unused_variants.len()
        ));
        for (archetype, register, variant) in unused_variants {
            report.push_str(&format!("  - {archetype}-v{variant} @ {register}\n"));
        }
    }
    report.push('\n');

    if unused_rotations.is_empty() {
        report.push_str("Unused rotations: none.\n");
    } else {
        report.push_str(&format!(
            "Unused rotations ({}) -- turn never authored/placed for that variant (may be intentional symmetry):\n",
            unused_rotations.len()
        ));
        for (archetype, register, variant, turn) in unused_rotations {
            report.push_str(&format!(
                "  - {archetype}-v{variant} @ {register}, turn {turn}\n"
            ));
        }
    }
    report.push('\n');

    if over_represented.is_empty() {
        report.push_str("Over-represented families: none.\n");
    } else {
        report.push_str(&format!(
            "Over-represented families (>=2x fair share): {}\n",
            over_represented.join(", ")
        ));
    }
    if under_represented.is_empty() {
        report.push_str("Under-represented families: none.\n");
    } else {
        report.push_str(&format!(
            "Under-represented families (<=0.5x fair share): {}\n",
            under_represented.join(", ")
        ));
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(
        archetype: &str,
        register: &str,
        variant: u16,
        rotations: &[(u8, u32)],
    ) -> CatalogSample {
        CatalogSample {
            archetype: archetype.to_string(),
            register: register.to_string(),
            variant,
            rotation_counts: rotations.iter().copied().collect(),
        }
    }

    #[test]
    fn detects_a_variant_that_is_never_placed() {
        let samples = vec![
            // hall_cap: placed under two rotations in one register.
            sample("hall_cap", "monolith", 0, &[(0, 5), (1, 5)]),
            // hall_turn: placed once.
            sample("hall_turn", "monolith", 0, &[(0, 3)]),
            // hall_dead_end: authored (weight-derived rotations present) but
            // every count is zero -- e.g. a weight: 0 module in the catalog.
            sample(
                "hall_dead_end",
                "monolith",
                0,
                &[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
            ),
        ];

        let out = tally_placement_distribution(&samples);

        assert_eq!(out.total_samples, 3);
        assert_eq!(out.total_instances, 13);
        assert_eq!(out.archetype_stats.len(), 3);
        assert_eq!(
            out.archetype_stats["hall_cap"],
            ArchetypeStats {
                variant_count: 1,
                total_instances: 10,
            }
        );
        assert_eq!(
            out.archetype_stats["hall_turn"],
            ArchetypeStats {
                variant_count: 1,
                total_instances: 3,
            }
        );
        assert_eq!(
            out.archetype_stats["hall_dead_end"],
            ArchetypeStats {
                variant_count: 1,
                total_instances: 0,
            }
        );

        assert_eq!(
            out.unused_variants,
            vec![("hall_dead_end".to_string(), "monolith".to_string(), 0)]
        );
        assert!(out.report.contains("hall_dead_end-v0 @ monolith"));
    }

    #[test]
    fn detects_unused_rotations_for_a_partially_placed_variant() {
        let samples = vec![sample("hall_cap", "monolith", 0, &[(0, 5), (1, 5)])];

        let out = tally_placement_distribution(&samples);

        // Turns 2..=5 never appear in the sample -> unused rotations.
        let missing: Vec<u8> = out
            .unused_rotations
            .iter()
            .map(|(_, _, _, turn)| *turn)
            .collect();
        assert_eq!(missing, vec![2, 3, 4, 5]);
        assert!(out.unused_variants.is_empty());
    }

    #[test]
    fn flags_over_and_under_represented_families() {
        let samples = vec![
            sample("hall_cap", "monolith", 0, &[(0, 90)]),
            sample("hall_turn", "monolith", 0, &[(0, 5)]),
            sample("hall_junction", "monolith", 0, &[(0, 5)]),
        ];

        let out = tally_placement_distribution(&samples);

        assert_eq!(out.over_represented, vec!["hall_cap".to_string()]);
        assert_eq!(
            out.under_represented,
            vec!["hall_junction".to_string(), "hall_turn".to_string()]
        );
    }

    #[test]
    fn live_tally_counts_archetypes_and_registers_per_cell() {
        let rows = vec![
            ("straight".to_string(), "monolith".to_string()),
            ("straight".to_string(), "monolith".to_string()),
            ("junction".to_string(), "wellshaft".to_string()),
            ("room".to_string(), "monolith".to_string()),
        ];
        let out = tally_live_placements(&rows, 3);
        assert_eq!(out.total_cells, 4);
        assert_eq!(out.attempts, 3);
        assert_eq!(out.archetype_instances["straight"], 2);
        assert_eq!(out.archetype_instances["junction"], 1);
        assert_eq!(out.archetype_instances["room"], 1);
        assert_eq!(out.register_instances["monolith"], 3);
        assert_eq!(out.register_instances["wellshaft"], 1);
        assert!(out.report.contains("straight: 2 (50.0%)"));
    }

    #[test]
    fn architecture_rows_skip_void_and_map_archetype_and_register() {
        use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld};

        // A real solve so the mapping is exercised end to end; arc_default is
        // the representative facility. Deterministic in the seed.
        let world = HexWfcWorld::generate(0xa11c_0000_0000_0000, HexWfcConfig::arc_default())
            .expect("arc_default solve");
        let rows = architecture_rows(&world);
        // Every row is a live cell (no void) and carries a non-empty label pair.
        assert!(!rows.is_empty());
        assert!(
            rows.iter()
                .all(|(a, r)| a != "void" && !a.is_empty() && !r.is_empty())
        );
        // The row count equals the non-void placements.
        let live = world
            .placements
            .values()
            .filter(|p| p.archetype != HexArchetype::Void)
            .count();
        assert_eq!(rows.len(), live);
    }

    #[test]
    fn samples_from_modules_expands_one_row_per_register_in_scope() {
        use crate::catalog::CompiledModule;
        use crate::source::ModuleKind;

        let module = CompiledModule {
            id: "authored/hall_cap".to_string(),
            source_path: "authored/hall_cap.map".to_string(),
            source_sha256: String::new(),
            kind: ModuleKind::Cell,
            archetype: "hall_cap".to_string(),
            variant: 0,
            levels: 1,
            room_role: None,
            register_scope: vec!["monolith".to_string(), "wellshaft".to_string()],
            rotations: vec![0, 1],
            weight: 10,
            footprint: Vec::new(),
            ports: Vec::new(),
            lights: Vec::new(),
            structural_hash: String::new(),
            legacy_key: None,
        };

        let samples = samples_from_modules(std::slice::from_ref(&module));
        assert_eq!(samples.len(), 2);
        assert!(samples.iter().any(|s| s.register == "monolith"));
        assert!(samples.iter().any(|s| s.register == "wellshaft"));
        for sample in &samples {
            assert_eq!(sample.rotation_counts.get(&0), Some(&10));
            assert_eq!(sample.rotation_counts.get(&1), Some(&10));
            assert_eq!(sample.rotation_counts.get(&2), None);
        }
    }
}
