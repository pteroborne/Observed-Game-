use observed_traversal::{TraversalEdgeId, TraversalGuideHash};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::InterfaceFingerprint;

pub const TRAVERSAL_CERTIFICATE_VERSION: u16 = 1;
const CERTIFICATE_HASH_DOMAIN: &[u8] = b"observed2.traversal-certificate";

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct TraversalCertificateHash(pub [u8; 32]);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CertifiedInterface {
    pub port: String,
    pub fingerprint: InterfaceFingerprint,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CertifiedPortPair {
    pub entry: String,
    pub exit: String,
    pub ticks: u32,
    pub intent_digest: [u8; 32],
    pub body_digest: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraversalCertificate {
    pub version: u16,
    pub module_id: String,
    pub family: String,
    pub structural_hash: [u8; 32],
    pub guide_hash: TraversalGuideHash,
    pub profile_hash: [u8; 32],
    pub interfaces: Vec<CertifiedInterface>,
    pub directed_pairs: Vec<CertifiedPortPair>,
    pub certificate_hash: TraversalCertificateHash,
}

impl TraversalCertificate {
    pub(crate) fn build(
        module_id: String,
        family: String,
        structural_hash: [u8; 32],
        guide_hash: TraversalGuideHash,
        profile_hash: [u8; 32],
        mut interfaces: Vec<CertifiedInterface>,
        mut directed_pairs: Vec<CertifiedPortPair>,
    ) -> Self {
        interfaces.sort_by(|a, b| a.port.cmp(&b.port));
        directed_pairs.sort_by(|a, b| (&a.entry, &a.exit).cmp(&(&b.entry, &b.exit)));
        let mut certificate = Self {
            version: TRAVERSAL_CERTIFICATE_VERSION,
            module_id,
            family,
            structural_hash,
            guide_hash,
            profile_hash,
            interfaces,
            directed_pairs,
            certificate_hash: TraversalCertificateHash([0; 32]),
        };
        certificate.certificate_hash = certificate.canonical_hash();
        certificate
    }

    #[must_use]
    pub fn verify_hash(&self) -> bool {
        self.version == TRAVERSAL_CERTIFICATE_VERSION
            && self.certificate_hash == self.canonical_hash()
            && self
                .interfaces
                .windows(2)
                .all(|pair| pair[0].port < pair[1].port)
            && self.directed_pairs.windows(2).all(|pair| {
                (&pair[0].entry, &pair[0].exit) < (&pair[1].entry, &pair[1].exit)
            })
    }

    #[must_use]
    pub fn to_pretty_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(
            self,
            ron::ser::PrettyConfig::default().new_line("\n".to_owned()),
        )
        .map_err(|error| error.to_string())
    }

    fn canonical_hash(&self) -> TraversalCertificateHash {
        let mut hash = Sha256::new();
        hash.update(CERTIFICATE_HASH_DOMAIN);
        hash.update(self.version.to_le_bytes());
        hash_string(&mut hash, &self.module_id);
        hash_string(&mut hash, &self.family);
        hash.update(self.structural_hash);
        hash.update(self.guide_hash.0);
        hash.update(self.profile_hash);
        hash.update((self.interfaces.len() as u64).to_le_bytes());
        for interface in &self.interfaces {
            hash_string(&mut hash, &interface.port);
            hash.update(interface.fingerprint.0);
        }
        hash.update((self.directed_pairs.len() as u64).to_le_bytes());
        for pair in &self.directed_pairs {
            hash_string(&mut hash, &pair.entry);
            hash_string(&mut hash, &pair.exit);
            hash.update(pair.ticks.to_le_bytes());
            hash.update(pair.intent_digest);
            hash.update(pair.body_digest);
        }
        TraversalCertificateHash(hash.finalize().into())
    }
}

fn hash_string(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificationFailureKind {
    MissingContract,
    InvalidContract,
    InvalidStructuralHash,
    InvalidGuide,
    InvalidHull,
    LandingOutsideAperture,
    LandingOutsideClearance,
    TerminalOutsideClearance,
    TerminalBindingMismatch,
    NoRoute,
    FollowUnavailable,
    Recovered,
    NonFiniteBody,
    TimedOut,
    Nonrepeatable,
    MissingVerticalRepresentative,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FailureTracePoint {
    pub tick: u32,
    pub feet: [f32; 3],
    pub target: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CertificationFailure {
    pub module_id: String,
    pub entry: Option<String>,
    pub exit: Option<String>,
    pub edge: Option<TraversalEdgeId>,
    pub stage: String,
    pub tick: u32,
    pub kind: CertificationFailureKind,
    pub message: String,
    pub last_feet: Option<[f32; 3]>,
    pub last_target: Option<[f32; 3]>,
    pub trace: Vec<FailureTracePoint>,
}

impl CertificationFailure {
    pub(crate) fn module(
        module_id: &str,
        kind: CertificationFailureKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            module_id: module_id.to_owned(),
            entry: None,
            exit: None,
            edge: None,
            stage: "validation".to_owned(),
            tick: 0,
            kind,
            message: message.into(),
            last_feet: None,
            last_target: None,
            trace: Vec::new(),
        }
    }
}

/// Which contract a report speaks for.
///
/// A compatibility report is real physical evidence about a pre-v3 module's
/// spine and deck, and deliberately not a substitute for the authoritative
/// certificate: a module that never declared a landing, aperture, or clearance
/// cannot have one proved.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificationAuthority {
    #[default]
    Contract,
    Compatibility,
    /// The module declares no traversal at all, so there is nothing to walk.
    NotApplicable,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CertificationReport {
    pub authority: CertificationAuthority,
    pub certificate: Option<TraversalCertificate>,
    pub failures: Vec<CertificationFailure>,
}

impl CertificationReport {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
            && (self.certificate.is_some()
                || self.authority == CertificationAuthority::NotApplicable)
    }

    #[must_use]
    pub fn to_pretty_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(
            self,
            ron::ser::PrettyConfig::default().new_line("\n".to_owned()),
        )
        .map_err(|error| error.to_string())
    }
}

/// One stacked vertical combination proved once for a whole fingerprint class.
///
/// `lower` and `upper` name the representative members actually simulated;
/// `members` names every `lower|upper` module pair that folds into the same
/// interface fingerprint and is therefore covered by that one run. This is the
/// field shape the runner needs: a Cartesian test over cosmetic members
/// produces the same physical scene many times, while a fingerprint class is
/// exactly the set of combinations whose landing, aperture, clearance, port,
/// and guide terminal agree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CertifiedVerticalCase {
    pub family: String,
    pub fingerprint: InterfaceFingerprint,
    pub lower: String,
    pub upper: String,
    pub members: Vec<String>,
    /// How many stacked cells this run actually built: two for a pairing, three
    /// for the representative column.
    pub cells: u8,
    pub ticks: u32,
    pub intent_digest: [u8; 32],
    pub body_digest: [u8; 32],
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusCertificationReport {
    pub version: u16,
    pub profile_hash: [u8; 32],
    pub modules: Vec<CertificationReport>,
    pub vertical_cases: Vec<CertifiedVerticalCase>,
    pub failures: Vec<CertificationFailure>,
}

impl CorpusCertificationReport {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty() && self.modules.iter().all(CertificationReport::passed)
    }

    /// How many modules were certified under each authority. Corpus commands
    /// print this so "0 failures" can never be mistaken for "0 checked".
    #[must_use]
    pub fn tally(&self) -> (usize, usize, usize) {
        let count = |wanted: CertificationAuthority| {
            self.modules
                .iter()
                .filter(|report| report.authority == wanted)
                .count()
        };
        (
            count(CertificationAuthority::Contract),
            count(CertificationAuthority::Compatibility),
            count(CertificationAuthority::NotApplicable),
        )
    }

    #[must_use]
    pub fn to_pretty_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(
            self,
            ron::ser::PrettyConfig::default().new_line("\n".to_owned()),
        )
        .map_err(|error| error.to_string())
    }
}
