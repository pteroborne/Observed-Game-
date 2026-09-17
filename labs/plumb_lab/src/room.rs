//! One authored room. Six surfaces and a couple of obstacles, nothing else.
//!
//! Deliberately not a WFC floor: this lab asks whether a body can stand and
//! walk on an arbitrary surface, and an authored box gives it a floor, a
//! ceiling and four walls to try that on with nothing else in the way.

use glam::Vec3;

/// Half-extent of the room's interior, in metres.
pub const HALF: f32 = 9.0;
/// Interior height.
pub const HEIGHT: f32 = 10.0;
/// Thickness of every shell surface.
pub const SHELL: f32 = 0.5;

/// Which surface of the room a solid is, for presentation and for reporting
/// which one the subject came to rest on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Floor,
    Ceiling,
    Wall,
    Obstacle,
}

#[derive(Clone, Copy, Debug)]
pub struct Solid {
    pub id: u32,
    pub center: Vec3,
    pub half: Vec3,
    pub surface: Surface,
}

/// The room, as axis-aligned boxes.
#[must_use]
pub fn solids() -> Vec<Solid> {
    let mut solids = Vec::new();
    let mut add = |id, center, half, surface| {
        solids.push(Solid {
            id,
            center,
            half,
            surface,
        });
    };
    // Floor and ceiling.
    add(
        1,
        Vec3::new(0., -SHELL, 0.),
        Vec3::new(HALF, SHELL, HALF),
        Surface::Floor,
    );
    add(
        2,
        Vec3::new(0., HEIGHT + SHELL, 0.),
        Vec3::new(HALF, SHELL, HALF),
        Surface::Ceiling,
    );
    // Four walls.
    for (id, sign) in [(3, 1.0_f32), (4, -1.0)] {
        add(
            id,
            Vec3::new(sign * (HALF + SHELL), HEIGHT * 0.5, 0.),
            Vec3::new(SHELL, HEIGHT * 0.5, HALF),
            Surface::Wall,
        );
    }
    for (id, sign) in [(5, 1.0_f32), (6, -1.0)] {
        add(
            id,
            Vec3::new(0., HEIGHT * 0.5, sign * (HALF + SHELL)),
            Vec3::new(HALF, HEIGHT * 0.5, SHELL),
            Surface::Wall,
        );
    }
    // Two low obstacles, so walking has something to step over. They are kept
    // clear of the cardinal lines through the spawn: a plumb slides the subject
    // along one of those, and an obstacle in the way makes it come to rest
    // against a crate instead of the surface the phase was asking about.
    add(
        7,
        Vec3::new(4.2, 0.35, 5.0),
        Vec3::new(1.2, 0.35, 1.2),
        Surface::Obstacle,
    );
    add(
        8,
        Vec3::new(-5.2, 0.6, 4.4),
        Vec3::new(0.8, 0.6, 1.6),
        Surface::Obstacle,
    );
    solids
}

/// Where the fixed camera sits: a corner at mid height, looking at the middle
/// of the room, so floor, ceiling and two walls are all in frame at once.
///
/// Mid height rather than up in the corner, which is where it started: from
/// beside the ceiling you cannot see the ceiling, and a subject walking on it
/// simply leaves the picture.
#[must_use]
pub fn camera() -> (Vec3, Vec3) {
    (
        Vec3::new(HALF + 13.0, HEIGHT * 0.62, HALF + 13.0),
        Vec3::new(0., HEIGHT * 0.45, 0.),
    )
}

/// The two walls between the camera and the room.
///
/// They are not drawn, so the room can be watched from outside as a cutaway.
/// They still exist in the collision world — the subject stands on one of them
/// in the east phase — which is the whole reason to omit them in presentation
/// rather than in the room.
pub const CUTAWAY: [u32; 2] = [3, 5];

/// Where the subject starts.
#[must_use]
pub fn spawn() -> Vec3 {
    Vec3::new(-1.0, 1.2, -2.0)
}
