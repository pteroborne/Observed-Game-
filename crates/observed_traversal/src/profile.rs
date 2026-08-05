//! Complete immutable traversal runtime identity.

use sha2::{Digest, Sha256};

use crate::{FollowerConfig, FpsConfig};

/// Version of the compatibility spine/deck guide semantics hashed into a
/// runtime profile. Graph-backed guides increment this only when their
/// interpretation changes, not merely when authored nodes change.
pub const TRAVERSAL_GUIDE_CONTRACT_VERSION: u16 = 1;

const PROFILE_HASH_VERSION: u16 = 2;
const PROFILE_HASH_DOMAIN: &[u8] = b"observed2.traversal-runtime-profile";

/// Physical clearance requirements derived from the controller body.
///
/// Authoring and certification consume this value instead of independently
/// copying capsule, step, slope, and ground-contact constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraversalRequirements {
    pub capsule_radius: f32,
    pub capsule_half_height: f32,
    pub required_headroom: f32,
    pub eye_height: f32,
    pub step_height: f32,
    pub controller_offset: f32,
    pub minimum_step_width: f32,
    pub maximum_slope_degrees: f32,
    pub minimum_slope_slide_degrees: f32,
    pub ground_snap: f32,
}

impl From<&FpsConfig> for TraversalRequirements {
    fn from(config: &FpsConfig) -> Self {
        Self::from_runtime(config, RapierKinematicSettings::shipped(config))
    }
}

impl TraversalRequirements {
    fn from_runtime(config: &FpsConfig, rapier: RapierKinematicSettings) -> Self {
        Self {
            capsule_radius: config.radius,
            capsule_half_height: config.half_height,
            required_headroom: config.half_height * 2.0,
            eye_height: config.eye_height,
            step_height: config.step_height,
            controller_offset: rapier.controller_offset,
            minimum_step_width: rapier.minimum_step_width,
            maximum_slope_degrees: rapier.maximum_slope_degrees,
            minimum_slope_slide_degrees: rapier.minimum_slope_slide_degrees,
            ground_snap: rapier.ground_snap,
        }
    }
}

/// Effective Rapier character-controller values shipped by the game.
///
/// These values were historically literals inside `step_character`, while
/// similarly named compatibility DTO fields carried different numbers. Making
/// the effective values explicit lets profile hashes and certificates identify
/// the physics that actually ran without changing that physics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RapierKinematicSettings {
    pub controller_offset: f32,
    pub minimum_step_width: f32,
    pub maximum_slope_degrees: f32,
    pub minimum_slope_slide_degrees: f32,
    pub ground_snap: f32,
}

impl RapierKinematicSettings {
    #[must_use]
    pub fn shipped(config: &FpsConfig) -> Self {
        Self {
            controller_offset: 0.01,
            minimum_step_width: config.radius * 1.25,
            maximum_slope_degrees: 50.0,
            minimum_slope_slide_degrees: 52.0,
            ground_snap: 0.08,
        }
    }
}

/// Every value needed to run and identify local traversal.
///
/// Construction derives requirements and the canonical hash together, so an
/// immutable profile cannot carry a stale clearance contract or identity.
#[derive(Clone, Debug, PartialEq)]
pub struct TraversalRuntimeProfile {
    controller: FpsConfig,
    rapier: RapierKinematicSettings,
    follower: FollowerConfig,
    requirements: TraversalRequirements,
    guide_contract_version: u16,
    profile_hash: [u8; 32],
}

impl TraversalRuntimeProfile {
    /// The shipped canonical hex-match profile.
    #[must_use]
    pub fn canonical_hex() -> Self {
        let mut controller = FpsConfig::deliberate_rapier();
        // Hex bot look commands historically carry applied radians per tick.
        // Keeping a unit look step preserves every replay/body trace while the
        // follower now performs the general look-step conversion explicitly.
        controller.look_step = 1.0;
        Self::new(
            controller,
            FollowerConfig::default(),
            TRAVERSAL_GUIDE_CONTRACT_VERSION,
        )
    }

    /// Build a complete profile around an existing controller using the
    /// compatibility follower and guide semantics.
    #[must_use]
    pub fn from_controller(controller: FpsConfig) -> Self {
        Self::new(
            controller,
            FollowerConfig::default(),
            TRAVERSAL_GUIDE_CONTRACT_VERSION,
        )
    }

    #[must_use]
    pub fn new(
        controller: FpsConfig,
        follower: FollowerConfig,
        guide_contract_version: u16,
    ) -> Self {
        let rapier = RapierKinematicSettings::shipped(&controller);
        Self::new_with_rapier(controller, rapier, follower, guide_contract_version)
    }

    #[must_use]
    pub fn new_with_rapier(
        controller: FpsConfig,
        rapier: RapierKinematicSettings,
        follower: FollowerConfig,
        guide_contract_version: u16,
    ) -> Self {
        let requirements = TraversalRequirements::from_runtime(&controller, rapier);
        let profile_hash =
            canonical_profile_hash(controller, rapier, follower, guide_contract_version);
        Self {
            controller,
            rapier,
            follower,
            requirements,
            guide_contract_version,
            profile_hash,
        }
    }

    #[must_use]
    pub fn controller(&self) -> FpsConfig {
        self.controller
    }

    #[must_use]
    pub fn rapier(&self) -> RapierKinematicSettings {
        self.rapier
    }

    #[must_use]
    pub fn follower(&self) -> FollowerConfig {
        self.follower
    }

    #[must_use]
    pub fn requirements(&self) -> TraversalRequirements {
        self.requirements
    }

    #[must_use]
    pub fn guide_contract_version(&self) -> u16 {
        self.guide_contract_version
    }

    /// Domain-separated SHA-256 over all controller/follower fields and guide
    /// semantics. This identity is intentionally not part of the current
    /// simulation content hash until the contract migration publishes it.
    #[must_use]
    pub fn profile_hash(&self) -> [u8; 32] {
        self.profile_hash
    }
}

fn canonical_profile_hash(
    controller: FpsConfig,
    rapier: RapierKinematicSettings,
    follower: FollowerConfig,
    guide_contract_version: u16,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_HASH_DOMAIN);
    hash.update(PROFILE_HASH_VERSION.to_le_bytes());
    for value in [
        controller.walk_speed,
        controller.run_speed,
        controller.ground_accel,
        controller.ground_decel,
        controller.air_accel,
        controller.gravity,
        controller.max_fall,
        controller.jump_speed,
        controller.jump_cooldown,
        controller.radius,
        controller.half_height,
        controller.eye_height,
        controller.look_step,
        controller.pitch_limit,
        controller.substep,
        controller.step_height,
        controller.controller_offset,
        controller.minimum_step_width,
        controller.maximum_slope_degrees,
        controller.ground_snap,
        follower.max_turn_per_tick,
        follower.climb_capture_radius,
        follower.movement_scale,
        rapier.controller_offset,
        rapier.minimum_step_width,
        rapier.maximum_slope_degrees,
        rapier.minimum_slope_slide_degrees,
        rapier.ground_snap,
    ] {
        hash.update(value.to_bits().to_le_bytes());
    }
    hash.update(guide_contract_version.to_le_bytes());
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_hex_preserves_the_shipped_controller_and_requirements() {
        let profile = TraversalRuntimeProfile::canonical_hex();
        let mut controller = FpsConfig::deliberate_rapier();
        controller.look_step = 1.0;

        assert_eq!(profile.controller(), controller);
        assert_eq!(
            profile.rapier(),
            RapierKinematicSettings::shipped(&controller)
        );
        assert_eq!(profile.follower(), FollowerConfig::default());
        assert_eq!(
            profile.requirements(),
            TraversalRequirements::from(&controller)
        );
        assert_eq!(profile.requirements().required_headroom, 1.8);
        assert_eq!(
            profile.guide_contract_version(),
            TRAVERSAL_GUIDE_CONTRACT_VERSION
        );
        let hash = profile
            .profile_hash()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hash,
            "29e48ecf1d206cd58a23a5f6e4db63ddff55aa2e99820f1425e8b1ce0d8415e6"
        );
    }

    #[test]
    fn canonical_hash_covers_every_controller_and_follower_field() {
        let base = TraversalRuntimeProfile::canonical_hex();
        let controller = base.controller();
        let rapier = base.rapier();
        let follower = base.follower();
        let expected = base.profile_hash();

        assert_eq!(
            expected,
            TraversalRuntimeProfile::new(controller, follower, TRAVERSAL_GUIDE_CONTRACT_VERSION)
                .profile_hash(),
            "identical bits must produce an identical profile identity"
        );

        macro_rules! changed_controller_field {
            ($field:ident) => {{
                let mut changed = controller;
                changed.$field = f32::from_bits(changed.$field.to_bits().wrapping_add(1));
                assert_ne!(
                    TraversalRuntimeProfile::new(
                        changed,
                        follower,
                        TRAVERSAL_GUIDE_CONTRACT_VERSION
                    )
                    .profile_hash(),
                    expected,
                    "controller field `{}` must participate in the hash",
                    stringify!($field)
                );
            }};
        }
        changed_controller_field!(walk_speed);
        changed_controller_field!(run_speed);
        changed_controller_field!(ground_accel);
        changed_controller_field!(ground_decel);
        changed_controller_field!(air_accel);
        changed_controller_field!(gravity);
        changed_controller_field!(max_fall);
        changed_controller_field!(jump_speed);
        changed_controller_field!(jump_cooldown);
        changed_controller_field!(radius);
        changed_controller_field!(half_height);
        changed_controller_field!(eye_height);
        changed_controller_field!(look_step);
        changed_controller_field!(pitch_limit);
        changed_controller_field!(substep);
        changed_controller_field!(step_height);
        changed_controller_field!(controller_offset);
        changed_controller_field!(minimum_step_width);
        changed_controller_field!(maximum_slope_degrees);
        changed_controller_field!(ground_snap);

        macro_rules! changed_rapier_field {
            ($field:ident) => {{
                let mut changed = rapier;
                changed.$field = f32::from_bits(changed.$field.to_bits().wrapping_add(1));
                assert_ne!(
                    canonical_profile_hash(
                        controller,
                        changed,
                        follower,
                        TRAVERSAL_GUIDE_CONTRACT_VERSION
                    ),
                    expected,
                    "effective Rapier field `{}` must participate in the hash",
                    stringify!($field)
                );
            }};
        }
        changed_rapier_field!(controller_offset);
        changed_rapier_field!(minimum_step_width);
        changed_rapier_field!(maximum_slope_degrees);
        changed_rapier_field!(minimum_slope_slide_degrees);
        changed_rapier_field!(ground_snap);

        macro_rules! changed_follower_field {
            ($field:ident) => {{
                let mut changed = follower;
                changed.$field = f32::from_bits(changed.$field.to_bits().wrapping_add(1));
                assert_ne!(
                    TraversalRuntimeProfile::new(
                        controller,
                        changed,
                        TRAVERSAL_GUIDE_CONTRACT_VERSION
                    )
                    .profile_hash(),
                    expected,
                    "follower field `{}` must participate in the hash",
                    stringify!($field)
                );
            }};
        }
        changed_follower_field!(max_turn_per_tick);
        changed_follower_field!(climb_capture_radius);
        changed_follower_field!(movement_scale);

        assert_ne!(
            TraversalRuntimeProfile::new(
                controller,
                follower,
                TRAVERSAL_GUIDE_CONTRACT_VERSION + 1
            )
            .profile_hash(),
            expected,
            "guide contract version must participate in the hash"
        );
    }
}
