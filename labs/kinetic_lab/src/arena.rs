//! One authored chamber. These solids drive rendering, collision and navigation.
use glam::Vec3;

pub const BRIDGE: u32 = 5;
pub const GENERATOR: Vec3 = Vec3::new(-12.5, 0.0, 7.0);
pub const STATION: Vec3 = Vec3::new(-9.5, 0.0, 7.0);
pub const PANEL: Vec3 = Vec3::new(-0.8, 0.0, 6.5);
pub const SPAWN: Vec3 = Vec3::new(-11.0, 0.0, 7.0);
pub const VOID_Y: f32 = -12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Floor,
    Wall,
    Catwalk,
    Landing,
    Bridge,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    pub id: u32,
    pub center: Vec3,
    pub half: Vec3,
    pub surface: Surface,
}

pub fn solids() -> Vec<Solid> {
    let mut result = Vec::new();
    let mut add = |id, center, half, surface| {
        result.push(Solid {
            id,
            center,
            half,
            surface,
        })
    };
    add(
        1,
        Vec3::new(-7.0, -0.5, 0.0),
        Vec3::new(8.0, 0.5, 12.0),
        Surface::Floor,
    );
    add(
        2,
        Vec3::new(10.0, -0.5, 7.5),
        Vec3::new(5.0, 0.5, 4.5),
        Surface::Floor,
    );
    add(
        3,
        Vec3::new(10.0, -0.5, -7.5),
        Vec3::new(5.0, 0.5, 4.5),
        Surface::Floor,
    );
    add(
        4,
        Vec3::new(-2.0, 2.75, -2.0),
        Vec3::new(2.0, 0.25, 6.0),
        Surface::Catwalk,
    );
    add(
        BRIDGE,
        Vec3::new(3.0, -0.25, 6.5),
        Vec3::new(2.0, 0.25, 1.5),
        Surface::Bridge,
    );
    // Visible recoverable shelf below the upper unrailed catwalk.
    add(
        6,
        Vec3::new(2.0, -3.25, -5.0),
        Vec3::new(1.0, 0.25, 3.0),
        Surface::Landing,
    );
    // Small return stairs from the shelf to the main deck. A 5 mm tread
    // nosing prevents floating-point seams from becoming unsupported ray samples.
    for i in 0..10 {
        let top = -2.7 + i as f32 * 0.3;
        add(
            20 + i,
            Vec3::new(2.0, top - 0.15, -1.8 + i as f32 * 0.4),
            Vec3::new(1.0, 0.15, 0.205),
            Surface::Landing,
        );
    }
    add(
        7,
        Vec3::new(2.0, -0.25, 3.0),
        Vec3::new(1.0, 0.25, 1.0),
        Surface::Landing,
    );
    for i in 0..10 {
        let top = (i + 1) as f32 * 0.3;
        add(
            40 + i,
            Vec3::new(-7.8 + i as f32 * 0.4, top * 0.5, 2.0),
            Vec3::new(0.205, top * 0.5, 1.5),
            Surface::Catwalk,
        );
    }
    add(
        60,
        Vec3::new(-15.25, 3.0, 0.0),
        Vec3::new(0.25, 3.0, 12.0),
        Surface::Wall,
    );
    add(
        61,
        Vec3::new(-7.0, 3.0, -12.25),
        Vec3::new(8.0, 3.0, 0.25),
        Surface::Wall,
    );
    add(
        62,
        Vec3::new(-7.0, 3.0, 12.25),
        Vec3::new(8.0, 3.0, 0.25),
        Surface::Wall,
    );
    add(
        63,
        Vec3::new(-8.0, 2.0, -8.5),
        Vec3::new(0.25, 2.0, 3.5),
        Surface::Wall,
    );
    add(
        64,
        Vec3::new(-5.5, 2.0, -5.0),
        Vec3::new(2.75, 2.0, 0.25),
        Surface::Wall,
    );
    add(
        65,
        Vec3::new(15.25, 3.0, 0.0),
        Vec3::new(0.25, 3.0, 12.0),
        Surface::Wall,
    );
    add(
        66,
        Vec3::new(10.0, 3.0, -12.25),
        Vec3::new(5.0, 3.0, 0.25),
        Surface::Wall,
    );
    add(
        67,
        Vec3::new(10.0, 3.0, 12.25),
        Vec3::new(5.0, 3.0, 0.25),
        Surface::Wall,
    );
    for (i, p) in [GENERATOR, STATION, PANEL].into_iter().enumerate() {
        add(
            80 + i as u32,
            p + Vec3::Y * 0.55,
            Vec3::new(0.4, 0.55, 0.3),
            Surface::Wall,
        );
    }
    result
}

/// Authored waypoints (feet). Edges are checked against actual walls/support at
/// construction and on bridge removal; no actor is permitted to path across void.
pub fn waypoints() -> Vec<Vec3> {
    let mut points = Vec::new();
    for x in [-13., -11., -9., -6., -4., -2., 0.] {
        for z in [-10., -7., -3., 0., 4., 7., 10.] {
            points.push(Vec3::new(x, 0., z));
        }
    }
    for x in [6., 9., 13.] {
        for z in [4., 7., 10.] {
            points.push(Vec3::new(x, 0., z));
        }
    }
    points.push(Vec3::new(3., 0., 6.5));
    for i in 0..=10 {
        points.push(Vec3::new(-8. + i as f32 * 0.4, i as f32 * 0.3, 2.));
    }
    for z in [-7., -4., -1., 2.] {
        points.push(Vec3::new(-2., 3., z));
    }
    points.push(Vec3::new(2., -3., -5.));
    points.push(Vec3::new(2., -3., -2.3));
    for i in 0..10 {
        points.push(Vec3::new(2., -2.7 + i as f32 * 0.3, -1.8 + i as f32 * 0.4));
    }
    points.push(Vec3::new(2., 0., 3.));
    points
}
