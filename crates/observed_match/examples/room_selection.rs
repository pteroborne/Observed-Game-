//! End-to-end proof that authored **whole-room modules** are actually selected
//! by the projector instead of the per-cell fallback.
//!
//! Run with:
//!
//! ```text
//! cargo run -p observed_match --example room_selection
//! ```
//!
//! Multi-hex rooms authored as N separate single-cell tiles render as N sealed
//! rooms with a wall at every internal seam. Authored as one room module they
//! render as a single continuous space. This loads the committed catalog,
//! solves a facility, projects it, and reports for every stamped room blueprint
//! whether the whole-room path or the per-cell fallback was taken.
//!
//! Detection: `push_room` stamps `source_cell = anchor` on every hull of a
//! whole-room module and consumes the whole footprint, so a room module shows
//! up as "non-anchor footprint cells emit no pieces of their own".

use std::collections::{BTreeMap, BTreeSet};

use observed_authoring::RuntimeHexCatalog;
use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld};
use observed_match::hex_wfc::HexWfcGeometrySnapshot;

const REGISTERS: [&str; 10] = [
    "shadow_screen",
    "monolith",
    "overlit_grid",
    "institutional",
    "facet_monument",
    "megastructure",
    "wellshaft",
    "infinite_gallery",
    "thinning",
    "liminal_grid",
];

fn main() {
    let base = std::path::Path::new("assets/tiles");
    let catalog = RuntimeHexCatalog::load(base, &REGISTERS).expect("committed catalog loads");
    println!(
        "catalog: {} cell prototypes, {} room prototypes",
        catalog.cells.len(),
        catalog.rooms.len()
    );
    for room in &catalog.rooms {
        println!(
            "  room module {:<32} role={:<14} cells={} ports={} hulls={}",
            room.id,
            room.room_role,
            room.footprint.len(),
            room.ports.len(),
            room.hulls.len()
        );
    }
    if catalog.rooms.is_empty() {
        println!("\nNo room modules in the catalog — every room will fragment into sealed cells.");
        return;
    }

    let config = HexWfcConfig {
        levels: 4,
        ..HexWfcConfig::default()
    };
    let seed = 0xA11C_0104_0000_0000;
    let world = HexWfcWorld::generate(seed, config).expect("facility solves");
    let snapshot =
        HexWfcGeometrySnapshot::project_with_rooms(&world, &catalog.cells, &catalog.rooms)
            .expect("projection succeeds");

    // Which cells emitted pieces of their own?
    let emitting: BTreeSet<_> = snapshot
        .pieces
        .iter()
        .map(|piece| piece.source_cell)
        .collect();

    let mut whole_room: BTreeMap<String, usize> = BTreeMap::new();
    let mut fragmented: BTreeMap<String, usize> = BTreeMap::new();
    let mut single_cell: BTreeMap<String, usize> = BTreeMap::new();
    for stamped in &world.blueprints {
        let role = format!("{:?}", stamped.role);
        if stamped.cells.len() == 1 {
            // Single-cell roles have no seams to hide; the per-cell path is
            // already correct for them.
            *single_cell.entry(role).or_default() += 1;
            continue;
        }
        // A whole-room module consumes the entire footprint under the anchor,
        // so non-anchor cells emit nothing of their own.
        let non_anchor_emitting = stamped
            .cells
            .iter()
            .filter(|cell| **cell != stamped.anchor && emitting.contains(cell))
            .count();
        if non_anchor_emitting == 0 {
            *whole_room.entry(role).or_default() += 1;
        } else {
            *fragmented.entry(role).or_default() += 1;
        }
    }

    let report = |title: &str, rows: &BTreeMap<String, usize>| {
        println!("  {title}");
        if rows.is_empty() {
            println!("    (none)");
        }
        for (role, count) in rows {
            println!("    {role:<18} x{count}");
        }
    };

    println!("\nStamped room blueprints in seed {seed:#018x}:");
    report(
        "multi-cell, rendered as ONE whole-room module:",
        &whole_room,
    );
    report(
        "multi-cell, FRAGMENTED into sealed cells (no matching module):",
        &fragmented,
    );
    report("single-cell (per-cell path is correct):", &single_cell);
}
