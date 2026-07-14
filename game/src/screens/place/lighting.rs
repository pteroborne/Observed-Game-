use crate::GameState;
use crate::layout::WALL_HEIGHT;
use crate::teleport;
use crate::view::assets::MatchAssets;
use crate::view::components::{FlickerLight, PassagePreview, PlaceGeometry};
use bevy::prelude::*;
use observed_style as style;

pub(crate) const FIXTURE_LIGHT_INTENSITY: f32 = 2_800.0;

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
pub(crate) fn fixture_positions(geom: &teleport::PlaceGeom, pools_rhythm: bool) -> Vec<Vec2> {
    if let Some(poly) = geom.poly.as_ref() {
        let n = poly.len();
        if n < 3 {
            return Vec::new();
        }
        if pools_rhythm {
            vec![(poly[0] + poly[n / 2]) * 0.5 * 0.4]
        } else {
            [0usize, n / 2]
                .into_iter()
                .map(|i| (poly[i] + poly[(i + 1) % n]) * 0.5 * 0.78)
                .collect()
        }
    } else {
        let hz = geom.half.y;
        if pools_rhythm {
            let count = ((hz / 9.5).floor() as usize).clamp(1, 2);
            (0..count)
                .map(|k| Vec2::new(0.0, -hz + (k as f32 + 0.5) * (2.0 * hz / count as f32)))
                .collect()
        } else {
            let count = ((hz / 6.0).floor() as usize).clamp(1, 3);
            (0..count)
                .map(|k| Vec2::new(0.0, -hz + (k as f32 + 0.5) * (2.0 * hz / count as f32)))
                .collect()
        }
    }
}

/// Spawn a place's full lighting — a shadow-casting key spotlight, an overhead fill, and a few
/// flickering ceiling fixtures — under `xform` (identity for the live place, the doorway-alignment
/// transform for a preview, so a preview is lit identically to the place you cross into).
/// The light colors and key settings come from the district `palette`.
pub(crate) fn spawn_place_lighting(
    commands: &mut Commands,
    assets: &MatchAssets,
    geom: &teleport::PlaceGeom,
    palette: &style::DistrictPalette,
    xform: Transform,
    preview: bool,
) {
    if geom.is_wellshaft() {
        spawn_wellshaft_lighting(commands, assets, palette, xform, preview);
        return;
    }
    let (hx, hz) = (geom.half.x, geom.half.y);
    let place_in = |local: Transform| xform.mul_transform(local);

    // 1. Shadow-casting key SpotLight: high in the room volume, raking
    // diagonally across the floor (the lab rigs' geometry). Two placement
    // constraints, both load-bearing: it must sit BELOW the ceiling slab (the
    // shell's ceiling casts shadows — a key above it lights nothing), and for
    // polygon rooms it must sit INSIDE the polygon, not the bounding box (a
    // box-corner position lands inside the cut corner's wall and shadows
    // itself to black). Halfway toward a vertex is interior for any convex
    // footprint.
    let key_xz = if let Some(poly) = geom.poly.as_ref() {
        // 0.3× the first vertex: inside any convex footprint AND inside the
        // Colonnade pillar ring (pillars stand near mid-radius).
        poly.first().copied().unwrap_or(Vec2::new(hx, hz)) * 0.3
    } else {
        Vec2::new(
            (hx * 0.75).max(2.0).min(hx - 0.3),
            (hz * 0.75).max(2.0).min(hz - 0.3),
        )
    };
    let key_pos = Vec3::new(key_xz.x, WALL_HEIGHT - 0.45, key_xz.y);
    let key_target = Vec3::new(-key_xz.x * 0.4, 0.0, -key_xz.y * 0.4);
    let local_key_xform = Transform::from_translation(key_pos).looking_at(key_target, Vec3::Y);

    let mut key_light = commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        SpotLight {
            color: palette.key_color,
            intensity: palette.key_intensity,
            range: palette.key_range,
            radius: palette.key_radius,
            shadows_enabled: palette.key_shadows_enabled,
            inner_angle: palette.key_inner_angle,
            outer_angle: palette.key_outer_angle,
            ..default()
        },
        place_in(local_key_xform),
        Name::new("Key SpotLight"),
    ));
    if preview {
        key_light.insert(PassagePreview);
    }

    // 2. Overhead fill (steady supporting role)
    let fill_intensity = FIXTURE_LIGHT_INTENSITY * 0.25;
    spawn_place_light(
        commands,
        place_in(Transform::from_xyz(0.0, WALL_HEIGHT - 0.4, 0.0)),
        palette.light_color,
        (hx.max(hz) + WALL_HEIGHT) * 1.6,
        FlickerLight {
            base: fill_intensity,
            idle: 0.0,
            phase: 0.0,
        },
        preview,
    );

    // 3. Flickering ceiling fixtures: failing office point lights spaced down the corridor/room.
    for (i, pos) in fixture_positions(geom, palette.pools_rhythm)
        .into_iter()
        .enumerate()
    {
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

        let (fixture_range, fixture_intensity) = if palette.pools_rhythm {
            (7.0, FIXTURE_LIGHT_INTENSITY * 0.25)
        } else {
            (11.0, FIXTURE_LIGHT_INTENSITY * 0.35)
        };

        spawn_place_light(
            commands,
            place_in(Transform::from_xyz(pos.x, WALL_HEIGHT - 0.6, pos.y)),
            palette.light_color,
            fixture_range,
            FlickerLight {
                base: fixture_intensity,
                idle: 0.7,
                phase,
            },
            preview,
        );
    }
}

/// The lighting-lab wellshaft register in playable form: one tight warm pool on every
/// radial landing around the hex pillar. Only the top lamp spends shadow budget; the
/// remaining pools are deliberately isolated unshadowed fills.
fn spawn_wellshaft_lighting(
    commands: &mut Commands,
    assets: &MatchAssets,
    palette: &style::DistrictPalette,
    xform: Transform,
    preview: bool,
) {
    let place_in = |local: Transform| xform.mul_transform(local);
    let tint = palette.key_color.to_srgba();
    let practical_color = Color::srgb(
        0.82 + tint.red * 0.18,
        0.34 + tint.green * 0.24,
        0.07 + tint.blue * 0.12,
    );
    for level in 0..crate::hallway::WELL_SHAFT_LEVELS {
        let y = level as f32 * crate::hallway::WELL_SHAFT_LEVEL_HEIGHT + 1.35;
        let landing = crate::hallway::wellshaft_landing_center(level);
        let direction = crate::hallway::wellshaft_level_direction(level);
        let tangent = Vec2::new(-direction.1, direction.0);
        let lamp_pos = Vec3::new(
            landing.0 + tangent.x * 0.85,
            y,
            landing.1 + tangent.y * 0.85,
        );
        let mut practical = commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.lamp_material.clone()),
            place_in(Transform::from_translation(lamp_pos).with_scale(Vec3::new(0.22, 0.3, 0.22))),
            Name::new("Wellshaft practical"),
        ));
        if preview {
            practical.insert(PassagePreview);
        }
        for offset in [
            Vec3::new(-0.18, 0.0, 0.0),
            Vec3::new(0.18, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -0.18),
            Vec3::new(0.0, 0.0, 0.18),
        ] {
            let mut cage = commands.spawn((
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(assets.gantry_deck_material.clone()),
                place_in(
                    Transform::from_translation(lamp_pos + offset)
                        .with_scale(Vec3::new(0.035, 0.46, 0.035)),
                ),
                Name::new("Wellshaft lamp cage"),
            ));
            if preview {
                cage.insert(PassagePreview);
            }
        }

        // Tight range keeps each lamp a warm island: the concrete between
        // landings must fall to buried dark or the register reads as a flat
        // grey shaft rather than "pools in the dark". Tuned so the darkest
        // framings still clear the luminance floor on the warm ledge glints.
        let intensity = 165_000.0;
        let mut light = commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            FlickerLight {
                base: intensity,
                idle: 0.08,
                phase: level as f32 * 1.7,
            },
            PointLight {
                color: practical_color,
                intensity,
                range: 5.5,
                shadows_enabled: level + 1 == crate::hallway::WELL_SHAFT_LEVELS,
                ..default()
            },
            place_in(Transform::from_translation(lamp_pos)),
            Name::new("Wellshaft practical light"),
        ));
        if preview {
            light.insert(PassagePreview);
        }
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
/// any textures (code-as-art): a single baseboard seam in the district's accent. Built
/// under `xform` and tagged for the same teardown/preview path as the rest of the place,
/// so previews match what you walk into.
///
/// The cornice seam and ceiling ribs were removed (2026-07-11, user playtest): both were
/// pinned to `WALL_HEIGHT`, so in tall places (wellshaft, gantry) they floated mid-shaft,
/// and in ordinary rooms the near-ceiling border never lined up cleanly with the wall/
/// ceiling junction.
pub(crate) fn spawn_surface_detail(
    commands: &mut Commands,
    assets: &MatchAssets,
    geom: &teleport::PlaceGeom,
    accent: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
) {
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
    let boundary_storage;
    let boundary = if let Some(poly) = geom.poly.as_ref() {
        poly.as_slice()
    } else {
        let (hx, hz) = (geom.half.x, geom.half.y);
        boundary_storage = [
            Vec2::new(-hx, -hz),
            Vec2::new(hx, -hz),
            Vec2::new(hx, hz),
            Vec2::new(-hx, hz),
        ];
        boundary_storage.as_slice()
    };
    let plan = teleport::plan_boundary(
        boundary,
        &geom.gaps,
        teleport::structural_height(geom, WALL_HEIGHT),
        WALL_HEIGHT,
    )
    .expect("surface detail must share a valid threshold aperture plan");
    for edge_index in 0..boundary.len() {
        let a = boundary[edge_index];
        let b = boundary[(edge_index + 1) % boundary.len()];
        let d = b - a;
        let len = d.length();
        if len < 0.05 {
            continue;
        }
        let mid = (a + b) * 0.5;
        let inward = inward_normal(a, b);
        let yaw = (-d.y).atan2(d.x);
        let dir = d.normalize_or_zero();
        let threshold_spans = plan
            .apertures
            .iter()
            .filter(|aperture| {
                aperture.edge_index == edge_index
                    && aperture.y_min < TRIM_LOW_Y + TRIM_HEIGHT * 0.5
                    && aperture.y_max > TRIM_LOW_Y - TRIM_HEIGHT * 0.5
            })
            .map(|aperture| {
                let center = (aperture.start + aperture.end) * 0.5;
                (
                    (center - mid).dot(dir),
                    aperture.start.distance(aperture.end) * 0.5,
                )
            })
            .collect::<Vec<_>>();
        if !preview || threshold_spans.is_empty() {
            for (offset, span_len) in trim_spans(len * 0.5, threshold_spans) {
                let p = mid + dir * offset + inward * TRIM_WALL_INSET;
                strip(
                    Vec3::new(p.x, TRIM_LOW_Y, p.y),
                    Vec3::new(span_len, TRIM_HEIGHT, TRIM_DEPTH),
                    yaw,
                    "Wall trim",
                );
            }
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
