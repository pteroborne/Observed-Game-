//! The shared **visual language** for Observed 2: a pure mapping from *semantic
//! state* to *visual treatment*. Presentation code (the labs and the assembled
//! `game`) asks this crate how to draw a thing; it never invents ad-hoc colours.
//!
//! Art direction is **neon-noir** — dark surfaces, neon emission, fog and bloom —
//! generated entirely from code (no authored textures/meshes), which plays to an
//! agent's strengths and is verifiable through the `OBSERVED2_CAPTURE` screenshot
//! loop. The crate encodes the **Legibility Contract**: any treatment flagged as a
//! *signal* (your path, threats, interactables, actors) must stay bright enough to
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
    pub const ALL: [SurfaceRole; 7] = [
        SurfaceRole::Plain,
        SurfaceRole::Spine,
        SurfaceRole::SafeBypass,
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
            emissive: LinearRgba::rgb(0.0, 0.02, 0.05),
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
            emissive: LinearRgba::rgb(0.0, 0.0, 0.0),
            signal: false,
            edge: Some(Color::srgb(0.12, 0.4, 0.62)),
        },
        SurfaceRole::Ceiling => Treatment {
            base_color: Color::srgb(0.02, 0.02, 0.035),
            emissive: LinearRgba::rgb(0.0, 0.0, 0.0),
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

/// Every documented role and its treatment — the single source of truth for an
/// on-screen legend. A consumer renders this so no coloured marker is unexplained.
pub fn legend() -> Vec<(&'static str, Treatment)> {
    let mut out = Vec::with_capacity(SurfaceRole::ALL.len() + MarkerRole::ALL.len());
    for role in SurfaceRole::ALL {
        out.push((role.label(), surface(role)));
    }
    for role in MarkerRole::ALL {
        out.push((role.label(), marker(role)));
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
    fn legend_covers_every_role_uniquely() {
        let legend = legend();
        assert_eq!(legend.len(), SurfaceRole::ALL.len() + MarkerRole::ALL.len(),);
        let mut labels: Vec<&str> = legend.iter().map(|(name, _)| *name).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), legend.len(), "every legend entry is unique");
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
