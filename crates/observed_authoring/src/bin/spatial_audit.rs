//! Reproducible initial eight-floor composition survey. Counts are evidence,
//! never a substitute for recognising places during a human playthrough.
use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld};
use observed_facility::map_spec::RoomRole;

fn main() {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/tiles".into());
    let catalog = observed_authoring::RuntimeHexCatalog::load(
        std::path::Path::new(&root),
        observed_authoring::tile_source::REGISTERS,
    )
    .expect("validated production content");
    let hash = catalog
        .simulation_content_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    eprintln!("24x17x8, shipped 9..=10-room policy, simulation {hash}");
    println!(
        "seed,rooms,floor0,floor1,floor2,floor3,floor4,floor5,floor6,floor7,decision_floor,attempts,solve_ms"
    );
    let config = HexWfcConfig::arc_default();
    // The runtime's current compact production board does not activate
    // the older 28x20x10 repeated-objective quota. Audit what players launch.
    for index in 0..24u64 {
        let seed = if index == 0 {
            1
        } else {
            0xA11C_E3D0_0000_0000 ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        };
        let started = std::time::Instant::now();
        match HexWfcWorld::generate_with_profile(seed, config, None, &catalog.composition) {
            Ok(world) => {
                let mut rooms = [0usize; 8];
                for room in &world.blueprints {
                    rooms[usize::from(room.anchor.level)] += 1;
                }
                let floors = rooms
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let decision = world
                    .blueprints
                    .iter()
                    .find(|room| room.role == RoomRole::Decision)
                    .map_or_else(String::new, |room| room.anchor.level.to_string());
                println!(
                    "{seed},{},{floors},{decision},{},{:.3}",
                    world.blueprints.len(),
                    world.last_attempts,
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
            Err(error) => {
                eprintln!("seed {seed}: {error:?}");
                std::process::exit(1);
            }
        }
    }
}
