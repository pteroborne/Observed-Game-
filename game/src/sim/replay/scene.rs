//! Owned visual history for canonical matches. No physics worlds or live resources.
use super::ReplayActorId;
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_core::{PlayerId, TeamId};
use observed_hex::HexCoord;
use observed_match::hex_wfc::{
    HexBodyPlace, HexGuardianStatus, HexReleasedGuardian, HexStructurePiece, HexWfcMatch,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayStructure {
    pub seed: u64,
    pub generation: u32,
    pub pieces: Vec<Arc<HexStructurePiece>>,
    pub registers: BTreeMap<HexCoord, ArchitectureRegister>,
    pub exit: HexCoord,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayBody {
    pub player: PlayerId,
    pub actor: ReplayActorId,
    pub team: TeamId,
    pub cell: HexCoord,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub place: HexBodyPlace,
    pub escaped: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayGuardian {
    /// None is the original major; released Guardians retain the rules' IDs.
    pub id: Option<u16>,
    pub position: Vec3,
    pub cell: HexCoord,
    pub minor: bool,
    pub status: HexGuardianStatus,
    pub target: Option<PlayerId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplaySceneFrame {
    pub sample: usize,
    pub tick: u64,
    pub facility: Arc<ReplayStructure>,
    pub prisons: BTreeMap<TeamId, Arc<ReplayStructure>>,
    pub bodies: Vec<ReplayBody>,
    pub guardians: Vec<ReplayGuardian>,
    pub eye_height: f32,
    pub highlights: Vec<HexCoord>,
    pub doors: Vec<(
        observed_match::ascent::sim::ThresholdKey,
        observed_match::ascent::sim::DoorState,
    )>,
    pub stations: Vec<HexCoord>,
    pub dark_floors: Vec<u8>,
}

impl ReplaySceneFrame {
    pub(super) fn capture(
        game: &HexWfcMatch,
        local: PlayerId,
        sample: usize,
        previous: Option<&Self>,
    ) -> Self {
        let facility = previous
            .filter(|p| p.facility.generation == game.facility.generation)
            .map(|p| Arc::clone(&p.facility))
            .unwrap_or_else(|| {
                Arc::new(ReplayStructure {
                    seed: game.facility.seed,
                    generation: game.facility.generation,
                    pieces: retain_pieces(
                        &game.geometry.pieces,
                        previous.map(|p| p.facility.as_ref()),
                    ),
                    registers: game.facility.architecture.clone(),
                    exit: game.facility.config.exit(),
                })
            });
        let prisons = game
            .prison
            .iter()
            .flat_map(|p| &p.mazes)
            .map(|(&team, maze)| {
                let retained = previous
                    .and_then(|p| p.prisons.get(&team))
                    .filter(|p| p.seed == maze.world.seed);
                (
                    team,
                    retained.cloned().unwrap_or_else(|| {
                        Arc::new(ReplayStructure {
                            seed: maze.world.seed,
                            generation: maze.world.generation,
                            pieces: retain_pieces(&maze.geometry.pieces, None),
                            registers: maze.world.architecture.clone(),
                            exit: maze.exit(),
                        })
                    }),
                )
            })
            .collect();
        let bodies = game
            .players
            .values()
            .map(|p| ReplayBody {
                player: p.id,
                actor: super::hex_actor_id(game, local, p.id),
                team: p.team,
                cell: p.cell,
                position: p.position,
                yaw: p.yaw,
                pitch: p.pitch,
                place: p.place,
                escaped: p.escaped,
            })
            .collect();
        let mut guardians = Vec::new();
        if game.guardian_hunts() {
            guardians.push(ReplayGuardian {
                id: None,
                position: game.guardian.position,
                cell: game.guardian.cell,
                minor: false,
                status: game.guardian.status,
                target: game.guardian.target,
            });
        }
        guardians.extend(game.released.iter().map(|(&id, g)| {
            let (minor, status, target) = match g {
                HexReleasedGuardian::Major(g) => (false, g.status, g.target),
                HexReleasedGuardian::Minor(g) => (true, HexGuardianStatus::Active, g.target),
            };
            ReplayGuardian {
                id: Some(id),
                position: g.position(),
                cell: g.cell(),
                minor,
                status,
                target,
            }
        }));
        Self {
            sample,
            tick: game.tick,
            facility,
            prisons,
            bodies,
            guardians,
            eye_height: game.eye_height(),
            highlights: Vec::new(),
            doors: Vec::new(),
            stations: Vec::new(),
            dark_floors: Vec::new(),
        }
    }
}

/// Interpolate only within one physical space. A catch, rescue or teleport snaps.
pub fn body_position(a: &ReplayBody, b: Option<&ReplayBody>, fraction: f32) -> Vec3 {
    match b {
        Some(b) if a.place == b.place && a.position.distance_squared(b.position) < 225.0 => {
            a.position.lerp(b.position, fraction)
        }
        _ => a.position,
    }
}

fn retain_pieces(
    pieces: &[HexStructurePiece],
    previous: Option<&ReplayStructure>,
) -> Vec<Arc<HexStructurePiece>> {
    let old: BTreeMap<_, _> = previous
        .into_iter()
        .flat_map(|p| &p.pieces)
        .map(|p| (p.id, p))
        .collect();
    pieces
        .iter()
        .map(|piece| {
            old.get(&piece.id)
                .filter(|old| old.as_ref() == piece)
                .map_or_else(|| Arc::new(piece.clone()), |old| Arc::clone(old))
        })
        .collect()
}

pub(super) fn event_label(kind: observed_match::hex_wfc::HexMatchEventKind) -> &'static str {
    use observed_match::hex_wfc::HexMatchEventKind::*;
    match kind {
        PlayerEscaped => "Observer reached exit",
        PlayerRecovered => "Observer recovered",
        MutationWarning => "Facility rewrite warning",
        MutationCommitted => "Facility rewritten",
        MutationNoChange => "Facility unchanged",
        MutationCancelled => "Rewrite cancelled",
        LanternCacheCollected => "Lantern collected",
        KeystoneCollected => "Keystone collected",
        DualStationProgress => "Team station charging",
        DualStationCompleted => "Team station complete",
        MonitorSurveyed => "Monitor surveyed",
        ExitDenied => "Exit locked",
        AnchorDeployed => "Anchor deployed",
        AnchorRecovered => "Anchor recovered",
        PadDeployed => "Teleport pad deployed",
        PadTraversed => "Teleport pad used",
        GuardianCatch => "Guardian caught Observer",
        GuardianReleased => "Guardian released",
        GuardianLost => "Guardian destroyed by fall",
        PlayerJailed => "Observer jailed",
        PlayerReleased => "Observer escaped prison",
        Jailbreak => "Teammate rescued from prison",
        PlayerLost => "Observer fell into void",
        KineticPush => "Minor Guardian pushed",
        KineticPull => "Minor Guardian pulled",
        KineticPlumb => "Minor Guardian gravity changed",
        MatchFinished => "Match finished",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpolation_never_crosses_prison_void_or_teleport_boundaries() {
        let a = ReplayBody {
            player: PlayerId(0),
            actor: ReplayActorId::LocalPlayer,
            team: TeamId(0),
            cell: HexCoord {
                q: 0,
                r: 0,
                level: 0,
            },
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            place: HexBodyPlace::Facility,
            escaped: false,
        };
        let mut b = a.clone();
        b.position = Vec3::X;
        assert_eq!(body_position(&a, Some(&b), 0.5), Vec3::X * 0.5);
        b.place = HexBodyPlace::Prison;
        assert_eq!(body_position(&a, Some(&b), 0.5), a.position);
        b.place = HexBodyPlace::Void;
        assert_eq!(body_position(&a, Some(&b), 0.5), a.position);
        b.place = a.place;
        b.position = Vec3::X * 20.0;
        assert_eq!(body_position(&a, Some(&b), 0.5), a.position);
    }
}
