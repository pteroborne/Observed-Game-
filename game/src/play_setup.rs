//! Pure, persisted match-setup intent shared by the Play Hub, local launch, and LAN host.
//!
//! Player preferences answer "how should the application feel?". This module answers
//! the separate question "what match should we create?". Keeping those domains apart
//! means a menu refactor cannot silently change authoritative roster rules.

use bevy::prelude::Resource;
use observed_facility::hex_wfc::HexWfcConfig;
use observed_match::hex_wfc::{HexMatchConfig, MAX_ROSTER};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayPreset {
    #[default]
    Solo,
    CoOp,
    TeamRace,
    Spectate,
    Custom,
}

impl PlayPreset {
    pub const ALL: [Self; 5] = [
        Self::Solo,
        Self::CoOp,
        Self::TeamRace,
        Self::Spectate,
        Self::Custom,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Solo => "Solo",
            Self::CoOp => "Co-op",
            Self::TeamRace => "Team race",
            Self::Spectate => "Spectate",
            Self::Custom => "Custom",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Solo => "Explore alone with the Guardian active.",
            Self::CoOp => "One four-seat team; empty seats are bot-filled.",
            Self::TeamRace => "Two teams of two race through the same facility.",
            Self::Spectate => "Watch two bot teams solve and traverse the facility.",
            Self::Custom => "Choose teams, team size, bot fill, and Guardian pressure.",
        }
    }
}

/// Editable setup retained when the player moves between frontend screens.
#[derive(Resource, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaySetupDraft {
    pub preset: PlayPreset,
    pub teams: u8,
    pub members_per_team: u8,
    pub fill_empty_seats: bool,
    pub guardian: bool,
}

impl Default for PlaySetupDraft {
    fn default() -> Self {
        Self::for_preset(PlayPreset::Solo)
    }
}

impl PlaySetupDraft {
    #[must_use]
    pub const fn for_preset(preset: PlayPreset) -> Self {
        match preset {
            PlayPreset::Solo => Self {
                preset,
                teams: 1,
                members_per_team: 1,
                fill_empty_seats: false,
                guardian: true,
            },
            PlayPreset::CoOp => Self {
                preset,
                teams: 1,
                members_per_team: 4,
                fill_empty_seats: true,
                guardian: true,
            },
            PlayPreset::TeamRace => Self {
                preset,
                teams: 2,
                members_per_team: 2,
                fill_empty_seats: true,
                guardian: true,
            },
            PlayPreset::Spectate => Self {
                preset,
                teams: 2,
                members_per_team: 2,
                fill_empty_seats: true,
                guardian: true,
            },
            PlayPreset::Custom => Self {
                preset,
                teams: 2,
                members_per_team: 2,
                fill_empty_seats: true,
                guardian: true,
            },
        }
    }

    pub fn select_preset(&mut self, preset: PlayPreset) {
        if preset == PlayPreset::Custom {
            self.preset = PlayPreset::Custom;
        } else {
            *self = Self::for_preset(preset);
        }
    }

    fn normalized_after_load(mut self) -> Self {
        if self.preset == PlayPreset::Custom {
            self.teams = self.teams.clamp(1, MAX_ROSTER);
            let maximum_team_size = (MAX_ROSTER / self.teams).max(1);
            self.members_per_team = self.members_per_team.clamp(1, maximum_team_size);
            self
        } else {
            Self::for_preset(self.preset)
        }
    }

    pub fn validate(&self) -> Result<ValidatedPlaySetup, PlaySetupError> {
        if self.teams == 0 {
            return Err(PlaySetupError::NoTeams);
        }
        if self.members_per_team == 0 {
            return Err(PlaySetupError::EmptyTeams);
        }
        let seats = self.teams.saturating_mul(self.members_per_team);
        if seats == 0 || seats > MAX_ROSTER {
            return Err(PlaySetupError::TooManySeats {
                requested: seats,
                maximum: MAX_ROSTER,
            });
        }
        Ok(ValidatedPlaySetup {
            preset: self.preset,
            teams: self.teams,
            members_per_team: self.members_per_team,
            fill_empty_seats: self.fill_empty_seats,
            guardian: self.guardian,
            spectator: self.preset == PlayPreset::Spectate,
        })
    }

    pub fn summary(&self) -> String {
        match self.validate() {
            Ok(validated) => validated.summary(),
            Err(error) => error.to_string(),
        }
    }
}

/// Load a setup independently from user preferences. Older builds stored three play
/// fields inside `saves/settings.json`; that shape is read once as a migration source.
#[must_use]
pub fn load_play_setup() -> PlaySetupDraft {
    #[cfg(test)]
    if crate::settings::TEST_CONFIG_DIR.with(|path| path.borrow().is_none()) {
        return PlaySetupDraft::default();
    }

    load_play_setup_from(
        &crate::settings::play_setup_path(),
        &crate::settings::legacy_settings_path(),
    )
}

fn load_play_setup_from(primary: &std::path::Path, legacy: &std::path::Path) -> PlaySetupDraft {
    if let Ok(text) = std::fs::read_to_string(primary)
        && let Ok(draft) = serde_json::from_str::<PlaySetupDraft>(&text)
    {
        return draft.normalized_after_load();
    }
    if let Ok(text) = std::fs::read_to_string(legacy)
        && let Some(draft) = legacy_play_setup(&text)
    {
        let _ = save_play_setup_to(primary, &draft);
        return draft;
    }
    PlaySetupDraft::default()
}

pub fn save_play_setup(play_setup: &PlaySetupDraft) {
    #[cfg(test)]
    if crate::settings::TEST_CONFIG_DIR.with(|path| path.borrow().is_none()) {
        return;
    }

    if let Err(error) = save_play_setup_to(&crate::settings::play_setup_path(), play_setup) {
        bevy::log::warn!("could not save play setup: {error}");
    }
}

fn save_play_setup_to(path: &std::path::Path, play_setup: &PlaySetupDraft) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(play_setup)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, json)?;
    if path.exists() {
        std::fs::copy(&temporary, path)?;
        std::fs::remove_file(temporary)
    } else {
        std::fs::rename(temporary, path)
    }
}

fn legacy_play_setup(text: &str) -> Option<PlaySetupDraft> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let object = value.as_object()?;
    let fill = object.get("bot_fill")?.as_bool()?;
    let guardian = object
        .get("guardian")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let mut draft = if fill {
        PlaySetupDraft::for_preset(PlayPreset::TeamRace)
    } else {
        PlaySetupDraft::for_preset(PlayPreset::Solo)
    };
    draft.guardian = guardian;
    if draft != PlaySetupDraft::for_preset(draft.preset) {
        draft.preset = PlayPreset::Custom;
    }
    Some(draft)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPlaySetup {
    pub preset: PlayPreset,
    pub teams: u8,
    pub members_per_team: u8,
    pub fill_empty_seats: bool,
    pub guardian: bool,
    pub spectator: bool,
}

impl ValidatedPlaySetup {
    #[must_use]
    pub const fn seats(self) -> u8 {
        self.teams * self.members_per_team
    }

    /// Local play has one human. If bot fill is disabled, no inert bodies are created.
    #[must_use]
    pub fn local_match_config(self, wfc: HexWfcConfig) -> HexMatchConfig {
        let (teams, members_per_team) = if self.fill_empty_seats {
            (self.teams, self.members_per_team)
        } else {
            (1, 1)
        };
        HexMatchConfig {
            teams,
            members_per_team,
            guardian: self.guardian,
            wfc,
        }
    }

    pub fn summary(self) -> String {
        let roster = if self.teams == 1 && self.members_per_team == 1 {
            "1 explorer".to_string()
        } else if self.teams == 1 {
            format!("{} seat co-op", self.members_per_team)
        } else {
            format!("{} teams x {}", self.teams, self.members_per_team)
        };
        format!(
            "{roster} | bots {} | Guardian {}",
            if self.fill_empty_seats { "fill" } else { "off" },
            if self.guardian { "on" } else { "off" },
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaySetupError {
    NoTeams,
    EmptyTeams,
    TooManySeats { requested: u8, maximum: u8 },
}

impl std::fmt::Display for PlaySetupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTeams => formatter.write_str("Choose at least one team."),
            Self::EmptyTeams => formatter.write_str("Choose at least one seat per team."),
            Self::TooManySeats { requested, maximum } => {
                write!(
                    formatter,
                    "{requested} seats exceeds the {maximum}-seat limit."
                )
            }
        }
    }
}

/// Why a launch exists. Presentation uses this for copy/back behavior; simulation
/// still receives only the validated match specification.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LaunchContext {
    #[default]
    Local,
    Rematch,
    Lan,
}

/// Player-facing shape of the match that was actually launched.
///
/// This is intentionally derived from the finalized launch request rather than the
/// editable setup draft. Results and replay presentation therefore describe the
/// roster the simulation received, even when local no-fill rules reduced a custom
/// draft to the single human seat that can really be driven.
#[derive(Resource, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActivePlaySession {
    pub(crate) kind: ActivePlayKind,
    pub(crate) teams: u8,
    pub(crate) members_per_team: u8,
    pub(crate) networked: bool,
}

impl ActivePlaySession {
    #[must_use]
    pub(crate) const fn from_launch(
        config: HexMatchConfig,
        spectator: bool,
        networked: bool,
    ) -> Self {
        let kind = if spectator {
            ActivePlayKind::Spectate
        } else if config.teams == 1 && config.members_per_team == 1 {
            ActivePlayKind::Solo
        } else if config.teams == 1 {
            ActivePlayKind::CoOp
        } else {
            ActivePlayKind::TeamRace
        };
        Self {
            kind,
            teams: config.teams,
            members_per_team: config.members_per_team,
            networked,
        }
    }

    pub(crate) fn summary(self) -> String {
        let roster = if self.teams == 1 {
            format!(
                "{} seat{}",
                self.members_per_team,
                plural_suffix(self.members_per_team)
            )
        } else {
            format!("{} teams x {}", self.teams, self.members_per_team)
        };
        format!(
            "{} | {roster} | {}",
            self.kind.label(),
            if self.networked { "LAN" } else { "local" }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActivePlayKind {
    Solo,
    CoOp,
    TeamRace,
    Spectate,
}

impl ActivePlayKind {
    #[must_use]
    const fn label(self) -> &'static str {
        match self {
            Self::Solo => "Solo",
            Self::CoOp => "Co-op",
            Self::TeamRace => "Team race",
            Self::Spectate => "Spectate",
        }
    }
}

const fn plural_suffix(count: u8) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid_and_have_stable_rosters() {
        let cases = [
            (PlayPreset::Solo, 1, 1, false, false),
            (PlayPreset::CoOp, 1, 4, true, false),
            (PlayPreset::TeamRace, 2, 2, true, false),
            (PlayPreset::Spectate, 2, 2, true, true),
        ];
        for (preset, teams, size, fill, spectator) in cases {
            let setup = PlaySetupDraft::for_preset(preset)
                .validate()
                .expect("shipped preset is valid");
            assert_eq!(setup.teams, teams);
            assert_eq!(setup.members_per_team, size);
            assert_eq!(setup.fill_empty_seats, fill);
            assert_eq!(setup.spectator, spectator);
        }
    }

    #[test]
    fn custom_setup_enforces_the_wire_roster_cap() {
        let valid = PlaySetupDraft {
            preset: PlayPreset::Custom,
            teams: 4,
            members_per_team: 4,
            fill_empty_seats: true,
            guardian: false,
        };
        assert_eq!(valid.validate().expect("4x4 is valid").seats(), 16);

        let invalid = PlaySetupDraft { teams: 5, ..valid };
        assert_eq!(
            invalid.validate(),
            Err(PlaySetupError::TooManySeats {
                requested: 20,
                maximum: 16,
            })
        );
    }

    #[test]
    fn bot_free_local_setup_creates_no_idle_bodies() {
        let setup = PlaySetupDraft {
            preset: PlayPreset::Custom,
            teams: 4,
            members_per_team: 4,
            fill_empty_seats: false,
            guardian: false,
        }
        .validate()
        .expect("draft validates");
        let config = setup.local_match_config(HexWfcConfig::default());
        assert_eq!((config.teams, config.members_per_team), (1, 1));
        assert!(!config.guardian);
    }

    #[test]
    fn serde_defaults_keep_partial_play_setup_readable_and_normalized() {
        let draft: PlaySetupDraft =
            serde_json::from_str(r#"{"preset":"co_op"}"#).expect("partial persisted draft loads");
        assert_eq!(
            draft.normalized_after_load(),
            PlaySetupDraft::for_preset(PlayPreset::CoOp)
        );
    }

    #[test]
    fn malformed_custom_roster_is_clamped_to_a_valid_sixteen_seat_shape() {
        let draft = PlaySetupDraft {
            preset: PlayPreset::Custom,
            teams: u8::MAX,
            members_per_team: u8::MAX,
            fill_empty_seats: true,
            guardian: true,
        }
        .normalized_after_load();
        assert_eq!(draft.teams, MAX_ROSTER);
        assert_eq!(draft.members_per_team, 1);
        assert!(draft.validate().is_ok());
    }

    #[test]
    fn legacy_migration_preserves_bot_fill_and_guardian_choices() {
        let migrated = legacy_play_setup(r#"{"bot_fill":false,"guardian":false}"#)
            .expect("legacy fields migrate");
        assert_eq!(migrated.preset, PlayPreset::Custom);
        assert!(!migrated.fill_empty_seats);
        assert!(!migrated.guardian);
    }

    #[test]
    fn corrupt_platform_setup_falls_back_to_and_migrates_legacy_settings() {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target/test-scratch")
            .join(format!(
                "play-setup-migration-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        let primary = scratch.join("platform/play_setup.json");
        let legacy = scratch.join("legacy/settings.json");
        std::fs::create_dir_all(primary.parent().expect("primary parent")).expect("primary dir");
        std::fs::create_dir_all(legacy.parent().expect("legacy parent")).expect("legacy dir");
        std::fs::write(&primary, "not json").expect("corrupt primary");
        std::fs::write(&legacy, r#"{"bot_fill":true,"guardian":false}"#).expect("legacy write");

        let migrated = load_play_setup_from(&primary, &legacy);
        assert_eq!(migrated.preset, PlayPreset::Custom);
        assert!(migrated.fill_empty_seats);
        assert!(!migrated.guardian);
        let persisted: PlaySetupDraft = serde_json::from_str(
            &std::fs::read_to_string(&primary).expect("migrated primary exists"),
        )
        .expect("migrated primary parses");
        assert_eq!(persisted, migrated);
        std::fs::remove_dir_all(scratch).expect("scratch cleanup");
    }

    #[test]
    fn active_session_describes_the_finalized_roster_not_the_draft() {
        let wfc = HexWfcConfig::default();
        let solo = ActivePlaySession::from_launch(
            HexMatchConfig {
                teams: 1,
                members_per_team: 1,
                guardian: true,
                wfc,
            },
            false,
            false,
        );
        assert_eq!(solo.kind, ActivePlayKind::Solo);

        let spectator = ActivePlaySession::from_launch(
            HexMatchConfig {
                teams: 2,
                members_per_team: 2,
                guardian: true,
                wfc,
            },
            true,
            true,
        );
        assert_eq!(spectator.kind, ActivePlayKind::Spectate);
        assert!(spectator.networked);
    }
}
