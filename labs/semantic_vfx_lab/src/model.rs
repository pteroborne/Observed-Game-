//! Pure semantic VFX data for the A4 lab.
//!
//! This module names the gameplay events, maps them to the shared semantic style
//! table, and provides a deterministic event timeline. The Bevy/Hanabi layer only
//! projects these events; it does not own or advance gameplay state.

use observed_style::{
    MarkerRole, ObservedState, SurfaceRole, Treatment, marker, observed_modulate, surface,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl EventPosition {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SemanticVfxRole {
    DoorOpen,
    DoorSlam,
    RerouteFlash,
    PressureGate,
    KeystonePickup,
    ExitUnlock,
    EquipmentPower,
}

impl SemanticVfxRole {
    pub const ALL: [Self; 7] = [
        Self::DoorOpen,
        Self::DoorSlam,
        Self::RerouteFlash,
        Self::PressureGate,
        Self::KeystonePickup,
        Self::ExitUnlock,
        Self::EquipmentPower,
    ];

    pub const DISCRETE: [Self; 6] = [
        Self::DoorOpen,
        Self::PressureGate,
        Self::RerouteFlash,
        Self::KeystonePickup,
        Self::DoorSlam,
        Self::ExitUnlock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::DoorOpen => "door open",
            Self::DoorSlam => "door slam",
            Self::RerouteFlash => "reroute flash",
            Self::PressureGate => "pressure gate",
            Self::KeystonePickup => "keystone pickup",
            Self::ExitUnlock => "exit unlock",
            Self::EquipmentPower => "equipment power",
        }
    }

    pub fn treatment(self) -> Treatment {
        match self {
            Self::DoorOpen => marker(MarkerRole::Exit),
            Self::DoorSlam => marker(MarkerRole::Collapse),
            Self::RerouteFlash => {
                observed_modulate(surface(SurfaceRole::Spine), ObservedState::Rerouting)
            }
            Self::PressureGate => surface(SurfaceRole::TrapArmed),
            Self::KeystonePickup => marker(MarkerRole::Control),
            Self::ExitUnlock => marker(MarkerRole::Exit),
            Self::EquipmentPower => marker(MarkerRole::Teammate),
        }
    }

    pub fn spec(self) -> VfxSpec {
        match self {
            Self::DoorOpen => VfxSpec::burst(22, 0.45, 0.22, 1.0, 3.5, 0.75),
            Self::DoorSlam => VfxSpec::burst(28, 0.55, 0.22, 1.35, 4.0, 0.85),
            Self::RerouteFlash => VfxSpec::burst(34, 0.55, 0.42, 1.25, 4.0, 0.85),
            Self::PressureGate => VfxSpec::burst(30, 0.45, 0.28, 1.45, 3.8, 0.75),
            Self::KeystonePickup => VfxSpec::burst(24, 0.55, 0.22, 1.1, 3.5, 0.85),
            Self::ExitUnlock => VfxSpec::burst(38, 0.65, 0.38, 1.25, 4.2, 0.95),
            Self::EquipmentPower => VfxSpec::continuous(12, 0.75, 0.18, 0.45, 3.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxSpec {
    pub particle_count: u32,
    pub lifetime_secs: f32,
    pub radius: f32,
    pub speed: f32,
    pub size_px: f32,
    pub cleanup_secs: f32,
    pub continuous: bool,
}

impl VfxSpec {
    const fn burst(
        particle_count: u32,
        lifetime_secs: f32,
        radius: f32,
        speed: f32,
        size_px: f32,
        cleanup_secs: f32,
    ) -> Self {
        Self {
            particle_count,
            lifetime_secs,
            radius,
            speed,
            size_px,
            cleanup_secs,
            continuous: false,
        }
    }

    const fn continuous(
        particle_count: u32,
        lifetime_secs: f32,
        radius: f32,
        speed: f32,
        size_px: f32,
    ) -> Self {
        Self {
            particle_count,
            lifetime_secs,
            radius,
            speed,
            size_px,
            cleanup_secs: f32::INFINITY,
            continuous: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScheduledVfx {
    pub role: SemanticVfxRole,
    pub at_secs: f32,
    pub position: EventPosition,
    pub seed: u32,
}

impl ScheduledVfx {
    pub const fn new(role: SemanticVfxRole, at_secs: f32, position: EventPosition) -> Self {
        Self {
            role,
            at_secs,
            position,
            seed: role_seed(role),
        }
    }
}

pub const SEQUENCE_SECONDS: f32 = 7.2;

pub fn canonical_sequence() -> Vec<ScheduledVfx> {
    vec![
        ScheduledVfx::new(
            SemanticVfxRole::DoorOpen,
            0.8,
            EventPosition::new(-2.9, 2.45, 5.8),
        ),
        ScheduledVfx::new(
            SemanticVfxRole::PressureGate,
            1.7,
            EventPosition::new(0.0, 2.75, -0.9),
        ),
        ScheduledVfx::new(
            SemanticVfxRole::RerouteFlash,
            2.8,
            EventPosition::new(0.0, 0.85, -4.2),
        ),
        ScheduledVfx::new(
            SemanticVfxRole::KeystonePickup,
            3.8,
            EventPosition::new(-3.8, 1.35, 0.9),
        ),
        ScheduledVfx::new(
            SemanticVfxRole::DoorSlam,
            4.8,
            EventPosition::new(3.6, 2.95, 2.9),
        ),
        ScheduledVfx::new(
            SemanticVfxRole::ExitUnlock,
            5.9,
            EventPosition::new(0.0, 4.3, -14.0),
        ),
    ]
}

pub fn crowded_sequence() -> Vec<ScheduledVfx> {
    SemanticVfxRole::DISCRETE
        .iter()
        .enumerate()
        .map(|(index, role)| {
            let mut event = canonical_sequence()
                .into_iter()
                .find(|event| event.role == *role)
                .expect("every discrete role exists in the canonical sequence");
            event.at_secs = 0.0;
            event.seed = role_seed(*role) + 10_000 + index as u32;
            event
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct VfxTimeline {
    events: Vec<ScheduledVfx>,
    next_index: usize,
}

impl Default for VfxTimeline {
    fn default() -> Self {
        Self::new(canonical_sequence())
    }
}

impl VfxTimeline {
    pub fn new(events: Vec<ScheduledVfx>) -> Self {
        Self {
            events,
            next_index: 0,
        }
    }

    pub fn reset(&mut self) {
        self.next_index = 0;
    }

    pub fn ready_until(&mut self, elapsed_secs: f32) -> Vec<ScheduledVfx> {
        let start = self.next_index;
        while self.next_index < self.events.len()
            && self.events[self.next_index].at_secs <= elapsed_secs
        {
            self.next_index += 1;
        }
        self.events[start..self.next_index].to_vec()
    }
}

const fn role_seed(role: SemanticVfxRole) -> u32 {
    match role {
        SemanticVfxRole::DoorOpen => 10_101,
        SemanticVfxRole::DoorSlam => 20_202,
        SemanticVfxRole::RerouteFlash => 30_303,
        SemanticVfxRole::PressureGate => 40_404,
        SemanticVfxRole::KeystonePickup => 50_505,
        SemanticVfxRole::ExitUnlock => 60_606,
        SemanticVfxRole::EquipmentPower => 70_707,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_style::{SIGNAL_MIN_LUMINANCE, luminance};

    #[test]
    fn every_role_uses_signal_tier_style() {
        for role in SemanticVfxRole::ALL {
            let treatment = role.treatment();
            assert!(treatment.signal, "{} must be signal-tier", role.label());
            assert!(
                luminance(treatment.emissive) >= SIGNAL_MIN_LUMINANCE,
                "{} VFX is too dim for fog/bloom",
                role.label(),
            );
        }
    }

    #[test]
    fn canonical_sequence_covers_every_discrete_role_in_order() {
        let sequence = canonical_sequence();
        let roles: Vec<SemanticVfxRole> = sequence.iter().map(|event| event.role).collect();
        assert_eq!(roles, SemanticVfxRole::DISCRETE);
        assert!(
            sequence
                .windows(2)
                .all(|pair| pair[0].at_secs < pair[1].at_secs)
        );
    }

    #[test]
    fn timeline_projects_each_event_once() {
        let mut timeline = VfxTimeline::default();
        assert!(timeline.ready_until(0.1).is_empty());
        assert_eq!(timeline.ready_until(1.0).len(), 1);
        assert!(timeline.ready_until(1.0).is_empty());
        assert_eq!(timeline.ready_until(SEQUENCE_SECONDS).len(), 5);
    }

    #[test]
    fn vfx_specs_are_kept_readable_not_screen_filling() {
        for role in SemanticVfxRole::ALL {
            let spec = role.spec();
            assert!(
                spec.lifetime_secs <= 0.75,
                "{} lasts too long",
                role.label()
            );
            assert!(
                spec.size_px <= 4.2,
                "{} particles are too large",
                role.label()
            );
            assert!(
                spec.particle_count <= 38,
                "{} spawns too many particles for a readability lab",
                role.label(),
            );
        }
    }
}
