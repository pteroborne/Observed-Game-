//! Declarative metadata table mapping profile scalars to interactive tunables.

use observed_facility::hex_wfc::profile::{
    HexCompositionProfile, MAX_SEARCH_CANDIDATES, SCORE_WEIGHT_MAX, SPACE_SHARE_MAX,
    SPACE_SHARE_MIN,
};
use observed_facility::hex_wfc::{HexArchetype, PROFILE_MAX, PROFILE_MIN};

use crate::StudioTab;

/// One editable scalar field inside a [`HexCompositionProfile`].
pub struct TunableField {
    /// What moving this actually does to the facility, in one line.
    ///
    /// Not documentation - UI copy, shown at rest beside the control. A row of
    /// numbers tells an author what a value *is* and never what it *means*, and
    /// `I don't know what it would do` is the reason a control goes untouched.
    /// A ratchet requires one per field, so a new tunable cannot ship mute.
    pub consequence: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub tab: StudioTab,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub get: fn(&HexCompositionProfile) -> f64,
    pub set: fn(&mut HexCompositionProfile, f64),
}

/// The scalar tunable fields present in a [`HexCompositionProfile`].
///
/// 4 composition tendencies + 9 archetype biases + 5 score weights + 3 space
/// shares + the search candidate count + the corridor routing switch.
pub const TUNABLE_FIELDS: &[TunableField] = &[
    // --- Space mix (3) ---
    // Shares, not multipliers, so the step is coarse and the range is wide: the
    // interesting span for void runs from the alphabet's own 4 up past 3,000,
    // and a slider fine enough for a bias would take a hundred presses to cross
    // it.
    TunableField {
        label: "space_void",
        consequence: "Higher empties the facility out, trading corridor for open nothing; the shipped default of 300 against hall's 100 gives roughly a fifth empty.",
        category: "Space mix",
        tab: StudioTab::Tuning,
        min: SPACE_SHARE_MIN,
        max: SPACE_SHARE_MAX,
        step: 100.0,
        get: |p| p.space_mix.void,
        set: |p, v| p.space_mix.void = v,
    },
    TunableField {
        label: "space_room",
        consequence: "Only decides anything inside a stamped room footprint, where room tiles are the sole legal space, so moving it changes very little.",
        category: "Space mix",
        tab: StudioTab::Tuning,
        min: SPACE_SHARE_MIN,
        max: SPACE_SHARE_MAX,
        step: 100.0,
        get: |p| p.space_mix.room,
        set: |p, v| p.space_mix.room = v,
    },
    TunableField {
        label: "space_hall",
        consequence: "Higher fills the facility with corridor at the expense of empty space; read it against space_void, since only their ratio matters.",
        category: "Space mix",
        tab: StudioTab::Tuning,
        min: SPACE_SHARE_MIN,
        max: SPACE_SHARE_MAX,
        step: 100.0,
        get: |p| p.space_mix.hall,
        set: |p, v| p.space_mix.hall = v,
    },
    // --- Corridor routing (1) ---
    // A switch rather than a scalar, carried in the same table so it appears
    // beside the shares it wants comparing against. Anything above zero is on.
    TunableField {
        label: "route_corridors",
        consequence: "On, the solver routes narrow corridors between the rooms and fixes their shape instead of hoping the collapse prefers narrow ones.",
        category: "Corridors",
        tab: StudioTab::Tuning,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        get: |p| f64::from(u8::from(p.route_corridors)),
        set: |p, v| p.route_corridors = v > 0.5,
    },
    // --- Tendencies (4) ---
    TunableField {
        label: "vertical_center_boost",
        consequence: "Higher pulls ramps and shafts toward the middle of the map, growing a legible vertical core instead of scattered climbs.",
        category: "Tendencies",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.tendencies.vertical_center_boost,
        set: |p, v| p.tendencies.vertical_center_boost = v,
    },
    TunableField {
        label: "vertical_edge_falloff",
        consequence: "Lower thins climbs at the outer rim, so the edge reads as flat perimeter rather than more of the same.",
        category: "Tendencies",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.tendencies.vertical_edge_falloff,
        set: |p, v| p.tendencies.vertical_edge_falloff = v,
    },
    TunableField {
        label: "room_low_level",
        consequence: "Lower pushes rooms off the ground floor, leaving it to connective corridor instead.",
        category: "Tendencies",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.tendencies.room_low_level,
        set: |p, v| p.tendencies.room_low_level = v,
    },
    TunableField {
        label: "room_high_level",
        consequence: "Higher stacks rooms toward the top floors, so upper levels read as chambers and lower ones as passage.",
        category: "Tendencies",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.tendencies.room_high_level,
        set: |p, v| p.tendencies.room_high_level = v,
    },
    // --- Archetype Biases (9) ---
    TunableField {
        label: "void_bias",
        consequence: "Higher leaves more of the lattice empty, opening voids the facility wraps around.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.void,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::Void, v),
    },
    TunableField {
        label: "room_bias",
        consequence: "Higher builds more room cells overall, at the cost of the corridor that links them.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.room,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::Room, v),
    },
    TunableField {
        label: "straight_bias",
        consequence: "Higher favours long runs, so routes read as corridors rather than as a series of turns.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.straight,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::Straight, v),
    },
    TunableField {
        label: "corner_bias",
        consequence: "Higher favours turns, making paths wind and hiding what is ahead.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.corner,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::Corner, v),
    },
    TunableField {
        label: "junction_bias",
        consequence: "Higher makes branching intersections common, so routes fork more and dead ends thin out.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.junction,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::Junction, v),
    },
    TunableField {
        label: "ramp_up_bias",
        consequence: "Higher builds more walkable ascents, which bots can climb, rather than shafts, which they cannot.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.ramp_up,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::RampUp, v),
    },
    TunableField {
        label: "ramp_head_bias",
        consequence: "The upper half of a ramp pair. Moves with ramp_up; on its own it mostly shifts where ascents top out.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.ramp_head,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::RampHead, v),
    },
    TunableField {
        label: "shaft_bias",
        consequence: "Higher adds vertical shafts. The facility is already shaft-heavy and the generic switchback is the fragile tile, so raising this tends to cost traversal reliability.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.shaft,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::Shaft, v),
    },
    TunableField {
        label: "expanse_bias",
        consequence: "Higher opens wall-free floor that neighbouring expanses join into, reading as one large volume rather than cells.",
        category: "Archetype Biases",
        tab: StudioTab::Tuning,
        min: PROFILE_MIN,
        max: PROFILE_MAX,
        step: 0.05,
        get: |p| p.archetype_bias.expanse,
        set: |p, v| p.archetype_bias = p.archetype_bias.with(HexArchetype::Expanse, v),
    },
    // --- Score Weights (5) ---
    TunableField {
        label: "connectivity_weight",
        consequence: "Scoring only. Higher prefers layouts where more rooms are reachable from spawn and routes are redundant.",
        category: "Score Weights",
        tab: StudioTab::Tuning,
        min: 0.0,
        max: SCORE_WEIGHT_MAX,
        step: 0.25,
        get: |p| p.score.connectivity,
        set: |p, v| p.score.connectivity = v,
    },
    TunableField {
        label: "elevation_weight",
        consequence: "Scoring only. Higher prefers layouts that use more of the available floors.",
        category: "Score Weights",
        tab: StudioTab::Tuning,
        min: 0.0,
        max: SCORE_WEIGHT_MAX,
        step: 0.25,
        get: |p| p.score.elevation,
        set: |p, v| p.score.elevation = v,
    },
    TunableField {
        label: "room_wholeness_weight",
        consequence: "Scoring only. Higher prefers layouts where rooms come from whole stamped blueprints rather than per-cell fallback.",
        category: "Score Weights",
        tab: StudioTab::Tuning,
        min: 0.0,
        max: SCORE_WEIGHT_MAX,
        step: 0.25,
        get: |p| p.score.room_wholeness,
        set: |p, v| p.score.room_wholeness = v,
    },
    TunableField {
        label: "variety_weight",
        consequence: "Scoring only. Higher prefers a mixed spread of traversal shapes, and will penalise a deliberately specialised composition.",
        category: "Score Weights",
        tab: StudioTab::Tuning,
        min: 0.0,
        max: SCORE_WEIGHT_MAX,
        step: 0.25,
        get: |p| p.score.variety,
        set: |p, v| p.score.variety = v,
    },
    TunableField {
        label: "rhythm_weight",
        consequence: "Scoring only. Higher prefers landmarks spaced evenly rather than clumped.",
        category: "Score Weights",
        tab: StudioTab::Tuning,
        min: 0.0,
        max: SCORE_WEIGHT_MAX,
        step: 0.25,
        get: |p| p.score.rhythm,
        set: |p, v| p.score.rhythm = v,
    },
    // --- Search (1) ---
    //
    // On the Solve tab rather than Tuning, because it is not a bias: it does not
    // change what the solver tends to build, it changes how many layouts get
    // built before one is chosen. Sitting it beside the biases would invite
    // reading it as one.
    TunableField {
        label: "search_candidates",
        consequence: "Solve this many layouts and keep the best-scoring. Candidate 0 is always this seed, so raising this can only improve the score -- but it multiplies solve time, and above 1 the seed no longer names one fixed facility.",
        category: "Search",
        tab: StudioTab::Solve,
        min: 1.0,
        max: MAX_SEARCH_CANDIDATES as f64,
        step: 1.0,
        get: |p| f64::from(p.search.candidates),
        // Rounded, not truncated: the slider reports a continuous value and
        // `2.999` must mean three candidates rather than two.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        set: |p, v| {
            p.search.candidates = v.round().clamp(1.0, f64::from(MAX_SEARCH_CANDIDATES)) as u32;
        },
    },
];
