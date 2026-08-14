//! What a player configures before a match starts.
//!
//! These are **match settings**, not developer switches. Every value has a name
//! a player would use ("Observation radius: 1 tile") rather than an internal field,
//! and the presets below are the intended way to move them — you dial this game
//! in by playing a preset and disagreeing with it, not by reading a spec.
//!
//! One rule holds the whole file together: **settings are fixed at match start.**
//! Nothing here is edited mid-match, so a match is reproducible from a
//! [`MatchSettings`] plus its action log, which is what makes the determinism
//! test meaningful and what any future replay or save would need.

use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{DistrictBias, HexArchetype, HexCompositionProfile, HexWfcConfig};

/// How many lattice cells each floor may contain.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BoardSize {
    /// One screen, one glance. The size to dial mechanics at.
    #[default]
    Compact,
    Standard,
    Sprawling,
    /// Exactly what the shipped game solves.
    Facility,
}

impl BoardSize {
    pub const ALL: [BoardSize; 4] = [
        BoardSize::Compact,
        BoardSize::Standard,
        BoardSize::Sprawling,
        BoardSize::Facility,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            BoardSize::Compact => "80 tiles (10 x 8)",
            BoardSize::Standard => "192 tiles (16 x 12)",
            BoardSize::Sprawling => "352 tiles (22 x 16)",
            BoardSize::Facility => "560 tiles (28 x 20)",
        }
    }

    /// The solver configuration this size means.
    ///
    /// Room counts rise with area rather than staying fixed: a `min_rooms` tuned
    /// for a compact board leaves a sprawling one mostly corridor, and the
    /// objective settings need enough distinct room roles to place their rooms.
    #[must_use]
    pub const fn config(self, levels: u8) -> HexWfcConfig {
        match self {
            BoardSize::Compact => HexWfcConfig {
                cols: 10,
                rows: 8,
                levels,
                min_rooms: 5,
                max_rooms: 7,
                retry_budget: 100,
                min_room_distance: 2,
            },
            BoardSize::Standard => HexWfcConfig {
                cols: 16,
                rows: 12,
                levels,
                min_rooms: 7,
                max_rooms: 9,
                retry_budget: 100,
                min_room_distance: 2,
            },
            BoardSize::Sprawling => HexWfcConfig {
                cols: 22,
                rows: 16,
                levels,
                min_rooms: 9,
                max_rooms: 10,
                retry_budget: 100,
                min_room_distance: 2,
            },
            BoardSize::Facility => HexWfcConfig {
                levels,
                ..HexWfcConfig::arc_default()
            },
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            BoardSize::Compact => BoardSize::Standard,
            BoardSize::Standard => BoardSize::Sprawling,
            BoardSize::Sprawling => BoardSize::Facility,
            BoardSize::Facility => BoardSize::Compact,
        }
    }
}

/// How far a unit's presence reaches — both what it reveals and what it freezes.
///
/// Sight remains the simulation rule: looking at structure is what stops it
/// rewiring. [`MatchSettings::reveal_full_map`] is deliberately separate and
/// presentation-only, so a teaching overlay cannot freeze the whole facility.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Sight {
    /// Only the cell a unit stands in: radius zero.
    Blind,
    /// The unit's cell and everything one open port away: radius one.
    #[default]
    Adjacent,
    /// Two hops through open ports: radius two.
    Corridor,
}

impl Sight {
    pub const ALL: [Sight; 3] = [Sight::Blind, Sight::Adjacent, Sight::Corridor];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Sight::Blind => "0 tiles (occupied tile only)",
            Sight::Adjacent => "1 tile",
            Sight::Corridor => "2 tiles",
        }
    }

    /// Hops through open ports a unit observes.
    ///
    #[must_use]
    pub const fn hops(self) -> u8 {
        match self {
            Sight::Blind => 0,
            Sight::Adjacent => 1,
            Sight::Corridor => 2,
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Sight::Blind => Sight::Adjacent,
            Sight::Adjacent => Sight::Corridor,
            Sight::Corridor => Sight::Blind,
        }
    }
}

/// How often unobserved structure re-collapses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShiftCadence {
    /// A static facility. The control case — play it once to feel what the
    /// shifting is worth.
    Off,
    #[default]
    EveryTurn,
    EveryOtherTurn,
    /// Every fourth turn.
    Slow,
}

impl ShiftCadence {
    pub const ALL: [ShiftCadence; 4] = [
        ShiftCadence::Off,
        ShiftCadence::EveryTurn,
        ShiftCadence::EveryOtherTurn,
        ShiftCadence::Slow,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ShiftCadence::Off => "Never",
            ShiftCadence::EveryTurn => "Every turn",
            ShiftCadence::EveryOtherTurn => "Every other turn",
            ShiftCadence::Slow => "Every 4 turns",
        }
    }

    /// Turn interval, or `None` when the facility never shifts.
    #[must_use]
    pub const fn interval(self) -> Option<u32> {
        match self {
            ShiftCadence::Off => None,
            ShiftCadence::EveryTurn => Some(1),
            ShiftCadence::EveryOtherTurn => Some(2),
            ShiftCadence::Slow => Some(4),
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            ShiftCadence::Off => ShiftCadence::EveryTurn,
            ShiftCadence::EveryTurn => ShiftCadence::EveryOtherTurn,
            ShiftCadence::EveryOtherTurn => ShiftCadence::Slow,
            ShiftCadence::Slow => ShiftCadence::Off,
        }
    }
}

/// How much unobserved structure one facility shift is allowed to reconsider.
///
/// The value scales with the complete multi-floor lattice, then stays inside a
/// solver-safe band. This makes the control comparable across map sizes while
/// keeping a compact teaching map from losing half a floor at once.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DecoherenceCoverage {
    Focused,
    #[default]
    Standard,
    Broad,
    Cascade,
}

impl DecoherenceCoverage {
    pub const ALL: [Self; 4] = [Self::Focused, Self::Standard, Self::Broad, Self::Cascade];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Focused => "Focused",
            Self::Standard => "Standard",
            Self::Broad => "Broad",
            Self::Cascade => "Cascade",
        }
    }

    #[must_use]
    pub fn target_cells(self, map_cells: usize) -> usize {
        let (divisor, minimum, maximum) = match self {
            Self::Focused => (24, 6, 24),
            Self::Standard => (16, 8, 32),
            Self::Broad => (10, 12, 48),
            Self::Cascade => (6, 16, 64),
        };
        map_cells.div_ceil(divisor).clamp(minimum, maximum)
    }

    #[must_use]
    pub fn max_cells(self, map_cells: usize) -> usize {
        self.target_cells(map_cells) * 2
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Focused => Self::Standard,
            Self::Standard => Self::Broad,
            Self::Broad => Self::Cascade,
            Self::Cascade => Self::Focused,
        }
    }
}

/// What a squad has to do besides reach the exit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Objectives {
    /// Get everyone to the exit. Nothing else.
    ExitOnly,
    /// Collect keystones before the exit will accept you.
    #[default]
    Keystones,
    /// Keystones, plus a station two units must hold together, plus monitors
    /// that survey the map around them.
    Full,
}

impl Objectives {
    pub const ALL: [Objectives; 3] = [
        Objectives::ExitOnly,
        Objectives::Keystones,
        Objectives::Full,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Objectives::ExitOnly => "Reach the exit",
            Objectives::Keystones => "Keystones",
            Objectives::Full => "Keystones + station",
        }
    }

    #[must_use]
    pub const fn keystones(self) -> bool {
        matches!(self, Objectives::Keystones | Objectives::Full)
    }

    #[must_use]
    pub const fn stations(self) -> bool {
        matches!(self, Objectives::Full)
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Objectives::ExitOnly => Objectives::Keystones,
            Objectives::Keystones => Objectives::Full,
            Objectives::Full => Objectives::ExitOnly,
        }
    }
}

/// Whether the Guardian hunts, and how fast.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GuardianSetting {
    Off,
    #[default]
    Slow,
    Hunting,
}

impl GuardianSetting {
    pub const ALL: [GuardianSetting; 3] = [
        GuardianSetting::Off,
        GuardianSetting::Slow,
        GuardianSetting::Hunting,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            GuardianSetting::Off => "Absent",
            GuardianSetting::Slow => "Patrolling",
            GuardianSetting::Hunting => "Hunting",
        }
    }

    /// Cells it closes per turn. Zero means it never moves.
    #[must_use]
    pub const fn steps_per_turn(self) -> u8 {
        match self {
            GuardianSetting::Off => 0,
            GuardianSetting::Slow => 1,
            GuardianSetting::Hunting => 2,
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            GuardianSetting::Off => GuardianSetting::Slow,
            GuardianSetting::Slow => GuardianSetting::Hunting,
            GuardianSetting::Hunting => GuardianSetting::Off,
        }
    }
}

/// Action point costs. Separated from [`MatchSettings`] because these are the
/// values most likely to be tuned as a group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionCosts {
    pub move_step: u8,
    pub interact: u8,
    pub anchor: u8,
    pub pad: u8,
}

impl Default for ActionCosts {
    fn default() -> Self {
        Self {
            move_step: 1,
            interact: 1,
            // Deliberately more than a step. An anchor is the strongest thing a
            // unit can do to the facility, and if it costs the same as walking
            // there is no decision in placing one.
            anchor: 2,
            pad: 1,
        }
    }
}

/// Everything the setup screen configures, and everything a match needs to be
/// reproducible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchSettings {
    pub board: BoardSize,
    /// Independently configured vertical extent of the board.
    pub floors: u8,
    pub seed: u64,
    pub squad_size: u8,
    pub action_points: u8,
    pub costs: ActionCosts,
    pub sight: Sight,
    /// Reveal the solved drawing without increasing observation or freeze range.
    pub reveal_full_map: bool,
    pub shift: ShiftCadence,
    /// Fraction of the facility eligible for one shift's relayout pocket.
    pub decoherence_coverage: DecoherenceCoverage,
    /// Show the pocket that is about to re-collapse, a turn before it does.
    pub telegraph: bool,
    pub anchors: bool,
    pub pads: bool,
    pub objectives: Objectives,
    pub keystones_required: u8,
    pub station_hold_turns: u8,
    pub guardian: GuardianSetting,
    pub rival_team: bool,
}

/// Squad sizes the setup screen offers.
pub const MIN_SQUAD: u8 = 2;
pub const MAX_SQUAD: u8 = 6;
/// Action points per unit per turn.
pub const MIN_ACTION_POINTS: u8 = 2;
pub const MAX_ACTION_POINTS: u8 = 8;
/// Floors per generated map.
pub const MIN_FLOORS: u8 = 1;
pub const MAX_FLOORS: u8 = 10;

impl Default for MatchSettings {
    fn default() -> Self {
        Self::standard()
    }
}

impl MatchSettings {
    /// A teaching match: the destination and every route are visible from turn one.
    #[must_use]
    pub fn guided() -> Self {
        Self {
            board: BoardSize::Compact,
            floors: 1,
            seed: 0x0000_0000_000c_0ffe,
            squad_size: 3,
            action_points: 6,
            costs: ActionCosts::default(),
            sight: Sight::Adjacent,
            reveal_full_map: true,
            shift: ShiftCadence::Slow,
            decoherence_coverage: DecoherenceCoverage::Focused,
            telegraph: true,
            anchors: true,
            pads: true,
            objectives: Objectives::ExitOnly,
            keystones_required: 1,
            station_hold_turns: 2,
            guardian: GuardianSetting::Off,
            rival_team: false,
        }
    }

    /// A first match: see everything coming, nothing chasing you.
    #[must_use]
    pub fn scout() -> Self {
        Self {
            board: BoardSize::Compact,
            floors: 3,
            seed: 0x0000_0000_000c_0ffe,
            squad_size: 3,
            action_points: 5,
            costs: ActionCosts::default(),
            sight: Sight::Corridor,
            reveal_full_map: false,
            shift: ShiftCadence::Slow,
            decoherence_coverage: DecoherenceCoverage::Focused,
            telegraph: true,
            anchors: true,
            pads: false,
            objectives: Objectives::ExitOnly,
            keystones_required: 1,
            station_hold_turns: 2,
            guardian: GuardianSetting::Off,
            rival_team: false,
        }
    }

    /// The intended shape of the game.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            board: BoardSize::Compact,
            floors: 3,
            seed: 0x0000_0000_000c_0ffe,
            squad_size: 3,
            action_points: 4,
            costs: ActionCosts::default(),
            sight: Sight::Blind,
            reveal_full_map: false,
            shift: ShiftCadence::EveryTurn,
            decoherence_coverage: DecoherenceCoverage::Standard,
            telegraph: true,
            anchors: true,
            pads: true,
            objectives: Objectives::Keystones,
            keystones_required: 1,
            station_hold_turns: 2,
            guardian: GuardianSetting::Slow,
            rival_team: false,
        }
    }

    /// Everything on, sight short, ground moving under you.
    #[must_use]
    pub fn collapse() -> Self {
        Self {
            board: BoardSize::Standard,
            floors: 4,
            seed: 0x5eed_0000_0000_0001,
            squad_size: 4,
            action_points: 4,
            costs: ActionCosts::default(),
            sight: Sight::Blind,
            reveal_full_map: false,
            shift: ShiftCadence::EveryTurn,
            decoherence_coverage: DecoherenceCoverage::Cascade,
            telegraph: false,
            anchors: true,
            pads: true,
            objectives: Objectives::Full,
            keystones_required: 1,
            station_hold_turns: 2,
            guardian: GuardianSetting::Hunting,
            rival_team: true,
        }
    }

    /// The solver configuration this match will run.
    #[must_use]
    pub fn facility(&self) -> HexWfcConfig {
        self.board.config(self.floors)
    }

    /// Target pocket size shown in setup before a solved world exists.
    #[must_use]
    pub fn decoherence_target_cells(&self) -> usize {
        let config = self.facility();
        let map_cells =
            usize::from(config.cols) * usize::from(config.rows) * usize::from(config.levels);
        self.decoherence_coverage.target_cells(map_cells)
    }

    /// Composition used by this tuning lab.
    ///
    /// This remains the real bounded production-profile path: no legal
    /// archetype is disabled and the solver's connectivity and room gates stay
    /// authoritative. The weights ask for more negative space, fewer repeated
    /// straight runs, and more turns, junctions, ramps, and open expanses.
    #[must_use]
    pub fn composition_profile(&self) -> HexCompositionProfile {
        let mut profile = HexCompositionProfile::baseline();
        profile.label = String::from("tactical negative space");
        profile.archetype_bias = profile
            .archetype_bias
            .with(HexArchetype::Void, 4.0)
            .with(HexArchetype::Room, 1.15)
            .with(HexArchetype::Straight, 0.55)
            .with(HexArchetype::Corner, 1.35)
            .with(HexArchetype::Junction, 1.65)
            .with(HexArchetype::RampUp, 1.45)
            .with(HexArchetype::RampHead, 1.45)
            .with(HexArchetype::Shaft, 0.50)
            .with(HexArchetype::Expanse, 1.70);
        profile.district_bias = ArchitectureRegister::ALL
            .into_iter()
            .map(|register| DistrictBias {
                register: register.slug().to_string(),
                bias: observed_facility::hex_wfc::ArchetypeBias::neutral()
                    .with(HexArchetype::Void, 4.0),
            })
            .collect();
        profile
    }

    /// Which named preset this equals, if any. Anything else is "Custom" — the
    /// setup screen reports it rather than silently pretending a preset is still
    /// selected after a value moved.
    #[must_use]
    pub fn preset_name(&self) -> &'static str {
        if *self == Self::guided() {
            "Guided"
        } else if *self == Self::scout() {
            "Scout"
        } else if *self == Self::standard() {
            "Standard"
        } else if *self == Self::collapse() {
            "Collapse"
        } else {
            "Custom"
        }
    }
}

/// One named preset: what the setup screen labels it, and what it builds.
#[derive(Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub build: fn() -> MatchSettings,
}

/// The named presets, in setup-screen order.
pub const PRESETS: [Preset; 4] = [
    Preset {
        name: "Guided",
        build: MatchSettings::guided,
    },
    Preset {
        name: "Scout",
        build: MatchSettings::scout,
    },
    Preset {
        name: "Standard",
        build: MatchSettings::standard,
    },
    Preset {
        name: "Collapse",
        build: MatchSettings::collapse,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_reports_its_own_name() {
        for preset in PRESETS {
            assert_eq!((preset.build)().preset_name(), preset.name);
        }
    }

    #[test]
    fn moving_any_value_off_a_preset_reports_custom() {
        let mut settings = MatchSettings::standard();
        settings.action_points += 1;
        assert_eq!(settings.preset_name(), "Custom");
    }

    /// Revealing the complete drawing remains independent from sight/freeze.
    #[test]
    fn full_map_is_a_presentation_setting_not_an_observation_radius() {
        let guided = MatchSettings::guided();
        assert!(guided.reveal_full_map);
        assert_eq!(guided.sight, Sight::Adjacent);
    }

    #[test]
    fn tactical_composition_is_valid_and_biases_toward_negative_space() {
        let profile = MatchSettings::standard().composition_profile();
        assert_eq!(profile.validate(), Ok(()));
        assert!(profile.archetype_bias.void > 1.0);
        assert!(profile.archetype_bias.straight < 1.0);
        assert!(profile.archetype_bias.junction > profile.archetype_bias.straight);
    }

    #[test]
    fn each_setting_cycles_through_all_of_its_values_and_wraps() {
        fn cycles<T: Copy + Eq + std::fmt::Debug>(all: &[T], next: impl Fn(T) -> T) {
            let mut seen = vec![all[0]];
            let mut value = all[0];
            for _ in 1..all.len() {
                value = next(value);
                assert!(!seen.contains(&value), "{value:?} repeats before wrapping");
                seen.push(value);
            }
            assert_eq!(next(value), all[0], "the last value wraps to the first");
            for expected in all {
                assert!(seen.contains(expected), "{expected:?} is unreachable");
            }
        }
        cycles(&BoardSize::ALL, BoardSize::next);
        cycles(&Sight::ALL, Sight::next);
        cycles(&ShiftCadence::ALL, ShiftCadence::next);
        cycles(&DecoherenceCoverage::ALL, DecoherenceCoverage::next);
        cycles(&Objectives::ALL, Objectives::next);
        cycles(&GuardianSetting::ALL, GuardianSetting::next);
    }

    /// A board has to hold the rooms its objectives need: start, exit, a keystone,
    /// and the station. A size that cannot is a setup screen offering a match
    /// that fails at generation.
    #[test]
    fn every_board_size_can_hold_a_full_objective_set() {
        for board in BoardSize::ALL {
            let config = board.config(MIN_FLOORS);
            assert!(
                config.min_rooms >= 5,
                "{} must fit start, exit, a keystone and a station",
                board.label()
            );
            assert!(config.max_rooms >= config.min_rooms);
            assert_eq!(config.levels, MIN_FLOORS);
            assert!(config.cols >= 4 && config.rows >= 4);
        }
    }

    #[test]
    fn floor_count_is_independent_of_tiles_per_floor() {
        let compact = BoardSize::Compact.config(7);
        let standard = BoardSize::Standard.config(7);
        assert_eq!(compact.levels, 7);
        assert_eq!(standard.levels, 7);
        assert_ne!((compact.cols, compact.rows), (standard.cols, standard.rows));
    }

    #[test]
    fn an_anchor_costs_more_than_a_step() {
        let costs = ActionCosts::default();
        assert!(costs.anchor > costs.move_step);
    }

    #[test]
    fn decoherence_coverage_scales_monotonically_and_stays_bounded() {
        for cells in [80, 240, 560, 1_680, 5_600] {
            let targets = DecoherenceCoverage::ALL.map(|coverage| coverage.target_cells(cells));
            assert!(targets.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(targets[0] >= 6);
            assert!(targets[3] <= 64);
        }
    }
}
