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

/// A 2.5D sprite rendered in the 3D match scene. Presentation-only: the yaw-facing
/// system rotates these quads toward the camera without changing simulation state.
#[derive(Component)]
pub(crate) struct BillboardSprite;

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

/// The visible doorway surface that displays one isolated render target.
#[derive(Component)]
pub(crate) struct PortalSurface {
    pub snapshot_id: Option<crate::sim::state::PlaceSnapshotId>,
}

/// A render-to-texture camera paired with one threshold transaction. The remote scene
/// is the source-aligned destination translated by `scene_offset`. Its projection is
/// fitted to the physical aperture every frame, so the texture contains the rays through
/// the opening rather than an arbitrary crop of the player's full-screen camera.
#[derive(Component)]
pub(crate) struct PortalPreviewCamera {
    pub scene_offset: Vec3,
    pub source_center: Vec3,
    pub source_normal: Vec3,
    pub aperture_size: Vec2,
    pub warm_frames: u8,
    pub snapshot_id: crate::sim::state::PlaceSnapshotId,
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq, Hash)]
pub(crate) enum MatchAudioCue {
    Ambience,
    Footstep,
    Door,
    Escape,
    Reroute,
    /// A rival team's footsteps bleeding through from a neighbouring room (Phase 42c
    /// sound bleed): reuses the same `footstep.ogg` drop-in slot as the player's own
    /// footstep cue, at reduced/attenuated volume, but tagged with its own cue variant
    /// so audits and tests can tell "I heard a rival" from "I took a step" without
    /// re-deriving it from volume/name heuristics.
    RivalBleed,
    UiClick,
    UiHover,
    Jump,
    Land,
    Klaxon,
    CollapseSting,
    ToolInteract,
    Keystone,
    ExitUnlock,
    GuardianDread,
}

#[cfg(test)]
impl MatchAudioCue {
    pub(crate) const ALL: [Self; 16] = [
        Self::Ambience,
        Self::Footstep,
        Self::Door,
        Self::Escape,
        Self::Reroute,
        Self::RivalBleed,
        Self::UiClick,
        Self::UiHover,
        Self::Jump,
        Self::Land,
        Self::Klaxon,
        Self::CollapseSting,
        Self::ToolInteract,
        Self::Keystone,
        Self::ExitUnlock,
        Self::GuardianDread,
    ];
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
    pub(crate) collapse_sting_place: Option<Place>,
}

/// Tracks which rival team/room pairing the sound-bleed system last cued, so a rival
/// standing in the same neighbouring room for many frames plays exactly one cue on
/// *first* appearance (or on changing room), not one per frame. Presentation-only
/// bookkeeping — reset every match, never read by the deterministic brain.
#[derive(Resource, Default, Debug)]
pub struct RivalBleedState {
    /// `(rival team index, last-heard room)` for every rival team currently bleeding
    /// sound into the player's current place.
    pub(crate) last_heard: Vec<(usize, RoomId)>,
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

/// The minimal interaction reticle dot.
#[derive(Component)]
pub(crate) struct InteractionReticle;

/// The in-match HUD status panel (top-left).
#[derive(Component)]
pub(crate) struct MatchHud;

/// Whether the match spawns the debug status HUD (top-left readouts) and the legend.
/// Off by default (Phase 50 immersion ruling): normal play communicates diegetically
/// and through the tac-map. Initialized once at app build from
/// `evidence::debug_hud_enabled()` (`OBSERVED2_DEBUG_HUD`, or implied by a
/// visual-audit/freecam session); held as a resource so tests can flip it without
/// touching process env.
#[derive(Resource, Default)]
pub(crate) struct DebugHud(pub bool);

/// Individual compact HUD readouts, spawned only under [`DebugHud`]. Tests query these
/// semantic markers instead of scraping one debug-heavy text block.
#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum MatchHudReadout {
    Objective,
    Keystone,
    Collapse,
    Standing,
    Controls,
    Debug,
}

/// The full-screen pause overlay.
#[derive(Component)]
pub(crate) struct PausePanel;

/// The in-match settings panel nested inside the pause overlay (Phase 48: the
/// pause-menu route to Settings — an overlay rather than a `GameState` transition,
/// since leaving `GameState::Match` tears down the whole session).
#[derive(Component)]
pub(crate) struct PauseSettingsPanel;

/// A dynamic row inside [`PauseSettingsPanel`], rebuilt each frame while shown (same
/// convention as [`TacMapElement`]).
#[derive(Component)]
pub(crate) struct PauseSettingsElement;

/// Read-only display of the active bot configuration on the pause overlay.
#[derive(Component)]
pub(crate) struct PauseConfigReadout;

/// Global UI sounds (available outside GameState::Match)
#[derive(Resource)]
pub(crate) struct UiAssets {
    pub(crate) click: Option<Handle<AudioSource>>,
    pub(crate) hover: Option<Handle<AudioSource>>,
}

/// Smooth first-person camera easing for movement and teleport feedback. This is
/// presentation-only and deliberately avoids shake/flash effects that would mask
/// gameplay signals.
#[derive(Resource, Default, Debug)]
pub(crate) struct CameraJuice {
    pub(crate) land_timer: f32,
    pub(crate) jump_timer: f32,
    pub(crate) teleport_ease_timer: f32,
}
