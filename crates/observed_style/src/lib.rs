//! The shared **visual language** for Observed 2: a pure mapping from *semantic
//! state* to *visual treatment*. Presentation code (the labs and the assembled
//! `game`) asks this crate how to draw a thing; it never invents ad-hoc colours.
//!
//! Art direction is **neon-noir** — dark surfaces, neon emission, fog and bloom —
//! generated entirely from code (no authored textures/meshes), which plays to an
//! agent's strengths and is verifiable through the `OBSERVED2_CAPTURE` screenshot
//! loop. The crate encodes the **Legibility Contract**: any treatment flagged as a
//! *signal* (your path, threats, interactables, actors, door reads) must stay bright enough to
//! punch through the atmosphere, and every on-screen role has exactly one documented
//! entry here (`legend`), so nothing is ever an unlabelled coloured marker.
//!
//! This is render-free data: it depends only on `bevy::color`. A consumer turns a
//! [`Treatment`] into a `StandardMaterial` (+ optional neon edge / light). The
//! `style_lab` lab is the visual proof app for these rules; the rules and their
//! tests live here.

use bevy::color::{Color, LinearRgba};

/// Minimum emissive luminance for a signal-tier treatment. Emissive is HDR (values
/// exceed 1.0 so the colour blooms), so signals stay legible through fog/bloom.
pub const SIGNAL_MIN_LUMINANCE: f32 = 2.0;

/// Non-signal structural surfaces must read as *dark* for neon-noir (the neon does
/// the talking, not the albedo).
pub const ATMOSPHERE_MAX_LUMINANCE: f32 = 0.1;

/// Minimum luminance for an outlined signal after a color-vision preview matrix is
/// applied. Outlines are drawn over dark neon-noir atmosphere, so this is a
/// contrast floor rather than a promise that every hue remains unique.
pub const OUTLINE_MIN_SIMULATED_LUMINANCE: f32 = 0.14;

/// Minimum logical-pixel width for gameplay-critical outlines.
pub const OUTLINE_MIN_WIDTH: f32 = 3.0;

/// How a single surface/marker is drawn. This is data, not rendering: a consumer
/// turns it into a `StandardMaterial` (+ optional neon edge / light).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Treatment {
    /// Albedo (mostly dark in neon-noir).
    pub base_color: Color,
    /// HDR emission — the neon. This is what blooms and what carries meaning.
    pub emissive: LinearRgba,
    /// Signal tier: a gameplay-critical cue that must always stay legible.
    pub signal: bool,
    /// Optional neon edge/rim colour (drawn as a wireframe outline).
    pub edge: Option<Color>,
}

/// A mesh-outline treatment for a gameplay-critical object. Width is deliberately
/// part of the semantic treatment so color is never the only channel carrying
/// meaning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutlineTreatment {
    /// The outline colour selected by semantic role.
    pub color: Color,
    /// Width in logical pixels.
    pub width: f32,
    /// Signal tier: gameplay-critical and must remain legible.
    pub signal: bool,
}

/// Relative luminance (Rec. 709) of a linear colour.
pub fn luminance(c: LinearRgba) -> f32 {
    0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue
}

/// Development-preview color-vision modes used by legibility checks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorVisionMode {
    /// Normal full-color vision.
    #[default]
    Normal,
    /// Red/green deficiency, missing long-wavelength cones.
    Protanopia,
    /// Red/green deficiency, missing medium-wavelength cones.
    Deuteranopia,
    /// Blue/yellow deficiency, missing short-wavelength cones.
    Tritanopia,
    /// No hue discrimination.
    Achromatopsia,
}

impl ColorVisionMode {
    pub const ALL: [ColorVisionMode; 5] = [
        ColorVisionMode::Normal,
        ColorVisionMode::Protanopia,
        ColorVisionMode::Deuteranopia,
        ColorVisionMode::Tritanopia,
        ColorVisionMode::Achromatopsia,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ColorVisionMode::Normal => "normal",
            ColorVisionMode::Protanopia => "protanopia",
            ColorVisionMode::Deuteranopia => "deuteranopia",
            ColorVisionMode::Tritanopia => "tritanopia",
            ColorVisionMode::Achromatopsia => "achromatopsia",
        }
    }

    pub fn next(self) -> Self {
        match self {
            ColorVisionMode::Normal => ColorVisionMode::Protanopia,
            ColorVisionMode::Protanopia => ColorVisionMode::Deuteranopia,
            ColorVisionMode::Deuteranopia => ColorVisionMode::Tritanopia,
            ColorVisionMode::Tritanopia => ColorVisionMode::Achromatopsia,
            ColorVisionMode::Achromatopsia => ColorVisionMode::Normal,
        }
    }
}

fn color_vision_matrix(mode: ColorVisionMode) -> [[f32; 3]; 3] {
    match mode {
        ColorVisionMode::Normal => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        ColorVisionMode::Protanopia => [
            [0.56667, 0.43333, 0.0],
            [0.55833, 0.44167, 0.0],
            [0.0, 0.24167, 0.75833],
        ],
        ColorVisionMode::Deuteranopia => {
            [[0.625, 0.375, 0.0], [0.70, 0.30, 0.0], [0.0, 0.30, 0.70]]
        }
        ColorVisionMode::Tritanopia => [
            [0.95, 0.5, 0.0],
            [0.0, 0.43333, 0.56667],
            [0.0, 0.475, 0.525],
        ],
        ColorVisionMode::Achromatopsia => [
            [0.299, 0.587, 0.114],
            [0.299, 0.587, 0.114],
            [0.299, 0.587, 0.114],
        ],
    }
}

/// Apply a deterministic color-vision preview matrix to a linear colour. This is
/// dev tooling: it previews likely contrast failures; it is not gameplay state.
pub fn simulate_linear_vision(c: LinearRgba, mode: ColorVisionMode) -> LinearRgba {
    let m = color_vision_matrix(mode);
    LinearRgba {
        red: m[0][0] * c.red + m[0][1] * c.green + m[0][2] * c.blue,
        green: m[1][0] * c.red + m[1][1] * c.green + m[1][2] * c.blue,
        blue: m[2][0] * c.red + m[2][1] * c.green + m[2][2] * c.blue,
        alpha: c.alpha,
    }
}

/// Apply a deterministic color-vision preview matrix to a display colour and
/// return the simulated linear colour.
pub fn simulate_color_vision(color: Color, mode: ColorVisionMode) -> LinearRgba {
    simulate_linear_vision(color.to_linear(), mode)
}

fn dim(color: Color, factor: f32) -> Color {
    let s = color.to_srgba();
    Color::srgb(s.red * factor, s.green * factor, s.blue * factor)
}

fn scale(e: LinearRgba, factor: f32) -> LinearRgba {
    LinearRgba::rgb(e.red * factor, e.green * factor, e.blue * factor)
}

/// A structural surface in the facility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceRole {
    /// Ordinary room floor.
    Plain,
    /// The protected objective corridor — the route you must follow.
    Spine,
    /// The longer, safe bypass around a hazard.
    SafeBypass,
    /// A raised gantry deck: the fast traversal route across a vertical hallway.
    GantryDeck,
    /// A lit gantry platform edge: the readable jump/fall commitment line.
    GantryEdge,
    /// A visible lower landing under a gantry jump map.
    Understory,
    /// A pressure-gate shortcut while it is dangerous to cross.
    TrapArmed,
    /// A pressure-gate shortcut while it is safe to cross.
    TrapIdle,
    /// A structural wall.
    Wall,
    /// An overhead ceiling panel.
    Ceiling,
}

impl SurfaceRole {
    pub const ALL: [SurfaceRole; 10] = [
        SurfaceRole::Plain,
        SurfaceRole::Spine,
        SurfaceRole::SafeBypass,
        SurfaceRole::GantryDeck,
        SurfaceRole::GantryEdge,
        SurfaceRole::Understory,
        SurfaceRole::TrapArmed,
        SurfaceRole::TrapIdle,
        SurfaceRole::Wall,
        SurfaceRole::Ceiling,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SurfaceRole::Plain => "plain floor",
            SurfaceRole::Spine => "spine route",
            SurfaceRole::SafeBypass => "safe bypass",
            SurfaceRole::GantryDeck => "gantry upper route",
            SurfaceRole::GantryEdge => "gantry jump edge",
            SurfaceRole::Understory => "gantry understory landing",
            SurfaceRole::TrapArmed => "trap armed",
            SurfaceRole::TrapIdle => "trap idle",
            SurfaceRole::Wall => "wall",
            SurfaceRole::Ceiling => "ceiling",
        }
    }
}

/// A discrete, always-legible gameplay marker. Every marker is signal-tier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkerRole {
    /// Your immediate target — the next room to reach.
    NextRoom,
    /// The final exit.
    Exit,
    /// The seizable control device.
    Control,
    /// A collapsing/threatened room.
    Collapse,
    /// You (the local player).
    You,
    /// Your teammate.
    Teammate,
    /// A rival team.
    Rival,
    /// The facility director (the AI adversary).
    Director,
}

/// A semantic read shown on a doorframe before committing to the room beyond it.
/// These are signal-tier because they are decision cues, and every glyph is backed
/// by a treatment here rather than by lab-local colour choices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoorIdentityRole {
    /// A side objective room holding a keystone.
    KeystoneVault,
    /// A power room with a small yield.
    PowerCache,
    /// A higher-pressure power room with a larger yield.
    Reactor,
    /// A room that can stabilise or command the facility.
    Control,
    /// A room that reveals broad map knowledge.
    Survey,
    /// A room that feeds nearby knowledge into the team-local map.
    Sensor,
    /// A false exit signal: the door advertises escape, but the room is a decoy.
    FalseExit,
    /// A directly exposed decoy after the lie has been resolved.
    Decoy,
    /// A low-value or empty room.
    DeadEnd,
}

/// Gameplay-critical mesh outline roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutlineRole {
    /// A passable, observed door threshold.
    OpenDoor,
    /// A closed or dangerous door threshold.
    ClosedDoor,
    /// A usable console, switch, station, or socket.
    Interactable,
    /// A traversal hazard or pressure gate.
    Hazard,
    /// An opposing runner.
    Rival,
    /// A distant required destination.
    ObjectiveBeacon,
    /// A carryable or collectible item.
    Pickup,
    /// The local player / body proxy in debug views.
    LocalPlayer,
}

impl OutlineRole {
    pub const ALL: [OutlineRole; 8] = [
        OutlineRole::OpenDoor,
        OutlineRole::ClosedDoor,
        OutlineRole::Interactable,
        OutlineRole::Hazard,
        OutlineRole::Rival,
        OutlineRole::ObjectiveBeacon,
        OutlineRole::Pickup,
        OutlineRole::LocalPlayer,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OutlineRole::OpenDoor => "open door",
            OutlineRole::ClosedDoor => "closed door",
            OutlineRole::Interactable => "interactable",
            OutlineRole::Hazard => "hazard",
            OutlineRole::Rival => "rival",
            OutlineRole::ObjectiveBeacon => "objective beacon",
            OutlineRole::Pickup => "pickup",
            OutlineRole::LocalPlayer => "local player",
        }
    }
}

impl MarkerRole {
    pub const ALL: [MarkerRole; 8] = [
        MarkerRole::NextRoom,
        MarkerRole::Exit,
        MarkerRole::Control,
        MarkerRole::Collapse,
        MarkerRole::You,
        MarkerRole::Teammate,
        MarkerRole::Rival,
        MarkerRole::Director,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MarkerRole::NextRoom => "next-room beacon",
            MarkerRole::Exit => "exit",
            MarkerRole::Control => "control device",
            MarkerRole::Collapse => "collapse",
            MarkerRole::You => "you",
            MarkerRole::Teammate => "teammate",
            MarkerRole::Rival => "rival",
            MarkerRole::Director => "director",
        }
    }
}

impl DoorIdentityRole {
    pub const ALL: [DoorIdentityRole; 9] = [
        DoorIdentityRole::KeystoneVault,
        DoorIdentityRole::PowerCache,
        DoorIdentityRole::Reactor,
        DoorIdentityRole::Control,
        DoorIdentityRole::Survey,
        DoorIdentityRole::Sensor,
        DoorIdentityRole::FalseExit,
        DoorIdentityRole::Decoy,
        DoorIdentityRole::DeadEnd,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DoorIdentityRole::KeystoneVault => "door read: keystone vault",
            DoorIdentityRole::PowerCache => "door read: power cache",
            DoorIdentityRole::Reactor => "door read: reactor",
            DoorIdentityRole::Control => "door read: control",
            DoorIdentityRole::Survey => "door read: survey",
            DoorIdentityRole::Sensor => "door read: sensor",
            DoorIdentityRole::FalseExit => "door read: false exit signal",
            DoorIdentityRole::Decoy => "door read: decoy exposed",
            DoorIdentityRole::DeadEnd => "door read: dead-end",
        }
    }

    pub fn glyph(self) -> char {
        match self {
            DoorIdentityRole::KeystoneVault => 'K',
            DoorIdentityRole::PowerCache => 'P',
            DoorIdentityRole::Reactor => 'R',
            DoorIdentityRole::Control => 'C',
            DoorIdentityRole::Survey => 'S',
            DoorIdentityRole::Sensor => 'N',
            DoorIdentityRole::FalseExit => 'E',
            DoorIdentityRole::Decoy => '!',
            DoorIdentityRole::DeadEnd => '.',
        }
    }

    pub fn ambience_label(self) -> &'static str {
        match self {
            DoorIdentityRole::KeystoneVault => "key chime",
            DoorIdentityRole::PowerCache => "capacitor hum",
            DoorIdentityRole::Reactor => "reactor thrum",
            DoorIdentityRole::Control => "servo chatter",
            DoorIdentityRole::Survey => "wideband ping",
            DoorIdentityRole::Sensor => "local scan ticks",
            DoorIdentityRole::FalseExit => "exit choir",
            DoorIdentityRole::Decoy => "broken exit echo",
            DoorIdentityRole::DeadEnd => "dead air",
        }
    }
}

/// How an observed/decohering region currently reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservedState {
    /// Observed → frozen and solid.
    Frozen,
    /// Unobserved → free to rewire; reads as ghostly.
    Unobserved,
    /// Mid atomic swap → reads as a magenta pulse.
    Rerouting,
}

impl ObservedState {
    pub const ALL: [ObservedState; 3] = [
        ObservedState::Frozen,
        ObservedState::Unobserved,
        ObservedState::Rerouting,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ObservedState::Frozen => "frozen (observed)",
            ObservedState::Unobserved => "unobserved (ghost)",
            ObservedState::Rerouting => "rerouting (pulse)",
        }
    }
}

/// The neon-noir treatment for a structural surface.
pub fn surface(role: SurfaceRole) -> Treatment {
    match role {
        SurfaceRole::Plain => Treatment {
            base_color: Color::srgb(0.03, 0.04, 0.07),
            emissive: LinearRgba::rgb(0.10, 0.14, 0.22),
            signal: false,
            edge: Some(Color::srgb(0.10, 0.26, 0.40)),
        },
        SurfaceRole::Spine => Treatment {
            base_color: Color::srgb(0.06, 0.05, 0.02),
            emissive: LinearRgba::rgb(1.6, 1.05, 0.25),
            signal: false,
            edge: Some(Color::srgb(1.0, 0.78, 0.3)),
        },
        SurfaceRole::SafeBypass => Treatment {
            base_color: Color::srgb(0.02, 0.06, 0.07),
            emissive: LinearRgba::rgb(0.10, 1.2, 1.4),
            signal: false,
            edge: Some(Color::srgb(0.2, 0.9, 1.0)),
        },
        SurfaceRole::GantryDeck => Treatment {
            base_color: Color::srgb(0.035, 0.045, 0.060),
            emissive: LinearRgba::rgb(0.16, 0.22, 0.30),
            signal: false,
            edge: Some(Color::srgb(0.42, 0.92, 1.0)),
        },
        SurfaceRole::GantryEdge => Treatment {
            base_color: Color::srgb(0.08, 0.05, 0.01),
            emissive: LinearRgba::rgb(7.0, 4.8, 0.8),
            signal: true,
            edge: Some(Color::srgb(1.0, 0.84, 0.26)),
        },
        SurfaceRole::Understory => Treatment {
            base_color: Color::srgb(0.01, 0.06, 0.055),
            emissive: LinearRgba::rgb(0.55, 4.4, 3.8),
            signal: true,
            edge: Some(Color::srgb(0.16, 1.0, 0.82)),
        },
        SurfaceRole::TrapArmed => Treatment {
            base_color: Color::srgb(0.10, 0.0, 0.0),
            emissive: LinearRgba::rgb(9.0, 0.4, 0.2),
            signal: true,
            edge: Some(Color::srgb(1.0, 0.2, 0.1)),
        },
        SurfaceRole::TrapIdle => Treatment {
            base_color: Color::srgb(0.08, 0.04, 0.0),
            emissive: LinearRgba::rgb(1.0, 0.35, 0.05),
            signal: false,
            edge: Some(Color::srgb(0.9, 0.45, 0.1)),
        },
        SurfaceRole::Wall => Treatment {
            base_color: Color::srgb(0.04, 0.05, 0.08),
            emissive: LinearRgba::rgb(0.12, 0.18, 0.28),
            signal: false,
            edge: Some(Color::srgb(0.12, 0.4, 0.62)),
        },
        SurfaceRole::Ceiling => Treatment {
            base_color: Color::srgb(0.02, 0.02, 0.035),
            emissive: LinearRgba::rgb(0.08, 0.10, 0.16),
            signal: false,
            edge: None,
        },
    }
}

/// The neon-noir treatment for a gameplay marker (always signal-tier).
pub fn marker(role: MarkerRole) -> Treatment {
    let (base, emissive) = match role {
        MarkerRole::NextRoom => (Color::srgb(1.0, 0.82, 0.3), LinearRgba::rgb(6.0, 4.2, 1.0)),
        MarkerRole::Exit => (Color::srgb(0.2, 1.0, 0.4), LinearRgba::rgb(0.4, 8.0, 1.4)),
        MarkerRole::Control => (Color::srgb(0.6, 0.3, 1.0), LinearRgba::rgb(4.5, 1.2, 9.0)),
        MarkerRole::Collapse => (Color::srgb(1.0, 0.2, 0.15), LinearRgba::rgb(9.0, 0.8, 0.3)),
        MarkerRole::You => (Color::srgb(0.6, 0.95, 1.0), LinearRgba::rgb(2.0, 6.0, 6.5)),
        MarkerRole::Teammate => (Color::srgb(0.3, 0.6, 1.0), LinearRgba::rgb(0.8, 2.5, 8.0)),
        MarkerRole::Rival => (Color::srgb(1.0, 0.5, 0.15), LinearRgba::rgb(8.0, 2.6, 0.4)),
        MarkerRole::Director => (Color::srgb(1.0, 0.2, 0.8), LinearRgba::rgb(7.0, 0.6, 5.0)),
    };
    Treatment {
        base_color: base,
        emissive,
        signal: true,
        edge: Some(base),
    }
}

/// The neon-noir treatment for a doorframe semantic read. Door reads are
/// gameplay-critical choice cues, so they are signal-tier and carry meaning through
/// glyph, colour, and ambience label instead of colour alone.
pub fn door_identity(role: DoorIdentityRole) -> Treatment {
    let (base, emissive) = match role {
        DoorIdentityRole::KeystoneVault => {
            (Color::srgb(1.0, 0.82, 0.3), LinearRgba::rgb(6.0, 4.2, 1.0))
        }
        DoorIdentityRole::PowerCache => {
            (Color::srgb(0.25, 0.82, 1.0), LinearRgba::rgb(0.8, 5.6, 6.8))
        }
        DoorIdentityRole::Reactor => (Color::srgb(1.0, 0.48, 0.18), LinearRgba::rgb(8.0, 3.2, 0.6)),
        DoorIdentityRole::Control => (Color::srgb(0.6, 0.3, 1.0), LinearRgba::rgb(4.5, 1.2, 9.0)),
        DoorIdentityRole::Survey => (Color::srgb(0.48, 1.0, 0.52), LinearRgba::rgb(1.0, 6.5, 1.8)),
        DoorIdentityRole::Sensor => (
            Color::srgb(0.20, 0.95, 0.86),
            LinearRgba::rgb(0.7, 5.2, 5.2),
        ),
        DoorIdentityRole::FalseExit => (Color::srgb(0.2, 1.0, 0.4), LinearRgba::rgb(0.4, 8.0, 1.4)),
        DoorIdentityRole::Decoy => (Color::srgb(1.0, 0.28, 0.75), LinearRgba::rgb(8.0, 0.8, 6.0)),
        DoorIdentityRole::DeadEnd => (
            Color::srgb(0.58, 0.62, 0.70),
            LinearRgba::rgb(2.4, 2.4, 2.4),
        ),
    };
    Treatment {
        base_color: base,
        emissive,
        signal: true,
        edge: Some(base),
    }
}

/// The mesh-outline treatment for a gameplay-critical object. These are all
/// signal-tier because the whole point of the outline layer is to keep essential
/// gameplay state visible through fog, bloom, distance, and overlap.
pub fn outline(role: OutlineRole) -> OutlineTreatment {
    let (color, width) = match role {
        OutlineRole::OpenDoor => (marker(MarkerRole::Exit).base_color, 3.5),
        OutlineRole::ClosedDoor => (marker(MarkerRole::Collapse).base_color, 6.0),
        OutlineRole::Interactable => (marker(MarkerRole::Control).base_color, 5.0),
        OutlineRole::Hazard => (marker(MarkerRole::Collapse).base_color, 8.0),
        OutlineRole::Rival => (marker(MarkerRole::Rival).base_color, 6.5),
        OutlineRole::ObjectiveBeacon => (marker(MarkerRole::NextRoom).base_color, 9.0),
        OutlineRole::Pickup => (marker(MarkerRole::Teammate).base_color, 4.0),
        OutlineRole::LocalPlayer => (marker(MarkerRole::You).base_color, 7.0),
    };
    OutlineTreatment {
        color,
        width,
        signal: true,
    }
}

/// Every documented outline role and treatment, for lab/game legends.
pub fn outline_legend() -> Vec<(&'static str, OutlineTreatment)> {
    OutlineRole::ALL
        .iter()
        .map(|role| (role.label(), outline(*role)))
        .collect()
}

/// Every documented door identity and treatment, for lab/game legends.
pub fn door_identity_legend() -> Vec<(&'static str, Treatment)> {
    DoorIdentityRole::ALL
        .iter()
        .map(|role| (role.label(), door_identity(*role)))
        .collect()
}

/// Modulate a surface treatment by how it is currently observed. Unobserved reads
/// ghostly (dimmed); rerouting pulses magenta. Crucially, signal-tier treatments
/// keep their emission when unobserved, so a gameplay-critical cue never disappears
/// just because the player looked away.
pub fn observed_modulate(mut t: Treatment, state: ObservedState) -> Treatment {
    match state {
        ObservedState::Frozen => t,
        ObservedState::Unobserved => {
            t.base_color = dim(t.base_color, 0.5);
            if !t.signal {
                t.emissive = scale(t.emissive, 0.25);
            }
            t
        }
        ObservedState::Rerouting => {
            // A strong magenta pulse, bright enough to register even on a dark
            // surface and even at the edge of vision.
            t.emissive = LinearRgba::rgb(
                t.emissive.red + 7.0,
                t.emissive.green + 0.4,
                t.emissive.blue + 5.5,
            );
            t.signal = true;
            t
        }
    }
}

/// The widest a district may set the global ambient fill. Districts vary the *mood* of a
/// neighbourhood, but ambient must stay a fill — never a wash — so the dark neon-noir
/// surfaces and the emissive signals keep doing the talking.
pub const DISTRICT_MAX_AMBIENT_BRIGHTNESS: f32 = 200.0;
/// The dimmest ambient a district may use, so structural surfaces stay readable.
pub const DISTRICT_MIN_AMBIENT_BRIGHTNESS: f32 = 40.0;
/// The nearest a district's distance fog may begin, so the near field is always clear.
pub const DISTRICT_MIN_FOG_START: f32 = 8.0;

/// A neighbourhood of the megastructure. A district varies only *atmosphere* — ambient
/// fill, distance fog, light temperature, and a structural accent — never signal-tier
/// markers or hazards, so the world reads as distinct places while the Legibility
/// Contract holds everywhere. The mapping from graph region to district is deterministic
/// ([`district_for`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum District {
    /// Cold archival blue — the default, clinical baseline.
    Archive,
    /// Warm reactor amber — hot, close, faintly threatening.
    Reactor,
    /// Dim overgrown green — an atrium reclaimed by something.
    Atrium,
    /// Industrial orange — a half-built foundry.
    Foundry,
    /// Desaturated cool grey — unfinished, hollow, purpose abandoned mid-thought.
    Hollow,
    /// Teal spillway — flooded, distant, echoing.
    Spillway,
}

impl District {
    pub const ALL: [District; 6] = [
        District::Archive,
        District::Reactor,
        District::Atrium,
        District::Foundry,
        District::Hollow,
        District::Spillway,
    ];

    pub fn label(self) -> &'static str {
        match self {
            District::Archive => "archive",
            District::Reactor => "reactor",
            District::Atrium => "atrium",
            District::Foundry => "foundry",
            District::Hollow => "hollow",
            District::Spillway => "spillway",
        }
    }

    /// Stable index into [`District::ALL`] — lets a consumer key a parallel array (e.g.
    /// precreated per-district materials) by district.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&d| d == self).unwrap_or(0)
    }
}

/// A district's atmosphere parameters. All are presentation-only inputs a consumer feeds
/// to the global ambient light, the camera's distance fog, and the place fill lights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistrictPalette {
    /// Global ambient fill colour.
    pub ambient_color: Color,
    /// Global ambient fill brightness (a *fill*, bounded by [`DISTRICT_MAX_AMBIENT_BRIGHTNESS`]).
    pub ambient_brightness: f32,
    /// Distance-fog colour (kept dark so it reads as depth, not haze).
    pub fog_color: Color,
    /// Distance-fog linear start/end (world units).
    pub fog_start: f32,
    pub fog_end: f32,
    /// Tint for the place's structural fill lights (the "temperature" of the room).
    pub light_color: Color,
    /// A non-signal structural accent emission for the neighbourhood (kept below the
    /// signal floor so it never competes with a real gameplay cue).
    pub accent: LinearRgba,
}

/// The atmosphere palette for a district.
pub fn district(d: District) -> DistrictPalette {
    match d {
        District::Archive => DistrictPalette {
            ambient_color: Color::srgb(0.34, 0.42, 0.62),
            ambient_brightness: 80.0,
            fog_color: Color::srgb(0.010, 0.015, 0.035),
            fog_start: 10.0,
            fog_end: 28.0,
            light_color: Color::srgb(0.72, 0.86, 1.0),
            accent: LinearRgba::rgb(0.10, 0.30, 0.55),
        },
        District::Reactor => DistrictPalette {
            ambient_color: Color::srgb(0.52, 0.38, 0.28),
            ambient_brightness: 70.0,
            fog_color: Color::srgb(0.040, 0.020, 0.012),
            fog_start: 8.0,
            fog_end: 24.0,
            light_color: Color::srgb(1.0, 0.78, 0.52),
            accent: LinearRgba::rgb(0.95, 0.45, 0.12),
        },
        District::Atrium => DistrictPalette {
            ambient_color: Color::srgb(0.30, 0.46, 0.34),
            ambient_brightness: 60.0,
            fog_color: Color::srgb(0.012, 0.030, 0.016),
            fog_start: 12.0,
            fog_end: 32.0,
            light_color: Color::srgb(0.66, 0.96, 0.72),
            accent: LinearRgba::rgb(0.18, 0.70, 0.30),
        },
        District::Foundry => DistrictPalette {
            ambient_color: Color::srgb(0.50, 0.36, 0.26),
            ambient_brightness: 75.0,
            fog_color: Color::srgb(0.036, 0.020, 0.013),
            fog_start: 8.0,
            fog_end: 22.0,
            light_color: Color::srgb(1.0, 0.68, 0.42),
            accent: LinearRgba::rgb(1.0, 0.52, 0.10),
        },
        District::Hollow => DistrictPalette {
            ambient_color: Color::srgb(0.42, 0.45, 0.50),
            ambient_brightness: 65.0,
            fog_color: Color::srgb(0.020, 0.022, 0.026),
            fog_start: 14.0,
            fog_end: 36.0,
            light_color: Color::srgb(0.82, 0.86, 0.92),
            accent: LinearRgba::rgb(0.35, 0.40, 0.48),
        },
        District::Spillway => DistrictPalette {
            ambient_color: Color::srgb(0.26, 0.46, 0.50),
            ambient_brightness: 75.0,
            fog_color: Color::srgb(0.010, 0.026, 0.030),
            fog_start: 10.0,
            fog_end: 26.0,
            light_color: Color::srgb(0.50, 0.95, 0.98),
            accent: LinearRgba::rgb(0.12, 0.60, 0.62),
        },
    }
}

/// Deterministic district for a region key (e.g. a room index), stable per facility
/// `seed`, so a neighbourhood keeps its identity across a match.
pub fn district_for(seed: u64, key: u32) -> District {
    let mut h = seed ^ (key as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    District::ALL[(h % District::ALL.len() as u64) as usize]
}

/// Every documented role and its treatment — the single source of truth for an
/// on-screen legend. A consumer renders this so no coloured marker is unexplained.
pub fn legend() -> Vec<(&'static str, Treatment)> {
    let mut out = Vec::with_capacity(
        SurfaceRole::ALL.len() + MarkerRole::ALL.len() + DoorIdentityRole::ALL.len(),
    );
    for role in SurfaceRole::ALL {
        out.push((role.label(), surface(role)));
    }
    for role in MarkerRole::ALL {
        out.push((role.label(), marker(role)));
    }
    for role in DoorIdentityRole::ALL {
        out.push((role.label(), door_identity(role)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_roles_are_visually_distinct() {
        let treatments: Vec<Treatment> = SurfaceRole::ALL.iter().map(|r| surface(*r)).collect();
        for i in 0..treatments.len() {
            for j in (i + 1)..treatments.len() {
                assert!(
                    treatments[i].base_color != treatments[j].base_color
                        || treatments[i].emissive != treatments[j].emissive,
                    "{} and {} look identical",
                    SurfaceRole::ALL[i].label(),
                    SurfaceRole::ALL[j].label(),
                );
            }
        }
    }

    #[test]
    fn marker_roles_are_visually_distinct() {
        let treatments: Vec<Treatment> = MarkerRole::ALL.iter().map(|r| marker(*r)).collect();
        for i in 0..treatments.len() {
            for j in (i + 1)..treatments.len() {
                assert!(
                    treatments[i].base_color != treatments[j].base_color
                        || treatments[i].emissive != treatments[j].emissive,
                    "{} and {} look identical",
                    MarkerRole::ALL[i].label(),
                    MarkerRole::ALL[j].label(),
                );
            }
        }
    }

    #[test]
    fn door_identity_roles_are_distinct_and_glyph_backed() {
        let treatments: Vec<Treatment> = DoorIdentityRole::ALL
            .iter()
            .map(|r| door_identity(*r))
            .collect();
        let mut glyphs = Vec::new();
        for i in 0..treatments.len() {
            glyphs.push(DoorIdentityRole::ALL[i].glyph());
            for j in (i + 1)..treatments.len() {
                assert!(
                    treatments[i].base_color != treatments[j].base_color
                        || treatments[i].emissive != treatments[j].emissive
                        || DoorIdentityRole::ALL[i].glyph() != DoorIdentityRole::ALL[j].glyph(),
                    "{} and {} look identical",
                    DoorIdentityRole::ALL[i].label(),
                    DoorIdentityRole::ALL[j].label(),
                );
            }
        }
        glyphs.sort_unstable();
        glyphs.dedup();
        assert_eq!(
            glyphs.len(),
            DoorIdentityRole::ALL.len(),
            "door identity glyphs are a non-colour channel and must stay unique",
        );
    }

    #[test]
    fn outline_roles_are_signal_tier_and_have_semantic_widths() {
        let mut widths = Vec::new();
        for role in OutlineRole::ALL {
            let treatment = outline(role);
            assert!(treatment.signal, "{} must be signal-tier", role.label());
            assert!(
                treatment.width >= OUTLINE_MIN_WIDTH,
                "{} outline is too narrow",
                role.label(),
            );
            widths.push(treatment.width);
        }
        widths.sort_by(|a, b| a.partial_cmp(b).unwrap());
        widths.dedup();
        assert!(
            widths.len() >= 5,
            "outline roles should not rely on colour alone"
        );
    }

    #[test]
    fn outline_colours_keep_contrast_under_colour_vision_preview() {
        for role in OutlineRole::ALL {
            let treatment = outline(role);
            for mode in ColorVisionMode::ALL {
                let simulated = simulate_color_vision(treatment.color, mode);
                assert!(
                    luminance(simulated) >= OUTLINE_MIN_SIMULATED_LUMINANCE,
                    "{} outline is too dim under {}: {simulated:?}",
                    role.label(),
                    mode.label(),
                );
            }
        }
    }

    #[test]
    fn every_marker_is_signal_tier() {
        for role in MarkerRole::ALL {
            assert!(marker(role).signal, "{} must be a signal", role.label());
        }
    }

    #[test]
    fn signals_punch_through_the_atmosphere() {
        // Every signal-tier treatment — markers, signal surfaces, and any observed
        // form that becomes a signal — must clear the legibility floor.
        let mut signals: Vec<Treatment> = Vec::new();
        for role in MarkerRole::ALL {
            signals.push(marker(role));
        }
        for role in DoorIdentityRole::ALL {
            signals.push(door_identity(role));
        }
        for role in SurfaceRole::ALL {
            let base = surface(role);
            if base.signal {
                signals.push(base);
            }
            for state in ObservedState::ALL {
                let t = observed_modulate(surface(role), state);
                if t.signal {
                    signals.push(t);
                }
            }
        }
        for t in signals {
            assert!(
                luminance(t.emissive) >= SIGNAL_MIN_LUMINANCE,
                "a signal-tier treatment is too dim to read through fog: {t:?}",
            );
        }
    }

    #[test]
    fn atmosphere_surfaces_stay_dark() {
        for role in [SurfaceRole::Plain, SurfaceRole::Wall, SurfaceRole::Ceiling] {
            let t = surface(role);
            assert!(
                luminance(t.base_color.to_linear()) < ATMOSPHERE_MAX_LUMINANCE,
                "{} should be dark atmosphere",
                role.label(),
            );
            assert!(!t.signal);
        }
    }

    #[test]
    fn unobserved_surface_is_dimmer_than_frozen() {
        let frozen = observed_modulate(surface(SurfaceRole::Spine), ObservedState::Frozen);
        let unobserved = observed_modulate(surface(SurfaceRole::Spine), ObservedState::Unobserved);
        assert!(luminance(unobserved.emissive) < luminance(frozen.emissive));
    }

    #[test]
    fn armed_trap_stays_legible_even_when_unobserved() {
        let t = observed_modulate(surface(SurfaceRole::TrapArmed), ObservedState::Unobserved);
        assert!(t.signal);
        assert!(luminance(t.emissive) >= SIGNAL_MIN_LUMINANCE);
    }

    #[test]
    fn rerouting_is_a_signal_on_any_surface() {
        for role in SurfaceRole::ALL {
            let t = observed_modulate(surface(role), ObservedState::Rerouting);
            assert!(t.signal);
            assert!(
                luminance(t.emissive) >= SIGNAL_MIN_LUMINANCE,
                "rerouting {} must read as a pulse",
                role.label(),
            );
        }
    }

    #[test]
    fn district_palettes_are_distinct() {
        let pals: Vec<DistrictPalette> = District::ALL.iter().map(|d| district(*d)).collect();
        for i in 0..pals.len() {
            for j in (i + 1)..pals.len() {
                assert!(
                    pals[i].ambient_color != pals[j].ambient_color
                        || pals[i].light_color != pals[j].light_color
                        || pals[i].fog_color != pals[j].fog_color,
                    "districts {} and {} look identical",
                    District::ALL[i].label(),
                    District::ALL[j].label(),
                );
            }
        }
    }

    #[test]
    fn district_atmosphere_stays_within_legible_bounds() {
        // Ambient is a bounded fill, fog is ordered and never crowds the near field, and
        // the structural accent stays below the signal floor — so districts never wash out
        // the neon-noir surfaces or masquerade as a gameplay signal.
        for d in District::ALL {
            let p = district(d);
            assert!(
                (DISTRICT_MIN_AMBIENT_BRIGHTNESS..=DISTRICT_MAX_AMBIENT_BRIGHTNESS)
                    .contains(&p.ambient_brightness),
                "{} ambient brightness out of bounds: {}",
                d.label(),
                p.ambient_brightness,
            );
            assert!(
                p.fog_start >= DISTRICT_MIN_FOG_START && p.fog_end > p.fog_start + 10.0,
                "{} fog range is not legible: {}..{}",
                d.label(),
                p.fog_start,
                p.fog_end,
            );
            assert!(
                luminance(p.accent) < SIGNAL_MIN_LUMINANCE,
                "{} accent must not read as a signal",
                d.label(),
            );
        }
    }

    #[test]
    fn district_assignment_is_deterministic_and_covers_the_set() {
        // Stable for a key, and across a facility's rooms every district can appear (the
        // mapping isn't degenerate).
        assert_eq!(district_for(42, 3), district_for(42, 3));
        let mut seen: Vec<District> = Vec::new();
        for key in 0..64u32 {
            let d = district_for(7, key);
            if !seen.contains(&d) {
                seen.push(d);
            }
        }
        assert!(
            seen.len() >= 4,
            "district mapping should spread across the set, saw {}",
            seen.len()
        );
    }

    #[test]
    fn legend_covers_every_role_uniquely() {
        let legend = legend();
        assert_eq!(
            legend.len(),
            SurfaceRole::ALL.len() + MarkerRole::ALL.len() + DoorIdentityRole::ALL.len(),
        );
        let mut labels: Vec<&str> = legend.iter().map(|(name, _)| *name).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), legend.len(), "every legend entry is unique");
    }

    #[test]
    fn door_identity_legend_covers_every_role_uniquely() {
        let legend = door_identity_legend();
        assert_eq!(legend.len(), DoorIdentityRole::ALL.len());
        let mut labels: Vec<&str> = legend.iter().map(|(name, _)| *name).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(
            labels.len(),
            legend.len(),
            "every door identity legend entry is unique"
        );
    }

    #[test]
    fn outline_legend_covers_every_role_uniquely() {
        let legend = outline_legend();
        assert_eq!(legend.len(), OutlineRole::ALL.len());
        let mut labels: Vec<&str> = legend.iter().map(|(name, _)| *name).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(
            labels.len(),
            legend.len(),
            "every outline legend entry is unique"
        );
    }
}
