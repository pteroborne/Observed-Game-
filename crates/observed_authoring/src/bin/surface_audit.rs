//! Offline evidence of coplanar competing overhead faces in the production corpus.
use glam::{Vec2, Vec3};
use observed_traversal::ConvexRenderMesh;

fn main() {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/tiles".into());
    let build = observed_authoring::build_catalog(std::path::Path::new(&root))
        .expect("valid source corpus");
    let mut modules = 0;
    let mut total = 0;
    for module in &build.catalog.modules {
        let hulls = &build
            .catalog
            .hull_sets
            .iter()
            .find(|h| h.structural_hash == module.structural_hash)
            .expect("hull set")
            .hulls;
        let mut triangles = Vec::new();
        for (owner, hull) in hulls.iter().enumerate() {
            let points: Vec<_> = hull.iter().copied().map(Vec3::from_array).collect();
            let Some(mesh) = ConvexRenderMesh::from_convex_hull(&points) else {
                continue;
            };
            for t in mesh.indices.chunks_exact(3) {
                let p = t
                    .iter()
                    .map(|i| Vec3::from_array(mesh.positions[*i as usize]))
                    .collect::<Vec<_>>();
                let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
                if n.y.abs() > 0.99 && p[0].y > 2.8 {
                    triangles.push((owner, p, n));
                }
            }
        }
        let mut conflicts = 0;
        for (i, (owner, a, n)) in triangles.iter().enumerate() {
            let center = (a[0] + a[1] + a[2]) / 3.0;
            if triangles[..i].iter().any(|(other, b, m)| {
                if owner == other || n.dot(*m) < 0.999 || (center - b[0]).dot(*m).abs() > 0.0001 {
                    return false;
                }
                let point = Vec2::new(center.x, center.z);
                let b = b.iter().map(|p| Vec2::new(p.x, p.z)).collect::<Vec<_>>();
                let signs =
                    [(0, 1), (1, 2), (2, 0)].map(|(i, j)| (b[j] - b[i]).perp_dot(point - b[i]));
                signs.iter().all(|s| *s > 0.0001) || signs.iter().all(|s| *s < -0.0001)
            }) {
                conflicts += 1;
            }
        }
        if conflicts > 0 {
            modules += 1;
            total += conflicts;
            println!(
                "{}\t{} competing overhead triangle centroids",
                module.id, conflicts
            );
        }
    }
    println!(
        "{} modules inspected; {modules} with overhead conflicts; {total} competing samples",
        build.catalog.modules.len()
    );
}
