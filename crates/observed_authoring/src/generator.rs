//! Parametric generator for 3D hex geometry: solid central core columns, canonical inner-wall ramps, and flat decks.

use std::fs;
use std::path::Path;

/// Generate the 7-hex solid core tower blueprint map files (Option A: 2 Ramps + 4 Flat Decks per floor).
pub fn generate_tower_tiles(output_dir: &Path) -> Result<Vec<String>, String> {
    fs::create_dir_all(output_dir).map_err(|err| format!("{}: {err}", output_dir.display()))?;

    let mut generated_files = Vec::new();

    // 1. Central Solid Core Column Tile (`tower_solid_core.map`)
    // 100% solid hexagonal pillar spanning Z: 0 to Z: 128 (8.0m level height).
    let solid_core_map = r#"// Observed 2 strict authored solid central core column tile.
// 100% solid hexagonal pillar spanning Z: 0 to Z: 128 (8.0m level height).
{
"classname" "worldspawn"
{
// Base Floor Slab (Z: 0 to 8)
( 112 64 0 ) ( 112 -64 0 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 0 ) ( 0 -128 0 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 0 ) ( -112 -64 0 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 0 ) ( -112 64 0 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 0 ) ( 0 128 0 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 0 ) ( 112 64 0 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 0 64 8 ) ( 64 0 8 ) __TB_empty 0 0 0 1 1
}
{
// Solid Central Core Column Brush (Z: 8 to 128)
( 104 56 8 ) ( 104 -56 8 ) ( 104 56 128 ) __TB_empty 0 0 0 1 1
( 104 -56 8 ) ( 0 -112 8 ) ( 104 -56 128 ) __TB_empty 0 0 0 1 1
( 0 -112 8 ) ( -104 -56 8 ) ( 0 -112 128 ) __TB_empty 0 0 0 1 1
( -104 -56 8 ) ( -104 56 8 ) ( -104 -56 128 ) __TB_empty 0 0 0 1 1
( -104 56 8 ) ( 0 112 8 ) ( -104 56 128 ) __TB_empty 0 0 0 1 1
( 0 112 8 ) ( 104 56 8 ) ( 0 112 128 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 64 0 8 ) ( 0 64 8 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}
}
{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/tower_solid_core"
"kind" "cell"
"archetype" "tower_solid_core"
"register" "generic"
"register_scope" "all"
"variant" "0"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "1"
}
{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}
"#
    .to_string();

    let solid_file = output_dir.join("tower_solid_core.map");
    fs::write(&solid_file, solid_core_map)
        .map_err(|err| format!("{}: {err}", solid_file.display()))?;
    generated_files.push(solid_file.display().to_string());

    // 2. Single Canonical Inner-Wall Ramp Tile (`tower_inner_ramp.map`)
    // 4.0m rise (64 TB units) along West inner face (touching core at 0,0).
    // SW corner (Z: 8) -> NW corner (Z: 72). Sixfold rotation policy handles all 6 perimeter sectors!
    let inner_ramp_map =
        r#"// Observed 2 strict authored canonical inner-wall ramp tile (Option A: 4.0m rise).
// SW corner Z: 8 ==> NW corner Z: 72. Inner wall runs along West face.
{
"classname" "worldspawn"
{
// Base Floor Slab (Z: 0 to 8)
( 112 64 0 ) ( 112 -64 0 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 0 ) ( 0 -128 0 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 0 ) ( -112 -64 0 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 0 ) ( -112 64 0 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 0 ) ( 0 128 0 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 0 ) ( 112 64 0 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 0 64 8 ) ( 64 0 8 ) __TB_empty 0 0 0 1 1
}
{
// Sloped Inner-Wall Ramp Deck (4.0m Rise along West face: Z: 8 to Z: 72)
( 40 64 0 ) ( 40 -64 0 ) ( 40 64 128 ) __TB_empty 0 0 0 1 1
( -104 -64 0 ) ( -104 64 0 ) ( -104 -64 8 ) __TB_empty 0 0 0 1 1
( -104 64 0 ) ( 40 64 0 ) ( -104 64 8 ) __TB_empty 0 0 0 1 1
( 40 -64 0 ) ( -104 -64 0 ) ( 40 -64 72 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( -104 -64 8 ) ( -104 64 8 ) ( 40 -64 72 ) __TB_empty 0 0 0 1 1
}
{
// Ceiling Header Slab (Z: 120 to 128)
( 112 64 120 ) ( 112 -64 120 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 120 ) ( 0 -128 120 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 120 ) ( -112 -64 120 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 120 ) ( -112 64 120 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 120 ) ( 0 128 120 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 120 ) ( 112 64 120 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 120 ) ( 64 0 120 ) ( 0 64 120 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}
}
{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/tower_inner_ramp"
"kind" "cell"
"archetype" "tower_inner_ramp"
"register" "generic"
"register_scope" "all"
"variant" "0"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "2"
}
{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "west"
"class" "door"
"name" "west_door"
"origin" "-112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "east"
"class" "door"
"name" "east_door"
"origin" "112 0 48"
}
"#
        .to_string();

    let ramp_file = output_dir.join("tower_inner_ramp.map");
    fs::write(&ramp_file, inner_ramp_map)
        .map_err(|err| format!("{}: {err}", ramp_file.display()))?;
    generated_files.push(ramp_file.display().to_string());

    // 3. Flat Landing Deck Tile (`tower_flat_deck.map`)
    // Flat floor landing platform (Z: 0 to 8 floor slab + open interior).
    let flat_deck_map = r#"// Observed 2 strict authored canonical flat landing deck tile.
// Flat landing deck for tower balcony levels.
{
"classname" "worldspawn"
{
// Base Floor Slab (Z: 0 to 8)
( 112 64 0 ) ( 112 -64 0 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 0 ) ( 0 -128 0 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 0 ) ( -112 -64 0 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 0 ) ( -112 64 0 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 0 ) ( 0 128 0 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 0 ) ( 112 64 0 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 0 64 8 ) ( 64 0 8 ) __TB_empty 0 0 0 1 1
}
{
// Ceiling Header Slab (Z: 120 to 128)
( 112 64 120 ) ( 112 -64 120 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 120 ) ( 0 -128 120 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 120 ) ( -112 -64 120 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 120 ) ( -112 64 120 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 120 ) ( 0 128 120 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 120 ) ( 112 64 120 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 120 ) ( 64 0 120 ) ( 0 64 120 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}
}
{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/tower_flat_deck"
"kind" "cell"
"archetype" "tower_flat_deck"
"register" "generic"
"register_scope" "all"
"variant" "0"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "2"
}
{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "west"
"class" "door"
"name" "west_door"
"origin" "-112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "east"
"class" "door"
"name" "east_door"
"origin" "112 0 48"
}
"#
    .to_string();

    let deck_file = output_dir.join("tower_flat_deck.map");
    fs::write(&deck_file, flat_deck_map)
        .map_err(|err| format!("{}: {err}", deck_file.display()))?;
    generated_files.push(deck_file.display().to_string());

    Ok(generated_files)
}
