//! Deterministic headless proof that authored traversal contracts match their
//! structural collision geometry.
//!
//! Certification answers one question the compiler cannot: a declared route is
//! a claim, and only the production controller can say whether a capsule
//! crosses it. So this module builds the scene from the same compiled hulls a
//! match projects, drives the same `follow_stateless` steering, and steps the
//! same Rapier character controller with the canonical
//! [`TraversalRuntimeProfile`](observed_traversal::TraversalRuntimeProfile).
//! There is no certification-only physics profile, and there must never be
//! one: a walk under different numbers certifies a body the game never spawns.

mod assembly;
mod model;
mod preflight;
mod route;
mod runner;
mod scene;

pub use assembly::{certify_catalog, certify_catalog_selection};
pub use model::{
    CertificationAuthority, CertificationFailure, CertificationFailureKind, CertificationReport,
    CertifiedInterface, CertifiedPortPair, CertifiedVerticalCase, CorpusCertificationReport,
    FailureTracePoint, TRAVERSAL_CERTIFICATE_VERSION, TraversalCertificate,
    TraversalCertificateHash,
};
pub use runner::{certify_compiled_module, certify_runtime_prototype};

#[cfg(test)]
mod tests;
