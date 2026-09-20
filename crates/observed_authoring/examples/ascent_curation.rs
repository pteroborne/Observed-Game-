//! Compare the existing composition with rarer incidental ascents, then optionally
//! write the validated profile and hash together. Required route pins remain intact.
use observed_authoring::{CompositionBuild, load_profile, write_profile_build};
use observed_facility::hex_wfc::profile::HexCompositionProfile;
use observed_facility::hex_wfc::{HexArchetype, HexSpace, HexWfcConfig, HexWfcWorld};
use std::path::Path;

fn rare_ascents(mut profile: HexCompositionProfile) -> HexCompositionProfile {
    profile.label = "architect_ascent_rare_circulation".to_string();
    for kind in [
        HexArchetype::RampUp,
        HexArchetype::RampHead,
        HexArchetype::Shaft,
    ] {
        profile.archetype_bias = profile.archetype_bias.with(kind, 0.25);
    }
    // Shaft variants already have integer weights of one or two. Merely lowering
    // their bias hits the positive-weight floor; raise lateral alternatives too.
    for kind in [
        HexArchetype::Straight,
        HexArchetype::Corner,
        HexArchetype::Junction,
        HexArchetype::Expanse,
    ] {
        profile.archetype_bias = profile.archetype_bias.with(kind, 4.0);
    }
    profile
}
fn survey(
    profile: &HexCompositionProfile,
    config: HexWfcConfig,
    seeds: std::ops::Range<u64>,
) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for seed in seeds {
        let world = HexWfcWorld::generate_with_profile(seed, config, None, profile)
            .expect("survey seed solves");
        assert!(
            world.route_between(config.spawn(), config.exit()).is_some(),
            "seed {seed} loses the required ascent route"
        );
        for tile in world.placements.values() {
            counts.0 += usize::from(tile.space != HexSpace::Void);
            counts.1 += usize::from(tile.archetype == HexArchetype::RampUp);
            counts.2 += usize::from(tile.archetype == HexArchetype::Shaft);
        }
    }
    counts
}
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
    let original = HexCompositionProfile::baseline();
    let proposed = rare_ascents(
        load_profile(&root)
            .expect("committed profile loads")
            .profile,
    );
    proposed.validate().expect("profile is bounded and valid");
    let compact = HexWfcConfig {
        levels: 4,
        ..HexWfcConfig::default()
    };
    for (label, config, seeds) in [
        ("compact 12x9x4 / seeds 0..16", compact, 0..16),
        ("production / seeds 0..4", HexWfcConfig::arc_default(), 0..4),
    ] {
        for (name, profile) in [("before", &original), ("after", &proposed)] {
            let (occupied, ramps, shafts) = survey(profile, config, seeds.clone());
            println!(
                "{label}: {name}: {occupied} occupied / {ramps} ramp starts / {shafts} tower cells"
            );
        }
    }
    if std::env::args().any(|arg| arg == "--write") {
        let build = CompositionBuild::new(proposed).expect("profile serializes");
        write_profile_build(&build, &root).expect("profile and hash written together");
        println!("wrote profile {}", build.content_hash);
    }
}
