//! The authoritative certification verdict for the selected module.
//!
//! Module Studio already carries two cheaper diagnostics, and neither of them
//! is a certificate. [`walk`](crate::module::walk) samples the authored
//! surfaces: fast, geometric, and useful for rejecting an obviously broken
//! module while the author is still holding the parameter. The Rapier audit
//! drives the production follower over a projected guide. Only certification
//! also carries the identity — the runtime profile hash, the guide hash, and
//! deterministic intent/body digests — that lets a result be compared against
//! what `tilec` proved.
//!
//! So this is deliberately a third, explicitly authoritative panel line
//! rather than a replacement for either: the fast probe may reject, and it
//! may never certify.

use observed_authoring::{
    CertificationAuthority, CertificationReport, TilePrototype, certify_runtime_prototype,
};
use observed_traversal::TraversalRuntimeProfile;

use crate::module::diagnose::Diagnosis;

/// Whether the authoritative certification is running, and what it found.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum CertificationState {
    #[default]
    Off,
    /// The selected file has no geometry to certify.
    NotApplicable,
    Complete(Box<CertificationReport>),
}

impl CertificationState {
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Off)
    }

    #[must_use]
    pub fn passed(&self) -> bool {
        matches!(self, Self::Complete(report) if report.passed())
    }
}

/// Certify one selected module under the canonical runtime profile.
///
/// The profile is [`TraversalRuntimeProfile::canonical_hex`] and nothing else.
/// A studio-local controller would show the author a body the game never
/// spawns, which is precisely the drift this arc exists to close.
#[must_use]
pub fn certify_selected(diagnosis: &Diagnosis) -> CertificationState {
    let Some(prototype) = diagnosis.prototype.as_ref() else {
        return CertificationState::NotApplicable;
    };
    CertificationState::Complete(Box::new(certify_prototype(&diagnosis.name(), prototype)))
}

fn certify_prototype(id: &str, prototype: &TilePrototype) -> CertificationReport {
    certify_runtime_prototype(id, prototype, &TraversalRuntimeProfile::canonical_hex())
}

/// One panel line describing the verdict.
#[must_use]
pub fn summary(state: &CertificationState) -> String {
    match state {
        CertificationState::Off => {
            String::from("CERTIFY off  [K] certify selected module (authoritative)")
        }
        CertificationState::NotApplicable => {
            String::from("CERTIFY n/a  the selected file has no geometry")
        }
        CertificationState::Complete(report) => match report.authority {
            CertificationAuthority::NotApplicable => {
                String::from("CERTIFY n/a  module declares no climb or deck")
            }
            authority => {
                let scope = match authority {
                    CertificationAuthority::Contract => "contract",
                    _ => "compatibility",
                };
                match report.certificate.as_ref() {
                    Some(certificate) => format!(
                        "CERTIFY ok ({scope})  {} pair(s), profile {}",
                        certificate.directed_pairs.len(),
                        hex_prefix(certificate.profile_hash)
                    ),
                    None => format!(
                        "CERTIFY FAILED ({scope})  {}",
                        report.failures.first().map_or_else(
                            || "no detail".to_string(),
                            |failure| format!(
                                "{:?} at {}: {}",
                                failure.kind, failure.stage, failure.message
                            )
                        )
                    ),
                }
            }
        },
    }
}

fn hex_prefix(hash: [u8; 32]) -> String {
    hash[..6].iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use observed_authoring::parse_tile;

    use super::*;
    use crate::module::diagnose::diagnose_file;
    use crate::module::probe::Probe;
    use crate::module::route::walk_module;

    fn diagnosis(stem: &str) -> Diagnosis {
        diagnose_file(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/tiles/authored")
                .join(format!("{stem}.map")),
        )
    }

    #[test]
    fn a_climbing_module_certifies_under_the_canonical_profile() {
        let state = certify_selected(&diagnosis("stair_tower_helix_01"));
        let CertificationState::Complete(report) = &state else {
            panic!("a tower has a declared climb to certify: {state:?}");
        };
        assert!(report.passed(), "{:#?}", report.failures);
        assert!(state.passed());
        let certificate = report.certificate.as_ref().expect("a passing certificate");
        assert_eq!(
            certificate.profile_hash,
            TraversalRuntimeProfile::canonical_hex().profile_hash(),
            "the studio must certify the body the match actually spawns"
        );
        assert!(
            certificate.directed_pairs.len() >= 2,
            "a climb is certified in both directions"
        );
        assert!(summary(&state).starts_with("CERTIFY ok"));
    }

    /// The fast geometric probe stays advisory. It is allowed to reject, and
    /// it is not allowed to stand in for a certificate — a clear surface walk
    /// carries no profile identity and no intent/body digest, so nothing
    /// downstream can compare it against what `tilec` proved.
    #[test]
    fn the_fast_geometric_probe_never_certifies_success() {
        let diagnosis = diagnosis("stair_tower_helix_01");
        let walk = walk_module(&diagnosis).expect("a tower has an advisory route");
        assert!(walk.is_clear(), "the advisory probe finds the tower clear");

        let prototype = diagnosis.prototype.as_ref().expect("geometry parsed");
        assert!(
            Probe::from_prototype(prototype).triangle_count() > 0,
            "the probe samples real surfaces"
        );
        // The advisory report has no certificate-shaped output at all: no
        // profile hash, no guide hash, no digests. That is the whole point.
        let certified = certify_selected(&diagnosis);
        assert!(
            certified.passed(),
            "and the authoritative run is what says so"
        );
    }

    #[test]
    fn a_module_that_declares_no_traversal_is_reported_as_not_applicable() {
        let diagnosis = diagnosis("hall_straight");
        let state = certify_selected(&diagnosis);
        let CertificationState::Complete(report) = &state else {
            panic!("geometry parsed, so certification ran");
        };
        assert_eq!(report.authority, CertificationAuthority::NotApplicable);
        assert!(report.passed(), "nothing declared is not a fault");
        assert!(summary(&state).contains("n/a"));
    }

    #[test]
    fn a_file_without_geometry_is_not_applicable() {
        let diagnosis = diagnose_file(Path::new("does/not/exist.map"));
        assert_eq!(
            certify_selected(&diagnosis),
            CertificationState::NotApplicable
        );
        assert!(!certify_selected(&diagnosis).passed());
        // Guard the fixture assumption the tests above depend on.
        assert!(parse_tile("").is_err());
    }
}
