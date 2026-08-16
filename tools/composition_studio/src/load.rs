//! Loading the authored content the studio opens against.
//!
//! The counterpart to [`crate::persist`], which owns writing. Split out of
//! `state.rs` when the browser build's embedded arms pushed that file past the
//! 600-line review budget: `state.rs` is what the systems read and write, and
//! this is the loading that establishes it.
//!
//! Every loader here has two arms. A filesystem host reads `assets/tiles`; a
//! browser build cannot discover or synchronously read a repository directory,
//! so it reads the same artifacts baked in at compile time. Both go through the
//! same schema parsing, sidecar agreement checks, and content-hash fold, so a
//! hosted studio cannot quietly show different content than a desktop one.

use std::sync::OnceLock;

use observed_authoring::{RoomPrototype, RuntimeHexCatalog, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::profile::HexCompositionProfile;

#[cfg(not(target_arch = "wasm32"))]
use crate::persist;
use crate::{CatalogHash, ProfileOrigin};

#[cfg(not(target_arch = "wasm32"))]
fn tile_dir() -> std::path::PathBuf {
    let cwd_relative = std::path::PathBuf::from("assets/tiles");
    if cwd_relative.exists() {
        return cwd_relative;
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
}

/// The authored corpus, baked into the binary for browser builds.
#[cfg(target_arch = "wasm32")]
mod embedded {
    pub const COMPILED_CATALOG: &str = include_str!("../../../assets/tiles/compiled_catalog.ron");
    pub const COMPILED_CATALOG_HASH: &str =
        include_str!("../../../assets/tiles/compiled_catalog.sha256");
    pub const COMPOSITION_PROFILE: &str =
        include_str!("../../../assets/tiles/composition_profile.ron");
    pub const COMPOSITION_PROFILE_HASH: &str =
        include_str!("../../../assets/tiles/composition_profile.sha256");
}

/// The authored tile corpus: per-cell prototypes and whole-room prototypes.
pub type Corpus = (Vec<TilePrototype>, Vec<RoomPrototype>);

/// The authored corpus, loaded once. The `Err` arm is carried rather than
/// discarded so the status line can name the failure.
pub fn corpus() -> &'static Result<Corpus, String> {
    static CORPUS: OnceLock<Result<Corpus, String>> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
        #[cfg(target_arch = "wasm32")]
        let loaded = RuntimeHexCatalog::from_embedded(
            embedded::COMPILED_CATALOG,
            embedded::COMPILED_CATALOG_HASH,
            embedded::COMPOSITION_PROFILE,
            embedded::COMPOSITION_PROFILE_HASH,
            &slugs,
        );
        #[cfg(not(target_arch = "wasm32"))]
        let loaded = RuntimeHexCatalog::load(&tile_dir(), &slugs);
        loaded
            .map(|loaded| (loaded.cells, loaded.rooms))
            .map_err(|error| format!("authored catalog unavailable: {error}"))
    })
}

/// Read the compiled catalog's committed digest — the other half of the fold.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn catalog_hash() -> CatalogHash {
    let path = persist::corpus_dir().join("compiled_catalog.sha256");
    match std::fs::read_to_string(&path) {
        Ok(text) if text.trim().len() == 64 => CatalogHash::Known(text.trim().to_string()),
        Ok(_) => CatalogHash::Unavailable(format!("{} is not a 64-char digest", path.display())),
        Err(error) => CatalogHash::Unavailable(format!("{}: {error}", path.display())),
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn catalog_hash() -> CatalogHash {
    let text = embedded::COMPILED_CATALOG_HASH.trim();
    if text.len() == 64 {
        CatalogHash::Known(text.to_string())
    } else {
        CatalogHash::Unavailable(String::from(
            "embedded compiled_catalog.sha256 is not a 64-char digest",
        ))
    }
}

/// The corpus profile, parsed from the baked-in copy.
///
/// There is no working-copy arm: a browser build is a read-only viewer, so the
/// scratch directory `Ctrl+S` writes to on the desktop never exists here.
#[cfg(target_arch = "wasm32")]
pub(crate) fn startup_profile() -> (HexCompositionProfile, String, ProfileOrigin) {
    use observed_authoring::composition::{parse_profile, profile_content_hash};
    match parse_profile(embedded::COMPOSITION_PROFILE) {
        Ok(profile) => match profile_content_hash(&profile) {
            Ok(hash) => (profile, hash, ProfileOrigin::Corpus),
            Err(error) => (
                profile,
                String::from("unavailable"),
                ProfileOrigin::Unreadable(format!("embedded profile hash: {error}")),
            ),
        },
        Err(error) => {
            let baseline = HexCompositionProfile::baseline();
            let hash =
                profile_content_hash(&baseline).unwrap_or_else(|_| String::from("unavailable"));
            (
                baseline,
                hash,
                ProfileOrigin::Unreadable(format!("embedded profile unreadable: {error}")),
            )
        }
    }
}

/// Load the profile to edit: the working copy if one exists, else the corpus.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn startup_profile() -> (HexCompositionProfile, String, ProfileOrigin) {
    if let Ok(build) = observed_authoring::composition::load_profile(&persist::working_dir()) {
        return (build.profile, build.content_hash, ProfileOrigin::Working);
    }
    match observed_authoring::composition::load_profile(&persist::corpus_dir()) {
        Ok(build) => (build.profile, build.content_hash, ProfileOrigin::Corpus),
        Err(error) => {
            let baseline = HexCompositionProfile::baseline();
            let hash = observed_authoring::composition::profile_content_hash(&baseline)
                .unwrap_or_else(|_| String::from("unavailable"));
            (
                baseline,
                hash,
                ProfileOrigin::Unreadable(format!("corpus profile unreadable: {error}")),
            )
        }
    }
}
