//! Presentation-side components and resources: markers on rendered entities (place
//! geometry, doorway leaves, avatars, HUD panels) and the small feedback-state
//! resources the visual/audio systems drive. Everything here is presentation-only —
//! it reads the simulation (`crate::sim`) and never writes it.

use bevy::prelude::*;
use observed_core::RoomId;

use crate::teleport::Place;

/// Marks the shared 3D camera (first-person during the Match).
#[derive(Component)]
pub(crate) struct GameCam;

/// Marks the startup directional light used for menus and non-match screens.
#[derive(Component)]
pub(crate) struct GameSun;

pub(crate) const MENU_SUN_ILLUMINANCE: f32 = 6_000.0;

/// Marks the spawned geometry of the *current place* (room or hallway) so it can be
/// torn down and rebuilt when the player teleports to the next place.
#[derive(Component)]
pub(crate) struct PlaceGeometry;

/// An animated door leaf filling a sealed doorway gap. Transparent passage thresholds no
/// longer get leaves; this remains for side doors / locked exits and any future sealed
/// door that needs to slide or stay shut. `closed_y` / `open_y` are the leaf's local-Y at
/// each extreme; `center` is the gap centre (XZ) for the proximity test. Presentation-only.
#[derive(Component)]
pub(crate) struct DoorLeaf {
    pub center: Vec2,
    pub closed_y: f32,
    pub open_y: f32,
    pub openable: bool,
}

/// A keystone pickup item sitting in a room; collected on contact.
#[derive(Component)]
pub(crate) struct KeystoneItem(pub RoomId);

/// A droppable single-player tool visible in the current place.
#[derive(Component)]
pub(crate) struct DroppedItemVisual;

/// Teleport animation resource for screen transitions
#[derive(Resource, Default, Debug)]
pub struct TeleportAnimation {
    pub timer: f32,
    pub max_time: f32,
    pub color: Color,
}

impl TeleportAnimation {
    pub fn trigger(&mut self, duration: f32, color: Color) {
        self.timer = duration;
        self.max_time = duration;
        self.color = color;
    }
}

/// Marker component for the full-screen teleport overlay UI node
#[derive(Component)]
pub(crate) struct TeleportOverlay;

/// Marker component for the rotating stargate glow cylinder
#[derive(Component)]
pub(crate) struct TeleportPadGlow;

/// A rival team's avatar, walking the player's current room while that team's clump
/// shares it. Holds the rival team index; managed entirely by `sync_rival_avatars`
/// (presentation-only — reads the brain, never writes it).
#[derive(Component)]
pub(crate) struct RivalAvatar {
    pub team: usize,
}

/// A place light driven by the flicker system. `base` is its steady-state intensity.
/// `idle` (0 = none) is the amplitude of a constant "failing fixture" flicker —
/// occasional brief dropouts — and `phase` decorrelates each fixture so they stutter
/// independently. A decoherence flash deepens every light's stutter on top of that.
#[derive(Component)]
pub(crate) struct FlickerLight {
    pub base: f32,
    pub idle: f32,
    pub phase: f32,
}

/// Geometry rendered behind a room's open doorway as a preview of the actual hallway
/// you'll teleport into (aligned to the opening). Also tagged [`PlaceGeometry`] so it is
/// torn down on the next teleport; this marker just lets it be queried/tested.
#[derive(Component)]
pub(crate) struct PassagePreview;

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum MatchAudioCue {
    Ambience,
    Footstep,
    Door,
    Escape,
    Reroute,
}

/// First-person feedback for graph **decoherence** (a committed reroute): when the
/// brain's `reroute_commits` advances — the unobserved structure has rewired — the game
/// stutters the place's lights, fires an audio sting, and slams the current place's doors
/// shut (the "re-hide"). No camera shake, no full-screen flash — the instability is
/// diegetic. Presentation-only; driven by the decohere-fx and flicker systems in the
/// match ambience module. Initialised to the live commit count on entering the Match so
/// it never fires on the first frame.
#[derive(Resource, Default)]
pub struct DecohereFx {
    /// The reroute-commit count we last reacted to.
    pub last_commits: u32,
    /// Seconds remaining on the active decoherence feedback (0 = idle).
    pub flash: f32,
}

#[derive(Resource)]
pub struct MatchAudioState {
    pub(crate) last_position: Vec3,
    pub(crate) stride_distance: f32,
    pub(crate) last_place: Place,
    pub(crate) escaped_count: usize,
}

/// Whether the tac-map overlay is currently shown (toggled with Tab).
#[derive(Resource, Default)]
pub struct TacMapState(pub bool);

/// The root node of the tac-map overlay (Visibility toggled with Tab).
#[derive(Component)]
pub(crate) struct TacMapPanel;

/// A dynamic child of the tac-map (room/bar/marker), rebuilt each frame while shown.
#[derive(Component)]
pub(crate) struct TacMapElement;

/// The in-match HUD status panel (top-left).
#[derive(Component)]
pub(crate) struct MatchHud;

/// The full-screen pause overlay.
#[derive(Component)]
pub(crate) struct PausePanel;
