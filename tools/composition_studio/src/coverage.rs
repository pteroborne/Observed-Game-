//! Does the authored catalog cover what the solver can ask for?
//!
//! A match dies at load with `MissingTile` when the solver places a cell the
//! catalog has no geometry for. That failure is cheap to prevent and expensive
//! to discover, so this module answers it while an author is still tuning.
//!
//! # Why this calls the projector's selector instead of restating its rule
//!
//! This module used to reproduce the projector's selection rule by hand,
//! including its `stair_tower` special case, so that coverage and projection
//! would agree. Restating a rule is a poor way to agree with it: the copy has to
//! be found and changed every time the original moves, and the failure mode is a
//! coverage panel that confidently contradicts the thing it predicts.
//!
//! It now asks [`observed_match::hex_wfc::HexTileCatalogue`] — the production
//! selector itself — both halves of the question: which register a cell's
//! assembly resolves against, and whether a demand is answered. There is no
//! second rule left to drift. [`CoverageReport::build`] still records what the
//! *real* projector returned, and a test still asserts the two never disagree.

use std::collections::BTreeMap;

use observed_authoring::{RoomPrototype, TilePrototype};
use observed_facility::hex_wfc::{
    HexCoord, HexWfcWorld, PortSignature, blueprint_for_role, placement_tile_archetype,
};
use observed_match::hex_wfc::{
    HexGeometryError, HexTileCatalogue, HexTileSupply, HexWfcGeometrySnapshot,
};

/// How a demanded `(archetype, register, signature)` is satisfied.
///
/// The projector's own verdict, not a local restatement of it: `Exact` means
/// geometry authored for this register answers the demand, `GenericFallback`
/// means only the shared kit does, and `Missing` means a match placing this cell
/// fails to load.
pub type Supply = HexTileSupply;

/// One distinct thing the live layout asks the catalog for.
#[derive(Clone, Debug)]
pub struct DemandRow {
    pub archetype: &'static str,
    pub register: String,
    pub signature: PortSignature,
    pub supply: Supply,
    /// Cells in the current layout that want this.
    pub cells: u32,
    /// One of them, so the author can go look.
    pub example: HexCoord,
}

/// Whether the real projector accepted this layout. The authoritative answer;
/// everything else here is prediction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectorVerdict {
    Projected,
    Failed(String),
    /// The catalog itself could not be loaded, so there was nothing to project.
    CorpusUnavailable,
}

/// Per-role room-module variety.
///
/// Every committed room module is `archetype: "sanctuary"`, `variant: 0`, one
/// per role, so every Decision room in every district is the same room. That is
/// `docs/bug_backlog.md` #25 in one line, and it is a first-class row here
/// rather than a footnote.
#[derive(Clone, Debug)]
pub struct RoomVarietyRow {
    pub role: &'static str,
    /// Runtime prototypes for this role. Not a variety measure on its own:
    /// `runtime_catalog` expands one authored module with `register_scope:
    /// ["all"]` into one prototype per register, so a role backed by a single
    /// `.map` still reports ten.
    pub prototypes: usize,
    /// Distinct authored modules (by source id) behind those prototypes. *This*
    /// is the variety number — how many different rooms an author drew.
    pub modules: usize,
    pub registers: usize,
}

impl RoomVarietyRow {
    /// One authored module means the same room every time, everywhere.
    #[must_use]
    pub fn is_thin(&self) -> bool {
        self.modules <= 1
    }
}

/// What the catalog holds that the solver never asks for.
#[derive(Clone, Debug, Default)]
pub struct NeverPlaced {
    /// Weight zero: present in the catalog and unselectable by construction.
    pub zero_weight: Vec<String>,
    /// Present, selectable, and not chosen anywhere in the current layout.
    pub unused_here: usize,
    pub corpus_cells: usize,
}

#[derive(Clone, Debug, Default)]
pub struct CoverageReport {
    pub demanded: Vec<DemandRow>,
    pub unmet: Vec<DemandRow>,
    pub room_variety: Vec<RoomVarietyRow>,
    pub never_placed: NeverPlaced,
    pub verdict: Option<ProjectorVerdict>,
}

impl CoverageReport {
    /// Build the report for a solved layout against a corpus.
    #[must_use]
    pub fn build(
        world: &HexWfcWorld,
        cells: &[TilePrototype],
        rooms: &[RoomPrototype],
        projected: Option<&HexWfcGeometrySnapshot>,
        projection_error: Option<&HexGeometryError>,
    ) -> Self {
        let catalogue = HexTileCatalogue::new(cells);
        let mut grouped: BTreeMap<(&'static str, String, PortSignature), (Supply, u32, HexCoord)> =
            BTreeMap::new();

        for (coord, placement) in &world.placements {
            // `None` means the cell is not per-cell geometry: Void, or a Room
            // that a stamped blueprint supplies instead.
            let Some(archetype) = placement_tile_archetype(placement) else {
                continue;
            };
            // The register comes from the cell's *assembly*, which for a
            // vertical column is its base cell — the selector answers that, so
            // this cannot drift from what the projector will do.
            let Some(register) = catalogue.assembly_register(world, *coord, archetype) else {
                continue;
            };
            let signature = placement_signature(placement);
            let supply = catalogue.supply(archetype, &register, signature);
            let entry = grouped
                .entry((archetype, register, signature))
                .or_insert((supply, 0, *coord));
            entry.1 += 1;
        }

        let demanded: Vec<DemandRow> = grouped
            .into_iter()
            .map(
                |((archetype, register, signature), (supply, cells, example))| DemandRow {
                    archetype,
                    register,
                    signature,
                    supply,
                    cells,
                    example,
                },
            )
            .collect();

        let unmet = demanded
            .iter()
            .filter(|row| row.supply == Supply::Missing)
            .cloned()
            .collect();

        let verdict = match (projected, projection_error) {
            (Some(_), _) => Some(ProjectorVerdict::Projected),
            (None, Some(error)) => Some(ProjectorVerdict::Failed(format!("{error:?}"))),
            (None, None) => None,
        };

        Self {
            demanded,
            unmet,
            room_variety: room_variety(rooms),
            never_placed: never_placed(cells, projected),
            verdict,
        }
    }

    /// Rows an author should act on, worst first.
    #[must_use]
    pub fn headline(&self) -> String {
        if !self.unmet.is_empty() {
            return format!("{} demand(s) UNCOVERED", self.unmet.len());
        }
        let thin = self.room_variety.iter().filter(|row| row.is_thin()).count();
        let generic = self
            .demanded
            .iter()
            .filter(|row| row.supply == Supply::GenericFallback)
            .count();
        format!("covered; {generic} on generic fallback, {thin} thin room role(s)")
    }
}

/// The port signature a placement presents, in the projector's own terms.
fn placement_signature(placement: &observed_facility::hex_wfc::HexPlacement) -> PortSignature {
    use observed_facility::hex_wfc::{HexFace, PortClass};
    let mut ports = [PortClass::Sealed; 8];
    for face in HexFace::LATERAL {
        if placement.is_open(face) {
            ports[face.index()] = PortClass::Door;
        }
    }
    ports[HexFace::Up.index()] = placement.up;
    ports[HexFace::Down.index()] = placement.down;
    PortSignature::try_from_ports(ports).unwrap_or(PortSignature(0))
}

fn room_variety(rooms: &[RoomPrototype]) -> Vec<RoomVarietyRow> {
    use observed_facility::map_spec::RoomRole;

    const ROLES: [RoomRole; 11] = [
        RoomRole::Start,
        RoomRole::Exit,
        RoomRole::Decision,
        RoomRole::DecoherenceFork,
        RoomRole::AnchorCheckpoint,
        RoomRole::TeleportRelay,
        RoomRole::Keystone,
        RoomRole::DualStation,
        RoomRole::GuardianControl,
        RoomRole::Monitor,
        RoomRole::Recovery,
    ];

    ROLES
        .into_iter()
        .map(|role| {
            let name = blueprint_for_role(role).name;
            let matching: Vec<&RoomPrototype> = rooms
                .iter()
                .filter(|room| room.room_role.eq_ignore_ascii_case(name))
                .collect();
            let mut registers: Vec<&str> = matching
                .iter()
                .map(|room| room.key.register.as_str())
                .collect();
            registers.sort_unstable();
            registers.dedup();
            // The source id survives register expansion, so distinct ids count
            // distinct authored rooms rather than distinct copies of one.
            let mut modules: Vec<&str> = matching.iter().map(|room| room.id.as_str()).collect();
            modules.sort_unstable();
            modules.dedup();
            RoomVarietyRow {
                role: name,
                prototypes: matching.len(),
                modules: modules.len(),
                registers: registers.len(),
            }
        })
        .collect()
}

fn never_placed(
    cells: &[TilePrototype],
    projected: Option<&HexWfcGeometrySnapshot>,
) -> NeverPlaced {
    let zero_weight: Vec<String> = cells
        .iter()
        .filter(|prototype| prototype.weight == 0)
        .map(|prototype| {
            format!(
                "{}/{}/{}",
                prototype.key.archetype, prototype.key.register, prototype.key.variant
            )
        })
        .collect();

    let used: usize = projected.map_or(0, |snapshot| {
        let mut keys: Vec<&observed_authoring::TileKey> = snapshot
            .pieces
            .iter()
            .filter_map(|piece| piece.tile.as_ref())
            .collect();
        keys.sort_by(|a, b| {
            (&a.archetype, &a.register, a.variant).cmp(&(&b.archetype, &b.register, b.variant))
        });
        keys.dedup();
        keys.len()
    });

    NeverPlaced {
        zero_weight,
        unused_here: cells.len().saturating_sub(used),
        corpus_cells: cells.len(),
    }
}
