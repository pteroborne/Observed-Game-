use std::path::{Path, PathBuf};

use observed_authoring::{
    ModuleKind, ModuleSummary, build_catalog, new_module_template, parse_authored_module,
    write_catalog_build,
};

fn usage() -> ! {
    eprintln!(
        "Observed 2 tile compiler\n\n\
         tilec new <cell|room> <stable-id> <output.map>\n\
         tilec validate <source.map>\n\
         tilec audit [source-root]\n\
         tilec audit-seams [source-root]\n\
         tilec gen-tower [output-dir]\n\
         tilec render-cad [output.svg]\n\
         tilec build [source-root] [catalog.ron] [manifest.ron]"
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
                "valid: {} | {:?} | {} footprint cells | {} ports | {} hulls | {} lights | contract {}",
                summary.id,
                summary.kind,
                summary.footprint_cells,
                summary.ports,
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
        _ => usage(),
    }
    Ok(())
}
