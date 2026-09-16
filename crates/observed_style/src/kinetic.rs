//! Kinetic chamber legend. Signals keep their emission when the generator is off.
use crate::{MarkerRole, SurfaceRole, Treatment, marker, surface};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Floor,
    Wall,
    Catwalk,
    Landing,
    Hazard,
    Minor,
    Prop,
    Target,
    Push,
    Pull,
    Powered,
    Unpowered,
    Text,
    Panel,
}

fn material(rgb: [f32; 3]) -> Treatment {
    Treatment {
        base_color: bevy::color::Color::srgb(rgb[0], rgb[1], rgb[2]),
        emissive: bevy::color::LinearRgba::BLACK,
        signal: false,
        edge: None,
    }
}
pub fn treatment(role: Role) -> Treatment {
    match role {
        Role::Floor => material([0.075, 0.10, 0.12]),
        Role::Wall => material([0.10, 0.14, 0.17]),
        Role::Catwalk => material([0.16, 0.20, 0.22]),
        Role::Landing => surface(SurfaceRole::Understory),
        Role::Hazard => marker(MarkerRole::Collapse),
        Role::Minor => marker(MarkerRole::Director),
        Role::Prop => material([0.30, 0.22, 0.12]),
        Role::Target => marker(MarkerRole::You),
        Role::Text => material([0.83, 0.91, 0.91]),
        Role::Push => marker(MarkerRole::NextRoom),
        Role::Pull => marker(MarkerRole::Teammate),
        Role::Powered => marker(MarkerRole::Control),
        Role::Unpowered => material([0.28, 0.31, 0.33]),
        Role::Panel => material([0.018, 0.027, 0.036]),
    }
}
pub const LEGEND: &[(Role, &str)] = &[
    (Role::Minor, "MINOR / moving threat"),
    (Role::Prop, "CRATE / movable mass"),
    (Role::Target, "BRACKETS / selected target"),
    (Role::Hazard, "STRIPES / unsafe edge or retracting support"),
    (Role::Landing, "LOWER DECK / survivable landing"),
    (Role::Powered, "ON / station supplies charge"),
    (Role::Unpowered, "OFF / station unavailable"),
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kinetic_signals_keep_the_shared_emission_floor() {
        for (role, _) in LEGEND {
            let t = treatment(*role);
            if t.signal {
                assert!(
                    crate::luminance(t.emissive) >= crate::SIGNAL_MIN_LUMINANCE,
                    "{role:?}"
                );
            }
        }
    }
}
