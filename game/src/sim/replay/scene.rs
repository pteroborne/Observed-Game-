//! Owned visual history for canonical matches. No physics worlds or live resources.
use super::ReplayActorId;
mod structure;
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_core::{PlayerId, TeamId};
use observed_hex::HexCoord;
use observed_match::hex_wfc::{
    HexBodyPlace, HexGuardianStatus, HexReleasedGuardian, HexStructurePiece, HexWfcMatch,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
#[cfg(test)]
use structure::retain_pieces;
use structure::retain_structure;

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayStructure {
    piece_indices: HashMap<observed_traversal::StableColliderId, usize>,
    cell_piece_ids: BTreeMap<HexCoord, Vec<observed_traversal::StableColliderId>>,
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
        local: Option<PlayerId>,
        sample: usize,
        previous: Option<&Self>,
    ) -> Self {
        let facility = previous
            .filter(|p| p.facility.generation == game.facility.generation)
            .map(|p| Arc::clone(&p.facility))
            .unwrap_or_else(|| {
                Arc::new(retain_structure(
                    &game.facility,
                    &game.geometry,
                    previous.map(|p| p.facility.as_ref()),
                    previous
                        .filter(|p| {
                            p.facility.generation.wrapping_add(1) == game.facility.generation
                        })
                        .filter(|_| !game.last_geometry_cells.is_empty())
                        .map(|_| &game.last_geometry_cells),
                ))
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
                        Arc::new(retain_structure(&maze.world, &maze.geometry, None, None))
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
                position: game.guardian.feet(),
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
                position: match g {
                    HexReleasedGuardian::Major(major) => major.feet(),
                    HexReleasedGuardian::Minor(minor) => {
                        minor.position
                            - minor.visual_frame().up()
                                * game.content().traversal_profile().controller().half_height
                    }
                },
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
    #[test]
    fn replay_reuses_untouched_pieces_and_keeps_prior_geometry_immutable() {
        use observed_match::hex_wfc::{HexPiecePart, HexStructureRole};
        use observed_traversal::{ColliderShape, StableColliderId};
        let piece = |id, cell| HexStructurePiece {
            id: StableColliderId(id),
            anchor: cell,
            source_cell: cell,
            role: HexStructureRole::Hall,
            part: HexPiecePart::Authored,
            surface: None,
            tile: None,
            center: Vec3::ZERO,
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: ColliderShape::Cuboid { half: Vec3::ONE },
        };
        let a = HexCoord::default();
        let b = HexCoord { q: 1, ..a };
        let original = vec![piece(1, a), piece(2, b)];
        let previous = ReplayStructure {
            seed: 1,
            generation: 0,
            pieces: retain_pieces(&original, None),
            registers: BTreeMap::new(),
            exit: a,
            piece_indices: original
                .iter()
                .enumerate()
                .map(|(index, piece)| (piece.id, index))
                .collect(),
            cell_piece_ids: BTreeMap::from([(a, vec![original[0].id]), (b, vec![original[1].id])]),
        };
        let mut rewritten = original.clone();
        rewritten.swap(0, 1);
        rewritten[1].center = Vec3::X;
        let retained = retain_pieces(&rewritten, Some(&previous));
        assert!(Arc::ptr_eq(&retained[0], &previous.pieces[1]));
        assert!(!Arc::ptr_eq(&retained[1], &previous.pieces[0]));
        assert_eq!(*previous.pieces[0], original[0]);
        assert_eq!(*retained[1], rewritten[1]);
        // A discontinuous history falls back to full equality checks.
        assert_eq!(retained, retain_pieces(&rewritten, Some(&previous)));
    }
}
