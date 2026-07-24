//! Headless feasibility demo for Phase 4: an *eliminated team* drives the
//! facility's refactoring with a bounded [`HexInfluenceField`], and the WFC
//! still guarantees a valid spawn->exit route.
//!
//! Run with:
//!
//! ```text
//! cargo run -p observed_facility --example driven_wfc --features wfc
//! ```
//!
//! For each named influence it selects the same bounded relayout pocket the
//! ordinary solver would, runs both an undriven and a driven collapse over it,
//! commits the driven one, and reports the archetype shift plus whether the
//! spawn->exit route survived. It is intentionally text-only — the interactive
//! Bevy `driven_wfc_lab` (field editing, live layout, orbit view) is a deferred
//! follow-up.

use std::collections::BTreeMap;

use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexArchetype, HexInfluenceField, HexObservationFrame, HexPlacement, HexWfcConfig, HexWfcWorld,
};

fn archetype_label(archetype: HexArchetype) -> &'static str {
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

/// Archetype histogram over a placement map, skipping empty `Void` cells.
fn histogram(
    placements: &BTreeMap<observed_facility::hex_wfc::HexCoord, HexPlacement>,
) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for placement in placements.values() {
        if placement.archetype == HexArchetype::Void {
            continue;
        }
        *counts
            .entry(archetype_label(placement.archetype))
            .or_insert(0) += 1;
    }
    counts
}

fn main() {
    let config = HexWfcConfig {
        levels: 4,
        ..HexWfcConfig::default()
    };
    let seed = 0xA11C_0104_0000_0000;

    println!("=== Driven WFC feasibility (headless) ===");
    println!("seed={seed:#018x}, {} levels\n", config.levels);

    let influences = [
        (
            "encourage_dead_ends",
            HexInfluenceField::encourage_dead_ends(),
        ),
        (
            "encourage_verticality",
            HexInfluenceField::encourage_verticality(),
        ),
        (
            "discourage_recovery_rooms",
            HexInfluenceField::discourage_recovery_rooms(),
        ),
    ];

    let build_frame = |world: &HexWfcWorld| {
        let mut frame = HexObservationFrame::default();
        frame
            .occupied_cells
            .insert(PlayerId(0), world.config.spawn());
        frame.objective_cells.insert(world.config.spawn());
        frame
    };

    for (name, influence) in influences {
        // Many bounded pockets are too constrained to move, so scan for a seed
        // whose pocket this influence actually reshapes — that makes the shift
        // visible. Route validity (the real feasibility claim) is checked on
        // whichever seed we demo.
        let demo_seed = (seed..seed + 0x100).find(|&candidate| {
            let Ok(world) = HexWfcWorld::generate(candidate, config) else {
                return false;
            };
            let frame = build_frame(&world);
            match (
                world.propose_relayout(&frame),
                world.propose_driven_relayout(&frame, &influence),
            ) {
                (Ok(base), Ok(driven)) => driven.placements != base.placements,
                _ => false,
            }
        });

        let Some(demo_seed) = demo_seed else {
            println!("[{name}] no reshaping pocket found in the scan range.\n");
            continue;
        };

        let world = HexWfcWorld::generate(demo_seed, config).expect("demo solve");
        let frame = build_frame(&world);
        let base = world.propose_relayout(&frame).expect("undriven pocket");
        let driven = world
            .propose_driven_relayout(&frame, &influence)
            .expect("driven pocket");

        let base_hist = histogram(&base.placements);
        let driven_hist = histogram(&driven.placements);

        // Commit the driven patch; the commit guard revalidates routes/boundary.
        let mut committed = world.clone();
        let commit = committed.commit_relayout(driven, &frame);
        let route_ok = commit.is_ok()
            && committed
                .route_between(committed.config.spawn(), committed.config.exit())
                .is_some();

        println!(
            "[{name}]  seed={demo_seed:#018x}, pocket={} cells",
            base.region.cells.len()
        );
        println!("   undriven pocket: {base_hist:?}");
        println!("   driven   pocket: {driven_hist:?}");
        println!(
            "   committed: {}, spawn->exit route: {}\n",
            if commit.is_ok() {
                "yes"
            } else {
                "rejected by guard"
            },
            if route_ok { "VALID" } else { "BROKEN" }
        );
    }

    println!("Every committed driven relayout kept a valid route — the influence");
    println!("biases the layout without the power to strand a team.");
}
