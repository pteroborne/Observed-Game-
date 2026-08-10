//! What the cutaway is cutting, where the followed body stands, and which body
//! is which.
//!
//! The cutaway itself is a *subtraction*: [`observed_style::iso::survives`]
//! removes the ceiling and the three walls between the camera and the interior,
//! and the storey filter removes every hull whose middle is on another floor.
//! Subtraction reads beautifully - it is the one thing in this project a
//! playtester praised unprompted - and it is silent about its own premises. A
//! viewer sees a floor plan hanging in black with no way to answer three
//! questions the picture is entirely made of:
//!
//! 1. **What is being cut.** That a cut is happening at all, at what height, and
//!    which of ten storeys is on show.
//! 2. **Where the viewer is.** The followed body has no visual at all -
//!    `entities::setup` deliberately skips `local_player`, because in play you
//!    are inside its head. Pulled back, that leaves the view following something
//!    invisible.
//! 3. **Which bodies are which.** Rivals are 0.25 m capsules at 46% alpha, sized
//!    for arm's length in a corridor. From forty metres out they are a few dark
//!    pixels, and nothing says whose they are.
//!
//! # The marks are derived from the cut, never parallel to it
//!
//! Everything here is a function of the same detent bearing and the same storey
//! the hulls are filtered with, reached from inside `sync_cutaway` rather than
//! from a system beside it. That is deliberate, and it is the lesson
//! `OverviewFrame` already taught this module: two derivations of one fact
//! drift, and when they do the annotation lies about the picture it annotates.
//! The section ring drops the same three of six sides the cutaway drops walls
//! on - **the hoop is open exactly where the building is open** - because it
//! asks the near-arc question with the same vector.
//!
//! # Affordable by construction
//!
//! Two meshes, five materials, and on the order of thirty entities: up to twelve
//! ring bars, a storey ladder, and three small parts per body inside the frame.
//! Nothing here scales with the ~109k hulls a production facility spawns, and
//! nothing here is a light - a Steam Deck has no margin for either.
//!
//! # Presentation only
//!
//! Reads `HexWfcRuntime` and writes nothing back. Bodies are located from the
//! authoritative snapshot; the marks are a picture of it and never an input to
//! it.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use observed_core::PlayerId;
use observed_facility::hex_wfc::HexCoord;
use observed_hex::{ACROSS_CORNERS, CORNERS, FLOOR_SLAB_TOP, TILE_LEVEL_HEIGHT};
use observed_style::{MarkerRole, SchematicRole, Treatment};

use super::super::camera::{detail_reach, tile_centre};
use super::{Cutaway, SpectatorOverview};
use crate::hex_wfc::sim::HexWfcRuntime;

/// Every root entity the rig owns, so one query retires all of it.
#[derive(Component)]
pub(in crate::hex_wfc) struct CutawayMark;

/// A body's marker root. The parts hang off it, so following a walking body is
/// one transform write rather than three.
#[derive(Component)]
pub(in crate::hex_wfc) struct BodyMark(PlayerId);

/// The static half of the rig is a function of these, and of nothing else.
///
/// While they hold, the ring and the ladder are already right and are left
/// alone; when any of them moves the rig is rebuilt whole. A rebuild is a few
/// dozen entities and happens at most once per tile crossing - which is also the
/// rate at which the camera itself steps, since the overview snaps to tile
/// centres rather than tracking the body.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::hex_wfc) struct Section {
    cell: HexCoord,
    detent: usize,
    tile_radius: u8,
    levels: u8,
    followed: PlayerId,
}

/// Two meshes and five materials, built once and kept.
pub(in crate::hex_wfc) struct MarkAssets {
    /// A unit cube. Every bar, plate and post is this under a scale.
    bar: Handle<Mesh>,
    /// A unit cylinder. Discs and pins are this under a scale.
    rod: Handle<Mesh>,
    /// The cut plane itself.
    datum: Handle<StandardMaterial>,
    /// The floor slab's edge, and the storeys the ladder is not standing on.
    quiet: Handle<StandardMaterial>,
    here: Handle<StandardMaterial>,
    teammate: Handle<StandardMaterial>,
    rival: Handle<StandardMaterial>,
}

/// Everything staging the marks needs, as one system parameter.
///
/// Bundled rather than spread across `sync_cutaway`'s signature because that
/// system's own job - deciding which hulls survive - is three parameters, and a
/// ten-parameter system reads as ten equal concerns rather than two.
#[derive(SystemParam)]
pub(in crate::hex_wfc) struct CutawayMarks<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    /// Every root, for retiring the rig in one pass.
    roots: Query<'w, 's, Entity, With<CutawayMark>>,
    /// The bodies, which are the only part written every frame.
    bodies: Query<
        'w,
        's,
        (
            &'static BodyMark,
            &'static mut Transform,
            &'static mut Visibility,
        ),
        Without<Cutaway>,
    >,
    assets: Local<'s, Option<MarkAssets>>,
    built: Local<'s, Option<Section>>,
}

/// How thick the section ring's bars are, in metres.
const RING_BAR: f32 = 0.34;
/// How far under the top of the storey band the cut plane is drawn.
///
/// The band is `[floor, floor + TILE_LEVEL_HEIGHT)` and a hull survives when its
/// middle is inside it, so the height the ceiling came off at is the top of the
/// band. Drawn a hair under it, or the ring z-fights the floor slab of the
/// storey above on the frames where one is still resident.
const DATUM_DROP: f32 = 0.25;
/// Rung spacing on the storey ladder.
///
/// Deliberately not `TILE_LEVEL_HEIGHT`: at true heights a ten-storey facility
/// is an 80 m spine in a frame 42 m across, so an honest ladder would leave the
/// picture. Compressed, the whole stack is one glance and the lit rung says
/// which floor of how many without a word of text.
const RUNG_RISE: f32 = 1.15;
/// How far outside the ring the ladder stands.
///
/// Small on purpose. The framing box is `tile_radius * TILE_SPAN` in plan and
/// the ring already reaches its vertices, so anything much further out is off
/// the side of the picture at the detents whose bearing runs along an axis.
const LADDER_STANDOFF: f32 = 1.5;
/// A body's pin, in metres. Longer than a storey is tall, so it crosses the cut
/// plane and is visible from outside whatever room the body is inside.
const PIN_HEIGHT: f32 = 9.6;
/// The followed body's pin, which has to win against three others.
const HERE_PIN_HEIGHT: f32 = 12.0;

impl CutawayMarks<'_, '_> {
    /// Stage the marks for this frame.
    ///
    /// Called from `sync_cutaway` with the same resources it filters hulls by,
    /// so the ring's open side and the walls' cut side cannot disagree.
    pub(in crate::hex_wfc) fn sync(
        &mut self,
        runtime: &HexWfcRuntime,
        overview: &SpectatorOverview,
    ) {
        let Some(bearing) = overview
            .active
            .then(|| observed_style::iso::detent_bearing(overview.detent))
        else {
            // Down with the overview. These are the only entities in the
            // facility that exist solely because it is up.
            *self.built = None;
            for entity in &self.roots {
                self.commands.entity(entity).despawn();
            }
            return;
        };

        let followed = runtime.local();
        let wanted = Section {
            cell: followed.cell,
            detent: overview.detent,
            tile_radius: overview.tile_radius,
            levels: runtime.match_state.facility.config.levels,
            followed: runtime.local_player,
        };
        // `roots.is_empty()` is not belt and braces. Leaving the match despawns
        // the rig through `DespawnOnExit` without telling this system, so a
        // signature surviving in a `Local` across a re-entry would describe a
        // rig that no longer exists and nothing would ever be drawn again.
        if *self.built != Some(wanted) || self.roots.is_empty() {
            for entity in &self.roots {
                self.commands.entity(entity).despawn();
            }
            let assets = self
                .assets
                .get_or_insert_with(|| MarkAssets::build(&mut self.meshes, &mut self.materials));
            build(&mut self.commands, assets, runtime, wanted, bearing);
            *self.built = Some(wanted);
            // The body marks went with the rest of the rig; their transforms
            // land next frame, which is the first frame their meshes render.
            return;
        }

        // Bodies move continuously inside a tile, so they are the one part
        // written every frame rather than rebuilt.
        let centre = tile_centre(followed.cell);
        let reach = detail_reach(overview.tile_radius);
        let drop = feet_drop(runtime);
        for (mark, mut transform, mut visibility) in &mut self.bodies {
            let Some(player) = runtime.match_state.players.get(&mark.0) else {
                *visibility = Visibility::Hidden;
                continue;
            };
            let feet = player.position - Vec3::Y * drop;
            // Only what the view actually frames. Markers for the whole roster
            // were a red herring in this module's history: scattered dots out in
            // the black that read as cells without geometry.
            let framed = Vec2::new(feet.x - centre.x, feet.z - centre.z).length() <= reach;
            let wanted = if framed && !player.escaped {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != wanted {
                *visibility = wanted;
            }
            transform.translation = feet;
            transform.rotation = Quat::from_rotation_y(-player.yaw);
        }
    }
}

/// The distance from a body's authoritative position down to its feet.
///
/// `position` is the capsule centre and the marks belong on the floor: a disc
/// floating at chest height is exactly the defect the teleport plates shipped
/// with. Read from the traversal profile rather than written as a constant, so
/// it cannot drift away from the body it is drawn under.
fn feet_drop(runtime: &HexWfcRuntime) -> f32 {
    runtime
        .match_state
        .content()
        .traversal_profile()
        .requirements()
        .capsule_half_height
}

/// Spawn the whole rig for `section`.
fn build(
    commands: &mut Commands,
    assets: &MarkAssets,
    runtime: &HexWfcRuntime,
    section: Section,
    bearing: Vec2,
) {
    let centre = tile_centre(section.cell);
    let floor = f32::from(section.cell.level) * TILE_LEVEL_HEIGHT;
    let datum = floor + TILE_LEVEL_HEIGHT - DATUM_DROP;
    let reach = detail_reach(section.tile_radius);

    for edge in hex_edges(centre, reach) {
        // The cut plane, as a hoop that is open where the building is open.
        // Same near-arc question the walls were cut with, so the three missing
        // sides of the ring are the three sides whose walls are missing.
        if edge.outward.dot(bearing) <= 0.0 {
            commands.spawn(bar(
                assets,
                assets.datum.clone(),
                edge,
                datum,
                Vec3::new(edge.length, RING_BAR, RING_BAR),
                "cutaway section datum",
            ));
        }
        // The storey's floor, all six sides: where the view stops is not where
        // the building stops, and without this the plan fades into black with
        // no way to tell which of the two happened.
        commands.spawn(bar(
            assets,
            assets.quiet.clone(),
            edge,
            floor + FLOOR_SLAB_TOP,
            Vec3::new(edge.length, RING_BAR * 0.5, RING_BAR * 0.5),
            "cutaway framed extent",
        ));
    }

    build_ladder(commands, assets, section, centre, bearing);
    build_bodies(commands, assets, runtime, section);
}

/// Which storey of how many, as a stack of rungs with one of them lit.
///
/// Stood on the near side - the side the walls came off - so it is never behind
/// the building it is describing.
fn build_ladder(
    commands: &mut Commands,
    assets: &MarkAssets,
    section: Section,
    centre: Vec3,
    bearing: Vec2,
) {
    let reach = detail_reach(section.tile_radius);
    let foot = centre + Vec3::new(bearing.x, 0.0, bearing.y) * (reach + LADDER_STANDOFF);
    let across = Vec2::new(-bearing.y, bearing.x);
    let levels = section.levels.max(1);
    let base = f32::from(section.cell.level) * TILE_LEVEL_HEIGHT + FLOOR_SLAB_TOP + 0.6;
    let span = f32::from(levels - 1) * RUNG_RISE;
    commands.spawn((
        CutawayMark,
        DespawnOnExit(crate::GameState::HexWfc),
        Mesh3d(assets.bar.clone()),
        MeshMaterial3d(assets.quiet.clone()),
        Transform {
            translation: Vec3::new(foot.x, base + span * 0.5, foot.z),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(0.14, span + 0.8, 0.14),
        },
        Name::new("cutaway storey ladder post"),
    ));
    for level in 0..levels {
        let standing = level == section.cell.level;
        let (material, width, thickness) = if standing {
            (assets.here.clone(), 3.2, 0.34)
        } else {
            (assets.quiet.clone(), 1.8, 0.18)
        };
        commands.spawn((
            CutawayMark,
            DespawnOnExit(crate::GameState::HexWfc),
            Mesh3d(assets.bar.clone()),
            MeshMaterial3d(material),
            Transform {
                translation: Vec3::new(foot.x, base + f32::from(level) * RUNG_RISE, foot.z),
                rotation: Quat::from_rotation_y((-across.y).atan2(across.x)),
                scale: Vec3::new(width, thickness, 0.5),
            },
            Name::new(format!("cutaway storey rung {level}")),
        ));
    }
}

/// One marker per body, in the three roles the style crate already names.
///
/// The followed body is `You` because in the overview it *is* the vantage: the
/// camera frames its tile, the storey filter draws its floor, and the key light
/// is aimed at it. A body on another storey keeps its own height, so a rival one
/// floor down reads as one floor down rather than as a dot in the plan.
fn build_bodies(
    commands: &mut Commands,
    assets: &MarkAssets,
    runtime: &HexWfcRuntime,
    section: Section,
) {
    let local_team = runtime.local().team;
    for player in runtime.match_state.players.values() {
        let (material, pin, disc) = if player.id == section.followed {
            (assets.here.clone(), HERE_PIN_HEIGHT, 1.5)
        } else if player.team == local_team {
            (assets.teammate.clone(), PIN_HEIGHT, 1.0)
        } else {
            (assets.rival.clone(), PIN_HEIGHT, 1.0)
        };
        commands
            .spawn((
                CutawayMark,
                BodyMark(player.id),
                DespawnOnExit(crate::GameState::HexWfc),
                // Hidden until the first `sync` places it, so a body outside the
                // frame never flashes at the origin on its first rendered frame.
                Visibility::Hidden,
                Transform::default(),
                Name::new(format!("cutaway body mark {}", player.id.0)),
            ))
            .with_children(|parts| {
                // A disc on the floor: plan position, read straight down.
                parts.spawn((
                    Mesh3d(assets.rod.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform {
                        translation: Vec3::Y * 0.06,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(disc * 2.0, 0.12, disc * 2.0),
                    },
                    Name::new("stance"),
                ));
                // A pin through the cut plane: which room, even from outside it.
                parts.spawn((
                    Mesh3d(assets.rod.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform {
                        translation: Vec3::Y * pin * 0.5,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(0.22, pin, 0.22),
                    },
                    Name::new("pin"),
                ));
                // A nose along the body's own heading: which way it is going,
                // which is the whole reason to watch a route from outside.
                parts.spawn((
                    Mesh3d(assets.bar.clone()),
                    MeshMaterial3d(material),
                    Transform {
                        translation: Vec3::new(0.0, 0.08, -(disc + 0.75)),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(0.42, 0.16, 1.5),
                    },
                    Name::new("heading"),
                ));
            });
    }
}

/// One side of the framed hexagon, in world plan coordinates.
#[derive(Clone, Copy)]
struct Edge {
    /// Midpoint of the side.
    mid: Vec2,
    /// Direction along the side, normalized.
    along: Vec2,
    length: f32,
    /// Outward normal, normalized. This is what the near-arc test reads.
    outward: Vec2,
}

/// The six sides of the facility's own hexagon, centred on `centre` and scaled
/// so its vertices sit `radius` out.
///
/// Built from [`CORNERS`] rather than a regular polygon, because the lattice's
/// hexagon is not regular - it reaches 8 m to a vertex and 7 m to a face - and a
/// ring that disagreed with the cells inside it would read as a second building.
fn hex_edges(centre: Vec3, radius: f32) -> [Edge; 6] {
    let scale = radius / (ACROSS_CORNERS * 0.5);
    let plan = Vec2::new(centre.x, centre.z);
    #[allow(clippy::cast_precision_loss)]
    let corner = |index: usize| {
        let (x, z) = CORNERS[index % CORNERS.len()];
        plan + Vec2::new(x as f32, z as f32) * scale
    };
    std::array::from_fn(|index| {
        let (a, b) = (corner(index), corner(index + 1));
        let span = b - a;
        let mid = (a + b) * 0.5;
        Edge {
            mid,
            along: span.normalize_or_zero(),
            length: span.length(),
            outward: (mid - plan).normalize_or_zero(),
        }
    })
}

/// One bar of a ring, laid along `edge` at height `y`.
fn bar(
    assets: &MarkAssets,
    material: Handle<StandardMaterial>,
    edge: Edge,
    y: f32,
    scale: Vec3,
    name: &'static str,
) -> impl Bundle {
    (
        CutawayMark,
        DespawnOnExit(crate::GameState::HexWfc),
        Mesh3d(assets.bar.clone()),
        MeshMaterial3d(material),
        Transform {
            translation: Vec3::new(edge.mid.x, y, edge.mid.y),
            rotation: Quat::from_rotation_y((-edge.along.y).atan2(edge.along.x)),
            scale,
        },
        Name::new(name),
    )
}

impl MarkAssets {
    fn build(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        // Two registers, deliberately: the section rig is *schematic* and the
        // bodies are *markers*.
        //
        // `SchematicRole` describes itself as "a facility diagram as a ship's
        // console would draw it ... read, not inhabited", which is exactly what
        // a section datum is - it is not a thing in the building, it is the
        // instrument laid over it. `Selected` is "the cell currently under
        // inspection", which is the cut ring; `Grid` is, verbatim, "the
        // annotation lines that carry no state of their own", which is the
        // framed extent, the ladder post, and the storeys nobody is standing on.
        //
        // The ladder's *lit* rung is the one crossing between the registers, and
        // it crosses on purpose: it wears `here` because what it reports is not
        // "this storey is under inspection" but "the body you are following is
        // on this one", which is the same fact the pin forty metres away is
        // reporting. Reading it off the capture settles it - a white rung at the
        // bottom of a green stack is unambiguous, and no viewer will mistake a
        // rung on a ladder for a body in a room.
        //
        // It also keeps the registers out of each other's way. The first pass
        // drew the ring in observation-panel footprint cyan, which is a
        // defensible read of "schematic outline" and within a few percent of
        // `MarkerRole::You`. Diluting the body hues is the one thing this packet
        // must not do: which body is which is a third of the job.
        Self {
            bar: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            rod: meshes.add(Cylinder::new(0.5, 1.0)),
            datum: mark_material(
                materials,
                observed_style::schematic(SchematicRole::Selected),
            ),
            quiet: mark_material(materials, observed_style::schematic(SchematicRole::Grid)),
            here: mark_material(materials, observed_style::marker(MarkerRole::You)),
            teammate: mark_material(materials, observed_style::marker(MarkerRole::Teammate)),
            rival: mark_material(materials, observed_style::marker(MarkerRole::Rival)),
        }
    }
}

/// A style treatment as a marker material, verbatim.
///
/// No scalar, no tint, no blend: every colour in this module is a treatment the
/// style crate already chose, and the tiering between the cut plane and the
/// annotation lines is carried by picking a different *role* rather than by
/// turning one role's emissive down. An attenuated role is a colour this file
/// invented, and the Legibility Contract is a contract.
fn mark_material(
    materials: &mut Assets<StandardMaterial>,
    treatment: Treatment,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        metallic: 0.0,
        perceptual_roughness: 0.55,
        ..default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ring must be the facility's hexagon rather than a regular one: eight
    /// metres to a vertex and seven to a face, scaled. The quantization is why
    /// the tolerance is a quarter-metre rather than an epsilon - two of the six
    /// sides sit fractionally inside the other four, exactly as the cells do.
    #[test]
    fn the_ring_traces_the_lattice_hexagon() {
        let centre = Vec3::new(28.0, 8.0, -14.0);
        let plan = Vec2::new(centre.x, centre.z);
        let scale = 42.0 / 8.0;
        for edge in hex_edges(centre, 42.0) {
            let face = (edge.mid - plan).length();
            assert!(
                (face - 7.0 * scale).abs() < 0.5,
                "a side should sit a face-radius out, got {face}"
            );
        }
    }

    /// The whole point of the ring: it is open on exactly the sides the cutaway
    /// takes walls off, at every detent. Three of six, never two and never four.
    #[test]
    fn the_ring_opens_on_the_cut_side_at_every_detent() {
        for detent in 0..observed_style::iso::AZIMUTH_DETENTS {
            let bearing = observed_style::iso::detent_bearing(detent);
            let open = hex_edges(Vec3::ZERO, 42.0)
                .into_iter()
                .filter(|edge| edge.outward.dot(bearing) > 0.0)
                .count();
            assert_eq!(open, 3, "detent {detent} drops {open} of six sides");
        }
    }

    /// A bar has to lie along its own side. The wrong rotation sign lays every
    /// bar across its edge instead, which draws a star rather than a ring.
    #[test]
    fn a_bar_is_rotated_onto_its_own_side() {
        for edge in hex_edges(Vec3::ZERO, 42.0) {
            let rotation = Quat::from_rotation_y((-edge.along.y).atan2(edge.along.x));
            let laid = rotation * Vec3::X;
            assert!(
                (laid.x - edge.along.x).abs() < 1e-4 && (laid.z - edge.along.y).abs() < 1e-4,
                "bar lies {laid:?}, side runs {:?}",
                edge.along
            );
        }
    }
}
