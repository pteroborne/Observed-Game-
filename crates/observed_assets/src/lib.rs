//! The **drop-in asset convention**: a small, data-driven manifest of asset
//! "slots", each a logical name + the file it expects under `assets/`. Drop a
//! free/CC0 file at a slot's path and a consumer uses it; leave it absent and the
//! consumer falls back to a procedural placeholder. Nothing else changes — no code
//! edits.
//!
//! This is the single source of truth for asset slots, shared by the `asset_lab`
//! showcase and the assembled `game` so neither re-declares literal asset paths.
//! Each slot is also exposed as a named `pub const` ([`CEILING`], [`PLAYER`], …), so
//! presentation code references a slot semantically (`observed_assets::CEILING.path`)
//! instead of hard-coding the string.
//!
//! The crate is pure (no Bevy app): the manifest and the present/absent check are
//! plain filesystem logic, so they unit-test without a running app. A consumer
//! projects the result into a 3D showcase or the match presentation.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetKind {
    Texture,
    Model,
    Sound,
}

impl AssetKind {
    pub fn label(self) -> &'static str {
        match self {
            AssetKind::Texture => "texture",
            AssetKind::Model => "model",
            AssetKind::Sound => "sound",
        }
    }

    /// Accepted file extensions for this kind (what Bevy can load with our features).
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            AssetKind::Texture => &["png", "jpg", "jpeg", "hdr"],
            AssetKind::Model => &["glb", "gltf"],
            AssetKind::Sound => &["ogg", "wav"],
        }
    }
}

/// One drop-in slot: a logical name, what kind of asset it is, and the path it
/// expects (relative to the `assets/` root), plus a hint of what to put there.
#[derive(Clone, Copy, Debug)]
pub struct AssetSlot {
    pub name: &'static str,
    pub kind: AssetKind,
    /// Path relative to the `assets/` directory.
    pub path: &'static str,
    pub hint: &'static str,
}

// --- the slots ---------------------------------------------------------------
//
// Each slot is a named `const` so consumers can reference it semantically (e.g.
// `observed_assets::CEILING.path`). `SLOTS` lists them all; add a row in both places
// to add a drop-in point.

pub const WALL: AssetSlot = AssetSlot {
    name: "wall",
    kind: AssetKind::Texture,
    path: "textures/wall.png",
    hint: "a CC0 wall/brick PBR albedo (ambientCG, Poly Haven)",
};
pub const FLOOR: AssetSlot = AssetSlot {
    name: "floor",
    kind: AssetKind::Texture,
    path: "textures/floor.png",
    hint: "a CC0 floor/concrete albedo (ambientCG)",
};
pub const PROP: AssetSlot = AssetSlot {
    name: "prop",
    kind: AssetKind::Model,
    path: "models/prop.glb",
    hint: "any CC0 glTF/GLB prop (Kenney, Quaternius, Poly Pizza)",
};
pub const CHIME: AssetSlot = AssetSlot {
    name: "chime",
    kind: AssetKind::Sound,
    path: "sounds/chime.ogg",
    hint: "a CC0 UI/chime sound, OGG or WAV (Kenney, Freesound)",
};
pub const CEILING: AssetSlot = AssetSlot {
    name: "ceiling",
    kind: AssetKind::Texture,
    path: "textures/ceiling.png",
    hint: "tileable dark metal ceiling panels",
};
pub const EXIT_PANEL: AssetSlot = AssetSlot {
    name: "exit_panel",
    kind: AssetKind::Texture,
    path: "textures/exit_panel.png",
    hint: "bright green EXIT or chevron panel",
};
pub const ENVIRONMENT: AssetSlot = AssetSlot {
    name: "environment",
    kind: AssetKind::Texture,
    path: "textures/environment.hdr",
    hint: "industrial interior HDR panorama",
};
pub const LIGHT_FIXTURE: AssetSlot = AssetSlot {
    name: "light_fixture",
    kind: AssetKind::Model,
    path: "models/light_fixture.glb",
    hint: "small ceiling-mounted industrial lamp",
};
pub const EXIT_GATE: AssetSlot = AssetSlot {
    name: "exit_gate",
    kind: AssetKind::Model,
    path: "models/exit_gate.glb",
    hint: "large sci-fi exit gate or elevator",
};
pub const PLAYER: AssetSlot = AssetSlot {
    name: "player",
    kind: AssetKind::Model,
    path: "models/player.glb",
    hint: "low-poly humanoid or robot teammate",
};
pub const BOT: AssetSlot = AssetSlot {
    name: "bot",
    kind: AssetKind::Model,
    path: "models/bot.glb",
    hint: "visually distinct rival robot",
};
pub const DOORWAY: AssetSlot = AssetSlot {
    name: "doorway",
    kind: AssetKind::Model,
    path: "models/doorway.glb",
    hint: "modular doorway frame",
};
pub const EQUIPMENT: AssetSlot = AssetSlot {
    name: "equipment",
    kind: AssetKind::Model,
    path: "models/equipment.glb",
    hint: "portable cable, relay, or power device",
};
pub const DECOR_CRATE: AssetSlot = AssetSlot {
    name: "decor_crate",
    kind: AssetKind::Model,
    path: "models/decor_crate.glb",
    hint: "industrial storage container",
};
pub const DECOR_CONSOLE: AssetSlot = AssetSlot {
    name: "decor_console",
    kind: AssetKind::Model,
    path: "models/decor_console.glb",
    hint: "facility terminal or pipe assembly",
};
pub const HAZARD: AssetSlot = AssetSlot {
    name: "hazard",
    kind: AssetKind::Model,
    path: "models/hazard.glb",
    hint: "warning beacon for collapsing rooms",
};
pub const FOOTSTEP: AssetSlot = AssetSlot {
    name: "footstep",
    kind: AssetKind::Sound,
    path: "sounds/footstep.ogg",
    hint: "short footstep movement cue",
};
pub const REROUTE: AssetSlot = AssetSlot {
    name: "reroute",
    kind: AssetKind::Sound,
    path: "sounds/reroute.ogg",
    hint: "mechanical shift or heavy door clunk",
};
pub const ESCAPE: AssetSlot = AssetSlot {
    name: "escape",
    kind: AssetKind::Sound,
    path: "sounds/escape.ogg",
    hint: "positive escape success sting",
};
pub const AMBIENCE: AssetSlot = AssetSlot {
    name: "ambience",
    kind: AssetKind::Sound,
    path: "sounds/ambience.ogg",
    hint: "looping industrial facility hum",
};
pub const AMBIENCE_ARCHIVE: AssetSlot = AssetSlot {
    name: "ambience_archive",
    kind: AssetKind::Sound,
    path: "sounds/ambience_archive.ogg",
    hint: "looping cold archival district bed",
};
pub const AMBIENCE_REACTOR: AssetSlot = AssetSlot {
    name: "ambience_reactor",
    kind: AssetKind::Sound,
    path: "sounds/ambience_reactor.ogg",
    hint: "looping warm reactor district bed",
};
pub const AMBIENCE_ATRIUM: AssetSlot = AssetSlot {
    name: "ambience_atrium",
    kind: AssetKind::Sound,
    path: "sounds/ambience_atrium.ogg",
    hint: "looping overgrown atrium district bed",
};
pub const AMBIENCE_FOUNDRY: AssetSlot = AssetSlot {
    name: "ambience_foundry",
    kind: AssetKind::Sound,
    path: "sounds/ambience_foundry.ogg",
    hint: "looping industrial foundry district bed",
};
pub const AMBIENCE_HOLLOW: AssetSlot = AssetSlot {
    name: "ambience_hollow",
    kind: AssetKind::Sound,
    path: "sounds/ambience_hollow.ogg",
    hint: "looping unfinished hollow district bed",
};
pub const AMBIENCE_SPILLWAY: AssetSlot = AssetSlot {
    name: "ambience_spillway",
    kind: AssetKind::Sound,
    path: "sounds/ambience_spillway.ogg",
    hint: "looping flooded spillway district bed",
};
/// District ambience slots in the same semantic order used by
/// `observed_style::District::ALL`: archive, reactor, atrium, foundry, hollow,
/// spillway. `observed_assets` stays style-free; consumers that know districts can
/// assert the counts match.
pub const DISTRICT_AMBIENCE: [AssetSlot; 6] = [
    AMBIENCE_ARCHIVE,
    AMBIENCE_REACTOR,
    AMBIENCE_ATRIUM,
    AMBIENCE_FOUNDRY,
    AMBIENCE_HOLLOW,
    AMBIENCE_SPILLWAY,
];
/// Optional (not in the game's required asset plan): a door open/close thunk on
/// entering or leaving a place. Silent until a file is dropped here.
pub const DOOR: AssetSlot = AssetSlot {
    name: "door",
    kind: AssetKind::Sound,
    path: "sounds/door.ogg",
    hint: "door open/close thunk on entering or leaving a place",
};
pub const KLAXON: AssetSlot = AssetSlot {
    name: "klaxon",
    kind: AssetKind::Sound,
    path: "sounds/klaxon.ogg",
    hint: "looping alarm sting when first-escape countdown is active",
};
pub const COLLAPSE_STING: AssetSlot = AssetSlot {
    name: "collapse_sting",
    kind: AssetKind::Sound,
    path: "sounds/collapse_sting.ogg",
    hint: "alarm warning when room collapse starts",
};
pub const UI_CLICK: AssetSlot = AssetSlot {
    name: "click",
    kind: AssetKind::Sound,
    path: "sounds/ui_click.ogg",
    hint: "UI button click sound",
};
pub const UI_HOVER: AssetSlot = AssetSlot {
    name: "hover",
    kind: AssetKind::Sound,
    path: "sounds/ui_hover.ogg",
    hint: "UI button hover/highlight sound",
};
pub const JUMP: AssetSlot = AssetSlot {
    name: "jump",
    kind: AssetKind::Sound,
    path: "sounds/jump.ogg",
    hint: "mechanical/gantry jump start cue",
};
pub const LAND: AssetSlot = AssetSlot {
    name: "land",
    kind: AssetKind::Sound,
    path: "sounds/land.ogg",
    hint: "mechanical/gantry land/impact cue",
};
pub const RUNNER_STAND: AssetSlot = AssetSlot {
    name: "runner_stand",
    kind: AssetKind::Texture,
    path: "sprites/runner_stand.png",
    hint: "upright CC0 2.5D runner sprite",
};
pub const RUNNER_WALK1: AssetSlot = AssetSlot {
    name: "runner_walk1",
    kind: AssetKind::Texture,
    path: "sprites/runner_walk1.png",
    hint: "upright CC0 2.5D runner walk frame",
};
pub const RUNNER_WALK2: AssetSlot = AssetSlot {
    name: "runner_walk2",
    kind: AssetKind::Texture,
    path: "sprites/runner_walk2.png",
    hint: "upright CC0 2.5D runner walk frame",
};
pub const RIVAL_STAND: AssetSlot = AssetSlot {
    name: "rival_stand",
    kind: AssetKind::Texture,
    path: "sprites/rival_stand.png",
    hint: "upright CC0 2.5D rival sprite",
};
pub const RIVAL_WALK1: AssetSlot = AssetSlot {
    name: "rival_walk1",
    kind: AssetKind::Texture,
    path: "sprites/rival_walk1.png",
    hint: "upright CC0 2.5D rival walk frame",
};
pub const RIVAL_WALK2: AssetSlot = AssetSlot {
    name: "rival_walk2",
    kind: AssetKind::Texture,
    path: "sprites/rival_walk2.png",
    hint: "upright CC0 2.5D rival walk frame",
};
pub const GUARDIAN_STAND: AssetSlot = AssetSlot {
    name: "guardian_stand",
    kind: AssetKind::Texture,
    path: "sprites/guardian_stand.png",
    hint: "upright CC0 2.5D guardian sprite",
};
pub const CONTROL_DEVICE: AssetSlot = AssetSlot {
    name: "control_device",
    kind: AssetKind::Texture,
    path: "sprites/control_device.png",
    hint: "CC0 2.5D control or deployable device sprite",
};

/// Every authored slot, in showcase order. Add a row (and a named const above) to
/// add a drop-in point.
pub const SLOTS: &[AssetSlot] = &[
    WALL,
    FLOOR,
    PROP,
    CHIME,
    CEILING,
    EXIT_PANEL,
    ENVIRONMENT,
    LIGHT_FIXTURE,
    EXIT_GATE,
    PLAYER,
    BOT,
    DOORWAY,
    EQUIPMENT,
    DECOR_CRATE,
    DECOR_CONSOLE,
    HAZARD,
    FOOTSTEP,
    REROUTE,
    ESCAPE,
    AMBIENCE,
    AMBIENCE_ARCHIVE,
    AMBIENCE_REACTOR,
    AMBIENCE_ATRIUM,
    AMBIENCE_FOUNDRY,
    AMBIENCE_HOLLOW,
    AMBIENCE_SPILLWAY,
    DOOR,
    KLAXON,
    COLLAPSE_STING,
    UI_CLICK,
    UI_HOVER,
    JUMP,
    LAND,
    RUNNER_STAND,
    RUNNER_WALK1,
    RUNNER_WALK2,
    RIVAL_STAND,
    RIVAL_WALK1,
    RIVAL_WALK2,
    GUARDIAN_STAND,
    CONTROL_DEVICE,
];

/// The authored slots a showcase wires up, as an owned vector (back-compat with the
/// original `manifest()` API). [`SLOTS`] is the same data as a `'static` slice.
pub fn manifest() -> Vec<AssetSlot> {
    SLOTS.to_vec()
}

/// Look a slot up by its logical name. Panics if the name is not in [`SLOTS`] — a
/// missing name is a programming error, caught by tests, not a runtime condition.
pub fn slot(name: &str) -> AssetSlot {
    *SLOTS
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no asset slot named {name:?}"))
}

/// The `assets/` root relative to the current working directory (where `cargo run`
/// resolves Bevy's asset folder). Returned absolute so the overlay can show exactly
/// where to drop files.
pub fn assets_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("assets")
}

/// Is a real file present for this slot under `root`?
pub fn slot_present(slot: &AssetSlot, root: &Path) -> bool {
    root.join(slot.path).is_file()
}

/// The absolute path a slot expects, for display.
pub fn slot_full_path(slot: &AssetSlot, root: &Path) -> PathBuf {
    root.join(slot.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_manifest_is_well_formed() {
        let slots = manifest();
        assert!(!slots.is_empty());
        // At least one of each kind, so a showcase exercises textures, models, sounds.
        assert!(slots.iter().any(|s| s.kind == AssetKind::Texture));
        assert!(slots.iter().any(|s| s.kind == AssetKind::Model));
        assert!(slots.iter().any(|s| s.kind == AssetKind::Sound));
    }

    #[test]
    fn slot_names_are_unique() {
        let names: BTreeSet<&str> = manifest().iter().map(|s| s.name).collect();
        assert_eq!(names.len(), manifest().len(), "slot names must be unique");
    }

    #[test]
    fn each_path_lives_in_a_subfolder_with_a_supported_extension() {
        for slot in manifest() {
            let (folder, file) = slot
                .path
                .split_once('/')
                .expect("slot path has a subfolder");
            assert!(!folder.is_empty() && !file.is_empty());
            let ext = file.rsplit('.').next().expect("file has an extension");
            assert!(
                slot.kind.extensions().contains(&ext),
                "{} uses {ext}, not a {} extension",
                slot.name,
                slot.kind.label()
            );
        }
    }

    #[test]
    fn presence_is_false_for_an_empty_root() {
        let root = Path::new("/definitely/not/a/real/assets/root");
        for slot in manifest() {
            assert!(!slot_present(&slot, root));
        }
    }

    #[test]
    fn named_lookup_matches_the_named_consts() {
        // The named consts and the `slot(name)` lookup are the two ways consumers
        // reference a slot; they must agree.
        assert_eq!(slot("ceiling").path, CEILING.path);
        assert_eq!(slot("player").path, PLAYER.path);
        assert_eq!(slot("door").path, DOOR.path);
        assert_eq!(slot("ambience_archive").path, AMBIENCE_ARCHIVE.path);
        assert_eq!(slot("runner_stand").path, RUNNER_STAND.path);
        assert_eq!(slot("control_device").path, CONTROL_DEVICE.path);
        // Every named const is reachable by its own name through the manifest.
        for s in SLOTS {
            assert_eq!(slot(s.name).path, s.path);
        }
    }

    #[test]
    fn district_ambience_slots_are_manifest_entries() {
        assert_eq!(DISTRICT_AMBIENCE.len(), 6);
        for district_slot in DISTRICT_AMBIENCE {
            assert_eq!(slot(district_slot.name).path, district_slot.path);
        }
    }
}
