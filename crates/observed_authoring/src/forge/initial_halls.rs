//! Complete flat-hall kits for initial district compositions.
//! Interfaces remain ordinary doors; the initial selector owns where the kits
//! form connected beats. Architect replacements use the ordinary hall lottery.
use super::GENERATED_NOTE;
use super::entities::{
    Meta, ceiling_fixture, lateral_port, tile_cell_default, wall_fixture, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, band, boxed, corners, door_wall_default, face_mid, hex_slab, prism, wall,
};

pub const GALLERY_BASE: u16 = 2000;
pub const COURT_BASE: u16 = 2100;
const TERRACE_GALLERY_BASE: u16 = 2200;
const TERRACE_COURT_BASE: u16 = 2300;
pub const TARGETS: [(&str, bool); 5] = [
    ("infinite_gallery", false),
    ("overlit_grid", false),
    ("shadow_screen", false),
    ("facet_monument", false),
    ("facet_monument", true),
];
mod dressing;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitialHallKind {
    Gallery,
    Court,
}

impl InitialHallKind {
    #[must_use]
    pub const fn base(self) -> u16 {
        match self {
            Self::Gallery => GALLERY_BASE,
            Self::Court => COURT_BASE,
        }
    }
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Gallery => "gallery",
            Self::Court => "court",
        }
    }
}

fn rotate(mask: u8) -> u8 {
    ((mask << 1) | (mask >> 5)) & 63
}
fn canonical(mask: u8) -> u8 {
    (0..6)
        .fold((mask, mask), |(best, at), _| (best.min(at), rotate(at)))
        .0
}
fn archetype(mask: u8) -> &'static str {
    if mask.count_ones() == 2 {
        let bits = (0..6)
            .filter(|face| mask & (1 << face) != 0)
            .collect::<Vec<_>>();
        let difference: usize = bits[1] - bits[0];
        match difference.min(6 - difference) {
            3 => "hall_straight",
            2 => "hall_turn_120",
            _ => "hall_turn_60",
        }
    } else if mask.count_ones() == 3 {
        "hall_junction_3way"
    } else {
        "hall_junction_4way"
    }
}

/// Reserved physical module IDs, deliberately excluded from the ordinary
/// lottery. Only an explicit initial choice may select one of these modules.
#[must_use]
pub fn kind_for_key(key: &crate::TileKey) -> Option<InitialHallKind> {
    definition_for_key(key).map(|(kind, _)| kind)
}

#[must_use]
pub fn is_terrace_key(key: &crate::TileKey) -> bool {
    definition_for_key(key).is_some_and(|(_, terrace)| terrace)
}

fn definition_for_key(key: &crate::TileKey) -> Option<(InitialHallKind, bool)> {
    if !matches!(
        key.register.as_str(),
        "infinite_gallery" | "overlit_grid" | "shadow_screen" | "facet_monument"
    ) {
        return None;
    }
    if !matches!(
        key.archetype.as_str(),
        "hall_straight"
            | "hall_turn_60"
            | "hall_turn_120"
            | "hall_junction_3way"
            | "hall_junction_4way"
    ) {
        return None;
    }
    let source = key.variant / 6;
    [
        (InitialHallKind::Gallery, false),
        (InitialHallKind::Court, false),
        (InitialHallKind::Gallery, true),
        (InitialHallKind::Court, true),
    ]
    .into_iter()
    .find(|&(kind, terrace)| {
        if terrace && key.register != "facet_monument" {
            return false;
        }
        source
            .checked_sub(source_base(kind, terrace))
            .and_then(|mask| u8::try_from(mask).ok())
            .is_some_and(|mask| {
                mask < 64 && (2..=4).contains(&mask.count_ones()) && canonical(mask) == mask
            })
    })
}

fn source_base(kind: InitialHallKind, terrace: bool) -> u16 {
    match (kind, terrace) {
        (_, false) => kind.base(),
        (InitialHallKind::Gallery, true) => TERRACE_GALLERY_BASE,
        (InitialHallKind::Court, true) => TERRACE_COURT_BASE,
    }
}

fn name_for(register: &str, kind: InitialHallKind, mask: u8, terrace: bool) -> String {
    let district = match register {
        "infinite_gallery" => "library",
        "overlit_grid" => "lumen",
        "shadow_screen" => "zen",
        "facet_monument" => {
            if terrace {
                "monument_terrace"
            } else {
                "monument"
            }
        }
        _ => unreachable!("initial district"),
    };
    format!("initial_{district}_{}_m{mask:02}", kind.slug())
}

fn trim(brush: String) -> String {
    brush.replace("__TB_empty", "observed_trim")
}

fn build(register: &str, kind: InitialHallKind, mask: u8, terrace: bool) -> String {
    let library = register == "infinite_gallery";
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    if !terrace {
        brushes.push_str(&hex_slab(120.0, LEVEL, 0.0, 0.0));
    }
    for face in 0..6 {
        if mask & (1 << face) != 0 {
            brushes.push_str(&door_wall_default(face, 0.0, LEVEL));
        } else {
            brushes.push_str(&wall(face, 0.0, if terrace { 24.0 } else { LEVEL }));
            if library {
                let courses: &[f64] = if kind == InitialHallKind::Gallery {
                    &[24.0, 44.0, 64.0]
                } else {
                    &[24.0, 48.0, 72.0, 96.0]
                };
                for &z in courses {
                    brushes.push_str(&trim(band(face, 8.0, 22.0, z, z + 3.0)));
                }
            } else if register == "overlit_grid" && kind == InitialHallKind::Court {
                brushes.push_str(&band(face, 8.0, 20.0, FLOOR_TOP, 19.2));
                brushes.push_str(&trim(band(face, 7.0, 21.0, 19.2, 21.0)));
            }
            brushes.push_str(&dressing::bay(register, kind, face, terrace));
            let (x, y) = face_mid(face);
            let scale = if kind == InitialHallKind::Gallery {
                0.75
            } else {
                0.88
            };
            let (x, y) = (x * scale, y * scale);
            let top = if kind == InitialHallKind::Gallery {
                88.0
            } else if terrace {
                72.0
            } else {
                120.0
            };
            brushes.push_str(&trim(boxed(
                (x - 4.0, y - 4.0, FLOOR_TOP),
                (x + 4.0, y + 4.0, top),
            )));
        }
    }
    let ceiling = if kind == InitialHallKind::Gallery {
        let plan = corners().map(|(x, y)| (x * 0.80, y * 0.80));
        // The canopy rests on the sealed-bay piers; its exposed underside
        // stays separate from the outer cap and preserves standing clearance.
        brushes.push_str(
            &prism(&plan, 88.0, 92.0, None, 0.0, 0.0).replace("__TB_empty", "observed_ceiling"),
        );
        // A centre hanger joins the canopy to the outer cap. Its centroid
        // stays outside wall sectors, so opening a flank never removes the
        // last support along with the sealed-bay piers.
        if !terrace {
            brushes.push_str(&trim(boxed((-5.0, -5.0, 92.0), (5.0, 5.0, 120.0))));
        }
        88.0
    } else {
        120.0
    };
    let (fixture, lights) = if terrace && kind == InitialHallKind::Court {
        wall_fixture(0, 0.05, 76.0, 16.0)
    } else {
        ceiling_fixture(0.0, 0.0, ceiling, 28.0, 7.0)
    };
    brushes.push_str(&fixture);
    let name = name_for(register, kind, mask, terrace);
    let mut out = format!("// Initial mutable hall composition: {name}.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{name}"),
            archetype(mask),
            i32::from(source_base(kind, terrace)) + i32::from(mask),
            1,
            1,
        )
        .with_register_scope(register)
        .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in 0..6 {
        if mask & (1 << face) != 0 {
            out.push_str(&lateral_port(
                face,
                "door",
                &format!("initial_{face}"),
                0,
                0,
                0,
            ));
        }
    }
    out.push_str(&lights);
    out
}

#[must_use]
pub fn generated() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (register, terrace) in TARGETS {
        for kind in [InitialHallKind::Gallery, InitialHallKind::Court] {
            for mask in 0..64u8 {
                if (2..=4).contains(&mask.count_ones()) && canonical(mask) == mask {
                    let name = name_for(register, kind, mask, terrace);
                    out.push((name, build(register, kind, mask, terrace)));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Vec2, Vec3};
    use observed_hex::{HexFace, PortClass, face_edge};
    use observed_traversal::rapier_controller::{
        RapierTraversalScene, step_character_with_settings, step_solid_character_with_settings,
    };
    use observed_traversal::{FpsBody, TraversalRuntimeProfile};
    use player_input::PlayerIntent;

    fn runtime() -> crate::RuntimeAuthoringCatalog {
        crate::build_catalog(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles"),
        )
        .expect("catalog")
        .catalog
        .runtime_catalog(&[
            "infinite_gallery",
            "overlit_grid",
            "shadow_screen",
            "facet_monument",
        ])
        .expect("runtime")
    }

    #[test]
    fn initial_kits_reproduce_and_cover_every_flat_hall_interface() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles/authored");
        let generated = generated();
        assert_eq!(generated.len(), 100);
        for (name, source) in generated {
            assert_eq!(
                std::fs::read_to_string(root.join(format!("{name}.map")))
                    .expect("source")
                    .replace("\r\n", "\n"),
                source
            );
            let module = crate::parse_authored_module(&source).expect("valid source");
            assert!(module.prototype.hulls.len() <= 45);
            assert!(
                module
                    .prototype
                    .lights
                    .iter()
                    .all(|light| light.attachment.is_some())
            );
        }
        let runtime = runtime();
        for (register, terrace) in TARGETS {
            for kind in [InitialHallKind::Gallery, InitialHallKind::Court] {
                for demand in observed_facility::hex_wfc::geometry_demands()
                    .into_iter()
                    .filter(|demand| {
                        matches!(
                            demand.archetype,
                            "hall_straight"
                                | "hall_turn_60"
                                | "hall_turn_120"
                                | "hall_junction_3way"
                                | "hall_junction_4way"
                        )
                    })
                {
                    assert!(
                        runtime
                            .cells
                            .iter()
                            .any(|tile| tile.key.register == register
                                && tile.key.archetype == demand.archetype
                                && tile.signature == demand.signature
                                && kind_for_key(&tile.key) == Some(kind)
                                && is_terrace_key(&tile.key) == terrace),
                        "{register} {kind:?}: missing {demand:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn observers_and_majors_walk_every_initial_module_door_pair() {
        let profile = TraversalRuntimeProfile::canonical_hex();
        for tile in runtime()
            .cells
            .iter()
            .filter(|tile| kind_for_key(&tile.key).is_some())
        {
            let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
            let doors = HexFace::LATERAL
                .into_iter()
                .filter(|&face| tile.signature.port(face) == PortClass::Door)
                .collect::<Vec<_>>();
            let threshold = |face| {
                let [a, b] = face_edge(face);
                Vec3::new(
                    f32::from(i16::try_from(a.0 + b.0).expect("local corner")) * 0.42,
                    0.5,
                    f32::from(i16::try_from(a.1 + b.1).expect("local corner")) * 0.42,
                )
            };
            for solid in [false, true] {
                let mut config = profile.controller();
                if solid {
                    config.radius = 1.35;
                    config.half_height = 1.4;
                    config.walk_speed = 7.0;
                }
                for &entry in &doors {
                    for &exit in &doors {
                        if entry == exit {
                            continue;
                        }
                        let mut body = FpsBody::spawned(
                            threshold(entry) + Vec3::Y * (config.half_height + 0.03),
                            0.0,
                        );
                        for goal in [Vec3::new(0.0, 0.5, 0.0), threshold(exit)] {
                            let mut arrived = false;
                            for _ in 0..900 {
                                let feet = body.position - Vec3::Y * config.half_height;
                                assert!(feet.y >= 0.49, "lost support on {:?}", tile.key);
                                if feet.distance(goal) < 0.25 {
                                    arrived = true;
                                    break;
                                }
                                let delta = goal - feet;
                                body.yaw = delta.x.atan2(-delta.z);
                                let intent = PlayerIntent {
                                    movement: Vec2::Y
                                        * Vec2::new(delta.x, delta.z).length().min(1.0),
                                    ..PlayerIntent::default()
                                };
                                let report = if solid {
                                    step_solid_character_with_settings(
                                        &scene,
                                        &mut body,
                                        intent,
                                        &config,
                                        profile.rapier(),
                                        1.0 / 60.0,
                                    )
                                } else {
                                    step_character_with_settings(
                                        &scene,
                                        &mut body,
                                        intent,
                                        &config,
                                        profile.rapier(),
                                        1.0 / 60.0,
                                    )
                                };
                                assert!(!report.jumped && !report.recovered);
                            }
                            assert!(
                                arrived,
                                "{:?}, solid={solid}, {entry:?}->{exit:?}: blocked at {:?}",
                                tile.key, body.position
                            );
                        }
                    }
                }
            }
        }
    }
}
