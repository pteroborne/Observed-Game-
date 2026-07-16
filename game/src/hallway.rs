//! Authored hallway pieces for the **teleport-connected** facility.
//!
//! In the teleport model each graph edge is realised as a discrete, pre-generated
//! **hallway piece** you teleport into: cross a room's doorway → you are placed at
//! the entry of that edge's current hallway → walk it → cross its far doorway → you
//! are placed in the destination room. Because you only ever occupy one space,
//! everything else is *unobserved by construction*, so an edge can freely swap which
//! hallway variation (and which destination) it shows whenever you are not inside it
//! — no shared maze grid, no off-camera-swap gymnastics, no doorway/hall alignment to
//! reconcile.
//!
//! This module is the pure data + selection: the authored template library and the
//! deterministic edge → variation mapping. The controller/presentation consume it.

use observed_core::RoomId;
use observed_facility::map_spec::TraversalArchetype;
use observed_traversal::gantry::{GANTRY_LENGTH, GANTRY_WIDTH, UPPER_DECK_Y};
use std::f32::consts::TAU;

use crate::layout::{HALL_STRETCH, HALL_WIDEN};

/// The traversal character of a hallway piece — the "edge personality" the
/// nodes/edges canon asks for (corridors = traverse / danger / mystery).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HallwayFlavor {
    /// A plain, short connector.
    Straight,
    /// A long, exposed run.
    Long,
    /// A broad S-bend sampled into gently turning structural walls. The wide lane hides
    /// its destination through curvature without reading as a cramped obstacle slalom.
    Chicane,
    /// An institutional corridor with two full-width right-angle turns and a larger
    /// empty work bay between them.
    Orthogonal,
    /// A pressure-gate hazard spans the hall (the risky shortcut).
    PressureGate,
    /// A step-up climb toward the next level.
    Climb,
    /// A wide, long **pseudo-room** that seems to go on — an open volume broken up by a
    /// regular grid of bare structural pillars (the "unfinished megastructure" read), with
    /// a clear central lane straight through. Expansive, not claustrophobic; branching
    /// among the columns rather than a tight corridor.
    Colonnade,
    /// A generated grid **labyrinth** — winding corridors, dead ends, and braided
    /// loops between the single entrance and exit (see [`crate::maze`]).
    Maze,
    /// A two-level jump-map hall. The pure two-level route proof lives in
    /// `observed_traversal::gantry`; the assembled teleport model projects its lower
    /// safe-bypass footprint until hallway-side third thresholds are supported.
    Gantry,
    /// A deep vertical connector: the player arrives on a high ring and follows a
    /// bidirectional perimeter stair down through isolated pools of practical light.
    Wellshaft,
}

impl HallwayFlavor {
    pub fn label(self) -> &'static str {
        match self {
            HallwayFlavor::Straight => "straight",
            HallwayFlavor::Long => "long",
            HallwayFlavor::Chicane => "chicane",
            HallwayFlavor::Orthogonal => "orthogonal",
            HallwayFlavor::PressureGate => "pressure gate",
            HallwayFlavor::Climb => "climb",
            HallwayFlavor::Colonnade => "colonnade",
            HallwayFlavor::Maze => "labyrinth",
            HallwayFlavor::Gantry => "gantry",
            HallwayFlavor::Wellshaft => "wellshaft",
        }
    }

    /// Whether this hallway carries a time-pressure hazard (drives the safe/risky
    /// choice the match brain already models on spine legs).
    pub fn is_hazardous(self) -> bool {
        matches!(self, HallwayFlavor::PressureGate)
    }
}

/// An authored hallway template: a discrete walkable piece between an entry and an
/// exit doorway. Dimensions are world units; kept simple and data-driven so the
/// library is easy to extend (the controller adds collision from these, the
/// presentation renders them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HallwayTemplate {
    pub name: &'static str,
    pub flavor: HallwayFlavor,
    /// Walkable length from the entry doorway to the exit doorway. For a `Maze` this is
    /// the derived footprint depth; the real geometry comes from `grid`.
    pub length: f32,
    /// Walkable width (derived footprint width for a `Maze`; see `grid`).
    pub width: f32,
    /// Net rise from entry to exit (0 for flat; positive climbs).
    pub rise: f32,
    /// For a `Maze` flavour, the labyrinth grid dimensions `(cols, rows)`; `None` for
    /// the simple straight/dogleg/climb pieces.
    pub grid: Option<(u8, u8)>,
}

/// The authored library. An index into this is a "variation". A *significant portion*
/// of the pieces are generated labyrinths (`Maze`), the rest are quick connectors and
/// set-pieces, so the facility's edges feel varied — some a single stride, some a maze.
pub const TEMPLATES: [HallwayTemplate; 14] = [
    HallwayTemplate {
        name: "short connector",
        flavor: HallwayFlavor::Straight,
        // Even the "short" connector is a comfortable run, never a single stride; the
        // length floor in `hallway_geom` keeps it that way under the scale multiplier.
        length: 15.0,
        width: 6.0,
        rise: 0.0,
        grid: None,
    },
    HallwayTemplate {
        name: "long hall",
        flavor: HallwayFlavor::Long,
        length: 28.0,
        width: 6.0,
        rise: 0.0,
        grid: None,
    },
    HallwayTemplate {
        name: "chicane",
        flavor: HallwayFlavor::Chicane,
        length: 26.0,
        // This is the clear curved-lane budget, not the overall swept footprint.
        width: 12.0,
        rise: 0.0,
        grid: None,
    },
    HallwayTemplate {
        name: "pressure gate",
        flavor: HallwayFlavor::PressureGate,
        length: 16.0,
        width: 6.5,
        rise: 0.0,
        grid: None,
    },
    HallwayTemplate {
        name: "stair climb",
        flavor: HallwayFlavor::Climb,
        length: 16.0,
        width: 6.0,
        rise: 0.9,
        grid: None,
    },
    // Pillared pseudo-rooms: wide, long, open volumes with a regular grid of structural
    // columns and a clear central lane. The interior pillars are generated in
    // `teleport::hallway_geom` from the footprint; `grid` stays `None`.
    HallwayTemplate {
        name: "colonnade",
        flavor: HallwayFlavor::Colonnade,
        length: 30.0,
        width: 16.0,
        rise: 0.0,
        grid: None,
    },
    HallwayTemplate {
        name: "great hall",
        flavor: HallwayFlavor::Colonnade,
        length: 26.0,
        width: 22.0,
        rise: 0.0,
        grid: None,
    },
    HallwayTemplate {
        name: "gantry",
        flavor: HallwayFlavor::Gantry,
        length: GANTRY_LENGTH,
        width: GANTRY_WIDTH,
        rise: UPPER_DECK_Y,
        grid: None,
    },
    // The labyrinths. `length`/`width` are the derived footprints (cols/rows × the
    // maze cell size of 4.2) for readability; `grid` drives the actual geometry.
    HallwayTemplate {
        name: "small labyrinth",
        flavor: HallwayFlavor::Maze,
        length: 50.4,
        width: 50.4,
        rise: 0.0,
        grid: Some((12, 12)),
    },
    HallwayTemplate {
        name: "twisting labyrinth",
        flavor: HallwayFlavor::Maze,
        length: 63.0,
        width: 75.6,
        rise: 0.0,
        grid: Some((15, 18)),
    },
    HallwayTemplate {
        name: "deep labyrinth",
        flavor: HallwayFlavor::Maze,
        length: 75.6,
        width: 88.2,
        rise: 0.0,
        grid: Some((18, 21)),
    },
    HallwayTemplate {
        name: "wide labyrinth",
        flavor: HallwayFlavor::Maze,
        length: 88.2,
        width: 63.0,
        rise: 0.0,
        grid: Some((21, 15)),
    },
    HallwayTemplate {
        name: "tall labyrinth",
        flavor: HallwayFlavor::Maze,
        length: 50.4,
        width: 100.8,
        rise: 0.0,
        grid: Some((12, 24)),
    },
    HallwayTemplate {
        name: "wellshaft",
        flavor: HallwayFlavor::Wellshaft,
        // Six radial landing bridges around the hex pillar. The geometry branch owns
        // the exact regular-hex footprint and intentionally skips hallway scaling.
        length: WELL_SHAFT_OUTER_RADIUS * 2.0,
        width: WELL_SHAFT_OUTER_APOTHEM * 2.0,
        rise: WELL_SHAFT_HEIGHT,
        grid: None,
    },
];

/// Catalogue-v2 institutional connector. It stays outside [`TEMPLATES`] so adding the
/// design cannot perturb `liminal_wfc_v1`'s variation indices or replay stream.
pub const ORTHOGONAL_TEMPLATE: HallwayTemplate = HallwayTemplate {
    name: "orthogonal factory hall",
    flavor: HallwayFlavor::Orthogonal,
    length: 34.0,
    width: 14.0,
    rise: 0.0,
    grid: None,
};

pub const WELL_SHAFT_LEVEL_HEIGHT: f32 = 3.0;
pub const WELL_SHAFT_LEVELS: usize = 18;
pub const WELL_SHAFT_HEIGHT: f32 = WELL_SHAFT_LEVEL_HEIGHT * (WELL_SHAFT_LEVELS - 1) as f32;
/// Visual hex-prism circumradius of the central pillar.
pub const WELL_SHAFT_PILLAR_RADIUS: f32 = 6.0;
/// Conservative square collision core for the pillar. Its corners stay inboard of
/// every tread's inner edge, so the controller resolves against the treads and is
/// never shoved off by a core lip poking up through them.
pub const WELL_SHAFT_PILLAR_COLLISION_HALF: f32 = 3.3;
pub const WELL_SHAFT_LANDING_RADIUS: f32 = 9.6;
pub const WELL_SHAFT_LANDING_HALF: f32 = 1.6;
pub const WELL_SHAFT_BRIDGE_END_RADIUS: f32 = 14.5;
pub const WELL_SHAFT_BRIDGE_WIDTH: f32 = 1.8;
pub const WELL_SHAFT_OUTER_APOTHEM: f32 = WELL_SHAFT_BRIDGE_END_RADIUS;
pub const WELL_SHAFT_OUTER_RADIUS: f32 = WELL_SHAFT_OUTER_APOTHEM / 0.866_025_4;

/// The spiral's treads are cantilevered from the pillar: each occupies this radial
/// band, whose inner edge overlaps the pillar (springs from the core) and whose
/// outer edge overlaps the landing platforms (shared, continuous floor).
pub const WELL_SHAFT_TREAD_INNER_RADIUS: f32 = 4.8;
pub const WELL_SHAFT_TREAD_OUTER_RADIUS: f32 = 9.2;
pub const WELL_SHAFT_TREAD_MID_RADIUS: f32 =
    (WELL_SHAFT_TREAD_INNER_RADIUS + WELL_SHAFT_TREAD_OUTER_RADIUS) * 0.5;
pub const WELL_SHAFT_TREAD_RADIAL_HALF: f32 =
    (WELL_SHAFT_TREAD_OUTER_RADIUS - WELL_SHAFT_TREAD_INNER_RADIUS) * 0.5;
/// Tangential half-width, chosen so adjacent treads overlap across the whole band
/// (contiguous, no gaps) while the exposed run stays a legal footing.
pub const WELL_SHAFT_TREAD_TANGENTIAL_HALF: f32 = 0.7;
/// Eight treads create seven legal-height risers; the first meets its lower landing
/// and the last meets the upper landing.
pub const WELL_SHAFT_STEPS_PER_FLIGHT: usize = 8;
pub const WELL_SHAFT_STEP_RISE: f32 =
    WELL_SHAFT_LEVEL_HEIGHT / (WELL_SHAFT_STEPS_PER_FLIGHT - 1) as f32;
/// Vertical closure of a tread: one riser plus a small lip, so each tread's front
/// face closes onto the tread below (no open risers) with minimal overhang.
pub const WELL_SHAFT_TREAD_CLOSURE: f32 = WELL_SHAFT_STEP_RISE + 0.12;
/// A short parapet on the mid-flight treads' outward edge, so the descent cannot be
/// walked off by accident. End treads abut landings and stay open to the bridges.
pub const WELL_SHAFT_GUARD_HEIGHT: f32 = 1.0;
pub const WELL_SHAFT_GUARD_THICKNESS: f32 = 0.24;
pub const WELL_SHAFT_DECK_THICKNESS: f32 = 0.24;

/// Unit radial direction for a landing. Level zero points to the live bottom exit;
/// level five points to the live elevated entry; levels 1–4 are sealed service bays.
pub fn wellshaft_level_direction(level: usize) -> (f32, f32) {
    let angle = level as f32 * TAU / 6.0;
    (angle.cos(), angle.sin())
}

pub fn wellshaft_landing_center(level: usize) -> (f32, f32) {
    let (x, z) = wellshaft_level_direction(level);
    (x * WELL_SHAFT_LANDING_RADIUS, z * WELL_SHAFT_LANDING_RADIUS)
}

pub fn wellshaft_landing_rest(level: usize) -> (f32, f32) {
    let center = wellshaft_landing_center(level);
    let direction = wellshaft_level_direction(level);
    (center.0 + direction.0 * 0.55, center.1 + direction.1 * 0.55)
}

/// Angle of the `step`-th tread of the flight rising from `lower_level`, evenly
/// spaced across the 60° the spiral turns between adjacent landings.
pub fn wellshaft_tread_angle(lower_level: usize, step: usize) -> f32 {
    let a0 = lower_level as f32 * TAU / 6.0;
    let a1 = (lower_level + 1) as f32 * TAU / 6.0;
    a0 + (a1 - a0) * (step as f32 + 0.5) / WELL_SHAFT_STEPS_PER_FLIGHT as f32
}

/// World-space XZ centre of a tread's footprint (at the mid-band radius). The tread
/// is a radial slab cantilevered from the pillar out to the landing band.
pub fn wellshaft_stair_center(lower_level: usize, step: usize) -> (f32, f32) {
    let angle = wellshaft_tread_angle(lower_level, step);
    (
        angle.cos() * WELL_SHAFT_TREAD_MID_RADIUS,
        angle.sin() * WELL_SHAFT_TREAD_MID_RADIUS,
    )
}

/// Whether the `step`-th tread of a flight carries an outward guard rail. End treads
/// abut the landings and stay open so the flight connects to the threshold bridges.
pub fn wellshaft_tread_has_guard(step: usize) -> bool {
    step > 0 && step + 1 < WELL_SHAFT_STEPS_PER_FLIGHT
}

pub fn wellshaft_template() -> &'static HallwayTemplate {
    TEMPLATES
        .iter()
        .find(|template| template.flavor == HallwayFlavor::Wellshaft)
        .expect("the authored hallway library contains the wellshaft")
}

pub fn gantry_template() -> &'static HallwayTemplate {
    TEMPLATES
        .iter()
        .find(|template| template.flavor == HallwayFlavor::Gantry)
        .expect("the authored hallway library contains the gantry")
}

pub fn template(index: usize) -> &'static HallwayTemplate {
    &TEMPLATES[index % TEMPLATES.len()]
}

/// Resolve a persisted catalogue-v2 traversal identity without involving the live
/// decoherence version. The generation key only chooses among same-archetype variants.
pub fn template_for_traversal(
    traversal: TraversalArchetype,
    generation_key: u64,
) -> &'static HallwayTemplate {
    let flavor = match traversal {
        TraversalArchetype::Straight => HallwayFlavor::Straight,
        TraversalArchetype::Long => HallwayFlavor::Long,
        TraversalArchetype::Pressure => HallwayFlavor::PressureGate,
        TraversalArchetype::Climb => HallwayFlavor::Climb,
        TraversalArchetype::Maze => HallwayFlavor::Maze,
        TraversalArchetype::Chicane => HallwayFlavor::Chicane,
        TraversalArchetype::GantryExpanse => HallwayFlavor::Gantry,
        TraversalArchetype::Wellshaft => HallwayFlavor::Wellshaft,
        TraversalArchetype::Colonnade => HallwayFlavor::Colonnade,
        TraversalArchetype::Orthogonal => return &ORTHOGONAL_TEMPLATE,
    };
    let count = TEMPLATES
        .iter()
        .filter(|template| template.flavor == flavor)
        .count();
    TEMPLATES
        .iter()
        .filter(|template| template.flavor == flavor)
        .nth(generation_key as usize % count.max(1))
        .expect("every persisted traversal archetype has an authored projection")
}

/// The liminal-scaled (length, width) for `template` (the Phase 46b dials in
/// `layout.rs`): `HALL_STRETCH`/`HALL_WIDEN` apply to every authored connector/set-piece
/// template EXCEPT the Gantry (its dimensions are the pure jump-map course's own
/// constants, authored and untouched) and the `grid`-driven labyrinths (their footprint
/// is cell-derived — `MAZE_CELL` × `grid`'s cols/rows in `teleport::geom::hallway_geom`
/// — so stretching the authored `length`/`width` fields here would only relabel that
/// cell-driven geometry without actually lengthening the maze; growing a labyrinth means
/// more cells, which reshuffles its interior generation, so it is left alone here and the
/// grid dimensions stay the tuning lever for maze size instead).
pub fn scaled_dims(template: &HallwayTemplate) -> (f32, f32) {
    if matches!(
        template.flavor,
        HallwayFlavor::Gantry | HallwayFlavor::Wellshaft
    ) || template.grid.is_some()
    {
        return (template.length, template.width);
    }
    (template.length * HALL_STRETCH, template.width * HALL_WIDEN)
}

/// An edge-symmetric 64-bit hash of the edge `(a, b)` plus a `mix` salt. The basis for
/// both variation selection and maze layout, so both are deterministic and treat
/// `(a, b)` the same as `(b, a)`.
fn edge_hash(a: RoomId, b: RoomId, seed: u64, mix: u64) -> u64 {
    let (lo, hi) = if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
    let mut h = seed
        ^ (lo as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (hi as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ mix.wrapping_mul(0x1656_67B1_9E37_79F9);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h
}

/// Deterministically pick a hallway variation for the edge between rooms `a` and `b`.
/// `version` increments each time the edge decoheres (reroutes) so an *unobserved*
/// edge can present a different piece next time — the "changed when you looked away"
/// loop. Edge-symmetric: `(a, b)` and `(b, a)` choose the same piece.
pub fn variation_for(a: RoomId, b: RoomId, seed: u64, version: u32) -> usize {
    // The final Wellshaft template is role-selected by the facility WFC rather than
    // entering the ordinary reroll pool. This keeps it sparse and prevents a locked
    // objective edge from acquiring vertical geometry through an unrelated variation.
    (edge_hash(a, b, seed, version as u64) as usize) % (TEMPLATES.len() - 1)
}

/// The seed for a maze hallway's interior layout. Keyed by the edge and the chosen
/// `variation`, so it is a pure function of the stored `Place::Hallway` — the maze
/// can't reshuffle under the player's feet — yet still changes when a reroute rolls a
/// new variation. Edge-symmetric, matching `variation_for`.
pub fn layout_seed(a: RoomId, b: RoomId, variation: usize) -> u64 {
    edge_hash(a, b, 0xA1B2_C3D4_5EED_F00D, variation as u64)
}

/// The shortest a hallway may ever be (world units). Even after the length multiplier,
/// no hallway drops below this, so a corridor always feels like a journey rather than a
/// single step (the "longer than expected" design rule). Applied in [`crate::teleport`].
pub const MIN_HALL_LENGTH: f32 = 14.0;

/// A deterministic length multiplier in `[1.0, 2.2]` for a straight-ish hallway, so
/// repeated connector templates render at visibly different depths (the seed is the
/// edge's `layout_seed`). The floor is 1.0 so the multiplier only ever *lengthens* a
/// hall past its authored base, never shrinks it below. Mazes ignore this — their size
/// comes from their grid.
pub fn length_scale(seed: u64) -> f32 {
    const LO: f32 = 1.0;
    const HI: f32 = 2.2;
    // Mix the seed first so even small/sequential seeds spread evenly.
    let mut h = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 31;
    let t = (h >> 40) as f32 / (1u64 << 24) as f32;
    LO + t * (HI - LO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_is_non_empty_and_well_formed() {
        assert!(!TEMPLATES.is_empty());
        for t in TEMPLATES {
            assert!(
                t.length > 0.0 && t.width > 0.0,
                "{} has real dimensions",
                t.name
            );
        }
        // Distinct names and distinct flavours (each piece is its own personality).
        let mut names: Vec<&str> = TEMPLATES.iter().map(|t| t.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TEMPLATES.len(), "template names are unique");
    }

    #[test]
    fn selection_is_deterministic_and_in_range() {
        let a = RoomId(1);
        let b = RoomId(5);
        let first = variation_for(a, b, 7, 0);
        assert_eq!(first, variation_for(a, b, 7, 0), "same inputs → same piece");
        assert!(first < TEMPLATES.len());
    }

    #[test]
    fn ordinary_variation_rolls_never_select_the_role_owned_wellshaft() {
        for a in 0..16 {
            for b in a + 1..16 {
                for version in 0..8 {
                    let picked = template(variation_for(RoomId(a), RoomId(b), 91, version));
                    assert_ne!(picked.flavor, HallwayFlavor::Wellshaft);
                }
            }
        }
    }

    #[test]
    fn selection_is_edge_symmetric() {
        for (a, b, seed, version) in [(0u32, 8u32, 1u64, 0u32), (3, 4, 99, 2), (2, 6, 42, 5)] {
            assert_eq!(
                variation_for(RoomId(a), RoomId(b), seed, version),
                variation_for(RoomId(b), RoomId(a), seed, version),
                "(a,b) and (b,a) pick the same hallway"
            );
        }
    }

    #[test]
    fn decohering_an_edge_can_present_a_new_piece() {
        // Across enough versions, at least one re-roll changes the chosen piece (the
        // "changed when unobserved" loop has something to show).
        let (a, b) = (RoomId(1), RoomId(2));
        let base = variation_for(a, b, 3, 0);
        let changed = (1..32).any(|version| variation_for(a, b, 3, version) != base);
        assert!(
            changed,
            "rerolling an edge should eventually change its hallway"
        );
    }

    #[test]
    fn a_different_seed_diverges() {
        let differ = (0..ROOM_PAIRS.len()).any(|i| {
            let (a, b) = ROOM_PAIRS[i];
            variation_for(RoomId(a), RoomId(b), 1, 0) != variation_for(RoomId(a), RoomId(b), 2, 0)
        });
        assert!(differ, "the seed actually influences selection");
    }

    #[test]
    fn liminal_scale_dials_apply_only_to_authored_connector_templates() {
        let straight = TEMPLATES
            .iter()
            .find(|template| template.grid.is_none() && template.flavor != HallwayFlavor::Gantry)
            .expect("a non-grid, non-gantry template exists");
        let (straight_len, straight_width) = scaled_dims(straight);
        assert!(
            straight_len > straight.length,
            "authored connector length should get the liminal stretch"
        );
        assert!(
            straight_width > straight.width,
            "authored connector width should get the liminal widen pass"
        );

        let maze = TEMPLATES
            .iter()
            .find(|template| template.grid.is_some())
            .expect("a grid-driven maze template exists");
        assert_eq!(
            scaled_dims(maze),
            (maze.length, maze.width),
            "grid-driven mazes keep their cell-derived footprint"
        );

        let gantry = TEMPLATES
            .iter()
            .find(|template| template.flavor == HallwayFlavor::Gantry)
            .expect("a gantry template exists");
        assert_eq!(
            scaled_dims(gantry),
            (gantry.length, gantry.width),
            "the pure gantry course keeps authored traversal dimensions"
        );
    }

    const ROOM_PAIRS: [(u32, u32); 6] = [(0, 1), (1, 2), (2, 5), (5, 8), (3, 4), (4, 7)];
}
