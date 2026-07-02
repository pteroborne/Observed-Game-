use crate::screens::{
    DespawnOnExit, FIXTURE_LIGHT_INTENSITY, FlickerLight, GameState, MatchAssets, PassagePreview,
    PlaceGeometry, WALL_HEIGHT,
};
use crate::teleport;
use bevy::prelude::*;
use std::f32::consts::PI;

const TRIM_LOW_Y: f32 = 0.12;
const TRIM_HEIGHT: f32 = 0.08;
const TRIM_DEPTH: f32 = 0.06;
const TRIM_WALL_INSET: f32 = 0.05;

fn trim_spans(half_len: f32, gaps: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    teleport::transition::wall_spans(half_len, gaps)
        .into_iter()
        .map(|(center, half)| (center, half * 2.0))
        .filter(|(_, len)| *len > 0.05)
        .collect()
}

/// Spawn one place point light, tagged for teardown and the decoherence/idle flicker.
pub(crate) fn spawn_place_light(
    commands: &mut Commands,
    transform: Transform,
    color: Color,
    range: f32,
    flicker: FlickerLight,
    preview: bool,
) {
    let intensity = flicker.base;
    let mut light = commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        flicker,
        PointLight {
            color,
            intensity,
            range,
            shadows_enabled: false,
            ..default()
        },
        transform,
        Name::new("Place light"),
    ));
    if preview {
        light.insert(PassagePreview);
    }
}

/// Local XZ positions for a place's ceiling fixtures: a couple of wall sconces in a polygon
/// room, or overhead lights spaced down a hallway. Deterministic from the geometry so a
/// place and its doorway preview place their fixtures identically.
pub(crate) fn fixture_positions(geom: &teleport::PlaceGeom) -> Vec<Vec2> {
    if let Some(poly) = geom.poly.as_ref() {
        let n = poly.len();
        if n < 3 {
            return Vec::new();
        }
        [0usize, n / 2]
            .into_iter()
            .map(|i| (poly[i] + poly[(i + 1) % n]) * 0.5 * 0.78)
            .collect()
    } else {
        let hz = geom.half.y;
        let count = ((hz / 6.0).floor() as usize).clamp(1, 3);
        (0..count)
            .map(|k| Vec2::new(0.0, -hz + (k as f32 + 0.5) * (2.0 * hz / count as f32)))
            .collect()
    }
}

/// Spawn a place's full lighting — an overhead fill plus a few flickering ceiling fixtures
/// — under `xform` (identity for the live place, the doorway-alignment transform for a
/// preview, so a preview is lit identically to the place you cross into). `light_color` is
/// the place's district temperature; the warm lamp bodies stay neutral.
pub(crate) fn spawn_place_lighting(
    commands: &mut Commands,
    assets: &MatchAssets,
    geom: &teleport::PlaceGeom,
    light_color: Color,
    xform: Transform,
    preview: bool,
) {
    let (hx, hz) = (geom.half.x, geom.half.y);
    let place_in = |local: Transform| xform.mul_transform(local);
    // Overhead fill (steady; only the decoherence flash stutters it).
    spawn_place_light(
        commands,
        place_in(Transform::from_xyz(0.0, WALL_HEIGHT - 0.4, 0.0)),
        light_color,
        (hx.max(hz) + WALL_HEIGHT) * 1.6,
        FlickerLight {
            base: FIXTURE_LIGHT_INTENSITY,
            idle: 0.0,
            phase: 0.0,
        },
        preview,
    );
    // Flickering ceiling fixtures: the "failing office light" look + per-place interest.
    for (i, pos) in fixture_positions(geom).into_iter().enumerate() {
        let phase = i as f32 * 1.7 + 0.4;
        let mut lamp = commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.lamp_material.clone()),
            place_in(
                Transform::from_xyz(pos.x, WALL_HEIGHT - 0.16, pos.y)
                    .with_scale(Vec3::new(0.8, 0.12, 0.8)),
            ),
            Name::new("Ceiling fixture"),
        ));
        if preview {
            lamp.insert(PassagePreview);
        }
        spawn_place_light(
            commands,
            place_in(Transform::from_xyz(pos.x, WALL_HEIGHT - 0.6, pos.y)),
            light_color,
            11.0,
            FlickerLight {
                base: FIXTURE_LIGHT_INTENSITY * 0.55,
                idle: 0.7,
                phase,
            },
            preview,
        );
    }
}

/// The inward unit normal of edge `a`→`b` of a polygon centred at the origin (points
/// toward the interior), for tucking wall trim proud of the face.
fn inward_normal(a: Vec2, b: Vec2) -> Vec2 {
    let d = (b - a).normalize_or_zero();
    let n = Vec2::new(-d.y, d.x);
    if n.dot((a + b) * 0.5) > 0.0 { -n } else { n }
}

/// Draw the structural neon linework that gives a place's shell surface interest without
/// any textures (code-as-art): a baseboard seam, a cornice seam, and thin ceiling ribs in
/// the district's accent. Built under `xform` and tagged for the same teardown/preview
/// path as the rest of the place, so previews match what you walk into.
pub(crate) fn spawn_surface_detail(
    commands: &mut Commands,
    assets: &MatchAssets,
    geom: &teleport::PlaceGeom,
    accent: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
) {
    let high = WALL_HEIGHT - 0.18;
    let ceiling_y = WALL_HEIGHT - 0.035;
    let mut strip = |center: Vec3, scale: Vec3, yaw: f32, name: &'static str| {
        let local = Transform::from_translation(center)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(scale);
        let mut e = commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(accent.clone()),
            xform.mul_transform(local),
            Name::new(name),
        ));
        if preview {
            e.insert(PassagePreview);
        }
    };
    if let Some(poly) = geom.poly.as_ref() {
        let n = poly.len();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let d = b - a;
            let len = d.length();
            if len < 0.05 {
                continue;
            }
            let mid = (a + b) * 0.5;
            let inward = inward_normal(a, b);
            let yaw = (-d.y).atan2(d.x);
            let dir = d.normalize_or_zero();
            let passage_gaps = geom
                .gaps
                .iter()
                .filter(|gap| gap.kind.is_passage() && (gap.center - mid).length() < 0.05)
                .map(|gap| ((gap.center - mid).dot(dir), gap.width * 0.5))
                .collect::<Vec<_>>();
            if !preview || passage_gaps.is_empty() {
                for (offset, span_len) in trim_spans(len * 0.5, passage_gaps) {
                    let p = mid + dir * offset + inward * TRIM_WALL_INSET;
                    for y in [TRIM_LOW_Y, high] {
                        strip(
                            Vec3::new(p.x, y, p.y),
                            Vec3::new(span_len, TRIM_HEIGHT, TRIM_DEPTH),
                            yaw,
                            "Wall trim",
                        );
                    }
                }
            }
            if i % 2 == 0 {
                let rib = b * 0.46;
                let rib_len = rib.length();
                if rib_len > 0.4 {
                    strip(
                        Vec3::new(rib.x * 0.5, ceiling_y, rib.y * 0.5),
                        Vec3::new(rib_len, 0.035, 0.045),
                        (-rib.y).atan2(rib.x),
                        "Ceiling rib",
                    );
                }
            }
        }
    } else {
        let (hx, hz) = (geom.half.x, geom.half.y);
        for y in [TRIM_LOW_Y, high] {
            for sign in [-1.0_f32, 1.0] {
                // North/South walls run along X, split around open thresholds.
                let gaps = geom
                    .gaps
                    .iter()
                    .filter(|gap| {
                        gap.kind.is_passage()
                            && (gap.normal.y - sign).abs() < 0.5
                            && gap.normal.x.abs() < 0.5
                    })
                    .map(|gap| (gap.center.x, gap.width * 0.5))
                    .collect::<Vec<_>>();
                if !preview || gaps.is_empty() {
                    for (cx, len) in trim_spans(hx, gaps) {
                        strip(
                            Vec3::new(cx, y, sign * (hz - TRIM_WALL_INSET)),
                            Vec3::new(len, TRIM_HEIGHT, TRIM_DEPTH),
                            0.0,
                            "Wall trim",
                        );
                    }
                }

                // West/East walls run along Z, split around any side thresholds.
                let gaps = geom
                    .gaps
                    .iter()
                    .filter(|gap| {
                        gap.kind.is_passage()
                            && (gap.normal.x - sign).abs() < 0.5
                            && gap.normal.y.abs() < 0.5
                    })
                    .map(|gap| (gap.center.y, gap.width * 0.5))
                    .collect::<Vec<_>>();
                if !preview || gaps.is_empty() {
                    for (cz, len) in trim_spans(hz, gaps) {
                        strip(
                            Vec3::new(sign * (hx - TRIM_WALL_INSET), y, cz),
                            Vec3::new(TRIM_DEPTH, TRIM_HEIGHT, len),
                            0.0,
                            "Wall trim",
                        );
                    }
                }
            }
        }
        let x_divisions = ((hx * 2.0) / 4.0).floor().max(1.0) as i32;
        let z_divisions = ((hz * 2.0) / 4.0).floor().max(1.0) as i32;
        for ix in -x_divisions..=x_divisions {
            let x = ix as f32 * hx / (x_divisions as f32 + 0.5);
            strip(
                Vec3::new(x, ceiling_y, 0.0),
                Vec3::new(2.0 * hz, 0.035, 0.045),
                PI * 0.5,
                "Ceiling rib",
            );
        }
        for iz in -z_divisions..=z_divisions {
            let z = iz as f32 * hz / (z_divisions as f32 + 0.5);
            strip(
                Vec3::new(0.0, ceiling_y, z),
                Vec3::new(2.0 * hx, 0.035, 0.045),
                0.0,
                "Ceiling rib",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_spans_leave_the_passage_open() {
        let spans = trim_spans(5.0, vec![(0.0, 1.25)]);

        assert_eq!(spans.len(), 2);
        assert!(
            spans.iter().all(|(center, len)| {
                let lo = center - len * 0.5;
                let hi = center + len * 0.5;
                hi <= -1.25 || lo >= 1.25
            }),
            "wall trim must not cross the open threshold: {spans:?}"
        );
    }
}
