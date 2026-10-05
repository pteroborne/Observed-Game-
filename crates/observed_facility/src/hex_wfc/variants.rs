//! The production variant alphabet: grounded lateral rooms and halls, and climb
//! compositions - a storey climbed across three cells, bonded by `Span` faces and,
//! under its landing, by `RampOpen` (`docs/climb_compositions_plan.md`). `ShaftOpen`
//! survives only inside the two-storey atrium room.

use std::collections::BTreeSet;

use observed_hex::{HexFace, PortClass, PortSignature};

use crate::map_spec::RoomRole;

use super::blueprint::{blueprint_cell_archetype, blueprint_for_role};
use super::{ClimbPart, ClimbTurn, HexArchetype, HexPlacement, HexSpace, lateral_bit};

/// One exact authored-tile requirement emitted by hex geometry projection.
///
/// Architecture register is deliberately not part of this value: the
/// authoring coverage gate requires every one of these semantic pairs in every
/// production register.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct HexGeometryDemand {
    pub archetype: &'static str,
    pub signature: PortSignature,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct HexVariant {
    pub space: HexSpace,
    pub archetype: HexArchetype,
    pub doors: u8,
    pub up: PortClass,
    pub down: PortClass,
    pub weight: u32,
}

impl HexVariant {
    /// The port signature this variant presents to its neighbors.
    pub(super) fn signature(self) -> PortSignature {
        let mut ports = [PortClass::Sealed; 8];
        let spans = self.archetype.span_mask();
        for face in HexFace::LATERAL {
            if self.doors & lateral_bit(face) != 0 {
                ports[face.index()] = if spans & lateral_bit(face) != 0 {
                    PortClass::Span
                } else {
                    PortClass::Door
                };
            }
        }
        ports[HexFace::Up.index()] = self.up;
        ports[HexFace::Down.index()] = self.down;
        PortSignature::try_from_ports(ports).expect("variant ports are valid by construction")
    }
}

/// The flat-hall weights, all scaled together.
///
/// Doubled from 6/2/20/5 when the stair towers gained their branching landing,
/// to hold the flat-against-vertical balance against a shaft family that had
/// grown to 169 entries; every flat family moved by the same factor, so their
/// proportions to each other were untouched. The towers have retired since, and
/// the climb compositions that replace them are weighted on their own terms
/// ([`CLIMB_WEIGHT`]), so these stay where they are rather than reopening a
/// balance nothing now leans on.
///
/// Every weight in the alphabet was multiplied by four together when the climbs gained
/// turned shapes, so that a turned shape could weigh a sixteenth of a straight one
/// without the climbs as a family weighing any more ([`TURNED_CLIMB_WEIGHT`]). A
/// uniform scale leaves every draw's odds where they were.
///
/// Nothing here changes how empty the facility is. `SpaceMix` draws the space
/// before the variant and normalises within each one, so scaling every Hall
/// weight together leaves the Void share exactly where `space_mix` puts it.
const FLAT_DEGREE_2: u32 = 48;
/// Three and four lateral doors, flat. A third of a straight, as before.
const FLAT_JUNCTION: u32 = 16;
/// Open expanse cells.
const FLAT_EXPANSE: u32 = 40;
/// Every cell of a climb composition, per heading.
///
/// Measured, not reasoned. With seven door masks at either end, at weights
/// comparable to the ramp family it replaced, climbs took a quarter of every
/// production storey (65 to 128 cells) and broke its expanses into fragments: 152
/// of 200 solve attempts failed the open-volume rule, and the slowest solve took
/// 48 s against a 4 s ceiling. A composition is a third of a storey's plan length,
/// so one is worth many flat cells, and the family weight has to say so. See the
/// measurement at `the_production_facility_holds_its_measured_baseline`.
///
/// 4 on the old scale, 16 on the new; then 10, because turned shapes fit where a
/// straight climb cannot. Across twelve unprofiled `arc_default` solves the straight
/// climbs built 3,328 climb cells; with turned shapes at the same family weight, 4,488,
/// and at 10, 3,312. Turned shapes also build 8% more cells in all (29,596 to 32,162),
/// whatever the climb weight: they close door patterns that used to fall to void.
const CLIMB_WEIGHT: u32 = 10;
/// A mid cell that turns, or a landing left by any face but the one ahead.
///
/// Each part's shapes share [`CLIMB_WEIGHT`] between them, the straight one keeping
/// what the turned ones do not take. At a quarter of the straight weight each and on
/// top of it, the turned shapes doubled the climbs a production facility builds (32
/// climb cells to 68 on seed 1) and the solve's attempts went from 3 to 10: a mid cell
/// or a landing drew as a climb twice as often, and fitted in more places.
const TURNED_CLIMB_WEIGHT: u32 = 1;

pub(super) fn catalogue() -> Vec<HexVariant> {
    let mut variants = vec![HexVariant {
        space: HexSpace::Void,
        archetype: HexArchetype::Void,
        doors: 0,
        up: PortClass::Sealed,
        down: PortClass::Sealed,
        weight: 16,
    }];

    // 1. Room variants: any lateral opening mask, optionally a vertical
    //    opening for the fixed two-level Guardian Control atrium. Stamped
    //    blueprint cells use matched room↔room openings across their internal
    //    seams and named doors where the footprint meets a hall.
    for &up in &[PortClass::Sealed, PortClass::ShaftOpen] {
        for &down in &[PortClass::Sealed, PortClass::ShaftOpen] {
            for mask in 0u8..64 {
                if mask == 0 && up == PortClass::Sealed && down == PortClass::Sealed {
                    continue;
                }
                let degree = mask.count_ones();
                let room_weight = match degree {
                    0 | 1 => 16,
                    2 => 12,
                    3 => 8,
                    _ => 4,
                };
                variants.push(HexVariant {
                    space: HexSpace::Room,
                    archetype: HexArchetype::Room,
                    doors: mask,
                    up,
                    down,
                    weight: room_weight,
                });
            }
        }
    }

    // 2. Flat hall and junction variants (lateral only).
    for mask in 1u8..64 {
        let degree = mask.count_ones();
        if degree == 2 {
            variants.push(HexVariant {
                space: HexSpace::Hall,
                archetype: hall_archetype(mask),
                doors: mask,
                up: PortClass::Sealed,
                down: PortClass::Sealed,
                weight: FLAT_DEGREE_2,
            });
        } else if (3..=4).contains(&degree) {
            variants.push(HexVariant {
                space: HexSpace::Hall,
                archetype: HexArchetype::Junction,
                doors: mask,
                up: PortClass::Sealed,
                down: PortClass::Sealed,
                weight: FLAT_JUNCTION,
            });
        }
    }

    // 3. Climb compositions (`docs/climb_compositions_plan.md`): a flight across
    //     three cells of one storey and a landing above the last, assembled by
    //     `Span` faces in order and by `RampOpen` above the high cell. The foot is
    //     entered straight on: a door on a side face would need flat floor in front
    //     of it, and at the foot of a flight that is most of a cell. The mid cell may
    //     turn the flight, and the landing may be left ahead, to either side or back
    //     over the flight.
    for &heading in &HexFace::LATERAL {
        let climb = |part, doors, up, down, weight| HexVariant {
            space: HexSpace::Hall,
            archetype: HexArchetype::Climb { part, heading },
            doors,
            up,
            down,
            weight,
        };
        #[allow(clippy::cast_possible_truncation)]
        let weight = |turn, shapes: usize| {
            if turn == ClimbTurn::Ahead {
                CLIMB_WEIGHT - (shapes as u32 - 1) * TURNED_CLIMB_WEIGHT
            } else {
                TURNED_CLIMB_WEIGHT
            }
        };
        let (on, back) = (lateral_bit(heading), lateral_bit(heading.opposite()));
        let sealed = PortClass::Sealed;
        variants.push(climb(
            ClimbPart::Foot,
            back | on,
            sealed,
            sealed,
            CLIMB_WEIGHT,
        ));
        for turn in ClimbTurn::BENDS {
            variants.push(climb(
                ClimbPart::Mid { turn },
                back | lateral_bit(turn.apply(heading)),
                sealed,
                sealed,
                weight(turn, ClimbTurn::BENDS.len()),
            ));
        }
        variants.push(climb(
            ClimbPart::High,
            back,
            PortClass::RampOpen,
            sealed,
            CLIMB_WEIGHT,
        ));
        for exit in ClimbTurn::EXITS {
            variants.push(climb(
                ClimbPart::Landing { exit },
                lateral_bit(exit.apply(heading)),
                sealed,
                PortClass::RampOpen,
                weight(exit, ClimbTurn::EXITS.len()),
            ));
        }
    }

    // 4. Open expanse cells. Only masks of four or more doors qualify: the
    //    archetype exists so neighbouring cells leave their shared faces open
    //    and merge into one volume, and a cell that seals half its perimeter
    //    cannot do that. Weight is modest at the alphabet level — the district
    //    profiles decide where expanses actually belong.
    for mask in 1u8..64 {
        if mask.count_ones() < 4 {
            continue;
        }
        variants.push(HexVariant {
            space: HexSpace::Hall,
            archetype: HexArchetype::Expanse,
            doors: mask,
            up: PortClass::Sealed,
            down: PortClass::Sealed,
            weight: FLAT_EXPANSE,
        });
    }

    variants
}

/// Every distinct port signature the solver can demand of a tile — the feed
/// for Phase 91's manifest coverage validator. Deterministically ordered
/// (`PortSignature` is `Ord`).
#[must_use]
pub fn demandable_signatures() -> Vec<PortSignature> {
    catalogue()
        .into_iter()
        .filter(|variant| variant.space.built())
        .map(HexVariant::signature)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Manifest archetype for a non-room placement, or `None` when the cell emits
/// no prefab. Rooms are projected once per stamped blueprint through
/// [`blueprint_cell_archetype`].
#[must_use]
pub fn placement_tile_archetype(placement: &HexPlacement) -> Option<&'static str> {
    match placement.archetype {
        HexArchetype::Void | HexArchetype::Room => None,
        HexArchetype::Straight => Some("hall_straight"),
        HexArchetype::Corner => Some(corner_tile_archetype(placement.doors)),
        HexArchetype::Junction if placement.doors.count_ones() == 3 => Some("hall_junction_3way"),
        HexArchetype::Junction => Some("hall_junction_4way"),
        HexArchetype::Expanse => Some("expanse"),
        HexArchetype::Cistern { .. } => Some("cistern"),
        HexArchetype::ArchiveWell { .. } => Some("archive_well"),
        HexArchetype::RainCourt { .. } => Some("rain_court"),
        HexArchetype::JadeNave { .. } => Some("jade_nave"),
        HexArchetype::LastPromenade { .. } => Some("last_promenade"),
        HexArchetype::SwitchingConcourse { .. } => Some("switching_concourse"),
        HexArchetype::Chargeworks { part, .. } => Some(match part {
            super::ChargeworksPart::Fabricator => "chargeworks_fabricator",
            super::ChargeworksPart::Transfer => "chargeworks_transfer",
            super::ChargeworksPart::Receiver => "chargeworks_receiver",
        }),
        HexArchetype::Climb { part, .. } => Some(climb_tile_archetype(part)),
    }
}

/// The authored tile family for each part of each shape. The heading picks the
/// tile's turn, so a shape and its mirror are separate families.
#[must_use]
pub const fn climb_tile_archetype(part: ClimbPart) -> &'static str {
    match part {
        ClimbPart::Foot => "climb_foot",
        ClimbPart::Mid { turn } => match turn {
            ClimbTurn::Ahead | ClimbTurn::Back => "climb_mid",
            ClimbTurn::Left => "climb_mid_left",
            ClimbTurn::SharpLeft => "climb_mid_sharp_left",
            ClimbTurn::SharpRight => "climb_mid_sharp_right",
            ClimbTurn::Right => "climb_mid_right",
        },
        ClimbPart::High => "climb_high",
        ClimbPart::Landing { exit } => match exit {
            ClimbTurn::Ahead | ClimbTurn::SharpLeft | ClimbTurn::SharpRight => "climb_landing",
            ClimbTurn::Left => "climb_landing_left",
            ClimbTurn::Right => "climb_landing_right",
            ClimbTurn::Back => "climb_landing_back",
        },
    }
}

/// Whether `a`, meeting `b` across its own `face`, joins a climb correctly: a span
/// meets only a span, and only the next cell of the same composition.
#[must_use]
pub fn spans_join(a: HexArchetype, face: HexFace, b: HexArchetype) -> bool {
    let a_span = a.span_mask() & lateral_bit(face) != 0;
    let b_span = b.span_mask() & lateral_bit(face.opposite()) != 0;
    if a_span != b_span {
        return false;
    }
    if !a_span {
        return true;
    }
    let (Some((_, a_out)), Some((b_in, _))) = (a.flight_faces(), b.flight_faces()) else {
        return false;
    };
    let (HexArchetype::Climb { part: a_part, .. }, HexArchetype::Climb { part: b_part, .. }) =
        (a, b)
    else {
        return false;
    };
    // Up the flight from `a` into `b`, or back down it from `b` into `a`.
    let up = a_out == Some(face) && b_in == Some(face.opposite());
    if up {
        return matches!(
            (a_part, b_part),
            (ClimbPart::Foot, ClimbPart::Mid { .. }) | (ClimbPart::Mid { .. }, ClimbPart::High)
        );
    }
    let (Some((a_in, _)), Some((_, b_out))) = (a.flight_faces(), b.flight_faces()) else {
        return false;
    };
    a_in == Some(face)
        && b_out == Some(face.opposite())
        && matches!(
            (b_part, a_part),
            (ClimbPart::Foot, ClimbPart::Mid { .. }) | (ClimbPart::Mid { .. }, ClimbPart::High)
        )
}

fn corner_tile_archetype(doors: u8) -> &'static str {
    let faces = HexFace::LATERAL
        .into_iter()
        .filter(|&face| doors & lateral_bit(face) != 0)
        .map(HexFace::index)
        .collect::<Vec<_>>();
    debug_assert_eq!(faces.len(), 2);
    let distance = faces[0].abs_diff(faces[1]);
    if distance.min(6 - distance) == 1 {
        "hall_turn_60"
    } else {
        "hall_turn_120"
    }
}

/// Every exact `(archetype, signature)` pair geometry projection can request.
///
/// This is intentionally narrower than [`demandable_signatures`], which is the
/// WFC propagation alphabet and includes generic Room variants that can never
/// leave a blueprint domain. The manifest
/// coverage gate and projector both consume this function so the contract
/// cannot drift back to a hand-written subset.
#[must_use]
pub fn geometry_demands() -> Vec<HexGeometryDemand> {
    let mut demands = BTreeSet::new();
    for variant in catalogue() {
        let placement = HexPlacement {
            coord: observed_hex::HexCoord::default(),
            space: variant.space,
            archetype: variant.archetype,
            doors: variant.doors,
            up: variant.up,
            down: variant.down,
        };
        if let Some(archetype) = placement_tile_archetype(&placement) {
            demands.insert(HexGeometryDemand {
                archetype,
                signature: variant.signature(),
            });
        }
    }

    const ROOM_ROLES: [RoomRole; 11] = [
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
    for role in ROOM_ROLES {
        let blueprint = blueprint_for_role(role);
        for (cell_index, &offset) in blueprint.cells.iter().enumerate() {
            let archetype = blueprint_cell_archetype(role, cell_index)
                .expect("every blueprint cell has an authored tile archetype");
            demands.insert(HexGeometryDemand {
                archetype,
                signature: blueprint.cell_signature(offset),
            });
        }
    }
    demands.into_iter().collect()
}

pub(super) fn hall_archetype(mask: u8) -> HexArchetype {
    if mask.count_ones() >= 3 {
        return HexArchetype::Junction;
    }
    let straight = HexFace::LATERAL
        .into_iter()
        .any(|face| mask == lateral_bit(face) | lateral_bit(face.opposite()));
    if straight {
        HexArchetype::Straight
    } else {
        HexArchetype::Corner
    }
}

pub(super) fn variants_compatible(a: HexVariant, b: HexVariant, face: HexFace) -> bool {
    if face.is_lateral() {
        let a_open = a.doors & lateral_bit(face) != 0;
        let b_open = b.doors & lateral_bit(face.opposite()) != 0;
        if a_open != b_open {
            return false;
        }
        if a_open && (a.space.unbuilt() || b.space.unbuilt()) {
            return false;
        }
        !a_open || spans_join(a.archetype, face, b.archetype)
    } else {
        let a_port = if face == HexFace::Up { a.up } else { a.down };
        let b_port = if face == HexFace::Up { b.down } else { b.up };
        if a_port as u8 != b_port as u8 {
            return false;
        }
        // Ramp halves only ever meet their partner across the shared RampOpen
        // face, in the correct vertical orientation and lateral direction.
        // The only vertical `RampOpen` bond is a climb's high cell under its landing.
        if a_port == PortClass::RampOpen {
            let (lower, upper) = if face == HexFace::Up { (a, b) } else { (b, a) };
            return climb_bond(lower.archetype, upper.archetype);
        }
        true
    }
}

/// Whether `lower` and `upper` are a climb's high cell and the landing above it.
#[must_use]
pub fn climb_bond(lower: HexArchetype, upper: HexArchetype) -> bool {
    matches!(
        (lower, upper),
        (
            HexArchetype::Climb { part: ClimbPart::High, heading: low },
            HexArchetype::Climb { part: ClimbPart::Landing { .. }, heading: high },
        ) if low == high
    )
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn geometry_demands_are_exact_and_exclude_non_emitters() {
        let demands = geometry_demands();
        let unique: BTreeSet<_> = demands.iter().copied().collect();
        assert_eq!(unique.len(), demands.len());
        assert!(
            !demands
                .iter()
                .any(|demand| demand.archetype.contains("shaft"))
        );
        // The climb compositions replaced the ramps and the stair towers: none of
        // either is ever demanded, and each part of a climb once per heading - but
        // the mid, which spans fore and aft, reads the same to its ports turned half
        // round, so its six headings are three signatures (and the projector picks
        // its tile by heading, `geometry::required_turn`).
        assert!(!demands.iter().any(|demand| demand.archetype == "hall_ramp"));
        assert!(
            !demands
                .iter()
                .any(|demand| demand.archetype == "stair_tower")
        );
        for (part, signatures) in [
            ("climb_foot", 6),
            ("climb_mid", 3),
            ("climb_high", 6),
            ("climb_landing", 6),
        ] {
            assert_eq!(
                demands
                    .iter()
                    .filter(|demand| demand.archetype == part)
                    .count(),
                signatures,
                "{part}"
            );
        }
        assert!(!demands.iter().any(|demand| demand.archetype == "void"));
        assert!(!demands.iter().any(|demand| demand.archetype == "room"));
    }
}
