//! Architecture ratchet for Arc G (docs/refactor_game_arc_plan.md): these tests scan
//! the crate's own sources and fail if the dissolved `screens` hub patterns creep back.
//! They enforce structure the compiler can't: no glob re-exports between modules, no
//! `use super::*` outside test modules, and the sim → view one-way street.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("src dir is readable") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn each_source() -> Vec<(PathBuf, String)> {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_sources(&src, &mut files);
    files
        .into_iter()
        .map(|p| {
            let text = fs::read_to_string(&p).expect("source file is readable");
            (p, text)
        })
        .collect()
}

/// Glob re-exports (`pub use x::*` / `pub(crate) use x::*`) rebuild the old any-to-any
/// hub: nothing states what it depends on, and every name change ripples everywhere.
/// Re-export explicitly by name instead.
#[test]
fn no_glob_reexports_between_game_modules() {
    let mut offenders = Vec::new();
    for (path, text) in each_source() {
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if (t.starts_with("pub use") || t.starts_with("pub(crate) use")) && t.contains("::*") {
                offenders.push(format!("{}:{}: {}", path.display(), i + 1, t));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "glob re-exports rebuild the screens-hub coupling; re-export by name:\n{}",
        offenders.join("\n")
    );
}

/// `use super::*` hides a module's real dependencies (everything resolves through the
/// parent). It stays idiomatic inside `#[cfg(test)]` modules only.
#[test]
fn no_super_glob_imports_outside_test_modules() {
    let mut offenders = Vec::new();
    for (path, text) in each_source() {
        // `tests.rs` is itself a `#[cfg(test)]` module (declared so in lib.rs).
        let mut in_test_scope = path.file_name().is_some_and(|f| f == "tests.rs");
        for (i, line) in text.lines().enumerate() {
            if line.contains("#[cfg(test)]") {
                // Test modules sit at the end of their file by convention, so the
                // remainder of this file is test scope.
                in_test_scope = true;
            }
            if !in_test_scope && line.trim_start().starts_with("use super::*") {
                offenders.push(format!("{}:{}", path.display(), i + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "`use super::*` outside a test module hides real dependencies; import explicitly:\n{}",
        offenders.join("\n")
    );
}

/// Presentation reads simulation, never the reverse: `sim/` must not know `view/` or
/// `screens/` exist.
#[test]
fn sim_never_imports_presentation() {
    let sim = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("sim");
    let mut files = Vec::new();
    rust_sources(&sim, &mut files);
    let mut offenders = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("sim source is readable");
        for (i, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            if code.contains("crate::view") || code.contains("crate::screens") {
                offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "sim/ must stay presentation-free (view reads sim, never the reverse):\n{}",
        offenders.join("\n")
    );
}

/// The hex sim adapter (`hex_wfc/sim.rs`) owns match construction and stepping. It reads
/// simulation crates only; like `full_wfc/sim.rs` it must never reach into presentation
/// (`crate::view` / `crate::screens`). The rest of `hex_wfc/` is the presentation shell
/// that reads the sim, never the reverse.
#[test]
fn hex_sim_never_imports_presentation() {
    let sim = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("hex_wfc")
        .join("sim.rs");
    let text = fs::read_to_string(&sim).expect("hex_wfc sim source is readable");
    let mut offenders = Vec::new();
    let mut in_test_scope = false;
    for (i, line) in text.lines().enumerate() {
        if line.contains("#[cfg(test)]") {
            in_test_scope = true;
        }
        let code = line.trim_start();
        if in_test_scope || code.starts_with("//") {
            continue;
        }
        if code.contains("crate::view") || code.contains("crate::screens") {
            offenders.push(format!("{}:{}: {}", sim.display(), i + 1, line.trim()));
        }
    }
    assert!(
        offenders.is_empty(),
        "hex_wfc/sim.rs must stay presentation-free (the view reads sim, never the reverse):\n{}",
        offenders.join("\n")
    );
}

/// Arc K/L keep agent-owned production modules reviewable. The scope is the canonical
/// full-WFC and hex-WFC paths so unrelated legacy fixtures can be retired independently
/// instead of weakening this ratchet.
#[test]
fn full_wfc_production_files_stay_under_six_hundred_lines() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("game is a workspace member")
        .to_path_buf();
    let roots = [
        workspace.join("game/src/full_wfc"),
        workspace.join("crates/observed_facility/src/full_wfc"),
        workspace.join("crates/observed_match/src/full_wfc"),
        // Only the game-side hex shell is agent-owned by this arc. The Phase 90–94
        // solver/match crates predate this ratchet (`hex_wfc/collapse.rs` is already
        // 604 lines) and are frozen as the stable substrate; they are not in scope.
        workspace.join("game/src/hex_wfc"),
    ];
    let mut offenders = Vec::new();
    for root in roots {
        let mut files = Vec::new();
        rust_sources(&root, &mut files);
        for path in files {
            let lines = fs::read_to_string(&path)
                .expect("full-WFC source is readable")
                .lines()
                .count();
            if lines > 600 {
                offenders.push(format!("{}: {lines} lines", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "split full-WFC files before extending them past 600 lines:\n{}",
        offenders.join("\n")
    );
}
