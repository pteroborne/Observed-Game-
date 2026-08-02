//! Draw the selected module, and light up whatever is wrong with it.

use bevy::prelude::*;
use observed_hex::{CORNERS, HexFace, TILE_LEVEL_HEIGHT, face_edge};
use observed_style::{SchematicRole, schematic};
use observed_traversal::ConvexRenderMesh;

use crate::module::app::ModuleState;
use crate::module::diagnose::Highlight;

/// `CORNERS` and `face_edge` are integer plan-view metres - the quantized
/// hexagon's whole point is that its corners are exact - so every use here goes
/// through one conversion rather than scattering `as f32` casts that would each
/// be a place to get the axis mapping wrong.
#[must_use]
fn plan(point: (i32, i32), y: f32) -> Vec3 {
    #[allow(clippy::cast_precision_loss)]
    Vec3::new(point.0 as f32, y, point.1 as f32)
}

/// Everything this system owns, despawned wholesale on rebuild.
#[derive(Component)]
pub struct ModuleVisual;

/// Radius of the marker dropped on an offending vertex, in metres.
///
/// Small enough to read as a point on a 14 m cell, large enough to find at the
/// default zoom. A vertex error is the one the author cannot locate by eye.
const VERTEX_MARKER_RADIUS: f32 = 0.35;

/// How far outside the hull the highlight sits, so it reads as an annotation
/// rather than as geometry the module actually contains.
const HIGHLIGHT_LIFT: f32 = 0.06;

/// Rebuild when the selection or the corpus moved.
pub fn rebuild_module_view(
    mut commands: Commands,
    mut state: ResMut<ModuleState>,
    existing: Query<Entity, With<ModuleVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let detent = state.detent;
    let cutaway = state.cutaway;
    // Cloned out before the diagnosis is borrowed; the path is a few dozen
    // points, and the alternative is threading a borrow through the whole
    // rebuild for nothing.
    let walk = state.walk.as_ref().map(|w| (w.path.clone(), w.failure));
    let Some(diagnosis) = state.current() else {
        return;
    };

    // Clean modules draw in the settled colour, failing ones in the alert
    // colour, so the verdict is legible before a word is read.
    let role = if diagnosis.is_clean() {
        SchematicRole::Pinned
    } else {
        SchematicRole::Volatile
    };
    let body = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Grid).base_color.with_alpha(0.85),
        perceptual_roughness: 0.85,
        ..default()
    });
    let marker = materials.add(StandardMaterial {
        base_color: schematic(role).base_color,
        emissive: schematic(role).emissive,
        unlit: true,
        ..default()
    });

    let bearing = crate::viewport::detent_bearing(detent);

    // A key light, off the view axis, so the hull faces take different values.
    // Ambient alone renders a module as one flat silhouette - which is exactly
    // what the first capture of this tool showed, and useless for judging
    // geometry.
    let key_from = Quat::from_rotation_y(crate::draw::KEY_LIGHT_OFFSET)
        * Vec3::new(bearing.x, 0.0, bearing.y)
        + Vec3::Y * 1.15;
    commands.spawn((
        DirectionalLight {
            illuminance: crate::draw::KEY_ILLUMINANCE,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(key_from * 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        ModuleVisual,
    ));

    let centres = cell_centres(diagnosis);

    let mut cut = 0usize;
    if let Some(prototype) = diagnosis.prototype.as_ref() {
        for (index, hull) in prototype.hulls.iter().enumerate() {
            let highlighted = diagnosis.highlight == Highlight::Hull(index);
            if !highlighted && !hull_survives(hull, &centres, bearing, cutaway) {
                cut += 1;
                continue;
            }
            let Some(render) = ConvexRenderMesh::from_convex_hull(hull) else {
                continue;
            };
            let Some(mesh) = crate::detail::mesh_from(&render) else {
                continue;
            };
            // The one hull a `DegenerateBrush` names is drawn in the alert
            // colour; everything else stays neutral so the marked one is the
            // only thing competing for attention.
            let material = if highlighted {
                marker.clone()
            } else {
                body.clone()
            };
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                ModuleVisual,
            ));
        }
    }

    // The cell outline is always drawn: it is the contract the module is being
    // judged against, and a footprint violation is only legible against it.
    let levels = diagnosis
        .prototype
        .as_ref()
        .map_or(1, |prototype| prototype.levels.max(1));
    spawn_cell_outline(&mut commands, &mut meshes, &mut materials, levels, &centres);

    spawn_highlight(&mut commands, &mut meshes, &marker, diagnosis.highlight);

    if let Some((path, failure)) = walk {
        spawn_walk(&mut commands, &mut meshes, &mut materials, &path, failure);
    }

    state.cut_hulls = cut;
}

/// Draw the walked path, and mark where it stopped.
///
/// Lifted clear of the floor so it reads as an annotation over the surface
/// rather than as geometry lying on it. The path is drawn in the settled
/// colour and the stopping point in the alert colour - the same pair the
/// viewport already uses for "this holds" and "this does not".
fn spawn_walk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    path: &[Vec3],
    failure: Option<crate::module::walk::WalkFailure>,
) {
    const LIFT: f32 = 0.12;
    let trail = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Pinned).base_color,
        emissive: schematic(SchematicRole::Pinned).emissive,
        unlit: true,
        ..default()
    });
    for pair in path.windows(2) {
        spawn_edge(
            commands,
            meshes,
            &trail,
            pair[0] + Vec3::Y * LIFT,
            pair[1] + Vec3::Y * LIFT,
        );
    }

    // A walk that stopped must show *where*. Drawing only the path would make a
    // route that got 20% along look like a route that finished.
    if let Some(failure) = failure {
        let stop = materials.add(StandardMaterial {
            base_color: schematic(SchematicRole::Volatile).base_color,
            emissive: schematic(SchematicRole::Volatile).emissive,
            unlit: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.30).mesh().ico(2).unwrap())),
            MeshMaterial3d(stop),
            Transform::from_translation(failure.at() + Vec3::Y * LIFT),
            ModuleVisual,
        ));
    }
}

/// Whether a hull survives the cutaway, measured against the nearest cell.
///
/// The classifier is [`crate::detail::survives`] - the same one the facility
/// view uses - so a module and the facility it lands in cut away identically.
/// A second rule here would let a module look right in one tool and wrong in
/// the other.
///
/// The nearest-cell part matters for whole-room modules. `survives` judges the
/// interior-fitting exemption by plan distance from *its cell's* origin, and a
/// room's hulls all live in one module-local frame, so a pillar in the cell at
/// `(1, 0)` is 14 m from the module origin and would be culled as a perimeter
/// wall. Measuring from the closest cell centre restores the per-cell meaning.
#[must_use]
pub fn hull_survives(hull: &[Vec3], centres: &[Vec3], bearing: Vec2, cutaway: bool) -> bool {
    let (min_y, max_y, centroid) = crate::detail::measure(hull);
    let local = nearest_local(centroid, centres);
    crate::detail::survives(min_y, max_y, local, bearing, cutaway)
}

/// `centroid` relative to whichever cell centre it is closest to, in plan.
#[must_use]
pub fn nearest_local(centroid: Vec3, centres: &[Vec3]) -> Vec3 {
    let plan = Vec2::new(centroid.x, centroid.z);
    centres
        .iter()
        .min_by(|a, b| {
            let da = plan.distance_squared(Vec2::new(a.x, a.z));
            let db = plan.distance_squared(Vec2::new(b.x, b.z));
            da.total_cmp(&db)
        })
        .map_or(centroid, |centre| {
            centroid - Vec3::new(centre.x, 0.0, centre.z)
        })
}

/// Where this module's cells sit, in module-local metres.
///
/// Taken from the validated footprint when there is one, the recipe when there
/// is not, and the origin as the last resort. The fallback chain matters: the
/// moment you most want a cutaway is when validation failed, and that is
/// exactly when `module` is `None`.
#[must_use]
pub fn cell_centres(diagnosis: &crate::module::diagnose::Diagnosis) -> Vec<Vec3> {
    if let Some(module) = diagnosis.module.as_ref() {
        let centres: Vec<Vec3> = module
            .footprint
            .iter()
            .map(|cell| plan_origin(i32::from(cell.q), i32::from(cell.r)))
            .collect();
        if !centres.is_empty() {
            return centres;
        }
    }
    if let Some(recipe) = diagnosis.recipe.as_ref() {
        let centres: Vec<Vec3> = recipe
            .cells
            .iter()
            .map(|cell| plan_origin(cell.q, cell.r))
            .collect();
        if !centres.is_empty() {
            return centres;
        }
    }
    vec![Vec3::ZERO]
}

/// Plan origin of cell `(q, r)` in world metres.
///
/// The same lattice `observed_hex::hex_origin_plan` defines, spelled out here
/// because that function takes a `HexCoord` whose `q`/`r` are `u16` and a
/// module footprint is `i16` - a room may legitimately sit at a negative
/// offset from its anchor. Pinned against the canonical version by a test.
#[must_use]
pub fn plan_origin(q: i32, r: i32) -> Vec3 {
    #[allow(clippy::cast_precision_loss)]
    Vec3::new((q * 14 + r * 7) as f32, 0.0, (r * 12) as f32)
}

/// The canonical hex prism, as edges. Drawn from `observed_hex::CORNERS` so it
/// is the same boundary the validator tests against, not an approximation of
/// it.
fn spawn_cell_outline(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    levels: u8,
    centres: &[Vec3],
) {
    let outline = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Grid).base_color.with_alpha(0.5),
        emissive: schematic(SchematicRole::Grid).emissive * 0.25,
        unlit: true,
        ..default()
    });
    let top = TILE_LEVEL_HEIGHT * f32::from(levels);
    // One prism per footprint cell. A single hexagon under a two-cell room
    // draws a boundary half the size of the one the module is judged against,
    // which is worse than drawing none.
    for centre in centres {
        for level in 0..=u32::from(levels) {
            #[allow(clippy::cast_precision_loss)]
            let y = TILE_LEVEL_HEIGHT * level as f32;
            for index in 0..CORNERS.len() {
                let a = CORNERS[index];
                let b = CORNERS[(index + 1) % CORNERS.len()];
                spawn_edge(
                    commands,
                    meshes,
                    &outline,
                    *centre + plan(a, y),
                    *centre + plan(b, y),
                );
            }
        }
        for corner in CORNERS {
            spawn_edge(
                commands,
                meshes,
                &outline,
                *centre + plan(corner, 0.0),
                *centre + plan(corner, top),
            );
        }
    }
}

/// Put the annotation where the fault is.
fn spawn_highlight(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    highlight: Highlight,
) {
    match highlight {
        Highlight::Vertex(point) => {
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(VERTEX_MARKER_RADIUS).mesh().ico(2).unwrap())),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(point),
                ModuleVisual,
            ));
        }
        Highlight::Face { face, .. } => {
            // Lateral faces get their edge drawn full height; up/down get the
            // whole cap ring, because "the top face" has no single edge.
            if let Some([a, b]) = lateral_edge(face) {
                for level in 0..2 {
                    let y = TILE_LEVEL_HEIGHT * level as f32 + HIGHLIGHT_LIFT;
                    spawn_edge(commands, meshes, material, plan(a, y), plan(b, y));
                }
                spawn_edge(
                    commands,
                    meshes,
                    material,
                    plan(a, 0.0),
                    plan(a, TILE_LEVEL_HEIGHT),
                );
                spawn_edge(
                    commands,
                    meshes,
                    material,
                    plan(b, 0.0),
                    plan(b, TILE_LEVEL_HEIGHT),
                );
            } else {
                let y = if face == HexFace::Up {
                    TILE_LEVEL_HEIGHT
                } else {
                    0.0
                } + HIGHLIGHT_LIFT;
                for index in 0..CORNERS.len() {
                    let a = CORNERS[index];
                    let b = CORNERS[(index + 1) % CORNERS.len()];
                    spawn_edge(commands, meshes, material, plan(a, y), plan(b, y));
                }
            }
        }
        Highlight::Cell(_) => {
            // Single-cell view, so the cell reference is always the one on
            // screen: ring its floor rather than pretending to offset it.
            for index in 0..CORNERS.len() {
                let a = CORNERS[index];
                let b = CORNERS[(index + 1) % CORNERS.len()];
                spawn_edge(
                    commands,
                    meshes,
                    material,
                    plan(a, HIGHLIGHT_LIFT),
                    plan(b, HIGHLIGHT_LIFT),
                );
            }
        }
        // Already handled by recolouring the hull itself.
        Highlight::Hull(_) => {}
        // Nothing in the geometry to point at; the panel carries the message.
        Highlight::Whole => {}
    }
}

/// The two plan-space corners of a lateral face, or `None` for up/down.
#[must_use]
pub fn lateral_edge(face: HexFace) -> Option<[(i32, i32); 2]> {
    face.is_lateral().then(|| face_edge(face))
}

/// A thin box standing in for a line. Bevy has no thick-line primitive, and a
/// gizmo would not survive a screenshot capture.
fn spawn_edge(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    from: Vec3,
    to: Vec3,
) {
    let delta = to - from;
    let length = delta.length();
    if length < f32::EPSILON {
        return;
    }
    let mesh = meshes.add(Cuboid::new(0.08, 0.08, length).mesh());
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(from + delta * 0.5).looking_to(delta.normalize(), Vec3::Y),
        ModuleVisual,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every lateral face must resolve to a real edge, and the caps must not
    /// pretend to have one. A silent `None` on a lateral face would drop the
    /// highlight for exactly the ports authors get wrong most.
    #[test]
    fn every_lateral_face_has_an_edge_and_the_caps_do_not() {
        for face in HexFace::ALL {
            let edge = lateral_edge(face);
            assert_eq!(
                edge.is_some(),
                face.is_lateral(),
                "{face:?} disagrees with its own laterality"
            );
            if let Some([a, b]) = edge {
                assert_ne!(a, b, "{face:?} has a degenerate edge");
            }
        }
    }

    /// The outline is drawn from the same corner table the validator enforces.
    ///
    /// The quantized hexagon is deliberately **not** regular - east and west sit
    /// 7 m out while north and south reach 8 - so this asserts those two spans
    /// rather than a single radius. A regular-hexagon assumption here is exactly
    /// the sub-metre gap the quantization exists to avoid.
    #[test]
    fn the_outline_uses_the_canonical_quantized_hexagon() {
        assert_eq!(CORNERS.len(), 6);
        let widest = CORNERS.iter().map(|corner| corner.0.abs()).max();
        let tallest = CORNERS.iter().map(|corner| corner.1.abs()).max();
        assert_eq!(widest, Some(7), "across flats must stay 14 m");
        assert_eq!(tallest, Some(8), "across corners must stay 16 m");
    }

    /// The plan conversion maps editor axes to world axes, not to themselves.
    /// Swapping y and z here would draw every outline rotated flat.
    #[test]
    fn plan_puts_the_hexagon_in_the_ground_plane() {
        let point = plan((7, -4), 2.5);
        assert!((point.x - 7.0).abs() < 1e-6);
        assert!((point.y - 2.5).abs() < 1e-6, "height must be Y");
        assert!((point.z + 4.0).abs() < 1e-6, "plan depth must be Z");
    }

    /// The plan formula must match the lattice the rest of the repo uses.
    /// Spelled out locally only because `hex_origin_plan` takes `u16`, so this
    /// pins the two together for every coordinate both can express.
    #[test]
    fn plan_origin_matches_the_canonical_lattice() {
        use observed_hex::{HexCoord, hex_origin_plan};
        for (q, r) in [(0, 0), (1, 0), (0, 1), (2, 3), (5, 1)] {
            let canonical = hex_origin_plan(HexCoord {
                q: q as u16,
                r: r as u16,
                level: 0,
            });
            let mine = plan_origin(q, r);
            #[allow(clippy::cast_precision_loss)]
            let expected = Vec3::new(canonical.0 as f32, 0.0, canonical.1 as f32);
            assert_eq!(mine, expected, "cell ({q}, {r}) disagrees with the lattice");
        }
    }

    /// With the cutaway off, nothing is hidden. A classifier that dropped a
    /// hull even when disabled would be invisible until someone noticed a wall
    /// missing.
    #[test]
    fn nothing_is_cut_when_the_cutaway_is_off() {
        let wall = vec![
            Vec3::new(7.0, 0.0, -4.0),
            Vec3::new(7.0, 0.0, 4.0),
            Vec3::new(7.0, 8.0, 4.0),
            Vec3::new(6.5, 8.0, -4.0),
        ];
        let centres = [Vec3::ZERO];
        for detent in 0..crate::viewport::AZIMUTH_DETENTS {
            let bearing = crate::viewport::detent_bearing(detent);
            assert!(hull_survives(&wall, &centres, bearing, false));
        }
    }

    /// The cutaway has to actually cut. A wall on the camera side goes; the
    /// same wall from the far side stays.
    #[test]
    fn a_near_wall_is_cut_and_a_far_wall_is_not() {
        let east_wall = vec![
            Vec3::new(7.0, 0.6, -4.0),
            Vec3::new(7.0, 0.6, 4.0),
            Vec3::new(7.0, 7.0, 4.0),
            Vec3::new(6.5, 7.0, -4.0),
        ];
        let centres = [Vec3::ZERO];
        let toward_east = Vec2::new(1.0, 0.0);
        let toward_west = Vec2::new(-1.0, 0.0);
        assert!(
            !hull_survives(&east_wall, &centres, toward_east, true),
            "a wall between the viewer and the interior must be cut"
        );
        assert!(
            hull_survives(&east_wall, &centres, toward_west, true),
            "the far wall is what you are looking at through the gap"
        );
    }

    /// The per-cell fix: a fitting in a room's second cell is 14 m from the
    /// module origin, and judged against that origin it reads as perimeter.
    /// Measured against its own cell it is interior, and stays.
    #[test]
    fn a_fitting_in_a_second_cell_is_not_mistaken_for_a_wall() {
        let cell = plan_origin(1, 0);
        let pillar: Vec<Vec3> = [
            (-0.5, 0.6, -0.5),
            (0.5, 0.6, -0.5),
            (0.5, 0.6, 0.5),
            (-0.5, 7.0, 0.5),
        ]
        .into_iter()
        .map(|(x, y, z)| cell + Vec3::new(x, y, z))
        .collect();

        let bearing = Vec2::new(1.0, 0.0);
        assert!(
            !hull_survives(&pillar, &[Vec3::ZERO], bearing, true),
            "this test is vacuous unless the origin-only measure would cut it"
        );
        assert!(
            hull_survives(&pillar, &[Vec3::ZERO, cell], bearing, true),
            "measured against its own cell the pillar is an interior fitting"
        );
    }
}
