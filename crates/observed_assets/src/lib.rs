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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
pub const LANTERN: AssetSlot = AssetSlot {
    name: "lantern",
    kind: AssetKind::Model,
    path: "models/lantern.glb",
    hint: "handheld caged anchor lantern or torch body",
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
/// Hallway-flavour ambience beds. Rooms take their district's bed; a hallway takes
/// one of these instead: the generic corridor bed, or the gantry bed when the hall
/// has raised decks. Optional drop-ins like the district beds.
pub const AMBIENCE_CORRIDOR: AssetSlot = AssetSlot {
    name: "ambience_corridor",
    kind: AssetKind::Sound,
    path: "sounds/ambience_corridor.ogg",
    hint: "looping generic hallway/corridor bed",
};
pub const AMBIENCE_GANTRY: AssetSlot = AssetSlot {
    name: "ambience_gantry",
    kind: AssetKind::Sound,
    path: "sounds/ambience_gantry.ogg",
    hint: "looping open-air gantry jump-hall bed",
};
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
pub const TOOL_INTERACT: AssetSlot = AssetSlot {
    name: "tool_interact",
    kind: AssetKind::Sound,
    path: "sounds/tool_interact.ogg",
    hint: "short CC0 tool pickup/drop acknowledgement",
};
pub const KEYSTONE: AssetSlot = AssetSlot {
    name: "keystone",
    kind: AssetKind::Sound,
    path: "sounds/keystone.ogg",
    hint: "short CC0 keystone pickup signal",
};
pub const EXIT_UNLOCK: AssetSlot = AssetSlot {
    name: "exit_unlock",
    kind: AssetKind::Sound,
    path: "sounds/exit_unlock.ogg",
    hint: "short CC0 exit unlock confirmation",
};
pub const GUARDIAN_DREAD: AssetSlot = AssetSlot {
    name: "guardian_dread",
    kind: AssetKind::Sound,
    path: "sounds/guardian_dread.ogg",
    hint: "subtle CC0 guardian proximity dread cue",
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
pub const KEYSTONE_CARD: AssetSlot = AssetSlot {
    name: "keystone_card",
    kind: AssetKind::Texture,
    path: "sprites/keystone_card.png",
    hint: "CC0 2.5D keystone access card sprite",
};
pub const KEYSTONE_CORE: AssetSlot = AssetSlot {
    name: "keystone_core",
    kind: AssetKind::Texture,
    path: "sprites/keystone_core.png",
    hint: "CC0 2.5D keystone power core sprite",
};
pub const EXIT_ACCESS_CARD: AssetSlot = AssetSlot {
    name: "exit_access_card",
    kind: AssetKind::Texture,
    path: "sprites/exit_access_card.png",
    hint: "CC0 2.5D exit authorization card sprite",
};
pub const ANCHOR_TORCH: AssetSlot = AssetSlot {
    name: "anchor_torch",
    kind: AssetKind::Texture,
    path: "sprites/anchor_torch.png",
    hint: "CC0 2.5D anchor torch body sprite",
};
pub const ROUTE_CELL: AssetSlot = AssetSlot {
    name: "route_cell",
    kind: AssetKind::Texture,
    path: "sprites/route_cell.png",
    hint: "CC0 2.5D route/mesh power cell sprite",
};
pub const RELAY_DEVICE: AssetSlot = AssetSlot {
    name: "relay_device",
    kind: AssetKind::Texture,
    path: "sprites/relay_device.png",
    hint: "CC0 2.5D portable relay node sprite",
};
pub const BATTERY_CHARGE: AssetSlot = AssetSlot {
    name: "battery_charge",
    kind: AssetKind::Texture,
    path: "sprites/battery_charge.png",
    hint: "CC0 2.5D battery unit charge sprite",
};
pub const REPAIR_TOKEN: AssetSlot = AssetSlot {
    name: "repair_token",
    kind: AssetKind::Texture,
    path: "sprites/repair_token.png",
    hint: "CC0 2.5D subsystem repair token sprite",
};
pub const RIVAL_ACTOR: AssetSlot = AssetSlot {
    name: "rival_actor",
    kind: AssetKind::Texture,
    path: "sprites/rival_actor.png",
    hint: "CC0 2.5D directional rival sheet",
};
pub const GUARDIAN_ACTOR: AssetSlot = AssetSlot {
    name: "guardian_actor",
    kind: AssetKind::Texture,
    path: "sprites/guardian_actor.png",
    hint: "CC0 2.5D directional guardian sheet",
};
pub const DECOR_COLUMN: AssetSlot = AssetSlot {
    name: "decor_column",
    kind: AssetKind::Texture,
    path: "sprites/decor_column.png",
    hint: "CC0 2.5D column decoration sprite",
};
pub const DECOR_TORCH: AssetSlot = AssetSlot {
    name: "decor_torch",
    kind: AssetKind::Texture,
    path: "sprites/decor_torch.png",
    hint: "CC0 2.5D wall torch decoration sprite",
};
pub const DECOR_LAB_CRATE: AssetSlot = AssetSlot {
    name: "decor_lab_crate",
    kind: AssetKind::Texture,
    path: "sprites/decor_lab_crate.png",
    hint: "CC0 2.5D lab crate decoration sprite",
};
pub const DECOR_LAB_TABLE: AssetSlot = AssetSlot {
    name: "decor_lab_table",
    kind: AssetKind::Texture,
    path: "sprites/decor_lab_table.png",
    hint: "CC0 2.5D lab table decoration sprite",
};
pub const WALL_ALBEDO_LAB: AssetSlot = AssetSlot {
    name: "wall_albedo_lab",
    kind: AssetKind::Texture,
    path: "textures/wall_albedo_lab.png",
    hint: "CC0 PBR wall albedo variant for lab-like rooms",
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
    LANTERN,
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
    AMBIENCE_CORRIDOR,
    AMBIENCE_GANTRY,
    DOOR,
    KLAXON,
    COLLAPSE_STING,
    UI_CLICK,
    UI_HOVER,
    JUMP,
    LAND,
    TOOL_INTERACT,
    KEYSTONE,
    EXIT_UNLOCK,
    GUARDIAN_DREAD,
    RUNNER_STAND,
    RUNNER_WALK1,
    RUNNER_WALK2,
    RIVAL_STAND,
    RIVAL_WALK1,
    RIVAL_WALK2,
    GUARDIAN_STAND,
    CONTROL_DEVICE,
    KEYSTONE_CARD,
    KEYSTONE_CORE,
    EXIT_ACCESS_CARD,
    ANCHOR_TORCH,
    ROUTE_CELL,
    RELAY_DEVICE,
    BATTERY_CHARGE,
    REPAIR_TOKEN,
    RIVAL_ACTOR,
    GUARDIAN_ACTOR,
    DECOR_COLUMN,
    DECOR_TORCH,
    DECOR_LAB_CRATE,
    DECOR_LAB_TABLE,
    WALL_ALBEDO_LAB,
];

/// Returns the static slice of all authored asset slots without dynamic vector allocation.
pub fn slots() -> &'static [AssetSlot] {
    SLOTS
}

/// The authored slots a showcase wires up, as an owned vector (back-compat with the
/// original `manifest()` API). [`SLOTS`] is the same data as a `'static` slice.
pub fn manifest() -> Vec<AssetSlot> {
    slots().to_vec()
}

/// Look a slot up by its logical name, returning `None` if the slot name is not in [`SLOTS`].
pub fn find_slot(name: &str) -> Option<AssetSlot> {
    SLOTS.iter().copied().find(|s| s.name == name)
}

/// Look a slot up by its logical name. Panics if the name is not in [`SLOTS`] — a
/// missing name is a programming error, caught by tests, not a runtime condition.
pub fn slot(name: &str) -> AssetSlot {
    find_slot(name).unwrap_or_else(|| panic!("no asset slot named {name:?}"))
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
        assert_eq!(slot("lantern").path, LANTERN.path);
        assert_eq!(slot("keystone_card").path, KEYSTONE_CARD.path);
        assert_eq!(slot("keystone_core").path, KEYSTONE_CORE.path);
        assert_eq!(slot("exit_access_card").path, EXIT_ACCESS_CARD.path);
        assert_eq!(slot("anchor_torch").path, ANCHOR_TORCH.path);
        assert_eq!(slot("route_cell").path, ROUTE_CELL.path);
        assert_eq!(slot("relay_device").path, RELAY_DEVICE.path);
        assert_eq!(slot("battery_charge").path, BATTERY_CHARGE.path);
        assert_eq!(slot("repair_token").path, REPAIR_TOKEN.path);
        assert_eq!(slot("guardian_dread").path, GUARDIAN_DREAD.path);
        assert_eq!(slot("rival_actor").path, RIVAL_ACTOR.path);
        assert_eq!(slot("guardian_actor").path, GUARDIAN_ACTOR.path);
        assert_eq!(slot("decor_column").path, DECOR_COLUMN.path);
        assert_eq!(slot("decor_torch").path, DECOR_TORCH.path);
        assert_eq!(slot("decor_lab_crate").path, DECOR_LAB_CRATE.path);
        assert_eq!(slot("decor_lab_table").path, DECOR_LAB_TABLE.path);
        assert_eq!(slot("wall_albedo_lab").path, WALL_ALBEDO_LAB.path);
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

    #[test]
    fn hallway_ambience_slots_are_manifest_entries() {
        for hall_slot in [AMBIENCE_CORRIDOR, AMBIENCE_GANTRY] {
            assert_eq!(slot(hall_slot.name).path, hall_slot.path);
        }
    }

    #[test]
    fn find_slot_returns_option() {
        assert_eq!(find_slot("wall").map(|s| s.path), Some(WALL.path));
        assert_eq!(find_slot("non_existent_slot_name"), None);
    }

    #[test]
    fn slots_matches_manifest() {
        assert_eq!(slots().len(), manifest().len());
        assert_eq!(slots(), manifest().as_slice());
    }
}
