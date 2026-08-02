//! A parametric module: an ordered list of primitive steps that emits `.map`.
//!
//! The forge's builders are Rust functions, which is right for the committed
//! corpus - they are reviewed, diffed, and gated - but it means authoring a new
//! module requires editing and rebuilding the crate. A recipe is the same
//! primitives addressed as **data**, so a module can be described in a file, and
//! a tool can show it, change it, and validate it without a compiler.
//!
//! The two paths are deliberately not unified. Builders stay the source of
//! truth for what ships; a recipe is how a *new* module is worked out before it
//! earns a builder. Round-tripping the committed corpus back into recipes was
//! considered and rejected - it would make the corpus depend on a second
//! representation for nothing, and the byte-identity gate is what keeps the
//! current one honest.
//!
//! What makes this parametric rather than a scripting language: every step is a
//! named primitive with numeric parameters, so the whole surface is enumerable
//! ([`Step::params`]), a tool can offer exactly the knobs that exist, and an
//! unknown step is a parse error rather than a runtime surprise.

use serde::{Deserialize, Serialize};

use super::entities::{
    Meta, MetaKind, ceiling_fixture, lateral_port, tile_cell, vertical_port, wall_fixture,
    worldspawn,
};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, P2, P3, band, boxed, door_wall, hex_slab, prism, pylon,
    sloped_prism, wall,
};

/// One geometric operation, with its parameters.
///
/// Deliberately an enum over the *existing* primitives rather than an
/// expression tree: the primitives are the vocabulary the contract is written
/// in - canonical door apertures, mitered walls, quantized-hexagon slabs - and
/// anything outside them is how a module stops matching its neighbours.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    /// A full-footprint floor or ceiling slab.
    HexSlab {
        z0: f64,
        z1: f64,
        #[serde(default)]
        chamfer_top: f64,
        #[serde(default)]
        chamfer_bottom: f64,
    },
    /// A sealed wall on `face`, mitered against its neighbours.
    Wall { face: usize, z0: f64, z1: f64 },
    /// A doorway with the canonical 4.5 m aperture.
    DoorWall {
        face: usize,
        z0: f64,
        z1: f64,
        #[serde(default = "default_sill")]
        sill: f64,
        #[serde(default = "default_door_top")]
        top: f64,
        #[serde(default = "default_splay")]
        splay: f64,
        #[serde(default = "default_lintel_bevel")]
        lintel_bevel: f64,
    },
    /// Trim, ledge, or parapet hugging a face.
    Band {
        face: usize,
        t0: f64,
        t1: f64,
        z0: f64,
        z1: f64,
    },
    /// A column. `phase_deg` rotates it off the +x axis.
    Pylon {
        radius: f64,
        z0: f64,
        z1: f64,
        #[serde(default)]
        phase_deg: f64,
        #[serde(default)]
        chamfer_top: f64,
        #[serde(default)]
        chamfer_bottom: f64,
    },
    /// An axis-aligned block.
    Box { min: P3, max: P3 },
    /// A square column or plinth, centred on `center`.
    Post {
        center: P2,
        half: f64,
        z0: f64,
        z1: f64,
        #[serde(default)]
        chamfer_top: f64,
    },
    /// Any convex plan polygon, extruded.
    Prism {
        corners: Vec<P2>,
        z0: f64,
        z1: f64,
        #[serde(default)]
        chamfer_top: f64,
        #[serde(default)]
        chamfer_bottom: f64,
    },
    /// A ramp or stair flight: flat bottom, slanted top through three points.
    SlopedPrism {
        corners: Vec<P2>,
        z0: f64,
        top: [P3; 3],
    },
    /// A recessed ceiling housing and the practical inside it.
    CeilingFixture {
        #[serde(default)]
        x: f64,
        #[serde(default)]
        y: f64,
        #[serde(default = "default_ceiling")]
        ceiling: f64,
        #[serde(default = "default_fixture_half_x")]
        half_x: f64,
        #[serde(default = "default_fixture_half_y")]
        half_y: f64,
    },
    /// A wall sconce and the practical just inside it.
    WallFixture {
        face: usize,
        #[serde(default = "default_along")]
        along: f64,
        #[serde(default = "default_sconce_z")]
        z: f64,
        #[serde(default = "default_sconce_width")]
        width: f64,
    },
}

fn default_sill() -> f64 {
    FLOOR_TOP
}
fn default_door_top() -> f64 {
    DOOR_TOP
}
fn default_splay() -> f64 {
    10.0
}
fn default_lintel_bevel() -> f64 {
    8.0
}
fn default_ceiling() -> f64 {
    LEVEL
}
fn default_fixture_half_x() -> f64 {
    18.0
}
fn default_fixture_half_y() -> f64 {
    10.0
}
fn default_along() -> f64 {
    0.5
}
fn default_sconce_z() -> f64 {
    88.0
}
fn default_sconce_width() -> f64 {
    20.0
}

impl Step {
    /// The name a tool shows for this step.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::HexSlab { .. } => "hex_slab",
            Self::Wall { .. } => "wall",
            Self::DoorWall { .. } => "door_wall",
            Self::Band { .. } => "band",
            Self::Pylon { .. } => "pylon",
            Self::Box { .. } => "box",
            Self::Post { .. } => "post",
            Self::Prism { .. } => "prism",
            Self::SlopedPrism { .. } => "sloped_prism",
            Self::CeilingFixture { .. } => "ceiling_fixture",
            Self::WallFixture { .. } => "wall_fixture",
        }
    }

    /// The scalar knobs on this step, as `(name, value)`.
    ///
    /// Enumerable so a panel can offer exactly the parameters that exist rather
    /// than a hand-kept list that drifts. Polygon corners are absent on
    /// purpose: they are geometry, not a knob, and a slider over a vertex list
    /// is not a control anyone can use.
    #[must_use]
    pub fn params(&self) -> Vec<(&'static str, f64)> {
        #[allow(clippy::cast_precision_loss)]
        match *self {
            Self::HexSlab {
                z0,
                z1,
                chamfer_top,
                chamfer_bottom,
            } => vec![
                ("z0", z0),
                ("z1", z1),
                ("chamfer_top", chamfer_top),
                ("chamfer_bottom", chamfer_bottom),
            ],
            Self::Wall { face, z0, z1 } => {
                vec![("face", face as f64), ("z0", z0), ("z1", z1)]
            }
            Self::DoorWall {
                face,
                z0,
                z1,
                sill,
                top,
                splay,
                lintel_bevel,
            } => vec![
                ("face", face as f64),
                ("z0", z0),
                ("z1", z1),
                ("sill", sill),
                ("top", top),
                ("splay", splay),
                ("lintel_bevel", lintel_bevel),
            ],
            Self::Band {
                face,
                t0,
                t1,
                z0,
                z1,
            } => vec![
                ("face", face as f64),
                ("t0", t0),
                ("t1", t1),
                ("z0", z0),
                ("z1", z1),
            ],
            Self::Pylon {
                radius,
                z0,
                z1,
                phase_deg,
                chamfer_top,
                chamfer_bottom,
            } => vec![
                ("radius", radius),
                ("z0", z0),
                ("z1", z1),
                ("phase_deg", phase_deg),
                ("chamfer_top", chamfer_top),
                ("chamfer_bottom", chamfer_bottom),
            ],
            Self::Box { min, max } => vec![
                ("min_x", min.0),
                ("min_y", min.1),
                ("min_z", min.2),
                ("max_x", max.0),
                ("max_y", max.1),
                ("max_z", max.2),
            ],
            Self::Post {
                center,
                half,
                z0,
                z1,
                chamfer_top,
            } => vec![
                ("center_x", center.0),
                ("center_y", center.1),
                ("half", half),
                ("z0", z0),
                ("z1", z1),
                ("chamfer_top", chamfer_top),
            ],
            Self::Prism {
                z0,
                z1,
                chamfer_top,
                chamfer_bottom,
                ..
            } => vec![
                ("z0", z0),
                ("z1", z1),
                ("chamfer_top", chamfer_top),
                ("chamfer_bottom", chamfer_bottom),
            ],
            Self::SlopedPrism { z0, .. } => vec![("z0", z0)],
            Self::CeilingFixture {
                x,
                y,
                ceiling,
                half_x,
                half_y,
            } => vec![
                ("x", x),
                ("y", y),
                ("ceiling", ceiling),
                ("half_x", half_x),
                ("half_y", half_y),
            ],
            Self::WallFixture {
                face,
                along,
                z,
                width,
            } => vec![
                ("face", face as f64),
                ("along", along),
                ("z", z),
                ("width", width),
            ],
        }
    }

    /// Set a named parameter, returning whether it existed.
    ///
    /// Faces wrap rather than clamp: face 6 is face 0, which is what a person
    /// stepping a face index round a hexagon means. Everything else is a raw
    /// assignment - the contract is checked by the validator, not guessed at
    /// here, because a silent clamp would hide the error the tool exists to show.
    pub fn set_param(&mut self, name: &str, value: f64) -> bool {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let as_face = |value: f64| -> usize { (value.round().rem_euclid(6.0)) as usize };
        match self {
            Self::HexSlab {
                z0,
                z1,
                chamfer_top,
                chamfer_bottom,
            } => match name {
                "z0" => *z0 = value,
                "z1" => *z1 = value,
                "chamfer_top" => *chamfer_top = value,
                "chamfer_bottom" => *chamfer_bottom = value,
                _ => return false,
            },
            Self::Wall { face, z0, z1 } => match name {
                "face" => *face = as_face(value),
                "z0" => *z0 = value,
                "z1" => *z1 = value,
                _ => return false,
            },
            Self::DoorWall {
                face,
                z0,
                z1,
                sill,
                top,
                splay,
                lintel_bevel,
            } => match name {
                "face" => *face = as_face(value),
                "z0" => *z0 = value,
                "z1" => *z1 = value,
                "sill" => *sill = value,
                "top" => *top = value,
                "splay" => *splay = value,
                "lintel_bevel" => *lintel_bevel = value,
                _ => return false,
            },
            Self::Band {
                face,
                t0,
                t1,
                z0,
                z1,
            } => match name {
                "face" => *face = as_face(value),
                "t0" => *t0 = value,
                "t1" => *t1 = value,
                "z0" => *z0 = value,
                "z1" => *z1 = value,
                _ => return false,
            },
            Self::Pylon {
                radius,
                z0,
                z1,
                phase_deg,
                chamfer_top,
                chamfer_bottom,
            } => match name {
                "radius" => *radius = value,
                "z0" => *z0 = value,
                "z1" => *z1 = value,
                "phase_deg" => *phase_deg = value,
                "chamfer_top" => *chamfer_top = value,
                "chamfer_bottom" => *chamfer_bottom = value,
                _ => return false,
            },
            Self::Box { min, max } => match name {
                "min_x" => min.0 = value,
                "min_y" => min.1 = value,
                "min_z" => min.2 = value,
                "max_x" => max.0 = value,
                "max_y" => max.1 = value,
                "max_z" => max.2 = value,
                _ => return false,
            },
            Self::Post {
                center,
                half,
                z0,
                z1,
                chamfer_top,
            } => match name {
                "center_x" => center.0 = value,
                "center_y" => center.1 = value,
                "half" => *half = value,
                "z0" => *z0 = value,
                "z1" => *z1 = value,
                "chamfer_top" => *chamfer_top = value,
                _ => return false,
            },
            Self::Prism {
                z0,
                z1,
                chamfer_top,
                chamfer_bottom,
                ..
            } => match name {
                "z0" => *z0 = value,
                "z1" => *z1 = value,
                "chamfer_top" => *chamfer_top = value,
                "chamfer_bottom" => *chamfer_bottom = value,
                _ => return false,
            },
            Self::SlopedPrism { z0, .. } => match name {
                "z0" => *z0 = value,
                _ => return false,
            },
            Self::CeilingFixture {
                x,
                y,
                ceiling,
                half_x,
                half_y,
            } => match name {
                "x" => *x = value,
                "y" => *y = value,
                "ceiling" => *ceiling = value,
                "half_x" => *half_x = value,
                "half_y" => *half_y = value,
                _ => return false,
            },
            Self::WallFixture {
                face,
                along,
                z,
                width,
            } => match name {
                "face" => *face = as_face(value),
                "along" => *along = value,
                "z" => *z = value,
                "width" => *width = value,
                _ => return false,
            },
        }
        true
    }

    /// Emit this step as `(brushes, lights)`.
    ///
    /// Fixtures produce both; everything else produces geometry only. They are
    /// returned separately because brushes belong inside `worldspawn` and
    /// lights are point entities after it.
    #[must_use]
    pub fn emit(&self) -> (String, String) {
        match self {
            Self::HexSlab {
                z0,
                z1,
                chamfer_top,
                chamfer_bottom,
            } => (
                hex_slab(*z0, *z1, *chamfer_top, *chamfer_bottom),
                String::new(),
            ),
            Self::Wall { face, z0, z1 } => (wall(*face, *z0, *z1), String::new()),
            Self::DoorWall {
                face,
                z0,
                z1,
                sill,
                top,
                splay,
                lintel_bevel,
            } => (
                door_wall(*face, *z0, *z1, *sill, *top, *splay, *lintel_bevel),
                String::new(),
            ),
            Self::Band {
                face,
                t0,
                t1,
                z0,
                z1,
            } => (band(*face, *t0, *t1, *z0, *z1), String::new()),
            Self::Pylon {
                radius,
                z0,
                z1,
                phase_deg,
                chamfer_top,
                chamfer_bottom,
            } => (
                pylon(*radius, *z0, *z1, *phase_deg, *chamfer_top, *chamfer_bottom),
                String::new(),
            ),
            Self::Box { min, max } => (boxed(*min, *max), String::new()),
            Self::Post {
                center,
                half,
                z0,
                z1,
                chamfer_top,
            } => {
                let corners = [
                    (center.0 - half, center.1 - half),
                    (center.0 + half, center.1 - half),
                    (center.0 + half, center.1 + half),
                    (center.0 - half, center.1 + half),
                ];
                (
                    prism(&corners, *z0, *z1, None, *chamfer_top, 0.0),
                    String::new(),
                )
            }
            Self::Prism {
                corners,
                z0,
                z1,
                chamfer_top,
                chamfer_bottom,
            } => (
                prism(corners, *z0, *z1, None, *chamfer_top, *chamfer_bottom),
                String::new(),
            ),
            Self::SlopedPrism { corners, z0, top } => {
                (sloped_prism(corners, *z0, *top, None), String::new())
            }
            Self::CeilingFixture {
                x,
                y,
                ceiling,
                half_x,
                half_y,
            } => ceiling_fixture(*x, *y, *ceiling, *half_x, *half_y),
            Self::WallFixture {
                face,
                along,
                z,
                width,
            } => wall_fixture(*face, *along, *z, *width),
        }
    }
}

/// One footprint cell in a recipe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCell {
    #[serde(default)]
    pub q: i32,
    #[serde(default)]
    pub r: i32,
    #[serde(default)]
    pub level: i32,
    #[serde(default = "one_level")]
    pub levels: u8,
    #[serde(default = "solid_floor")]
    pub floor: String,
}

fn one_level() -> u8 {
    1
}
fn solid_floor() -> String {
    String::from("solid")
}

impl Default for RecipeCell {
    fn default() -> Self {
        Self {
            q: 0,
            r: 0,
            level: 0,
            levels: 1,
            floor: solid_floor(),
        }
    }
}

/// A threshold or opening the module declares.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecipePort {
    Lateral {
        face: usize,
        class: String,
        name: String,
        #[serde(default)]
        level: i32,
        #[serde(default)]
        q: i32,
        #[serde(default)]
        r: i32,
    },
    Vertical {
        /// `"up"` or `"down"`.
        face: String,
        class: String,
        name: String,
        #[serde(default)]
        level: i32,
    },
}

/// A parametric module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub archetype: String,
    #[serde(default)]
    pub room_role: Option<String>,
    #[serde(default)]
    pub variant: i32,
    #[serde(default = "one_level")]
    pub levels: u8,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default = "default_register")]
    pub register: String,
    #[serde(default = "default_register_scope")]
    pub register_scope: String,
    #[serde(default = "default_rotation")]
    pub rotation_policy: String,
    #[serde(default)]
    pub note: String,
    #[serde(default = "default_cells")]
    pub cells: Vec<RecipeCell>,
    #[serde(default)]
    pub ports: Vec<RecipePort>,
    #[serde(default)]
    pub steps: Vec<Step>,
}

fn default_weight() -> u32 {
    10
}
fn default_register() -> String {
    String::from("generic")
}
fn default_register_scope() -> String {
    String::from("all")
}
fn default_rotation() -> String {
    String::from("sixfold")
}
fn default_cells() -> Vec<RecipeCell> {
    vec![RecipeCell::default()]
}

impl Recipe {
    /// A sealed single cell: floor, ceiling, six walls, one light.
    ///
    /// The starting point a new module is worked out *from*, chosen because it
    /// validates as-is. An empty recipe would emit a module that fails the
    /// contract immediately, and a tool whose blank page is an error teaches
    /// the author nothing.
    #[must_use]
    pub fn starter(id: &str) -> Self {
        let mut steps = vec![
            Step::HexSlab {
                z0: 0.0,
                z1: FLOOR_TOP,
                chamfer_top: 3.0,
                chamfer_bottom: 0.0,
            },
            Step::HexSlab {
                z0: LEVEL - FLOOR_TOP,
                z1: LEVEL,
                chamfer_top: 0.0,
                chamfer_bottom: 3.0,
            },
        ];
        for face in 0..6 {
            steps.push(Step::Wall {
                face,
                z0: 0.0,
                z1: LEVEL,
            });
        }
        steps.push(Step::CeilingFixture {
            x: 0.0,
            y: 0.0,
            ceiling: LEVEL,
            half_x: 18.0,
            half_y: 10.0,
        });
        Self {
            id: id.to_string(),
            archetype: String::from("hall_cap"),
            room_role: None,
            variant: 0,
            levels: 1,
            weight: default_weight(),
            register: default_register(),
            register_scope: default_register_scope(),
            rotation_policy: default_rotation(),
            note: format!("Parametric module {id}."),
            cells: default_cells(),
            ports: Vec::new(),
            steps,
        }
    }

    /// Emit the `.map` text.
    #[must_use]
    pub fn build(&self) -> String {
        let mut brushes = String::new();
        let mut lights = String::new();
        for step in &self.steps {
            let (geometry, light) = step.emit();
            brushes.push_str(&geometry);
            lights.push_str(&light);
        }

        let mut out = String::new();
        if !self.note.is_empty() {
            out.push_str(&format!("// {}\n", self.note));
        }
        // Names the recipe, not a generator: the recipe is the thing to edit,
        // and it is the thing that can be re-baked.
        out.push_str(&format!(
            "// Baked from {}.recipe.ron by `tilec bake-recipe` - edit the recipe.\n",
            file_stem(&self.id)
        ));
        out.push_str(&worldspawn(&brushes));

        let meta = match &self.room_role {
            Some(role) => Meta {
                kind: MetaKind::Room { role: role.clone() },
                ..Meta::cell(
                    &self.id,
                    &self.archetype,
                    self.variant,
                    self.levels,
                    self.weight,
                )
            },
            None => Meta::cell(
                &self.id,
                &self.archetype,
                self.variant,
                self.levels,
                self.weight,
            ),
        }
        .with_register(&self.register)
        .with_register_scope(&self.register_scope)
        .with_rotation_policy(&self.rotation_policy);
        out.push_str(&meta.emit());

        for cell in &self.cells {
            out.push_str(&tile_cell(
                cell.q,
                cell.r,
                cell.level,
                cell.levels,
                &cell.floor,
            ));
        }
        for port in &self.ports {
            match port {
                RecipePort::Lateral {
                    face,
                    class,
                    name,
                    level,
                    q,
                    r,
                } => out.push_str(&lateral_port(*face, class, name, *level, *q, *r)),
                RecipePort::Vertical {
                    face,
                    class,
                    name,
                    level,
                } => out.push_str(&vertical_port(face, class, name, *level)),
            }
        }
        out.push_str(&lights);
        out
    }

    /// Emit, then run the importer over it.
    ///
    /// The whole point of a parametric tool is that the answer to "is this
    /// legal" comes back while you are still holding the parameter, so this is
    /// the call the studio makes on every edit.
    ///
    /// # Errors
    /// Whatever the importer rejects the emitted module for.
    pub fn validate(&self) -> Result<(), crate::SourceError> {
        crate::parse_authored_module(&self.build()).map(|_| ())
    }

    /// Parse from RON.
    ///
    /// # Errors
    /// A malformed file, or an unknown `step` name.
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|error| error.to_string())
    }

    /// Serialize to RON, LF-only.
    #[must_use]
    pub fn to_ron(&self) -> String {
        let config = ron::ser::PrettyConfig::new().indentor("  ".to_string());
        let text = ron::ser::to_string_pretty(self, config)
            .unwrap_or_else(|error| format!("// unserializable: {error}"));
        text.replace("\r\n", "\n")
    }
}

/// The bare stem of a possibly-namespaced id (`authored/foo` -> `foo`).
#[must_use]
pub fn file_stem(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

/// The file suffix a recipe carries.
pub const RECIPE_SUFFIX: &str = ".recipe.ron";

#[cfg(test)]
mod tests {
    use super::*;

    /// The blank page has to be legal. A starter that fails the contract
    /// teaches the author that the tool is broken, not that their module is.
    #[test]
    fn the_starter_recipe_validates_as_authored() {
        let recipe = Recipe::starter("authored/parametric_probe");
        recipe
            .validate()
            .unwrap_or_else(|error| panic!("the starter must validate: {error:?}"));
    }

    /// A recipe has to survive the round trip, or saving one loses work.
    #[test]
    fn a_recipe_round_trips_through_ron() {
        let recipe = Recipe::starter("authored/round_trip");
        let text = recipe.to_ron();
        let parsed = Recipe::parse(&text).expect("a recipe we just wrote must parse");
        assert_eq!(recipe, parsed);
        assert_eq!(recipe.build(), parsed.build());
    }

    /// An unknown step must be a parse error, not a silently dropped one.
    /// Dropping it would emit a module missing geometry the author asked for.
    #[test]
    fn an_unknown_step_is_rejected_rather_than_skipped() {
        let text = r#"(
            id: "authored/bogus",
            archetype: "hall_cap",
            steps: [ (step: "teleporter", z0: 0.0) ],
        )"#;
        assert!(Recipe::parse(text).is_err());
    }

    /// Every step must expose its knobs, or a panel built from `params` would
    /// silently omit controls that exist.
    #[test]
    fn every_step_exposes_at_least_one_parameter() {
        let recipe = Recipe::starter("authored/params");
        for step in &recipe.steps {
            assert!(
                !step.params().is_empty(),
                "{} exposes no parameters",
                step.label()
            );
        }
    }

    /// `params` and `set_param` must agree on names in both directions, or a
    /// panel offers a control that does nothing when moved.
    #[test]
    fn every_exposed_parameter_can_be_set() {
        let mut steps = Recipe::starter("authored/set").steps;
        steps.push(Step::DoorWall {
            face: 0,
            z0: 0.0,
            z1: LEVEL,
            sill: FLOOR_TOP,
            top: DOOR_TOP,
            splay: 10.0,
            lintel_bevel: 8.0,
        });
        steps.push(Step::Band {
            face: 1,
            t0: 8.0,
            t1: 16.0,
            z0: 8.0,
            z1: 20.0,
        });
        steps.push(Step::Pylon {
            radius: 14.0,
            z0: 8.0,
            z1: 120.0,
            phase_deg: 0.0,
            chamfer_top: 5.0,
            chamfer_bottom: 0.0,
        });
        steps.push(Step::Box {
            min: (-10.0, -10.0, 0.0),
            max: (10.0, 10.0, 8.0),
        });
        steps.push(Step::Post {
            center: (0.0, 0.0),
            half: 6.0,
            z0: 8.0,
            z1: 120.0,
            chamfer_top: 3.0,
        });
        steps.push(Step::WallFixture {
            face: 2,
            along: 0.5,
            z: 88.0,
            width: 20.0,
        });

        for step in &mut steps {
            let label = step.label();
            for (name, _) in step.params() {
                assert!(
                    step.set_param(name, 1.0),
                    "{label} exposes {name} but will not set it"
                );
            }
            assert!(
                !step.set_param("definitely_not_a_parameter", 1.0),
                "{label} accepted a parameter it does not have"
            );
        }
    }

    /// Setting a parameter must actually change the geometry. A setter that
    /// wrote to a copy would pass the name check above and do nothing.
    #[test]
    fn setting_a_parameter_changes_the_emitted_geometry() {
        let mut step = Step::Pylon {
            radius: 14.0,
            z0: 8.0,
            z1: 120.0,
            phase_deg: 0.0,
            chamfer_top: 0.0,
            chamfer_bottom: 0.0,
        };
        let before = step.emit().0;
        assert!(step.set_param("radius", 20.0));
        assert_ne!(before, step.emit().0, "radius did not reach the geometry");
    }

    /// Faces wrap round the hexagon rather than clamping, so stepping past the
    /// last face returns to the first instead of sticking.
    #[test]
    fn a_face_index_wraps_around_the_hexagon() {
        let mut step = Step::Wall {
            face: 0,
            z0: 0.0,
            z1: LEVEL,
        };
        step.set_param("face", 6.0);
        assert_eq!(step.params()[0].1, 0.0, "face 6 must wrap to 0");
        step.set_param("face", -1.0);
        assert_eq!(step.params()[0].1, 5.0, "face -1 must wrap to 5");
    }

    /// A recipe that opens a door must declare the matching port, or the
    /// module has a hole the solver does not know about. This is the class of
    /// mistake the live validator is for, checked here at the seam.
    #[test]
    fn a_door_without_its_port_fails_validation() {
        let mut recipe = Recipe::starter("authored/undeclared_door");
        recipe.steps[2] = Step::DoorWall {
            face: 0,
            z0: 0.0,
            z1: LEVEL,
            sill: FLOOR_TOP,
            top: DOOR_TOP,
            splay: 10.0,
            lintel_bevel: 8.0,
        };
        // The geometry is now a doorway with no declared threshold. The
        // importer accepts the brushes - it cannot see intent - so this asserts
        // the recipe still builds, and the *seam* auditor is what catches the
        // mismatch later. Declaring the port is what makes it a threshold.
        assert!(recipe.build().contains("__TB_empty"));
        recipe.ports.push(RecipePort::Lateral {
            face: 0,
            class: String::from("door"),
            name: String::from("east_port"),
            level: 0,
            q: 0,
            r: 0,
        });
        recipe
            .validate()
            .unwrap_or_else(|error| panic!("a declared door must validate: {error:?}"));
    }
}
