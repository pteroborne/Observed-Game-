use std::path::{Path, PathBuf};

use observed_authoring::{
    CompositionBuild, CorpusCertificationReport, ModuleKind, ModuleSummary, build_catalog,
    certify_catalog, certify_catalog_selection, fold_simulation_content_hash, load_profile,
    new_module_template, parse_authored_module, write_catalog_build, write_profile_build,
};
use observed_traversal::TraversalRuntimeProfile;

fn usage() -> ! {
    eprintln!(
        "Observed 2 tile compiler\n\n\
         tilec new <cell|room> <stable-id> <output.map>\n\
         tilec validate <source.map>\n\
         tilec audit [source-root]\n\
         tilec audit-seams [source-root]\n\
         tilec audit-distribution [source-root]\n\
         tilec audit-districts [source-root]\n\
         tilec gen-tower [output-dir]\n\
         tilec render-cad [output.svg]\n\
         tilec build [source-root] [catalog.ron] [manifest.ron]\n\
         tilec emit-fgd [output.fgd]\n\
         tilec gen-tiles [output-dir]\n\n\
         Traversal certification (the production follower, profile, and Rapier):\n\
         tilec certify <module-id|family|archetype> [source-root]\n\
         tilec certify-corpus [source-root] [certificate.ron]\n\n\
         Parametric recipes (a module described as data, not code):\n\
         tilec new-recipe <id> [out.recipe.ron]\n\
         tilec bake-recipe <in.recipe.ron> [out.map]\n\n\
         Composition profile (the authored solve controls):\n\
         tilec profile-new [source-root]\n\
         tilec profile-validate [source-root]\n\
         tilec profile-hash [source-root]\n\
         tilec profile-write [source-root]"
    );
    std::process::exit(2);
}

fn read_module(path: &Path) -> Result<observed_authoring::AuthoredModule, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    parse_authored_module(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("tilec: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        usage();
    };
    match command.as_str() {
        "new" => {
            let kind = match args.next().as_deref() {
                Some("cell") => ModuleKind::Cell,
                Some("room") => ModuleKind::Room,
                _ => usage(),
            };
            let id = args.next().unwrap_or_else(|| usage());
            let output = PathBuf::from(args.next().unwrap_or_else(|| usage()));
            if args.next().is_some() {
                usage();
            }
            if output.exists() {
                return Err(format!("{} already exists", output.display()));
            }
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            std::fs::write(&output, new_module_template(&id, kind))
                .map_err(|error| format!("{}: {error}", output.display()))?;
            println!("created {} ({kind:?})", output.display());
        }
        "validate" => {
            let path = PathBuf::from(args.next().unwrap_or_else(|| usage()));
            if args.next().is_some() {
                usage();
            }
            let module = read_module(&path)?;
            let summary = ModuleSummary::from(&module);
            println!(
                "valid: {} | {:?} | {} footprint cells | {} ports | {} sockets | {} hulls | {} lights | contract {}",
                summary.id,
                summary.kind,
                summary.footprint_cells,
                summary.ports,
                summary.sockets,
                summary.hulls,
                summary.lights,
                if summary.strict {
                    "v2 strict"
                } else {
                    "v1 legacy"
                }
            );
        }
        "audit" => {
            let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/tiles".to_string()));
            if args.next().is_some() {
                usage();
            }
            let built = build_catalog(&root).map_err(|error| error.to_string())?;
            let audit = built.audit;
            println!(
                "valid: {} sources ({} strict, {} legacy) | {} hull sets ({} shared references) | {} runtime entries\ncontent hash {}",
                audit.sources,
                audit.strict_sources,
                audit.legacy_sources,
                audit.hull_sets,
                audit.shared_hull_references,
                audit.compatibility_manifest_entries,
                audit.content_hash
            );
        }
        "audit-seams" => {
            let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/tiles".to_string()));
            if args.next().is_some() {
                usage();
            }
            let report = observed_authoring::seam_auditor::audit_seams(&root)?;
            println!("{}", report.report);
        }
        "audit-distribution" => {
            let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/tiles".to_string()));
            if args.next().is_some() {
                usage();
            }
            let built = build_catalog(&root).map_err(|error| error.to_string())?;

            // Live per-seed placements from a representative solve.
            use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld};
            let seed = 0xa11c_0000_0000_0000;
            match HexWfcWorld::generate(seed, HexWfcConfig::arc_default()) {
                Ok(world) => {
                    let rows = observed_authoring::distribution::architecture_rows(&world);
                    let live = observed_authoring::distribution::tally_live_placements(
                        &rows,
                        world.last_attempts,
                    );
                    println!("{}", live.report);
                }
                Err(error) => {
                    println!("LIVE PLACEMENT DISTRIBUTION: solve failed ({error:?}) — skipping.\n");
                }
            }

            // Catalog alphabet (variant/rotation granularity the solver lacks).
            let samples =
                observed_authoring::distribution::samples_from_modules(&built.catalog.modules);
            let report = observed_authoring::distribution::tally_placement_distribution(&samples);
            println!("{}", report.report);
        }
        "gen-tower" => {
            let out_dir = PathBuf::from(
                args.next()
                    .unwrap_or_else(|| "assets/tiles/authored".to_string()),
            );
            if args.next().is_some() {
                usage();
            }
            let generated = observed_authoring::generator::generate_tower_tiles(&out_dir)?;
            println!(
                "generated {} tower tiles -> {}",
                generated.len(),
                out_dir.display()
            );
        }
        "render-cad" => {
            let first = args.next();
            let second = args.next();
            if args.next().is_some() {
                usage();
            }
            let (source_map, out_path) = match (first, second) {
                (Some(map), Some(out)) => (Some(PathBuf::from(map)), PathBuf::from(out)),
                (Some(path), None) => {
                    if path.ends_with(".map") {
                        (
                            Some(PathBuf::from(&path)),
                            PathBuf::from("docs/evidence/cad_blueprint.svg"),
                        )
                    } else {
                        (None, PathBuf::from(path))
                    }
                }
                (None, None) => (
                    None,
                    PathBuf::from("docs/evidence/tower_7hex/cad_blueprint.svg"),
                ),
                _ => usage(),
            };

            if let Some(map_path) = source_map {
                let module = read_module(&map_path)?;
                let dynamic_hulls: Vec<_> = module
                    .prototype
                    .hulls
                    .iter()
                    .enumerate()
                    .map(|(id, points)| observed_authoring::DynamicHull {
                        id: id as u32,
                        label: format!("Hull #{}", id),
                        points: points.iter().map(|p| [p.x, p.y, p.z]).collect(),
                    })
                    .collect();

                let title = format!("OBSERVED 2 CAD BLUEPRINT: {}", module.id);
                let subtitle = format!(
                    "ARCHETYPE: {} | KIND: {:?} | FOOTPRINT CELLS: {}",
                    module.archetype,
                    module.kind,
                    module.footprint.len()
                );
                observed_authoring::render_dynamic_cad_blueprint(
                    &title,
                    &subtitle,
                    &dynamic_hulls,
                    &out_path,
                )?;
                println!(
                    "rendered dynamic CAD blueprint for {} -> {}",
                    map_path.display(),
                    out_path.display()
                );
            } else {
                observed_authoring::render_cad_blueprint(&out_path)?;
                println!("rendered default CAD blueprint -> {}", out_path.display());
            }

            let jpg_path = out_path.with_extension("jpg");
            let _ = std::process::Command::new("python")
                .args([
                    "tools/convert_svg_to_jpg.py",
                    &out_path.to_string_lossy(),
                    &jpg_path.to_string_lossy(),
                ])
                .status();
        }
        "audit-districts" => {
            let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/tiles".to_string()));
            if args.next().is_some() {
                usage();
            }
            let audit = observed_authoring::audit_district_variations(&root)
                .map_err(|error| error.to_string())?;
            println!("{}", audit.report);
        }
        "build" => {
            let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/tiles".to_string()));
            let catalog_path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join("compiled_catalog.ron"));
            let manifest_path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join("manifest.ron"));
            if args.next().is_some() {
                usage();
            }
            let built = build_catalog(&root).map_err(|error| error.to_string())?;
            write_catalog_build(&built, &catalog_path, &manifest_path)
                .map_err(|error| error.to_string())?;
            println!(
                "built {} modules -> {}\ncompatibility manifest: {} entries -> {}\ncontent hash {}",
                built.audit.sources,
                catalog_path.display(),
                built.audit.compatibility_manifest_entries,
                manifest_path.display(),
                built.audit.content_hash
            );
        }
        // The composition profile is content, like the compiled catalog: it is
        // generated, hashed, committed, and never hand-edited. These four
        // commands are the whole supported way to touch it from the shell.
        "profile-new" => {
            let root = profile_root(&mut args);
            let path = root.join(observed_authoring::COMPOSITION_PROFILE_FILE);
            if path.exists() {
                return Err(format!(
                    "{} already exists; use profile-write to regenerate it",
                    path.display()
                ));
            }
            let build = CompositionBuild::baseline().map_err(|error| error.to_string())?;
            write_profile_build(&build, &root).map_err(|error| error.to_string())?;
            println!(
                "created {} at the baseline composition\ncontent hash {}",
                path.display(),
                build.content_hash
            );
        }
        "profile-validate" => {
            let root = profile_root(&mut args);
            let build = load_profile(&root).map_err(|error| error.to_string())?;
            let profile = &build.profile;
            println!(
                "{} v{} label {:?}\n\
                 tendencies {}\n\
                 district biases {}  pin sets {}  pins {}\n\
                 search candidates {}\n\
                 content hash {}\n\
                 {}",
                observed_authoring::COMPOSITION_PROFILE_FILE,
                profile.version,
                profile.label,
                if profile.tendencies.enabled {
                    "on"
                } else {
                    "off"
                },
                profile.district_bias.len(),
                profile.pin_sets.len(),
                profile
                    .pin_sets
                    .iter()
                    .map(|set| set.pins.len())
                    .sum::<usize>(),
                profile.search.candidates,
                build.content_hash,
                if profile.is_baseline() {
                    "baseline (solves exactly as an unprofiled build)"
                } else {
                    "authored (changes what the solver builds)"
                },
            );
        }
        "profile-hash" => {
            let root = profile_root(&mut args);
            let profile = load_profile(&root).map_err(|error| error.to_string())?;
            let catalog_sidecar = root.join("compiled_catalog.sha256");
            let catalog_hash = std::fs::read_to_string(&catalog_sidecar)
                .map_err(|error| format!("{}: {error}", catalog_sidecar.display()))?;
            let folded = fold_simulation_content_hash(catalog_hash.trim(), &profile.content_hash);
            let folded_hex: String = folded.iter().map(|byte| format!("{byte:02x}")).collect();
            println!(
                "catalog     {}\nprofile     {}\nsimulation  {folded_hex}\n\n\
                 The simulation hash is what a LAN peer must match to join.",
                catalog_hash.trim(),
                profile.content_hash
            );
        }
        "profile-write" => {
            let root = profile_root(&mut args);
            let path = root.join(observed_authoring::COMPOSITION_PROFILE_FILE);
            // Re-serializing what is already there is how a profile edited by
            // the studio gets its sidecar back in agreement; falling back to the
            // baseline is how a repository without one gets its first.
            let build = match std::fs::read_to_string(&path) {
                Ok(text) => {
                    let profile = observed_authoring::parse_profile(&text)
                        .map_err(|error| error.to_string())?;
                    CompositionBuild::new(profile).map_err(|error| error.to_string())?
                }
                Err(_) => CompositionBuild::baseline().map_err(|error| error.to_string())?,
            };
            write_profile_build(&build, &root).map_err(|error| error.to_string())?;
            println!(
                "wrote {} and its sidecar\ncontent hash {}",
                path.display(),
                build.content_hash
            );
        }
        "new-recipe" => {
            let id = args.next().unwrap_or_else(|| usage());
            let path = PathBuf::from(args.next().unwrap_or_else(|| {
                format!(
                    "assets/tiles/authored/{}{}",
                    observed_authoring::forge::recipe::file_stem(&id),
                    observed_authoring::forge::recipe::RECIPE_SUFFIX
                )
            }));
            // Never clobber: a recipe is work, and `new` is the command someone
            // reaches for by reflex.
            if path.exists() {
                return Err(format!(
                    "{} already exists; edit it or pass another path",
                    path.display()
                ));
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let recipe = observed_authoring::forge::recipe::Recipe::starter(&id);
            recipe.validate().map_err(|error| format!("{error:?}"))?;
            std::fs::write(&path, recipe.to_ron().as_bytes()).map_err(|error| error.to_string())?;
            println!(
                "wrote {}\npreview it live: cargo run -p composition_studio --bin module-studio",
                path.display()
            );
        }
        "bake-recipe" => {
            let source = PathBuf::from(args.next().unwrap_or_else(|| usage()));
            let text = std::fs::read_to_string(&source)
                .map_err(|error| format!("{}: {error}", source.display()))?;
            let recipe = observed_authoring::forge::recipe::Recipe::parse(&text)?;
            // Refuse to bake something the importer would reject. A `.map` that
            // fails validation in the corpus fails the whole `tilec build`, so
            // the cheap moment to catch it is here.
            recipe.validate().map_err(|error| format!("{error:?}"))?;
            let target = PathBuf::from(args.next().unwrap_or_else(|| {
                source
                    .with_file_name(format!(
                        "{}.map",
                        observed_authoring::forge::recipe::file_stem(&recipe.id)
                    ))
                    .display()
                    .to_string()
            }));
            std::fs::write(&target, recipe.build().as_bytes())
                .map_err(|error| error.to_string())?;
            println!(
                "baked {} -> {} ({} steps)",
                source.display(),
                target.display(),
                recipe.steps.len()
            );
        }
        "gen-tiles" => {
            let dir = PathBuf::from(
                args.next()
                    .unwrap_or_else(|| "assets/tiles/authored".to_string()),
            );
            if args.next().is_some() {
                usage();
            }
            std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            let generated = observed_authoring::forge::generate_all();
            let mut changed = 0usize;
            for (name, text) in &generated {
                let path = dir.join(format!("{name}.map"));
                // Only rewrite what actually moved. The corpus is committed, and
                // touching 58 mtimes every run makes the module watcher
                // re-validate everything for nothing.
                let current = std::fs::read_to_string(&path).unwrap_or_default();
                if current.replace("\r\n", "\n") == *text {
                    continue;
                }
                // Written as bytes so Windows does not translate the LF endings
                // the byte-identity gate asserts on.
                std::fs::write(&path, text.as_bytes()).map_err(|error| error.to_string())?;
                changed += 1;
            }
            println!(
                "{} modules generated into {} ({changed} changed)",
                generated.len(),
                dir.display()
            );
        }
        // The fast command: one module, family, or archetype. Compiles the
        // corpus in memory and writes nothing.
        "certify" => {
            let selector = args.next().unwrap_or_else(|| usage());
            let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/tiles".to_string()));
            if args.next().is_some() {
                usage();
            }
            let built = build_catalog(&root).map_err(|error| error.to_string())?;
            let report = certify_catalog_selection(
                &built.catalog,
                &selector,
                &TraversalRuntimeProfile::canonical_hex(),
            );
            print_certification(&report, &selector)?;
        }
        // The integration command. A certificate is generated content, so it
        // is only written when a path is named explicitly; the catalog
        // publisher owns committing one.
        "certify-corpus" => {
            let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/tiles".to_string()));
            let output = args.next().map(PathBuf::from);
            if args.next().is_some() {
                usage();
            }
            let built = build_catalog(&root).map_err(|error| error.to_string())?;
            let report = certify_catalog(&built.catalog, &TraversalRuntimeProfile::canonical_hex());
            let verdict = print_certification(&report, &root.display().to_string());
            if let Some(path) = output {
                let text = report.to_pretty_ron()?;
                std::fs::write(&path, text.as_bytes())
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                println!("wrote {}", path.display());
            }
            verdict?;
        }
        "emit-fgd" => {
            // Defaults to the committed path so the common case is "regenerate
            // in place"; a explicit path is for diffing before overwriting.
            let path = PathBuf::from(
                args.next()
                    .unwrap_or_else(|| observed_authoring::fgd::FGD_PATH.to_string()),
            );
            if args.next().is_some() {
                usage();
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let text = observed_authoring::fgd::emit_fgd();
            // Written as bytes, not via a text writer: Windows would otherwise
            // translate the LF endings the byte-identity test asserts on.
            std::fs::write(&path, text.as_bytes()).map_err(|error| error.to_string())?;
            println!(
                "wrote {} ({} entities, {} bytes)",
                path.display(),
                text.matches("@PointClass").count(),
                text.len()
            );
        }
        _ => usage(),
    }
    Ok(())
}

/// Print one certification report and say whether it passed.
///
/// The tally is printed unconditionally so a clean run can never be read as
/// evidence that anything was actually walked: a corpus of pre-contract
/// modules reports zero contract certificates, and that is the honest answer
/// until the v3 compiler publishes them.
fn print_certification(report: &CorpusCertificationReport, scope: &str) -> Result<(), String> {
    let (contract, compatibility, not_applicable) = report.tally();
    let profile_hex: String = report.profile_hash[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let columns = report
        .vertical_cases
        .iter()
        .filter(|case| case.cells > 2)
        .count();
    println!(
        "{scope}: {} modules ({contract} contract, {compatibility} compatibility, {not_applicable} no traversal)\n\
         vertical interface classes: {} certified ({columns} representative column{})\n\
         runtime profile {profile_hex}",
        report.modules.len(),
        report.vertical_cases.len() - columns,
        if columns == 1 { "" } else { "s" }
    );

    let mut failures = report.failures.clone();
    for module in &report.modules {
        failures.extend(module.failures.iter().cloned());
    }
    if failures.is_empty() {
        println!("certified: no traversal fault found");
        return Ok(());
    }
    for failure in &failures {
        println!(
            "\nFAILED {} [{:?}] at stage {} tick {}\n  {}",
            failure.module_id, failure.kind, failure.stage, failure.tick, failure.message
        );
        if let (Some(entry), Some(exit)) = (&failure.entry, &failure.exit) {
            println!("  pair {entry:?} -> {exit:?}");
        }
        if let Some(feet) = failure.last_feet {
            println!("  last feet {feet:?} target {:?}", failure.last_target);
        }
    }
    Err(format!(
        "{} traversal certification failures",
        failures.len()
    ))
}

/// Every profile command takes the same optional source root.
fn profile_root(args: &mut impl Iterator<Item = String>) -> PathBuf {
    let root = PathBuf::from(args.next().unwrap_or_else(|| "assets/tiles".to_string()));
    if args.next().is_some() {
        usage();
    }
    root
}
