//! Reading a facility isometrically: where the camera stands, which floor is
//! under inspection, and what has to be cut away to see inside.
//!
//! This is visual language, which is why it lives here rather than in whichever
//! tool happened to need it first. Two surfaces read a facility this way - the
//! composition studio and the game's spectator overview - and if they each kept
//! their own detents, their own cutaway rule and their own idea of what a
//! "floor" is, they would teach the reader two different facilities. The
//! tileforge Python/Rust split is the cautionary version of that, and it cost an
//! arc.
//!
//! Pure maths over `glam`, no ECS and no renderer, so the crate stays what its
//! manifest says it is. Bevy's `Vec3`/`Quat` *are* glam's, so a caller builds a
//! `Transform` from these directly.

use glam::{EulerRot, Quat, Vec2, Vec3};

/// The isometric pitch: about 35 degrees down.
///
/// Shallow enough to read plan relationships, steep enough that a wall has
/// visible height. Every capture in `docs/evidence/` is framed at this angle.
pub const ISO_PITCH: f32 = -0.615_479_7;

/// The six view bearings, in 60-degree steps anchored at 45 degrees.
///
/// Detents matter because the cutaway drops the walls facing the camera: orbit
/// freely and walls pop in and out at arbitrary angles. Six steps match the six
/// hex faces, so each detent has one unambiguous set of three near walls.
///
/// Anchored at `FRAC_PI_4` rather than 0 or 30 degrees deliberately - detent 0
/// is exactly the camera every earlier capture was framed with, so having
/// detents at all does not silently invalidate evidence already committed.
pub const AZIMUTH_DETENTS: usize = 6;

/// Yaw for `detent`, wrapping.
#[must_use]
pub fn detent_yaw(detent: usize) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let step = std::f32::consts::TAU / AZIMUTH_DETENTS as f32;
    #[allow(clippy::cast_precision_loss)]
    let index = (detent % AZIMUTH_DETENTS) as f32;
    std::f32::consts::FRAC_PI_4 + step * index
}

/// The plan-space direction from the scene toward the camera at `detent`.
///
/// This is what the cutaway tests hull azimuths against: a perimeter hull whose
/// centroid points this way is between the viewer and the interior.
#[must_use]
pub fn detent_bearing(detent: usize) -> Vec2 {
    let yaw = detent_yaw(detent);
    // The camera sits along `rotation * +Z`; in plan that is (sin yaw, cos yaw).
    Vec2::new(yaw.sin(), yaw.cos())
}

/// Where an orthographic isometric camera stands to frame a box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsoFraming {
    pub translation: Vec3,
    pub rotation: Quat,
    /// Orthographic scale, in world units per pixel.
    pub units_per_pixel: f32,
    /// Far plane, sized to the box's diagonal so nothing clips.
    pub far: f32,
}

/// Frame the box `min..max` from `detent`, fitted to a `width` x `height`
/// viewport with a little air.
#[must_use]
pub fn frame(min: Vec3, max: Vec3, detent: usize, width: f32, height: f32) -> IsoFraming {
    let rotation = Quat::from_euler(EulerRot::YXZ, detent_yaw(detent), ISO_PITCH, 0.0);
    let centre = (min + max) * 0.5;
    let inverse = rotation.inverse();
    let mut extent = Vec2::ZERO;
    for corner in 0..8u8 {
        let point = Vec3::new(
            if corner & 1 == 0 { min.x } else { max.x },
            if corner & 2 == 0 { min.y } else { max.y },
            if corner & 4 == 0 { min.z } else { max.z },
        );
        extent = extent.max((inverse * (point - centre)).truncate().abs());
    }
    let units_per_pixel = (extent.x * 2.0 / width)
        .max(extent.y * 2.0 / height)
        .max(f32::MIN_POSITIVE)
        * 1.08;
    let diagonal = (max - min).length().max(1.0);
    IsoFraming {
        translation: centre + rotation * Vec3::Z * diagonal,
        rotation,
        units_per_pixel,
        far: diagonal * 2.0,
    }
}

/// How to light an isometric cutaway.
///
/// A cutaway is mostly interiors, and an interior lit the way play lights it -
/// a tight pool over the body's own cell - is a black hole with one bright
/// spot. These are the values the composition studio arrived at, shared for the
/// same reason the cutaway rule is: the game's spectator overview is looking at
/// the same building the same way, and two answers to "how bright is an
/// interior" would make it two buildings again.
pub mod light {
    /// Key strength. Deliberately modest: a surface pass bright enough to drown
    /// an overlay is worse than one that reads on its own.
    pub const KEY_ILLUMINANCE: f32 = 2_600.0;

    /// Fill, so a cut-open interior is shaded rather than a black hole. Low
    /// enough to stay atmosphere rather than becoming signal.
    pub const AMBIENT_BRIGHTNESS: f32 = 260.0;

    /// How far the key sits off the view axis, in radians (~40 degrees).
    ///
    /// Head-on light flattens: a wall face and the floor it stands on return
    /// the same value. Off-axis is what gives them different values, which is
    /// the whole point of lighting a cutaway.
    pub const KEY_OFFSET: f32 = 0.7;

    /// One local practical pool per visible architecture register.
    ///
    /// An overview may show several districts at once, so a single global
    /// fog/ambient palette would necessarily lie about all but one of them. A
    /// pool per register is what lets each district keep its own light colour
    /// while they share a frame.
    pub const PRACTICAL_INTENSITY: f32 = 22_000.0;
    /// Wide enough to wash a district rather than spot one cell of it.
    pub const PRACTICAL_RANGE: f32 = 34.0;
    pub const PRACTICAL_RADIUS: f32 = 3.0;
    /// How far above the deck the pool hangs.
    pub const PRACTICAL_LIFT: f32 = 12.0;
}

/// Which floors are drawn.
///
/// The cycle runs `0, 1, ... N-1, All` and wraps.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Layer {
    Single(u8),
    #[default]
    All,
}

impl Layer {
    /// The single level being drawn, or `None` for all of them.
    #[must_use]
    pub fn level(self) -> Option<u8> {
        match self {
            Self::Single(level) => Some(level),
            Self::All => None,
        }
    }

    /// Whether `level` is drawn at all.
    ///
    /// A single-floor view still draws the floor above and below, so you can
    /// see how they relate - a plan with no context above or below says nothing
    /// about vertical connection. Everything further is dropped, which is what
    /// keeps a ten-level facility from stacking into an unreadable thicket.
    #[must_use]
    pub fn draws(self, level: u8) -> bool {
        match self {
            Self::All => true,
            Self::Single(focus) => level.abs_diff(focus) <= 1,
        }
    }

    /// Whether `level` is the floor under inspection rather than context.
    /// Only the focus floor gets solid detail; the rest is line work.
    #[must_use]
    pub fn is_focus(self, level: u8) -> bool {
        match self {
            Self::All => true,
            Self::Single(focus) => level == focus,
        }
    }

    /// How the floor names itself in chrome. ASCII only - the shipped font
    /// carries nothing else.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Single(level) => format!("layer {level}"),
            Self::All => "all layers".to_string(),
        }
    }

    #[must_use]
    pub fn next(self, levels: u8) -> Self {
        match self {
            Self::Single(level) if level + 1 < levels => Self::Single(level + 1),
            Self::Single(_) => Self::All,
            Self::All => Self::Single(0),
        }
    }

    #[must_use]
    pub fn previous(self, levels: u8) -> Self {
        match self {
            Self::Single(0) => Self::All,
            Self::Single(level) => Self::Single(level - 1),
            Self::All => Self::Single(levels.saturating_sub(1)),
        }
    }
}

/// Anything at or below this is floor, and floor always stays.
const FLOOR_TOP: f32 = 0.75;
/// A hull whose *lowest* point clears this is roofing.
const HEAD_CLEARANCE: f32 = 2.2;
/// Inside this plan radius a hull is a fitting, never a perimeter wall.
const INTERIOR_RADIUS: f32 = 6.5;
/// A hull within 90 degrees of the camera bearing is between you and the room.
const NEAR_ARC_COS: f32 = 0.0;

/// Structural region of an authored convex hull in an isometric interior view.
/// This is presentation classification only: it never changes collision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HullRegion {
    Floor,
    Ceiling,
    Interior,
    Perimeter,
}

/// Classify one cell-local hull before a view decides whether to hide, dim, or
/// shorten it. Keeping this beside [`survives`] gives cutaways and low-wall
/// plans one answer to what counts as architectural perimeter.
#[must_use]
pub fn hull_region(min_y: f32, max_y: f32, local: Vec3) -> HullRegion {
    if max_y <= FLOOR_TOP {
        HullRegion::Floor
    } else if min_y >= HEAD_CLEARANCE {
        HullRegion::Ceiling
    } else if Vec2::new(local.x, local.z).length() < INTERIOR_RADIUS {
        HullRegion::Interior
    } else {
        HullRegion::Perimeter
    }
}

/// Whether a hull survives the cutaway.
///
/// `bearing` is the direction from the scene toward the camera in the plan (XZ)
/// plane, normalized; `local` is the hull's centroid relative to its cell.
///
/// The camera looks down at about 35 degrees, so a cell rendered whole shows
/// its ceiling and the three walls between you and its interior - which is to
/// say nothing you wanted. Three tests decide what survives, in this order:
///
/// 1. **Floor always stays.** Checked first so nothing below can drop it; cull
///    it and cells appear to float.
/// 2. **Ceiling goes.** Min-Y and not the centroid: a pillar spanning floor to
///    ceiling has a high centroid, and dropping it would delete structure
///    rather than roofing.
/// 3. **Near walls go.** At any bearing that is three of the six, which falls
///    out of hex geometry rather than being a tuned number.
#[must_use]
pub fn survives(min_y: f32, max_y: f32, local: Vec3, bearing: Vec2, cutaway: bool) -> bool {
    if !cutaway {
        return true;
    }
    match hull_region(min_y, max_y, local) {
        HullRegion::Floor | HullRegion::Interior => true,
        HullRegion::Ceiling => false,
        HullRegion::Perimeter => {
            let plan = Vec2::new(local.x, local.z);
            plan.normalize_or_zero().dot(bearing) <= NEAR_ARC_COS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Detent 0 must stay the historical 45-degree camera, or every committed
    /// capture is reframed by a feature that was supposed to be additive.
    #[test]
    fn detent_zero_is_the_historical_camera() {
        assert!((detent_yaw(0) - std::f32::consts::FRAC_PI_4).abs() < 1e-6);
    }

    /// Six steps, wrapping, one per hex face.
    #[test]
    fn detents_wrap_and_step_sixty_degrees() {
        // Up to the last: the step from the last one is negative by
        // construction, because `detent_yaw` wraps rather than running on.
        for detent in 0..AZIMUTH_DETENTS - 1 {
            let step = detent_yaw(detent + 1) - detent_yaw(detent);
            assert!(
                (step - std::f32::consts::TAU / 6.0).abs() < 1e-5,
                "detent {detent} steps {step}"
            );
        }
        assert!((detent_yaw(AZIMUTH_DETENTS) - detent_yaw(0)).abs() < 1e-6);
    }

    /// The bearing must point from the scene toward where the camera stands,
    /// or the cutaway drops the far walls and keeps the near ones.
    #[test]
    fn the_bearing_points_at_the_camera() {
        for detent in 0..AZIMUTH_DETENTS {
            let framing = frame(Vec3::splat(-1.0), Vec3::splat(1.0), detent, 100.0, 100.0);
            let toward_camera = framing.translation.normalize();
            let bearing = detent_bearing(detent);
            let plan = Vec2::new(toward_camera.x, toward_camera.z).normalize_or_zero();
            assert!(
                plan.dot(bearing) > 0.99,
                "detent {detent}: bearing {bearing:?} disagrees with camera {plan:?}"
            );
        }
    }

    /// Floor stays, roof goes, pillars stay, and near walls go while far walls
    /// remain. The whole point of the cutaway in four assertions.
    #[test]
    fn the_cutaway_keeps_the_floor_and_drops_what_is_in_front() {
        let bearing = Vec2::new(1.0, 0.0);
        assert!(
            survives(0.0, 0.5, Vec3::new(9.0, 0.2, 0.0), bearing, true),
            "floor must survive even on the near side"
        );
        assert!(
            !survives(3.0, 4.0, Vec3::new(0.0, 3.5, 0.0), bearing, true),
            "a ceiling must be cut"
        );
        assert!(
            survives(0.0, 4.0, Vec3::new(0.0, 2.0, 0.0), bearing, true),
            "a full-height pillar is structure, not roofing"
        );
        assert!(
            !survives(0.8, 3.0, Vec3::new(9.0, 1.5, 0.0), bearing, true),
            "the wall between the viewer and the room must be cut"
        );
        assert!(
            survives(0.8, 3.0, Vec3::new(-9.0, 1.5, 0.0), bearing, true),
            "the far wall must remain, or the room has no back"
        );
    }

    /// Cutaway off means nothing is dropped, whatever the geometry.
    #[test]
    fn no_cutaway_keeps_everything() {
        assert!(survives(
            3.0,
            4.0,
            Vec3::new(9.0, 3.5, 0.0),
            Vec2::new(1.0, 0.0),
            false
        ));
    }

    /// A single floor draws its neighbours as context but focuses only itself.
    #[test]
    fn a_single_floor_carries_its_neighbours_as_context() {
        let layer = Layer::Single(4);
        assert!(layer.draws(3) && layer.draws(4) && layer.draws(5));
        assert!(!layer.draws(2) && !layer.draws(6));
        assert!(layer.is_focus(4));
        assert!(!layer.is_focus(3) && !layer.is_focus(5));
    }

    /// The cycle reaches every floor and then `All`, both ways, and wraps.
    #[test]
    fn the_layer_cycle_reaches_every_floor_and_wraps() {
        const LEVELS: u8 = 3;
        let mut seen = Vec::new();
        let mut layer = Layer::Single(0);
        for _ in 0..=LEVELS {
            seen.push(layer);
            layer = layer.next(LEVELS);
        }
        assert_eq!(
            seen,
            vec![
                Layer::Single(0),
                Layer::Single(1),
                Layer::Single(2),
                Layer::All
            ]
        );
        assert_eq!(layer, Layer::Single(0), "the cycle must wrap");
        assert_eq!(Layer::Single(0).previous(LEVELS), Layer::All);
        assert_eq!(Layer::All.previous(LEVELS), Layer::Single(LEVELS - 1));
    }
}
