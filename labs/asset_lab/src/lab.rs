use bevy::{ecs::system::SystemParam, gltf::GltfAssetLabel, prelude::*};

use crate::manifest::{AssetKind, AssetSlot, assets_root, manifest, slot_full_path, slot_present};

const PLACEHOLDER: Color = Color::srgb(1.0, 0.0, 1.0); // the classic "missing asset" magenta
const LOADED_TINT: Color = Color::WHITE;
const PEDESTAL: Color = Color::srgb(0.22, 0.25, 0.32);

#[derive(Component)]
pub(crate) struct AssetCam;

#[derive(Component)]
pub(crate) struct AssetUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
pub(crate) struct HelpText;

#[derive(Component)]
pub(crate) struct DebugPanel;

/// A resolved view of a slot for the overlay: present or placeholder, and where.
#[derive(Clone)]
pub struct SlotStatus {
    pub name: &'static str,
    pub kind: AssetKind,
    pub full_path: String,
    pub present: bool,
}

#[derive(Resource, Default)]
pub struct AssetStatus {
    pub slots: Vec<SlotStatus>,
}

impl AssetStatus {
    pub fn loaded_count(&self) -> usize {
        self.slots.iter().filter(|s| s.present).count()
    }
}

/// The dropped sound (if any), played on Space.
#[derive(Resource, Default)]
pub struct SoundSlot(pub Option<Handle<AudioSource>>);

fn find(slots: &[AssetSlot], name: &str) -> AssetSlot {
    *slots.iter().find(|s| s.name == name).expect("slot exists")
}

pub(crate) fn setup_lab(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        AssetCam,
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 10.0).looking_at(Vec3::new(0.0, 1.4, 0.0), Vec3::Y),
        Name::new("Asset Showcase Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 11_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.5, 0.0)),
        Name::new("Showcase Sun"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.5, 0.55, 0.7),
        brightness: 220.0,
        ..default()
    });

    let slots = manifest();
    let root = assets_root();
    let present = |name: &str| {
        let slot = find(&slots, name);
        (slot, slot_present(&slot, &root))
    };

    let mut texture_material = |name: &str| {
        let (slot, has) = present(name);
        if has {
            materials.add(StandardMaterial {
                base_color: LOADED_TINT,
                base_color_texture: Some(asset_server.load(slot.path)),
                perceptual_roughness: 0.9,
                ..default()
            })
        } else {
            materials.add(StandardMaterial {
                base_color: PLACEHOLDER,
                perceptual_roughness: 0.9,
                ..default()
            })
        }
    };

    // Floor (texture slot "floor").
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(16.0, 16.0))),
        MeshMaterial3d(texture_material("floor")),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Name::new("Floor"),
    ));

    // Wall (texture slot "wall") — an upright quad facing the camera.
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(5.5, 3.6))),
        MeshMaterial3d(texture_material("wall")),
        Transform::from_xyz(-3.6, 2.0, 0.0),
        Name::new("Wall"),
    ));

    // Prop (model slot "prop") — a glTF scene on a pedestal, or a placeholder cube.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 0.5, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial::from_color(PEDESTAL))),
        Transform::from_xyz(3.6, 0.25, 0.0),
        Name::new("Pedestal"),
    ));
    let (prop, prop_present) = present("prop");
    if prop_present {
        commands.spawn((
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(prop.path))),
            Transform::from_xyz(3.6, 0.5, 0.0).with_scale(Vec3::splat(1.0)),
            Name::new("Prop model"),
        ));
    } else {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            MeshMaterial3d(materials.add(StandardMaterial::from_color(PLACEHOLDER))),
            Transform::from_xyz(3.6, 1.0, 0.0),
            Name::new("Prop placeholder"),
        ));
    }

    // Sound (slot "chime") — keep the handle to play on Space.
    let (chime, chime_present) = present("chime");
    let sound = if chime_present {
        Some(asset_server.load::<AudioSource>(chime.path))
    } else {
        None
    };
    commands.insert_resource(SoundSlot(sound));

    // Build the overlay status.
    let status = AssetStatus {
        slots: slots
            .iter()
            .map(|slot| SlotStatus {
                name: slot.name,
                kind: slot.kind,
                full_path: slot_full_path(slot, &root).display().to_string(),
                present: slot_present(slot, &root),
            })
            .collect(),
    };
    commands.insert_resource(status);

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            AssetUiRoot,
            Name::new("Asset Lab UI Root"),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                DebugPanel,
                Node {
                    width: px(560),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.95)),
                BorderColor::all(Color::srgba(0.6, 0.9, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Scanning assets…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.94, 1.0)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.95)),
                BorderColor::all(Color::srgba(0.6, 0.9, 1.0, 0.6)),
                children![(
                    Text::new(
                        "ASSET DROP-IN SHOWCASE\n\
                         Space   Play the 'chime' sound (if dropped)\n\
                         F1      Toggle this overlay\n\n\
                         Each slot below shows where to drop a free/CC0 file. Drop a\n\
                         file at the listed path, re-run, and it replaces the magenta\n\
                         placeholder — no code changes. Textures = PNG/JPG, models =\n\
                         glTF/GLB, sounds = OGG/WAV.\n\n\
                         Sources: ambientCG, Poly Haven (textures); Kenney, Quaternius,\n\
                         Poly Pizza (models); Kenney, Freesound (sounds).",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.94, 0.98)),
                )],
            ));
        });
}

pub(crate) fn play_sound(
    keyboard: Res<ButtonInput<KeyCode>>,
    sound: Res<SoundSlot>,
    mut commands: Commands,
) {
    if keyboard.just_pressed(KeyCode::Space)
        && let Some(handle) = &sound.0
    {
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings::DESPAWN,
            Name::new("Chime"),
        ));
    }
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel: Single<&mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    mut help: Single<&mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        let next = if **panel == Visibility::Hidden {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        **panel = next;
        **help = next;
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    status: Res<'w, AssetStatus>,
    sound: Res<'w, SoundSlot>,
    cams: Query<'w, 's, (), With<AssetCam>>,
    ui_roots: Query<'w, 's, (), With<AssetUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
}

pub(crate) fn update_debug_text(context: DebugContext) {
    let cams = context.cams.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = cams == 1 && ui_roots == 1;

    let root = assets_root();
    let mut lines = String::new();
    for slot in &context.status.slots {
        let mark = if slot.present {
            "LOADED     "
        } else {
            "placeholder"
        };
        lines.push_str(&format!(
            "  [{}] {:<6} {:<8} {}\n",
            mark,
            slot.name,
            slot.kind.label(),
            slot.full_path,
        ));
    }

    let mut text = context.text.into_inner();
    **text = format!(
        "ASSET DROP-IN  {}\n\
         assets root   {}\n\
         loaded        {} / {}\n\
         chime ready   {}\n\n\
         slots (drop a file at the path to fill it):\n{}\n\
         cameras {cams}  UI {ui_roots}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        root.display(),
        context.status.loaded_count(),
        context.status.slots.len(),
        if context.sound.0.is_some() {
            "yes (Space)"
        } else {
            "no"
        },
        lines,
    );
}
