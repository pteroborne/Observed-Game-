//! Generate the TrenchBroom entity definition from the importer's own tables.
//!
//! `tools/trenchbroom/Observed2/Observed2.fgd` was hand-written, and drifted:
//! `tile_socket`, `tile_stair_node`, and `tile_deck_node` are all fully
//! supported by [`crate::source`] and [`crate::tile`], and none of them could
//! be placed from TrenchBroom's entity browser. An author who wanted a room
//! socket had no way to make one in the editor, which is a large part of why
//! `tools/tileforge.py` became the de-facto source of truth for authored
//! geometry.
//!
//! The fix is not to update the file - it is to stop the file being a second
//! source. Every choice list here is built from the same name tables the
//! importer parses with, and the tests assert both directions: everything the
//! editor offers must import, and everything the importer accepts must be
//! offered. Drift becomes a failing build rather than a silently missing
//! entity.
//!
//! Emitted with the generated header and LF endings, and committed, so the
//! working copy is diffable and `committed_fgd_matches_generated_fgd` can fail
//! the moment the two disagree.
//!
//! # The version-3 `tile_meta` keys
//!
//! `family`, `assembly_scope` and `family_weight` are offered, so a contracted
//! module can be authored in the editor rather than only through
//! [`crate::forge`]. `assembly_scope`'s choice list is built from
//! `source::assembly_scope_name`, the same table the importer parses with, so
//! it cannot drift from what will actually load.
//!
//! All three are optional and default to neutral values, because they are
//! optional in the importer: a source with no `family` compiles as version 1/2
//! and carries no contract. An author placing an ordinary tile should not have
//! to answer a question about assemblies.
//!
//! Leaving them out was not harmless. This module exists because the editor
//! could not place `tile_socket`, and that gap is a large part of why the
//! Python generator became the de-facto source of truth. Repeating it for the
//! contract keys would have made hand-authored version-3 tiles impossible in
//! exactly the editor the pipeline is named for.

use observed_hex::{HexFace, PortClass};

use crate::contract::AssemblyScope;
use crate::source::RoomSocketKind;
use crate::source::{assembly_scope_name, floor_name, kind_name, rotation_name};
use crate::tile::{class_name, face_name};
use crate::{FloorPolicy, ModuleKind, RotationPolicy};

/// Path of the committed definition, relative to the repo root.
pub const FGD_PATH: &str = "tools/trenchbroom/Observed2/Observed2.fgd";

/// One `key(type) : "label" : default` line, with optional choices.
struct Property {
    key: &'static str,
    ty: &'static str,
    label: &'static str,
    default: String,
    choices: Vec<(String, &'static str)>,
}

impl Property {
    fn scalar(key: &'static str, ty: &'static str, label: &'static str, default: &str) -> Self {
        Self {
            key,
            ty,
            label,
            default: if ty == "string" {
                format!("{default:?}")
            } else {
                default.to_string()
            },
            choices: Vec::new(),
        }
    }

    fn choices(
        key: &'static str,
        label: &'static str,
        default: &str,
        choices: Vec<(String, &'static str)>,
    ) -> Self {
        Self {
            key,
            ty: "choices",
            label,
            default: format!("{default:?}"),
            choices,
        }
    }

    fn render(&self, out: &mut String) {
        out.push_str(&format!(
            "    {}({}) : {:?} : {}",
            self.key, self.ty, self.label, self.default
        ));
        if self.choices.is_empty() {
            out.push('\n');
            return;
        }
        out.push_str(" = [\n");
        for (value, label) in &self.choices {
            out.push_str(&format!("        {value:?} : {label:?}\n"));
        }
        out.push_str("    ]\n");
    }
}

struct Entity {
    classname: &'static str,
    colour: &'static str,
    half_size: i32,
    description: &'static str,
    properties: Vec<Property>,
}

impl Entity {
    fn render(&self, out: &mut String) {
        let half = self.half_size;
        out.push_str(&format!(
            "@PointClass color({}) size({} {} {}, {half} {half} {half}) = {} : {:?} [\n",
            self.colour, -half, -half, -half, self.classname, self.description
        ));
        for property in &self.properties {
            property.render(out);
        }
        out.push_str("]\n");
    }
}

/// Human labels for the face choices. Derived names are machine-shaped
/// (`south_east`); the editor's dropdown deserves prose.
fn face_label(face: HexFace) -> &'static str {
    match face {
        HexFace::East => "East",
        HexFace::SouthEast => "South East",
        HexFace::SouthWest => "South West",
        HexFace::West => "West",
        HexFace::NorthWest => "North West",
        HexFace::NorthEast => "North East",
        HexFace::Up => "Up",
        HexFace::Down => "Down",
    }
}

fn class_label(class: PortClass) -> &'static str {
    match class {
        PortClass::Sealed => "Sealed (not authorable)",
        PortClass::Door => "Walkable threshold",
        PortClass::RampOpen => "Ramp continuation",
        PortClass::ShaftOpen => "Vertical shaft opening",
    }
}

fn floor_label(policy: FloorPolicy) -> &'static str {
    match policy {
        FloorPolicy::Solid => "Continuous walkable floor required",
        FloorPolicy::Ramp => "Continuous one-level ramp required",
        FloorPolicy::Open => "Declared shaft/opening; floor is intentionally absent",
    }
}

fn kind_label(kind: ModuleKind) -> &'static str {
    match kind {
        ModuleKind::Cell => "One-cell structural kit",
        ModuleKind::Room => "Whole-room multi-cell module",
    }
}

fn rotation_label(policy: RotationPolicy) -> &'static str {
    match policy {
        RotationPolicy::None => "No rotation",
        RotationPolicy::SixFold => "All six 60-degree rotations",
    }
}

fn assembly_scope_label(scope: AssemblyScope) -> &'static str {
    match scope {
        AssemblyScope::Cell => "Chosen per lattice cell",
        AssemblyScope::VerticalColumn => "Chosen once for a whole shaft column",
    }
}

fn socket_label(kind: RoomSocketKind) -> &'static str {
    match kind {
        RoomSocketKind::Keystone => "Contested keystone",
        RoomSocketKind::StationA => "Team A station",
        RoomSocketKind::StationB => "Team B station",
        RoomSocketKind::Monitor => "Living monitor",
        RoomSocketKind::LanternCache => "Lantern cache",
        RoomSocketKind::GuardianControl => "Guardian control",
        RoomSocketKind::Recovery => "Recovery point",
        RoomSocketKind::Exit => "Gated exit",
    }
}

/// The face choices, in `HexFace::ALL` order.
fn face_choices() -> Vec<(String, &'static str)> {
    HexFace::ALL
        .into_iter()
        .map(|face| (face_name(face).to_string(), face_label(face)))
        .collect()
}

/// The port classes an author may actually write.
///
/// `Sealed` is filtered out deliberately: it is the *absence* of a port, is
/// what an unlisted face already means, and `class_from_name` rejects it. The
/// filter is by round-trip through the importer rather than by a second hand
/// list, so if `Sealed` ever became parseable this would offer it without
/// anyone remembering to come back here.
fn class_choices() -> Vec<(String, &'static str)> {
    [
        PortClass::Sealed,
        PortClass::Door,
        PortClass::RampOpen,
        PortClass::ShaftOpen,
    ]
    .into_iter()
    .filter(|class| crate::tile::class_from_name(class_name(*class)).is_ok())
    .map(|class| (class_name(class).to_string(), class_label(class)))
    .collect()
}

fn entities() -> Vec<Entity> {
    vec![
        Entity {
            classname: "tile_meta",
            colour: "70 180 255",
            half_size: 8,
            description: "Module metadata (exactly one)",
            properties: vec![
                Property::scalar("authoring_version", "integer", "Contract version", "2"),
                Property::scalar("id", "string", "Stable source ID", "district/module_name"),
                Property::choices(
                    "kind",
                    "Module kind",
                    kind_name(ModuleKind::Cell),
                    [ModuleKind::Cell, ModuleKind::Room]
                        .into_iter()
                        .map(|kind| (kind_name(kind).to_string(), kind_label(kind)))
                        .collect(),
                ),
                Property::scalar("archetype", "string", "WFC archetype", "hall_cap"),
                Property::scalar(
                    "room_role",
                    "string",
                    "Room role (room kind only)",
                    "decision",
                ),
                Property::scalar(
                    "register",
                    "string",
                    "Legacy compatibility register",
                    "generic",
                ),
                Property::scalar(
                    "register_scope",
                    "string",
                    "all or comma-separated register overrides",
                    "all",
                ),
                Property::scalar("variant", "integer", "Legacy compatibility variant", "0"),
                Property::scalar("levels", "integer", "Total local level extent", "1"),
                Property::choices(
                    "rotation_policy",
                    "Compiler rotation expansion",
                    rotation_name(RotationPolicy::SixFold),
                    [RotationPolicy::None, RotationPolicy::SixFold]
                        .into_iter()
                        .map(|policy| (rotation_name(policy).to_string(), rotation_label(policy)))
                        .collect(),
                ),
                Property::scalar(
                    "weight",
                    "integer",
                    "Deterministic WFC weight (1..1000)",
                    "1",
                ),
                // The version-3 assembly keys. Optional in the importer - a
                // source without `family` compiles as version 1/2 and carries
                // no contract - so they are offered with empty/neutral defaults
                // rather than forced on every module an author places.
                Property::scalar(
                    "family",
                    "string",
                    "Assembly family; blank for a compatibility module",
                    "",
                ),
                Property::choices(
                    "assembly_scope",
                    "What the family is chosen for",
                    assembly_scope_name(AssemblyScope::Cell),
                    [AssemblyScope::Cell, AssemblyScope::VerticalColumn]
                        .into_iter()
                        .map(|scope| {
                            (
                                assembly_scope_name(scope).to_string(),
                                assembly_scope_label(scope),
                            )
                        })
                        .collect(),
                ),
                Property::scalar(
                    "family_weight",
                    "integer",
                    "Chooses the family once per assembly (1..1000)",
                    "1",
                ),
            ],
        },
        Entity {
            classname: "tile_cell",
            colour: "80 255 150",
            half_size: 6,
            description: "Occupied hex/level footprint",
            properties: vec![
                Property::scalar("q", "integer", "Relative axial q", "0"),
                Property::scalar("r", "integer", "Relative axial r", "0"),
                Property::scalar("level", "integer", "Relative base level", "0"),
                Property::scalar("levels", "integer", "Contiguous vertical levels", "1"),
                Property::choices(
                    "floor",
                    "Floor contract",
                    floor_name(FloorPolicy::Solid),
                    [FloorPolicy::Solid, FloorPolicy::Ramp, FloorPolicy::Open]
                        .into_iter()
                        .map(|policy| (floor_name(policy).to_string(), floor_label(policy)))
                        .collect(),
                ),
            ],
        },
        Entity {
            classname: "tile_port",
            colour: "255 190 70",
            half_size: 7,
            description: "Named external connection",
            properties: vec![
                Property::scalar("q", "integer", "Owning relative axial q", "0"),
                Property::scalar("r", "integer", "Owning relative axial r", "0"),
                Property::scalar("level", "integer", "Owning relative level", "0"),
                Property::choices(
                    "face",
                    "Hex-prism face",
                    face_name(HexFace::East),
                    face_choices(),
                ),
                Property::choices(
                    "class",
                    "Typed connection",
                    class_name(PortClass::Door),
                    class_choices(),
                ),
                Property::scalar("name", "string", "Stable threshold name", "east_threshold"),
            ],
        },
        Entity {
            classname: "tile_socket",
            colour: "255 120 220",
            half_size: 7,
            description: "Gameplay attachment point inside a room module",
            properties: vec![
                Property::scalar("id", "string", "Stable socket ID", "keystone_main"),
                Property::choices(
                    "kind",
                    "What attaches here",
                    RoomSocketKind::Keystone.slug(),
                    RoomSocketKind::ALL
                        .into_iter()
                        .map(|kind| (kind.slug().to_string(), socket_label(kind)))
                        .collect(),
                ),
                Property::scalar("q", "integer", "Owning relative axial q", "0"),
                Property::scalar("r", "integer", "Owning relative axial r", "0"),
                Property::scalar("level", "integer", "Owning relative level", "0"),
                // The importer parses yaw as `f32`, but TrenchBroom's property
                // types have no float, so this narrows authoring to whole
                // degrees. Deliberate and harmless - faces are 60 degrees
                // apart - but it is a narrowing, not a match, and worth saying
                // so rather than leaving the next reader to find it.
                Property::scalar("yaw", "integer", "Facing, in whole degrees", "0"),
            ],
        },
        Entity {
            classname: "tile_stair_node",
            colour: "170 200 255",
            half_size: 5,
            description: "Ordered stair path node; index orders the run",
            properties: vec![Property::scalar(
                "index",
                "integer",
                "Position along the run (unique)",
                "0",
            )],
        },
        Entity {
            classname: "tile_deck_node",
            colour: "150 220 220",
            half_size: 5,
            description: "Ordered deck path node; index orders the walk",
            properties: vec![Property::scalar(
                "index",
                "integer",
                "Position along the walk (unique)",
                "0",
            )],
        },
        Entity {
            classname: "tile_light",
            colour: "255 244 170",
            half_size: 5,
            description: "Semantic practical light source",
            properties: vec![Property::choices(
                "kind",
                "Presentation-owned treatment",
                "practical",
                vec![(
                    "practical".to_string(),
                    "District-coloured architectural practical",
                )],
            )],
        },
    ]
}

/// Render the whole definition. LF-only, so a Windows checkout and a CI diff
/// agree byte for byte.
#[must_use]
pub fn emit_fgd() -> String {
    let mut out = String::new();
    out.push_str("// Observed 2 strict tile-source entities. Geometry stays in worldspawn.\n");
    out.push_str("// Generated by `tilec emit-fgd` from the importer's own tables.\n");
    out.push_str("// Do not hand-edit: regenerate instead, or the editor and the\n");
    out.push_str("// importer will disagree about what is authorable.\n");
    for entity in entities() {
        out.push('\n');
        entity.render(&mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::RoomSocketKind;

    fn choices_for(classname: &str, key: &str) -> Vec<String> {
        entities()
            .into_iter()
            .find(|entity| entity.classname == classname)
            .unwrap_or_else(|| panic!("no {classname} entity"))
            .properties
            .into_iter()
            .find(|property| property.key == key)
            .unwrap_or_else(|| panic!("{classname} has no {key}"))
            .choices
            .into_iter()
            .map(|(value, _)| value)
            .collect()
    }

    /// Direction one: nothing the editor offers may fail to import. An author
    /// who picks a value from a dropdown and then cannot compile the module has
    /// been lied to by the tool that offered it.
    #[test]
    fn every_advertised_choice_is_one_the_importer_accepts() {
        for value in choices_for("tile_port", "face") {
            assert!(
                crate::tile::face_from_name(&value).is_ok(),
                "the editor offers face {value:?}, which the importer rejects"
            );
        }
        for value in choices_for("tile_port", "class") {
            assert!(
                crate::tile::class_from_name(&value).is_ok(),
                "the editor offers class {value:?}, which the importer rejects"
            );
        }
        for value in choices_for("tile_cell", "floor") {
            assert!(
                crate::source::floor_from_name(&value).is_some(),
                "the editor offers floor {value:?}, which the importer rejects"
            );
        }
        for value in choices_for("tile_meta", "kind") {
            assert!(
                crate::source::kind_from_name(&value).is_some(),
                "the editor offers kind {value:?}, which the importer rejects"
            );
        }
        for value in choices_for("tile_meta", "rotation_policy") {
            assert!(
                crate::source::rotation_from_name(&value).is_some(),
                "the editor offers rotation_policy {value:?}, which the importer rejects"
            );
        }
        for value in choices_for("tile_socket", "kind") {
            assert!(
                RoomSocketKind::parse(&value).is_some(),
                "the editor offers socket kind {value:?}, which the importer rejects"
            );
        }
    }

    /// Direction two: nothing the importer accepts may go unoffered. This is
    /// the direction that was actually broken - three whole entities were
    /// supported and unreachable from the editor.
    #[test]
    fn every_importable_value_is_offered_to_the_author() {
        let faces = choices_for("tile_port", "face");
        for face in HexFace::ALL {
            assert!(
                faces.contains(&face_name(face).to_string()),
                "{face:?} imports but is not offered"
            );
        }

        let sockets = choices_for("tile_socket", "kind");
        for kind in RoomSocketKind::ALL {
            assert!(
                sockets.contains(&kind.slug().to_string()),
                "socket {kind:?} imports but is not offered"
            );
        }

        let classes = choices_for("tile_port", "class");
        for class in [
            PortClass::Sealed,
            PortClass::Door,
            PortClass::RampOpen,
            PortClass::ShaftOpen,
        ] {
            let name = class_name(class).to_string();
            assert_eq!(
                crate::tile::class_from_name(&name).is_ok(),
                classes.contains(&name),
                "{class:?} is offered and importable, or neither - not one of the two"
            );
        }
    }

    /// Every entity the importer looks for must be placeable. This is the
    /// ratchet: adding a `classname` check to the importer without adding it
    /// here fails the build.
    #[test]
    fn every_importer_entity_is_placeable_in_the_editor() {
        let placeable: Vec<&str> = entities().iter().map(|entity| entity.classname).collect();
        for classname in [
            "tile_meta",
            "tile_cell",
            "tile_port",
            "tile_socket",
            "tile_stair_node",
            "tile_deck_node",
            "tile_light",
        ] {
            assert!(
                placeable.contains(&classname),
                "{classname} is read by the importer but cannot be placed"
            );
        }
    }

    /// The committed file is what TrenchBroom actually loads, so it must be
    /// what this function produces. Without this the generator can be correct
    /// and the editor still wrong.
    #[test]
    fn committed_fgd_matches_generated_fgd() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(FGD_PATH);
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        // Git may hand back CRLF on Windows; the contract is on content.
        let committed = committed.replace("\r\n", "\n");
        assert_eq!(
            committed,
            emit_fgd(),
            "{} is stale; regenerate with `tilec emit-fgd`",
            path.display()
        );
    }

    /// TrenchBroom's parser is not forgiving, and a definition it rejects fails
    /// silently as an empty entity browser.
    #[test]
    fn the_definition_is_balanced_and_ascii() {
        let text = emit_fgd();
        assert!(text.is_ascii(), "the FGD must stay ASCII");
        assert_eq!(
            text.matches('[').count(),
            text.matches(']').count(),
            "unbalanced brackets"
        );
        assert_eq!(
            text.matches("@PointClass").count(),
            entities().len(),
            "an entity did not render"
        );
        assert!(!text.contains('\r'), "the FGD must be LF-only");
    }
}
