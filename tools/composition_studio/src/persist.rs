//! Saving, and the deliberate gap between "saved" and "shipped".
//!
//! Every profile edit moves the folded simulation content hash, and that hash
//! is what a LAN peer must match to join. So an author iterating in this tool
//! would break their teammates on every keystroke if `Ctrl+S` wrote straight
//! into the corpus.
//!
//! It does not. `Ctrl+S` writes to a scratch working directory that nothing
//! ships from. Putting a profile into `assets/tiles/` is a separate, explicitly
//! confirmed action that names the hash it is about to change.

use std::path::{Path, PathBuf};

use observed_authoring::composition::{
    CompositionBuild, fold_simulation_content_hash, profile_content_hash, write_profile_build,
};
use observed_facility::hex_wfc::profile::HexCompositionProfile;

use crate::{CatalogHash, StudioState};

/// Where `Ctrl+S` writes. Not shipped, not hashed into any build.
#[must_use]
pub fn working_dir() -> PathBuf {
    PathBuf::from("scratch/composition")
}

/// The committed corpus — what the game and the dedicated server actually load.
#[must_use]
pub fn corpus_dir() -> PathBuf {
    let relative = PathBuf::from("assets/tiles");
    if relative.exists() {
        relative
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
    }
}

fn validation_message(profile: &HexCompositionProfile) -> Result<(), String> {
    profile.validate().map_err(|defects| {
        let lines: Vec<String> = defects.iter().map(|defect| format!("- {defect}")).collect();
        format!("profile is invalid:\n{}", lines.join("\n"))
    })
}

/// Write the in-memory profile to the working directory.
pub fn save_working_profile(state: &mut StudioState) -> Result<(), String> {
    validation_message(&state.profile)?;

    let dir = working_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;

    let build = CompositionBuild::new(state.profile.clone()).map_err(|error| error.to_string())?;
    write_profile_build(&build, &dir).map_err(|error| error.to_string())?;

    state.saved = state.profile.clone();
    state.saved_hash = build.content_hash;
    state.origin = crate::ProfileOrigin::Working;
    Ok(())
}

/// What promoting this profile would do, phrased for a confirmation prompt.
///
/// Naming both hashes is the point: "you are about to change the value every
/// peer must match" is not something to discover after the fact.
#[must_use]
pub fn promotion_summary(state: &StudioState) -> String {
    let before = simulation_hash(state, &state.saved);
    let after = simulation_hash(state, &state.profile);
    // ASCII only: the tool ships no font asset, so anything outside Bevy's
    // default subset renders as a blank box.
    format!(
        "PROMOTE TO CORPUS?\n\n\
         Writes {}\n\
         the profile the game and the dedicated server load.\n\n\
         simulation hash {before}\n\
         becomes         {after}\n\n\
         Every LAN peer must take this same change to stay joinable.\n\n\
         [Enter] confirm    [Esc] cancel",
        corpus_dir()
            .join(observed_authoring::COMPOSITION_PROFILE_FILE)
            .display(),
    )
}

/// Copy the in-memory profile into the committed corpus.
///
/// Refuses when the corpus profile could not be read at startup: overwriting a
/// file the tool failed to parse would destroy an authored composition to
/// resolve a loading bug.
pub fn promote_to_corpus(state: &mut StudioState) -> Result<(), String> {
    if let crate::ProfileOrigin::Unreadable(detail) = &state.origin {
        return Err(format!(
            "refusing to promote over a corpus profile this tool could not read ({detail}); \
             fix it with `tilec profile-validate` first"
        ));
    }
    validation_message(&state.profile)?;

    let dir = corpus_dir();
    let build = CompositionBuild::new(state.profile.clone()).map_err(|error| error.to_string())?;
    write_profile_build(&build, &dir).map_err(|error| error.to_string())?;

    state.saved = state.profile.clone();
    state.saved_hash = build.content_hash;
    state.origin = crate::ProfileOrigin::Corpus;
    Ok(())
}

/// The folded simulation content hash for `profile`, truncated for display.
///
/// Returns `"unavailable"` — never a zeroed or placeholder digest — when the
/// compiled catalog's sidecar could not be read. A wrong-looking hash is a
/// blank; a right-looking wrong hash is a trap.
#[must_use]
pub fn simulation_hash(state: &StudioState, profile: &HexCompositionProfile) -> String {
    let CatalogHash::Known(catalog) = &state.catalog_hash else {
        return String::from("unavailable");
    };
    let Ok(profile_hash) = profile_content_hash(profile) else {
        return String::from("unavailable");
    };
    let folded = fold_simulation_content_hash(catalog, &profile_hash);
    folded
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
