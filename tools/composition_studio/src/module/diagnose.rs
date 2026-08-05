//! Turn one authored `.map` file into a diagnosis: the geometry to draw, and
//! where in it the problem is.
//!
//! The importer's error vocabulary is already precise - `FootprintViolation`
//! reports the offending vertex *and* the bound it broke, `PortOriginMismatch`
//! reports expected against actual, `RampTooSteep` reports the slope. All of it
//! has only ever had one place to be seen: a line of CLI text. Reading
//! "vertex [118.0, 0.0, 44.0] escapes east" and then finding that vertex in
//! TrenchBroom is a translation the author should not be doing by hand.
//!
//! Two parses, deliberately, and this is the load-bearing decision in the file.
//! `parse_authored_module` runs `validate_module` internally and returns `Err`
//! with no module attached - so on exactly the failures this tool exists to
//! show, it would have nothing to draw. `parse_tile` is called first and
//! separately for the geometry, so a module that parses but fails validation is
//! still rendered, with the offending part lit. A validator that goes blank at
//! the moment it finds something is no better than the CLI it replaces.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use observed_authoring::forge::recipe::{RECIPE_SUFFIX, Recipe};
use observed_authoring::{
    AuthoredModule, ModuleCellRef, ModuleSummary, SourceError, TileError, TilePrototype,
    editor_origin_to_world, parse_authored_module, parse_tile,
};
use observed_hex::HexFace;

/// Where in the module to point, once something is wrong.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Highlight {
    /// A single point, in world metres.
    Vertex(Vec3),
    Cell(ModuleCellRef),
    Face {
        cell: ModuleCellRef,
        face: HexFace,
    },
    /// Index into [`TilePrototype::hulls`].
    Hull(usize),
    /// Nothing in the geometry to point at - the file is malformed, or the
    /// fault is a property rather than a place.
    ///
    /// A legitimate answer, not a fallback: `MissingMeta` has no location. The
    /// exhaustive matches below are what stop it becoming a lazy default.
    Whole,
}

/// What one file currently is.
pub struct Diagnosis {
    pub path: PathBuf,
    /// Geometry, present whenever the brushes parsed - including when
    /// validation then failed.
    pub prototype: Option<TilePrototype>,
    pub module: Option<AuthoredModule>,
    pub summary: Option<ModuleSummary>,
    /// `None` when the module is clean.
    pub error: Option<String>,
    pub highlight: Highlight,
    /// Present when this came from a `.recipe.ron` rather than a `.map`.
    ///
    /// A recipe is previewed by building it in memory and diagnosing the
    /// result, so everything downstream - renderer, highlight, panel - is
    /// unchanged. That is the point: a parametric module is judged by exactly
    /// the same importer as a committed one, while you are still holding the
    /// parameter.
    pub recipe: Option<Recipe>,
}

impl Diagnosis {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.error.is_none()
    }

    /// The file's stem, which is what an author calls it.
    /// `foo.recipe.ron` is `foo`, not `foo.recipe`: the double extension is a
    /// file-naming convention, not part of the module name, and a list
    /// showing both `foo` and `foo.recipe` would read as two modules.
    #[must_use]
    pub fn name(&self) -> String {
        let raw = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.display().to_string());
        if let Some(stem) = raw.strip_suffix(RECIPE_SUFFIX) {
            return stem.to_string();
        }
        self.path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or(raw)
    }

    /// Whether this module is parametric, and therefore editable here.
    #[must_use]
    pub fn is_parametric(&self) -> bool {
        self.recipe.is_some()
    }
}

/// Read and diagnose `path`.
#[must_use]
pub fn diagnose_file(path: &Path) -> Diagnosis {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return Diagnosis {
                path: path.to_path_buf(),
                prototype: None,
                module: None,
                summary: None,
                error: Some(format!("cannot read: {error}")),
                highlight: Highlight::Whole,
                recipe: None,
            };
        }
    };
    if is_recipe(path) {
        return diagnose_recipe(path, &text);
    }
    diagnose_text(path, &text)
}

/// Whether `path` is a parametric recipe rather than a baked module.
#[must_use]
pub fn is_recipe(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(RECIPE_SUFFIX))
}

/// Build a recipe and diagnose what it produced.
///
/// A recipe that fails to *parse* has no geometry at all, and says so plainly
/// rather than reporting a module error about a module that was never built.
#[must_use]
pub fn diagnose_recipe(path: &Path, text: &str) -> Diagnosis {
    match Recipe::parse(text) {
        Ok(recipe) => {
            let mut diagnosis = diagnose_text(path, &recipe.build());
            diagnosis.recipe = Some(recipe);
            diagnosis
        }
        Err(error) => Diagnosis {
            path: path.to_path_buf(),
            prototype: None,
            module: None,
            summary: None,
            error: Some(format!("the recipe does not parse:\n{error}")),
            highlight: Highlight::Whole,
            recipe: None,
        },
    }
}

/// Diagnose already-read source. Split out so the tests do not need files.
#[must_use]
pub fn diagnose_text(path: &Path, text: &str) -> Diagnosis {
    // Geometry first and on its own, so a validation failure still has
    // something to draw. See the module docs.
    let prototype = parse_tile(text).ok();

    match parse_authored_module(text) {
        Ok(module) => Diagnosis {
            path: path.to_path_buf(),
            summary: Some(ModuleSummary::from(&module)),
            prototype: prototype.or_else(|| Some(module.prototype.clone())),
            module: Some(module),
            error: None,
            highlight: Highlight::Whole,
            recipe: None,
        },
        Err(error) => Diagnosis {
            path: path.to_path_buf(),
            prototype,
            module: None,
            summary: None,
            highlight: source_highlight(&error),
            error: Some(describe(&error)),
            recipe: None,
        },
    }
}

/// Where to point for a [`SourceError`].
///
/// Exhaustive on purpose: a new variant is a compile error here, which is the
/// only reliable way to stop one shipping invisible. `Highlight::Whole` is a
/// real answer for the property-shaped faults, but it has to be chosen.
#[must_use]
pub fn source_highlight(error: &SourceError) -> Highlight {
    match error {
        SourceError::Tile(inner) => tile_highlight(inner),

        // Placed faults: the author can be shown the cell.
        SourceError::DuplicateCell(cell)
        | SourceError::SocketCellMissing(cell)
        | SourceError::PortCellMissing(cell)
        | SourceError::FloorGap(cell)
        | SourceError::MissingRampSurface(cell) => Highlight::Cell(*cell),
        SourceError::Headroom { cell, .. } | SourceError::RampTooSteep { cell, .. } => {
            Highlight::Cell(*cell)
        }

        // Faults that belong to one face of one cell.
        SourceError::DuplicatePort { cell, face }
        | SourceError::PortOnInternalFace { cell, face }
        | SourceError::MissingPortOrigin { cell, face }
        | SourceError::PortOriginMismatch { cell, face, .. } => Highlight::Face {
            cell: *cell,
            face: *face,
        },

        // Property- or whole-module-shaped: nothing to point at.
        SourceError::Parse(_)
        | SourceError::InvalidProperty { .. }
        | SourceError::DuplicateSocketId(_)
        | SourceError::SocketContract { .. }
        | SourceError::CellModuleFootprint
        | SourceError::RoomRoleMissing
        | SourceError::DisconnectedFootprint
        | SourceError::ComplexityBudget { .. } => Highlight::Whole,
    }
}

/// Where to point for a [`TileError`]. Exhaustive for the same reason.
#[must_use]
pub fn tile_highlight(error: &TileError) -> Highlight {
    match error {
        // The one error that names a point in space, which is exactly the one
        // that is hardest to find by hand in the editor.
        TileError::FootprintViolation { vertex, .. } => {
            Highlight::Vertex(editor_origin_to_world(*vertex))
        }
        TileError::DegenerateBrush { index } => Highlight::Hull(*index),

        // Ports name a face but not a cell at this level: tile-level errors are
        // single-cell by construction, so the origin cell is the right answer.
        TileError::DuplicatePort { face } | TileError::InvalidPort { face, .. } => {
            Highlight::Face {
                cell: ModuleCellRef {
                    q: 0,
                    r: 0,
                    level: 0,
                },
                face: *face,
            }
        }

        TileError::Parse(_)
        | TileError::MissingMeta
        | TileError::DuplicateMeta
        | TileError::MissingProperty { .. }
        | TileError::UnknownFace(_)
        | TileError::UnknownClass(_)
        | TileError::UnknownLightKind(_)
        | TileError::InvalidLevels
        | TileError::DuplicateStairNode { .. }
        | TileError::StairSpineDescends { .. }
        | TileError::StairSpineSelfCrossing { .. }
        | TileError::StairSpineTooShort
        | TileError::DuplicateDeckNode { .. }
        | TileError::DeckPathTooShort => Highlight::Whole,
    }
}

/// One line an author can act on.
///
/// `Debug` is the importer's own rendering and already carries the numbers;
/// what it lacks is the instruction. These prefixes say what to change, because
/// "PortOriginMismatch { expected: [112.0, 0.0, 48.0], actual: [110.0, 0.0,
/// 48.0] }" tells you the fault and not the fix.
#[must_use]
pub fn describe(error: &SourceError) -> String {
    let hint = match error {
        SourceError::Tile(TileError::FootprintViolation { boundary, .. }) => {
            Some(format!("a brush vertex crosses the {boundary} bound"))
        }
        SourceError::Tile(TileError::MissingMeta) => {
            Some("no tile_meta entity: every module needs exactly one".to_string())
        }
        SourceError::PortOriginMismatch { .. } => {
            Some("move the port to its face-edge midpoint".to_string())
        }
        SourceError::Headroom { meters, .. } => Some(format!(
            "{meters:.2} m of headroom; the authoring contract needs {:.2} m",
            crate::module::walk::AUTHORING_HEADROOM_STANDARD_METERS
        )),
        SourceError::RampTooSteep { slope, .. } => {
            Some(format!("slope {slope:.2} is above what a body can walk"))
        }
        SourceError::DisconnectedFootprint => {
            Some("the cells do not touch; a module has to be one piece".to_string())
        }
        SourceError::ComplexityBudget { hulls, maximum } => {
            Some(format!("{hulls} hulls, budget is {maximum}"))
        }
        _ => None,
    };
    match hint {
        Some(hint) => format!("{hint}\n{error:?}"),
        None => format!("{error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell() -> ModuleCellRef {
        ModuleCellRef {
            q: 1,
            r: -1,
            level: 0,
        }
    }

    /// The point of the tool: a module that parses but fails validation must
    /// still render. If this regresses, the viewport goes blank at exactly the
    /// moment it has something to show, and the tool is worse than the CLI.
    ///
    /// Built by breaking one property of a real committed module rather than by
    /// hand-writing brushes. Hand-written brush planes are easy to get subtly
    /// degenerate, and a fixture that fails to parse would pass this test for
    /// the wrong reason - it would have no geometry because the geometry was
    /// bad, not because validation ate it.
    #[test]
    fn a_module_that_fails_validation_still_has_geometry_to_draw() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/tiles/authored/hall_cap.map");
        let good = std::fs::read_to_string(&path).expect("hall_cap.map");
        assert!(
            diagnose_text(&path, &good).is_clean(),
            "the fixture must start clean or this proves nothing"
        );

        // Nudge the east port off its face-edge midpoint: parses, fails
        // validation, and the fault has a place.
        let broken = good.replace("\"origin\" \"112 0 48\"", "\"origin\" \"104 0 48\"");
        assert_ne!(broken, good, "the port origin line moved; update this test");

        let diagnosis = diagnose_text(&path, &broken);
        assert!(
            !diagnosis.is_clean(),
            "the nudged port must fail validation"
        );
        assert!(
            diagnosis.prototype.is_some(),
            "geometry must survive a validation failure: {:?}",
            diagnosis.error
        );
        assert!(
            matches!(diagnosis.highlight, Highlight::Face { .. }),
            "a misplaced port must point at its face, got {:?}",
            diagnosis.highlight
        );
    }

    /// A clean module reports clean and carries its summary.
    #[test]
    fn the_committed_corpus_diagnoses_clean() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles/authored");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("authored corpus") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("map") {
                continue;
            }
            let diagnosis = diagnose_file(&path);
            assert!(
                diagnosis.is_clean(),
                "{} is committed but does not validate: {:?}",
                path.display(),
                diagnosis.error
            );
            assert!(diagnosis.summary.is_some());
            checked += 1;
        }
        assert!(
            checked > 0,
            "no authored modules found in {}",
            dir.display()
        );
    }

    /// The placed errors must point somewhere. A diagnosis that always says
    /// `Whole` is the CLI with extra steps.
    #[test]
    fn placed_errors_point_at_the_thing_that_is_wrong() {
        assert_eq!(
            source_highlight(&SourceError::FloorGap(cell())),
            Highlight::Cell(cell())
        );
        assert_eq!(
            source_highlight(&SourceError::Headroom {
                cell: cell(),
                meters: 1.9
            }),
            Highlight::Cell(cell())
        );
        assert_eq!(
            source_highlight(&SourceError::PortOriginMismatch {
                cell: cell(),
                face: HexFace::East,
                expected: [112.0, 0.0, 48.0],
                actual: [110.0, 0.0, 48.0],
            }),
            Highlight::Face {
                cell: cell(),
                face: HexFace::East
            }
        );
        assert_eq!(
            tile_highlight(&TileError::DegenerateBrush { index: 3 }),
            Highlight::Hull(3)
        );
    }

    /// The footprint vertex arrives in editor units and must be placed in world
    /// metres, or the marker lands 16x away from the brush it describes.
    #[test]
    fn a_footprint_vertex_is_converted_out_of_editor_units() {
        let highlight = tile_highlight(&TileError::FootprintViolation {
            vertex: [112.0, 0.0, 48.0],
            boundary: "east".to_string(),
        });
        let Highlight::Vertex(point) = highlight else {
            panic!("a footprint violation must point at its vertex, got {highlight:?}");
        };
        // 16 units per metre, and TrenchBroom Z-up becomes world Y-up.
        assert!((point.x - 7.0).abs() < 1e-4, "x was {}", point.x);
        assert!((point.y - 3.0).abs() < 1e-4, "y was {}", point.y);
        assert!(point.z.abs() < 1e-4, "z was {}", point.z);
    }

    /// The message has to say what to change, not only what is wrong.
    #[test]
    fn a_description_carries_an_instruction_and_the_numbers() {
        let text = describe(&SourceError::Headroom {
            cell: cell(),
            meters: 1.94,
        });
        assert!(text.contains("1.94"), "{text}");
        assert!(
            text.contains("2.20"),
            "the contract value must be shown: {text}"
        );
    }
}
