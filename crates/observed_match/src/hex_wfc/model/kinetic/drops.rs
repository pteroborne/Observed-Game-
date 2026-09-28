//! Where a push sends a minor: over an edge to its death, onto a deck below, or nowhere.
//!
//! The kinetic tool kills nothing; a fall does (`released`). These answer, for a point a
//! body could stand on and a level push along a direction, what the architecture does with
//! the minor: a geometric probe walks a minor's capsule through the colliders for a push's
//! slide to the first place with no floor under it, and a replay plays the shove on a copy
//! of the match and reports whether it ended in `GuardianLost`. The probe nominates; only
//! a replay decides, because a railing often stands just past a slab's edge and a fall
//! carries forward onto roofs a straight-down ray misses (`kinetic::edges`).
//!
//! `kinetic::edges` measures production facilities with them. [`HexWfcMatch::killing_push`]
//! and [`HexWfcMatch::stage_minor`] stage evidence captures, as `jail` does: play never
//! calls them.

use glam::Vec3;
use observed_hex::{FLOOR_SLAB_TOP, HexCoord, HexFace, PortClass, hex_origin};

use super::super::released::{HexReleasedGuardian, MINOR_BREAKING_DROP, OUT_OF_WORLD_DEPTH};
use super::super::{HEX_INPUT_VERSION, HexInputFrame, HexReleasedKind};
use super::{HexWfcMatch, KINETIC_PUSH_SPEED};

/// How far a level push slides a minor on the flat (`KINETIC_STAGGER_FRICTION`).
const SLIDE: f32 = 5.5;
const STEP: f32 = 0.25;
/// Directions a push is tried in, round the compass.
pub(crate) const DIRECTIONS: u8 = 12;
/// Ids the replays release their minors under, clear of any the rules allocate.
const TRIAL_MINORS: u16 = 60_000;

/// What a level push from a point meets first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PushEnd {
    /// A wall or railing, or it stops on the floor.
    Held,
    /// It goes over onto a lower deck it survives.
    Landing,
    /// It goes over, and nothing catches it before it is out of the facility, or what does
    /// is further down than it survives.
    Lethal,
}

/// A push that kills: a minor standing with its feet at `feet`, in `cell`, pushed level
/// along `direction`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HexKillingPush {
    pub cell: HexCoord,
    pub feet: Vec3,
    pub direction: Vec3,
}

/// Direction `step` of [`DIRECTIONS`].
pub(crate) fn direction(step: u8) -> Vec3 {
    let angle = f32::from(step) * std::f32::consts::TAU / f32::from(DIRECTIONS);
    Vec3::new(angle.cos(), 0.0, angle.sin())
}

/// Whether a stair or ramp links `cell` to another floor, where a push is a climb.
pub(crate) fn linked_vertically(
    world: &observed_facility::hex_wfc::HexWfcWorld,
    cell: HexCoord,
) -> bool {
    let grid = world.config.grid();
    let links = |at: Option<HexCoord>, face: HexFace| {
        at.and_then(|at| world.placements.get(&at))
            .is_some_and(|p| p.ports().port(face) != PortClass::Sealed)
    };
    links(Some(cell), HexFace::Up)
        || links(Some(cell), HexFace::Down)
        || links(grid.neighbor(cell, HexFace::Down), HexFace::Up)
        || links(grid.neighbor(cell, HexFace::Up), HexFace::Down)
}

impl HexWfcMatch {
    /// Points in `cell` a body can stand on: its middle, and two rings round it.
    #[must_use]
    pub fn standing_points(&self, cell: HexCoord) -> Vec<Vec3> {
        let floor = Vec3::from_array(hex_origin(cell)) + Vec3::Y * FLOOR_SLAB_TOP;
        std::iter::once(Vec3::ZERO)
            .chain([3.0f32, 5.5].into_iter().flat_map(|radius| {
                (0..6u8).map(move |step| {
                    let angle = f32::from(step) * std::f32::consts::TAU / 6.0;
                    Vec3::new(angle.cos(), 0.0, angle.sin()) * radius
                })
            }))
            .filter_map(|offset| self.stands_at(floor + offset))
            .collect()
    }

    /// Whether a body fits standing with its feet at `feet`, on solid floor at that height.
    #[must_use]
    pub fn stands_clear(&self, feet: Vec3) -> bool {
        self.stands_at(feet).is_some()
    }

    /// Walk a minor's capsule from `feet` along `direction` for a level push's slide.
    pub(crate) fn push_end(&self, feet: Vec3, direction: Vec3) -> PushEnd {
        let config = self.content.traversal_profile().controller();
        let lift = config.half_height + 0.05;
        let mut travelled = STEP;
        while travelled <= SLIDE {
            let centre = feet + direction * travelled + Vec3::Y * lift;
            if !self
                .physics
                .capsule_is_clear(centre, config.radius, config.half_height)
            {
                return PushEnd::Held;
            }
            if self
                .physics
                .ray_distance(centre, Vec3::NEG_Y, lift + 0.4)
                .is_none()
            {
                // Over the edge only if the whole capsule clears it: a railing standing just
                // beyond a slab's edge stops a body whose rim is still on the floor.
                let clears = (1..=4u8).all(|i| {
                    let past = centre + direction * (config.radius * f32::from(i) * 0.5 + 0.1);
                    self.physics
                        .capsule_is_clear(past, config.radius, config.half_height)
                });
                if !clears {
                    return PushEnd::Held;
                }
                let beyond = centre + direction * (config.radius + 0.1);
                let lost_below = self.geometry.arena.floor_y - OUT_OF_WORLD_DEPTH;
                return match self.physics.ray_distance(beyond, Vec3::NEG_Y, 400.0) {
                    // Caught by a lower deck: a landing, unless the fall breaks it.
                    Some(drop)
                        if beyond.y - drop > lost_below && drop - lift <= MINOR_BREAKING_DROP =>
                    {
                        PushEnd::Landing
                    }
                    _ => PushEnd::Lethal,
                };
            }
            travelled += STEP;
        }
        PushEnd::Held
    }

    /// Put minor `id` at rest with its feet at `feet`, in `cell`, releasing it there if the
    /// match has no minor of that id. Whether there is one there now.
    ///
    /// For evidence captures, as [`Self::jail`] is: play releases minors only by a wave or a
    /// requisition, wherever the rules put them.
    pub fn stage_minor(&mut self, id: u16, cell: HexCoord, feet: Vec3) -> bool {
        if !self.released.contains_key(&id)
            && !self.release_guardian(id, HexReleasedKind::Minor, cell)
        {
            return false;
        }
        let config = self.content.traversal_profile().controller();
        let Some(HexReleasedGuardian::Minor(minor)) = self.released.get_mut(&id) else {
            return false;
        };
        minor.cell = cell;
        minor.stand_at(feet + Vec3::Y * (config.half_height + 0.02));
        true
    }

    /// A push that kills a minor, found as `kinetic::edges` finds them - the highest floor
    /// first, where the falls are - and proven by playing it on a copy of the match. At most
    /// `tries` replays. For evidence captures: see [`Self::stage_minor`].
    #[must_use]
    pub fn killing_push(&self, tries: usize) -> Option<HexKillingPush> {
        let mut cells: Vec<HexCoord> = self
            .facility
            .placements
            .iter()
            .filter(|&(&cell, placement)| {
                placement.space.built()
                    && !linked_vertically(&self.facility, cell)
                    && self
                        .prison
                        .as_ref()
                        .is_none_or(|prison| !prison.lobby.contains(&cell))
            })
            .map(|(&cell, _)| cell)
            .collect();
        cells.sort_by_key(|cell| (std::cmp::Reverse(cell.level), *cell));
        let mut trials = PushTrials::new(self);
        let mut tried = 0;
        for cell in cells {
            for feet in self.standing_points(cell) {
                for step in 0..DIRECTIONS {
                    let direction = direction(step);
                    if self.push_end(feet, direction) != PushEnd::Lethal {
                        continue;
                    }
                    if tried == tries {
                        return None;
                    }
                    tried += 1;
                    if trials.kills(cell, feet, direction) {
                        return Some(HexKillingPush {
                            cell,
                            feet,
                            direction,
                        });
                    }
                    // One replay a point: the next point is likelier than the next bearing.
                    break;
                }
            }
        }
        None
    }
}

/// Replays shoves as the simulation plays them, one minor at a time, on one copy of a
/// match: release a minor, stand it, push it, and see whether the fall takes it.
pub(crate) struct PushTrials {
    game: HexWfcMatch,
    next: u16,
}

impl PushTrials {
    pub(crate) fn new(game: &HexWfcMatch) -> Self {
        Self {
            game: game.clone(),
            next: TRIAL_MINORS,
        }
    }

    /// Whether a minor standing at `feet` in `cell`, pushed level along `direction`, is lost.
    pub(crate) fn kills(&mut self, cell: HexCoord, feet: Vec3, direction: Vec3) -> bool {
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        let game = &mut self.game;
        if !game.stage_minor(id, cell, feet) {
            return false;
        }
        if let Some(HexReleasedGuardian::Minor(minor)) = game.released.get_mut(&id) {
            minor.shove(direction * KINETIC_PUSH_SPEED);
        }
        for _ in 0..150 {
            let frame = HexInputFrame {
                version: HEX_INPUT_VERSION,
                tick: game.tick + 1,
                commands: std::collections::BTreeMap::new(),
            };
            game.step(&frame);
            if !game.released.contains_key(&id) {
                return true;
            }
        }
        game.remove_released(id);
        false
    }
}
