//! The authored composition profile: every knob that shapes *what* the solver
//! tends to build, as data instead of compile-time constants.
//!
//! Until this module existed, composition was unauthorable. The geometry
//! tendencies in [`super::context`] and the score weights in [`super::score`]
//! were literals, so retuning a district meant editing Rust and rebuilding, and
//! there was no way to compare two compositions in one process.
//!
//! # What this is *not*
//!
//! This is not configuration. [`super::HexWfcConfig`] is `Copy` and travels
//! inside a LAN launch datagram; a `String`/`Vec`-bearing profile on it would
//! change the wire format. The profile is **content**: it lives beside the
//! compiled tile catalog, is hashed into the simulation content hash that gates
//! LAN compatibility, and reaches the solver as a borrowed parameter.
//!
//! # The two invariants a profile may never break
//!
//! 1. **A legal variant is never driven to zero weight.** Every multiplier here
//!    is validated into `[PROFILE_MIN, PROFILE_MAX]`, strictly positive, and
//!    [`super::context::effective_weight`] floors any positive static weight at
//!    `1` after scaling. So every solve the unweighted selector could complete
//!    still completes — solvability is preserved no matter how a profile is
//!    tuned. A profile expresses *tendencies*; a genuine prohibition is a pin
//!    (see [`PinIntent::Forbid`]), which is checked rather than merely unlikely.
//! 2. **The RNG draw sequence is unchanged.** The collapse loop draws twice per
//!    observation: once to break min-entropy ties, once for the weighted
//!    lottery. The candidate set is a function of *domain lengths only* —
//!    weights never enter it — so a profile moves only `total` and the
//!    per-variant comparison values. Draw count and order are identical, and
//!    [`HexCompositionProfile::baseline`] reproduces today's layouts exactly.
//!
//!    The one deliberate exception is pins: a pin filters a cell's initial
//!    domain, which changes domain lengths, which changes the tie-break
//!    candidate set. **A profile with pins produces a different layout for the
//!    same seed.** That is intended — pins are hashed content, so a layout is
//!    still reproducible from (seed, profile).

use observed_content::ArchitectureRegister;

use super::{HexArchetype, HexSpace, PortClass};

/// Bumped whenever a stored profile's meaning changes. A profile carrying any
/// other version is rejected outright rather than reinterpreted.
///
/// **Also bump it whenever the solver's output changes for a fixed seed.** The
/// LAN handshake gates on `fold_simulation_content_hash`, which folds the
/// compiled catalog digest and the profile digest and nothing else - no part of
/// `hex_wfc` reaches it directly. So two peers running different solver builds
/// pass the handshake and then generate different facilities, which is a desync
/// with no diagnostic attached to it.
///
/// This constant is written into the stored profile, so moving it moves the
/// profile digest, moves the folded hash, and makes the handshake refuse a
/// mismatched peer instead. It is the only channel by which a solver change can
/// reach that hash; nothing else will notice.
pub const COMPOSITION_PROFILE_VERSION: u16 = 1;

/// The widest a score component's weight may be set. Unlike the lottery
/// multipliers, `0.0` *is* legal here: scoring is post-hoc and disabling a
/// component cannot threaten solvability.
pub const SCORE_WEIGHT_MAX: f64 = 16.0;

/// The most candidate layouts one solve may evaluate. Each candidate is a full
/// independent solve with its own retry budget, so this bounds match-start
/// latency.
pub const MAX_SEARCH_CANDIDATES: u32 = 8;

/// The authored composition of a facility: tendencies, biases, scoring, search
/// policy, and pinned authorial intent.
///
/// [`Self::baseline`] is byte-for-byte today's behaviour, so an absent or
/// untouched profile changes nothing.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HexCompositionProfile {
    pub version: u16,
    /// Free-text name for the authoring tool's benefit; never read by the solver.
    pub label: String,
    pub tendencies: CompositionTendencies,
    pub archetype_bias: ArchetypeBias,
    pub district_bias: Vec<DistrictBias>,
    pub score: ScoreWeights,
    pub search: SearchPolicy,
    pub pin_sets: Vec<PinSet>,
}

/// The geometry-context tendencies, previously the five constants at the top of
/// [`super::context`]. Verticals cluster toward the central axis; atria favour
/// the upper levels.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompositionTendencies {
    /// Whether positional tendencies apply at all. Off reverts to flat static
    /// weights, exactly as the pre-Phase-107 solver behaved.
    pub enabled: bool,
    /// Weight multiplier at the central axis for vertical archetypes.
    pub vertical_center_boost: f64,
    /// Weight multiplier at the grid edge for vertical archetypes.
    pub vertical_edge_falloff: f64,
    /// Weight multiplier for rooms at the lowest level.
    pub room_low_level: f64,
    /// Weight multiplier for rooms at the highest level.
    pub room_high_level: f64,
}

/// A per-archetype weight multiplier.
///
/// Named fields, deliberately **not** `[f64; 9]`. Arc O Phase 108 measured what
/// adding a tenth [`HexArchetype`] costs — sixteen match sites across five
/// crates, two of which the compiler could not catch. This struct is a
/// seventeenth site that it *can*: a new archetype breaks this build until
/// somebody decides what its bias means.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArchetypeBias {
    pub void: f64,
    pub room: f64,
    pub straight: f64,
    pub corner: f64,
    pub junction: f64,
    pub ramp_up: f64,
    pub ramp_head: f64,
    pub shaft: f64,
    pub expanse: f64,
}

/// An extra per-archetype bias applied only inside one district, on top of the
/// register's built-in identity table.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DistrictBias {
    /// An [`ArchitectureRegister`] slug. Stored as text so the RON stays
    /// human-diffable and `observed_content` needs no serde derive; resolved
    /// and checked by [`HexCompositionProfile::validate`].
    pub register: String,
    pub bias: ArchetypeBias,
}

/// Relative importance of each [`super::score::LayoutScore`] component when
/// candidate layouts are compared.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScoreWeights {
    pub connectivity: f64,
    pub elevation: f64,
    pub room_wholeness: f64,
    pub variety: f64,
    pub rhythm: f64,
}

/// How hard the solver looks for a good layout, as opposed to a legal one.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SearchPolicy {
    /// Candidate layouts to solve and score. `1` is today's behaviour: solve
    /// once, keep it.
    ///
    /// Raising this **changes which facility a seed produces**, because the
    /// winner is drawn from derived candidate seeds rather than `seed` itself.
    pub candidates: u32,
    /// Replaces [`super::HexWfcConfig::retry_budget`] when set.
    pub retry_budget_override: Option<u32>,
}

/// A named set of authored cell constraints, together with the lattice they
/// were drawn on.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PinSet {
    pub id: String,
    /// Why these pins exist, for whoever reads the diff.
    pub note: String,
    /// The lattice these pins were authored against. A mismatch with the live
    /// config is a hard validation error — never a silent reinterpretation,
    /// because the same `(q, r, level)` means a different place on a different
    /// grid.
    pub cols: u16,
    pub rows: u16,
    pub levels: u8,
    pub pins: Vec<HexPin>,
}

/// One authored constraint on one cell.
///
/// Flat `q`/`r`/`level` rather than a serialized [`super::HexCoord`]:
/// `observed_hex` is deliberately dependency-free and must stay that way.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HexPin {
    pub q: u16,
    pub r: u16,
    pub level: u8,
    pub intent: PinIntent,
}

/// What an author asserted about a cell.
///
/// There is deliberately no `District` variant. District assignment runs
/// through `relayout::initial_architecture`, a separate pass over an
/// already-solved world; folding it in here would misrepresent the mechanism.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PinIntent {
    /// The cell must be void, room, or hall.
    Space(HexSpace),
    /// The cell must take exactly this traversal grammar.
    Archetype(HexArchetype),
    /// The cell must take none of these archetypes.
    Forbid(Vec<HexArchetype>),
    /// The cell must present exactly these ports: `doors` is a lateral face
    /// bitmask (see `super::lateral_bit`), plus the two vertical classes.
    Ports {
        doors: u8,
        up: PinPortClass,
        down: PinPortClass,
    },
}

/// A serializable mirror of [`PortClass`].
///
/// `observed_hex` is deliberately dependency-free — it is the one crate in the
/// stack with no `[dependencies]` at all — so it cannot carry a serde derive,
/// not even an optional one. Mirroring the four variants here keeps that
/// promise and gives the stored RON names that are stable independently of the
/// grid crate's internals.
///
/// The exhaustive `From` conversions below are the drift guard: adding a fifth
/// [`PortClass`] fails this build until somebody decides whether an author may
/// pin it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PinPortClass {
    Sealed,
    Door,
    RampOpen,
    ShaftOpen,
}

impl From<PortClass> for PinPortClass {
    fn from(class: PortClass) -> Self {
        match class {
            PortClass::Sealed => Self::Sealed,
            PortClass::Door => Self::Door,
            PortClass::RampOpen => Self::RampOpen,
            PortClass::ShaftOpen => Self::ShaftOpen,
        }
    }
}

impl From<PinPortClass> for PortClass {
    fn from(class: PinPortClass) -> Self {
        match class {
            PinPortClass::Sealed => Self::Sealed,
            PinPortClass::Door => Self::Door,
            PinPortClass::RampOpen => Self::RampOpen,
            PinPortClass::ShaftOpen => Self::ShaftOpen,
        }
    }
}

/// Why a profile was rejected. Every variant names the offending field or value
/// so the authoring tool can point at it.
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileDefect {
    UnsupportedVersion {
        found: u16,
        expected: u16,
    },
    BiasOutOfRange {
        field: &'static str,
        value: f64,
        min: f64,
        max: f64,
    },
    ScoreWeightOutOfRange {
        field: &'static str,
        value: f64,
        max: f64,
    },
    NonFiniteWeight {
        field: &'static str,
    },
    UnknownRegister {
        slug: String,
    },
    DuplicateDistrictBias {
        slug: String,
    },
    CandidatesOutOfRange {
        found: u32,
        max: u32,
    },
    DuplicatePinSetId {
        id: String,
    },
    EmptyForbidList {
        pin_set: String,
        q: u16,
        r: u16,
        level: u8,
    },
}

impl core::fmt::Display for ProfileDefect {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                write!(f, "profile version {found} is not the supported {expected}")
            }
            Self::BiasOutOfRange {
                field,
                value,
                min,
                max,
            } => write!(f, "{field} = {value} is outside [{min}, {max}]"),
            Self::ScoreWeightOutOfRange { field, value, max } => {
                write!(f, "score weight {field} = {value} is outside [0, {max}]")
            }
            Self::NonFiniteWeight { field } => write!(f, "{field} is not a finite number"),
            Self::UnknownRegister { slug } => write!(f, "no architecture register named {slug:?}"),
            Self::DuplicateDistrictBias { slug } => {
                write!(f, "register {slug:?} has more than one district bias")
            }
            Self::CandidatesOutOfRange { found, max } => {
                write!(f, "search candidates {found} is outside [1, {max}]")
            }
            Self::DuplicatePinSetId { id } => write!(f, "more than one pin set named {id:?}"),
            Self::EmptyForbidList {
                pin_set,
                q,
                r,
                level,
            } => write!(
                f,
                "pin set {pin_set:?} forbids nothing at ({q}, {r}, {level}); \
                 an empty forbid list constrains no variant"
            ),
        }
    }
}

impl ArchetypeBias {
    /// No bias — every archetype at its natural weight.
    #[must_use]
    pub const fn neutral() -> Self {
        Self {
            void: 1.0,
            room: 1.0,
            straight: 1.0,
            corner: 1.0,
            junction: 1.0,
            ramp_up: 1.0,
            ramp_head: 1.0,
            shaft: 1.0,
            expanse: 1.0,
        }
    }

    /// The multiplier for one archetype.
    #[must_use]
    pub const fn get(&self, archetype: HexArchetype) -> f64 {
        match archetype {
            HexArchetype::Void => self.void,
            HexArchetype::Room => self.room,
            HexArchetype::Straight => self.straight,
            HexArchetype::Corner => self.corner,
            HexArchetype::Junction => self.junction,
            HexArchetype::RampUp => self.ramp_up,
            HexArchetype::RampHead => self.ramp_head,
            HexArchetype::Shaft => self.shaft,
            HexArchetype::Expanse => self.expanse,
        }
    }

    /// Set one archetype's multiplier. Chainable, for tests and the tool.
    #[must_use]
    pub const fn with(mut self, archetype: HexArchetype, factor: f64) -> Self {
        match archetype {
            HexArchetype::Void => self.void = factor,
            HexArchetype::Room => self.room = factor,
            HexArchetype::Straight => self.straight = factor,
            HexArchetype::Corner => self.corner = factor,
            HexArchetype::Junction => self.junction = factor,
            HexArchetype::RampUp => self.ramp_up = factor,
            HexArchetype::RampHead => self.ramp_head = factor,
            HexArchetype::Shaft => self.shaft = factor,
            HexArchetype::Expanse => self.expanse = factor,
        }
        self
    }

    /// Every field paired with its serialized name, for validation and for the
    /// authoring tool's slider list.
    #[must_use]
    pub const fn fields(&self) -> [(&'static str, f64); 9] {
        [
            ("void", self.void),
            ("room", self.room),
            ("straight", self.straight),
            ("corner", self.corner),
            ("junction", self.junction),
            ("ramp_up", self.ramp_up),
            ("ramp_head", self.ramp_head),
            ("shaft", self.shaft),
            ("expanse", self.expanse),
        ]
    }
}

impl Default for ArchetypeBias {
    fn default() -> Self {
        Self::neutral()
    }
}

impl CompositionTendencies {
    /// The values the solver shipped with before tendencies became authorable.
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            enabled: super::context::COMPOSITION_TENDENCIES_ENABLED,
            vertical_center_boost: super::context::VERTICAL_CENTER_BOOST,
            vertical_edge_falloff: super::context::VERTICAL_EDGE_FALLOFF,
            room_low_level: super::context::ROOM_LOW_LEVEL,
            room_high_level: super::context::ROOM_HIGH_LEVEL,
        }
    }

    #[must_use]
    pub const fn fields(&self) -> [(&'static str, f64); 4] {
        [
            ("vertical_center_boost", self.vertical_center_boost),
            ("vertical_edge_falloff", self.vertical_edge_falloff),
            ("room_low_level", self.room_low_level),
            ("room_high_level", self.room_high_level),
        ]
    }
}

impl ScoreWeights {
    /// The fixed weights [`super::score::score_layout`] shipped with.
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            connectivity: super::score::WEIGHT_CONNECTIVITY,
            elevation: super::score::WEIGHT_ELEVATION,
            room_wholeness: super::score::WEIGHT_ROOM_WHOLENESS,
            variety: super::score::WEIGHT_VARIETY,
            rhythm: super::score::WEIGHT_RHYTHM,
        }
    }

    #[must_use]
    pub const fn fields(&self) -> [(&'static str, f64); 5] {
        [
            ("connectivity", self.connectivity),
            ("elevation", self.elevation),
            ("room_wholeness", self.room_wholeness),
            ("variety", self.variety),
            ("rhythm", self.rhythm),
        ]
    }
}

impl SearchPolicy {
    /// Solve once, keep it — today's behaviour.
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            candidates: 1,
            retry_budget_override: None,
        }
    }
}

impl HexCompositionProfile {
    /// The profile that reproduces today's solver byte for byte.
    ///
    /// Every entry point that does not take a profile delegates here, so this
    /// is the definition of "unchanged" that the regression tests assert
    /// against.
    #[must_use]
    pub fn baseline() -> Self {
        Self {
            version: COMPOSITION_PROFILE_VERSION,
            label: String::from("baseline"),
            tendencies: CompositionTendencies::baseline(),
            archetype_bias: ArchetypeBias::neutral(),
            district_bias: Vec::new(),
            score: ScoreWeights::baseline(),
            search: SearchPolicy::baseline(),
            pin_sets: Vec::new(),
        }
    }

    /// Whether this profile leaves the solve exactly as [`Self::baseline`]
    /// would. Used to keep the fast path honest and to label the tool's status
    /// bar.
    #[must_use]
    pub fn is_baseline(&self) -> bool {
        *self == Self::baseline()
    }

    /// The authored multiplier for `archetype`, independent of district.
    #[must_use]
    pub const fn bias_for(&self, archetype: HexArchetype) -> f64 {
        self.archetype_bias.get(archetype)
    }

    /// The authored multiplier for `archetype` inside `register`'s district,
    /// or `1.0` when the profile says nothing about that district.
    ///
    /// This stacks *on top of* the register's built-in identity table in
    /// [`super::context`]; it does not replace it.
    #[must_use]
    pub fn district_bias_for(
        &self,
        register: ArchitectureRegister,
        archetype: HexArchetype,
    ) -> f64 {
        let slug = register.slug();
        self.district_bias
            .iter()
            .find(|entry| entry.register == slug)
            .map_or(1.0, |entry| entry.bias.get(archetype))
    }

    /// Every defect in this profile, or `Ok(())`. Reports all of them at once
    /// so an author fixes one round of errors rather than one error.
    ///
    /// # Errors
    ///
    /// Returns every [`ProfileDefect`] found, in a stable order.
    pub fn validate(&self) -> Result<(), Vec<ProfileDefect>> {
        let mut defects = Vec::new();

        if self.version != COMPOSITION_PROFILE_VERSION {
            defects.push(ProfileDefect::UnsupportedVersion {
                found: self.version,
                expected: COMPOSITION_PROFILE_VERSION,
            });
        }

        for (name, value) in self.tendencies.fields() {
            check_multiplier(name, value, &mut defects);
        }
        for (name, value) in self.archetype_bias.fields() {
            check_multiplier(name, value, &mut defects);
        }

        let mut seen_registers: Vec<&str> = Vec::new();
        for entry in &self.district_bias {
            let known = ArchitectureRegister::ALL
                .iter()
                .any(|register| register.slug() == entry.register);
            if !known {
                defects.push(ProfileDefect::UnknownRegister {
                    slug: entry.register.clone(),
                });
            }
            if seen_registers.contains(&entry.register.as_str()) {
                defects.push(ProfileDefect::DuplicateDistrictBias {
                    slug: entry.register.clone(),
                });
            } else {
                seen_registers.push(entry.register.as_str());
            }
            for (name, value) in entry.bias.fields() {
                check_multiplier(name, value, &mut defects);
            }
        }

        for (name, value) in self.score.fields() {
            if !value.is_finite() {
                defects.push(ProfileDefect::NonFiniteWeight { field: name });
            } else if !(0.0..=SCORE_WEIGHT_MAX).contains(&value) {
                defects.push(ProfileDefect::ScoreWeightOutOfRange {
                    field: name,
                    value,
                    max: SCORE_WEIGHT_MAX,
                });
            }
        }

        if self.search.candidates < 1 || self.search.candidates > MAX_SEARCH_CANDIDATES {
            defects.push(ProfileDefect::CandidatesOutOfRange {
                found: self.search.candidates,
                max: MAX_SEARCH_CANDIDATES,
            });
        }

        let mut seen_sets: Vec<&str> = Vec::new();
        for set in &self.pin_sets {
            if seen_sets.contains(&set.id.as_str()) {
                defects.push(ProfileDefect::DuplicatePinSetId { id: set.id.clone() });
            } else {
                seen_sets.push(set.id.as_str());
            }
            for pin in &set.pins {
                if matches!(&pin.intent, PinIntent::Forbid(list) if list.is_empty()) {
                    defects.push(ProfileDefect::EmptyForbidList {
                        pin_set: set.id.clone(),
                        q: pin.q,
                        r: pin.r,
                        level: pin.level,
                    });
                }
            }
        }

        if defects.is_empty() {
            Ok(())
        } else {
            Err(defects)
        }
    }
}

/// A lottery multiplier must be finite and inside the shared bias band, so no
/// single input can dominate the draw or starve a legal variant.
fn check_multiplier(field: &'static str, value: f64, defects: &mut Vec<ProfileDefect>) {
    if !value.is_finite() {
        defects.push(ProfileDefect::NonFiniteWeight { field });
    } else if !(super::context::PROFILE_MIN..=super::context::PROFILE_MAX).contains(&value) {
        defects.push(ProfileDefect::BiasOutOfRange {
            field,
            value,
            min: super::context::PROFILE_MIN,
            max: super::context::PROFILE_MAX,
        });
    }
}

impl Default for HexCompositionProfile {
    fn default() -> Self {
        Self::baseline()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_baseline_profile_validates() {
        assert_eq!(HexCompositionProfile::baseline().validate(), Ok(()));
    }

    #[test]
    fn the_baseline_profile_is_neutral_everywhere() {
        let profile = HexCompositionProfile::baseline();
        for archetype in [
            HexArchetype::Void,
            HexArchetype::Room,
            HexArchetype::Straight,
            HexArchetype::Corner,
            HexArchetype::Junction,
            HexArchetype::RampUp,
            HexArchetype::RampHead,
            HexArchetype::Shaft,
            HexArchetype::Expanse,
        ] {
            assert_eq!(profile.bias_for(archetype), 1.0, "{archetype:?}");
            for register in ArchitectureRegister::ALL {
                assert_eq!(
                    profile.district_bias_for(register, archetype),
                    1.0,
                    "{register:?}/{archetype:?}"
                );
            }
        }
    }

    /// The tendencies and score weights must *be* the constants they replaced,
    /// or "baseline reproduces today" is a claim without a check.
    #[test]
    fn the_baseline_tendencies_are_the_shipped_constants() {
        let t = CompositionTendencies::baseline();
        assert!(t.enabled);
        assert!((t.vertical_center_boost - 1.2).abs() < f64::EPSILON);
        assert!((t.vertical_edge_falloff - 0.9).abs() < f64::EPSILON);
        assert!((t.room_low_level - 0.9).abs() < f64::EPSILON);
        assert!((t.room_high_level - 1.2).abs() < f64::EPSILON);

        let s = ScoreWeights::baseline();
        assert!((s.connectivity - 3.0).abs() < f64::EPSILON);
        assert!((s.elevation - 1.5).abs() < f64::EPSILON);
        assert!((s.room_wholeness - 2.0).abs() < f64::EPSILON);
        assert!((s.variety - 1.5).abs() < f64::EPSILON);
        assert!((s.rhythm - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_wrong_version_is_rejected() {
        let mut profile = HexCompositionProfile::baseline();
        profile.version = COMPOSITION_PROFILE_VERSION + 1;
        let defects = profile.validate().expect_err("version must be checked");
        assert!(defects.iter().any(|defect| matches!(
            defect,
            ProfileDefect::UnsupportedVersion { found, .. } if *found == COMPOSITION_PROFILE_VERSION + 1
        )));
    }

    /// The band is what keeps invariant 1 true, so its edges are pinned.
    #[test]
    fn a_bias_outside_the_shared_band_is_rejected() {
        for out_of_range in [0.0, 0.1, 4.5, -1.0] {
            let mut profile = HexCompositionProfile::baseline();
            profile.archetype_bias = profile
                .archetype_bias
                .with(HexArchetype::Shaft, out_of_range);
            let defects = profile
                .validate()
                .expect_err("{out_of_range} must be rejected");
            assert!(
                defects
                    .iter()
                    .any(|defect| matches!(defect, ProfileDefect::BiasOutOfRange { field, .. } if *field == "shaft")),
                "{out_of_range} produced {defects:?}"
            );
        }
    }

    #[test]
    fn the_band_edges_themselves_are_accepted() {
        for edge in [
            super::super::context::PROFILE_MIN,
            super::super::context::PROFILE_MAX,
        ] {
            let mut profile = HexCompositionProfile::baseline();
            profile.archetype_bias = profile.archetype_bias.with(HexArchetype::Shaft, edge);
            assert_eq!(profile.validate(), Ok(()), "edge {edge} must be legal");
        }
    }

    #[test]
    fn a_non_finite_bias_is_rejected() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut profile = HexCompositionProfile::baseline();
            profile.archetype_bias = profile.archetype_bias.with(HexArchetype::Room, bad);
            let defects = profile.validate().expect_err("non-finite must be rejected");
            assert!(
                defects
                    .iter()
                    .any(|defect| matches!(defect, ProfileDefect::NonFiniteWeight { field } if *field == "room"))
            );
        }
    }

    /// Score weights are post-hoc, so zero is legal there and only there.
    #[test]
    fn a_zero_score_weight_is_legal_but_a_zero_bias_is_not() {
        let mut profile = HexCompositionProfile::baseline();
        profile.score.rhythm = 0.0;
        assert_eq!(profile.validate(), Ok(()));

        let mut profile = HexCompositionProfile::baseline();
        profile.archetype_bias = profile.archetype_bias.with(HexArchetype::Shaft, 0.0);
        assert!(profile.validate().is_err());
    }

    #[test]
    fn an_unknown_register_slug_is_rejected() {
        let mut profile = HexCompositionProfile::baseline();
        profile.district_bias.push(DistrictBias {
            register: String::from("not_a_district"),
            bias: ArchetypeBias::neutral(),
        });
        let defects = profile.validate().expect_err("slug must be checked");
        assert!(defects.iter().any(|defect| matches!(
            defect,
            ProfileDefect::UnknownRegister { slug } if slug == "not_a_district"
        )));
    }

    #[test]
    fn every_real_register_slug_is_accepted() {
        for register in ArchitectureRegister::ALL {
            let mut profile = HexCompositionProfile::baseline();
            profile.district_bias.push(DistrictBias {
                register: register.slug().to_string(),
                bias: ArchetypeBias::neutral().with(HexArchetype::Shaft, 2.0),
            });
            assert_eq!(profile.validate(), Ok(()), "{register:?}");
            assert!(
                (profile.district_bias_for(register, HexArchetype::Shaft) - 2.0).abs()
                    < f64::EPSILON
            );
        }
    }

    #[test]
    fn a_duplicate_district_bias_is_rejected() {
        let mut profile = HexCompositionProfile::baseline();
        for _ in 0..2 {
            profile.district_bias.push(DistrictBias {
                register: ArchitectureRegister::Wellshaft.slug().to_string(),
                bias: ArchetypeBias::neutral(),
            });
        }
        let defects = profile.validate().expect_err("duplicates must be caught");
        assert!(
            defects
                .iter()
                .any(|defect| matches!(defect, ProfileDefect::DuplicateDistrictBias { .. }))
        );
    }

    #[test]
    fn candidates_outside_the_bound_are_rejected() {
        for bad in [0, MAX_SEARCH_CANDIDATES + 1] {
            let mut profile = HexCompositionProfile::baseline();
            profile.search.candidates = bad;
            let defects = profile.validate().expect_err("{bad} must be rejected");
            assert!(
                defects
                    .iter()
                    .any(|defect| matches!(defect, ProfileDefect::CandidatesOutOfRange { .. }))
            );
        }
    }

    #[test]
    fn an_empty_forbid_list_is_rejected() {
        let mut profile = HexCompositionProfile::baseline();
        profile.pin_sets.push(PinSet {
            id: String::from("set"),
            note: String::new(),
            cols: 12,
            rows: 9,
            levels: 1,
            pins: vec![HexPin {
                q: 1,
                r: 1,
                level: 0,
                intent: PinIntent::Forbid(Vec::new()),
            }],
        });
        let defects = profile.validate().expect_err("empty forbid must be caught");
        assert!(
            defects
                .iter()
                .any(|defect| matches!(defect, ProfileDefect::EmptyForbidList { .. }))
        );
    }

    /// Every defect is reported in one pass; an author should not have to
    /// re-run validation once per mistake.
    #[test]
    fn validate_reports_every_defect_at_once() {
        let mut profile = HexCompositionProfile::baseline();
        profile.version = 99;
        profile.archetype_bias = profile.archetype_bias.with(HexArchetype::Shaft, 0.0);
        profile.search.candidates = 0;
        let defects = profile.validate().expect_err("three defects");
        assert_eq!(defects.len(), 3, "{defects:?}");
    }

    /// The mirror must agree with the grid crate in both directions, or a pin
    /// would silently mean a different port than it reads as.
    #[test]
    fn the_port_class_mirror_round_trips() {
        for class in [
            PortClass::Sealed,
            PortClass::Door,
            PortClass::RampOpen,
            PortClass::ShaftOpen,
        ] {
            let mirrored = PinPortClass::from(class);
            assert_eq!(PortClass::from(mirrored), class, "{class:?}");
        }
    }

    #[test]
    fn is_baseline_tracks_any_edit() {
        let mut profile = HexCompositionProfile::baseline();
        assert!(profile.is_baseline());
        profile.archetype_bias = profile.archetype_bias.with(HexArchetype::Shaft, 1.5);
        assert!(!profile.is_baseline());
    }
}
