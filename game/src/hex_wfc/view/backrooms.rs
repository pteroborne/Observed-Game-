//! Shallow construction fitted to the actual Backrooms hulls, never inferred
//! from a nominal hex. Decoration belongs to its streamed cell parent.
use super::{
    assets::HexWfcVisualAssets, mesh_group::MeshGroupKey, spectate::Cutaway, support::points,
};
use bevy::prelude::*;
use observed_authoring::HullSurface;
use observed_content::ArchitectureRegister;
use observed_hex::{HexCoord, hex_origin};
use observed_match::hex_wfc::HexStructurePiece;

fn strip(a: Vec2, b: Vec2, bottom: f32, height: f32, thickness: f32) -> Vec<Vec3> {
    let normal = Vec2::new(-(b - a).y, (b - a).x).normalize_or_zero() * thickness * 0.5;
    [a - normal, b - normal, b + normal, a + normal]
        .into_iter()
        .flat_map(|p| {
            [
                Vec3::new(p.x, bottom, p.y),
                Vec3::new(p.x, bottom + height, p.y),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_real_beveled_straight_cap_keeps_its_fluorescent_field() {
        let runtime = crate::hex_wfc::view::tests::test_runtime();
        let coord = runtime
            .match_state
            .facility
            .placements
            .values()
            .find(|p| {
                p.coord.level == 0
                    && p.archetype == observed_facility::hex_wfc::HexArchetype::Straight
            })
            .unwrap()
            .coord;
        let pieces = runtime
            .match_state
            .geometry
            .pieces
            .iter()
            .filter(|p| p.source_cell == coord)
            .collect::<Vec<_>>();
        let mut world = World::new();
        let parent = world.spawn_empty().id();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let mut assets = HexWfcVisualAssets::for_test(&mut materials);
        let decoration = spawn(
            &mut commands,
            &mut assets,
            &mut meshes,
            parent,
            coord,
            &pieces,
        );
        queue.apply(&mut world);
        assert!(
            decoration.fluorescent_field,
            "beveled source ceilings must receive the panel field"
        );
        assert!(
            world
                .query::<&Name>()
                .iter(&world)
                .any(|name| name.as_str() == "Backrooms fluorescent field")
        );
    }
}

/// Clip one grid rail to a real convex ceiling footprint.
fn grid_line(plan: &[Vec2], across: bool, fixed: f32) -> Option<(Vec2, Vec2)> {
    let mut hits = Vec::new();
    for index in 0..plan.len() {
        let a = plan[index];
        let b = plan[(index + 1) % plan.len()];
        let (u, v) = if across { (a.y, b.y) } else { (a.x, b.x) };
        if (u - fixed) * (v - fixed) <= 0.0 && (v - u).abs() > 1e-5 {
            hits.push(a.lerp(b, (fixed - u) / (v - u)));
        }
    }
    hits.sort_by(|a, b| {
        if across {
            a.x.total_cmp(&b.x)
        } else {
            a.y.total_cmp(&b.y)
        }
    });
    let (a, b) = (*hits.first()?, *hits.last()?);
    (a.distance(b) > 0.05).then_some((a, b))
}

#[derive(Default)]
pub(super) struct Decoration {
    pub(super) meshes: usize,
    pub(super) fluorescent_field: bool,
}

pub(super) fn spawn(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coord: HexCoord,
    pieces: &[&HexStructurePiece],
) -> Decoration {
    let origin = Vec3::from_array(hex_origin(coord));
    let mut skirts = Vec::new();
    let mut rails = Vec::new();
    let mut panels = Vec::new();
    for piece in pieces {
        let hull = points(piece, origin);
        if hull.is_empty() {
            continue;
        }
        let lo = hull
            .iter()
            .copied()
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let hi = hull
            .iter()
            .copied()
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        let plan = observed_traversal::plan_convex_hull(&hull);
        if hi.y - lo.y > 1.8 && lo.y <= 0.55 && hi.y >= 2.0 {
            // Every exposed wall/column edge has its own cove base. The thin
            // overlap into its support avoids cracks without affecting bodies.
            for i in 0..plan.len() {
                let (a, b) = (plan[i], plan[(i + 1) % plan.len()]);
                if a.distance(b) > 0.15 {
                    skirts.push(strip(a, b, lo.y.max(0.5), 0.12, 0.035));
                }
            }
        }
        let area = plan
            .iter()
            .enumerate()
            .map(|(i, a)| a.perp_dot(plan[(i + 1) % plan.len()]))
            .sum::<f32>()
            .abs()
            * 0.5;
        if piece.surface == Some(HullSurface::Ceiling)
            && (3.45..3.70).contains(&lo.y)
            && area > 16.0
        {
            // World phase persists across tile origins and rotations. The
            // raster's joints use the same 600 x 1200 mm construction module.
            for (across, pitch, offset, min, max) in [
                (true, 1.2, origin.z, lo.z, hi.z),
                (false, 0.6, origin.x, lo.x, hi.x),
            ] {
                let first = ((min + offset) / pitch).ceil() as i32;
                let last = ((max + offset) / pitch).floor() as i32;
                for index in first..=last {
                    if let Some((a, b)) = grid_line(&plan, across, index as f32 * pitch - offset) {
                        rails.push(strip(a, b, lo.y - 0.018, 0.022, 0.024));
                    }
                }
            }
            for i in 0..plan.len() {
                rails.push(strip(
                    plan[i],
                    plan[(i + 1) % plan.len()],
                    lo.y - 0.02,
                    0.025,
                    0.04,
                ));
            }
            let inside = |point: Vec2| {
                (0..plan.len()).all(|i| {
                    let a = plan[i];
                    let b = plan[(i + 1) % plan.len()];
                    (b - a).perp_dot(point - a) >= -0.001
                })
            };
            for column in (((lo.x + origin.x) / 0.6).ceil() as i32)
                ..(((hi.x + origin.x) / 0.6).floor() as i32)
            {
                for row in (((lo.z + origin.z) / 1.2).ceil() as i32)
                    ..(((hi.z + origin.z) / 1.2).floor() as i32)
                {
                    if (column + row).rem_euclid(3) != 0 {
                        continue;
                    }
                    let a = Vec2::new(
                        column as f32 * 0.6 - origin.x + 0.025,
                        row as f32 * 1.2 - origin.z + 0.025,
                    );
                    let b = a + Vec2::new(0.55, 1.15);
                    if [a, b, Vec2::new(a.x, b.y), Vec2::new(b.x, a.y)]
                        .into_iter()
                        .all(inside)
                    {
                        panels.push(strip(
                            Vec2::new((a.x + b.x) * 0.5, a.y),
                            Vec2::new((a.x + b.x) * 0.5, b.y),
                            lo.y - 0.025,
                            0.012,
                            0.55,
                        ));
                    }
                }
            }
        }
    }
    // Overlapping authored cap segments must not duplicate visible construction.
    let mut seen = std::collections::BTreeSet::new();
    rails.retain(|hull| {
        let mut key = hull
            .iter()
            .map(|p| {
                [
                    (p.x * 1000.0).round() as i32,
                    (p.y * 1000.0).round() as i32,
                    (p.z * 1000.0).round() as i32,
                ]
            })
            .collect::<Vec<_>>();
        key.sort();
        seen.insert(key)
    });
    let mut decoration = Decoration::default();
    for (name, hulls) in [
        ("cove base", skirts),
        ("ceiling T-bars", rails),
        ("fluorescent field", panels),
    ] {
        if hulls.is_empty() {
            continue;
        }
        let refs = hulls.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let key = format!(
            "backrooms-{name}-{:?}-{:?}",
            pieces.first().and_then(|piece| piece.tile.as_ref()),
            if name != "cove base" {
                (coord.q % 9, coord.r % 9)
            } else {
                (0, 0)
            }
        );
        let Some(mesh) = assets.merged_mesh_for(meshes, Some(&key), MeshGroupKey::Trim, &refs)
        else {
            continue;
        };
        let min_y = hulls
            .iter()
            .flatten()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);
        let max_y = hulls
            .iter()
            .flatten()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        let field = name == "fluorescent field";
        let material = if field {
            assets.register(ArchitectureRegister::LiminalGrid).fixture()
        } else {
            assets.material_for_group(ArchitectureRegister::LiminalGrid, MeshGroupKey::Trim)
        };
        let mut entity = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(origin),
            ChildOf(parent),
            Cutaway {
                local: Vec3::ZERO,
                min_y,
                max_y,
                origin_y: origin.y,
                cell_level: coord.level,
                climb_wall: false,
            },
            Name::new(format!("Backrooms {name}")),
        ));
        entity.insert((bevy::light::NotShadowCaster, super::NeverShadowCaster));
        if name != "cove base" {
            entity.insert(super::spectate::CeilingCutaway);
        }
        if field {
            entity.insert(super::HexPractical(coord));
        }
        decoration.meshes += 1;
        decoration.fluorescent_field |= field;
    }
    decoration
}
