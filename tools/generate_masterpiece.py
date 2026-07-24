#!/usr/bin/env python3
"""
Masterpiece Hex Tile Generator: "The Helix Cathedral Sanctuary"
Generates a perfectly snapped, v2-strict TrenchBroom .map file for Observed 2.
Features a base floor slab, sloped inner-wall ramp, central core obelisk, ceiling vault header,
and precise spatial ports.
"""

from pathlib import Path

def generate_masterpiece_map():
    map_content = """// Observed 2 Masterpiece Authored Tile: "The Helix Cathedral Sanctuary"
// Archetype: sanctuary | Kind: Cell | Register: monument | Rotation: SixFold
// Multi-tiered cathedral cell combining central obelisk, sloped inner ramp helix, & vaulted ceiling.

{
"classname" "worldspawn"
{
// 1. Base Hex Floor Slab (Z: 0 to 8)
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
// 2. Central Obelisk Column (Z: 8 to 128)
( 40 40 8 ) ( 40 -40 8 ) ( 40 40 128 ) __TB_empty 0 0 0 1 1
( 40 -40 8 ) ( -40 -40 8 ) ( 40 -40 128 ) __TB_empty 0 0 0 1 1
( -40 -40 8 ) ( -40 40 8 ) ( -40 -40 128 ) __TB_empty 0 0 0 1 1
( -40 40 8 ) ( 40 40 8 ) ( -40 40 128 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 32 0 8 ) ( 0 32 8 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 32 128 ) ( 32 0 128 ) __TB_empty 0 0 0 1 1
}
{
// 3. Sloped Inner-Wall Ramp Deck (4.0m Rise along West face: Z: 8 to Z: 72)
( 40 64 0 ) ( 40 -64 0 ) ( 40 64 128 ) __TB_empty 0 0 0 1 1
( -104 -64 0 ) ( -104 64 0 ) ( -104 -64 8 ) __TB_empty 0 0 0 1 1
( -104 64 0 ) ( 40 64 0 ) ( -104 64 8 ) __TB_empty 0 0 0 1 1
( 40 -64 0 ) ( -104 -64 0 ) ( 40 -64 72 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( -104 -64 8 ) ( -104 64 8 ) ( 40 -64 72 ) __TB_empty 0 0 0 1 1
}
{
// 4. Vaulted Ceiling Header Slab (Z: 120 to 128)
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
"id" "authored/masterpiece_helix_sanctuary"
"kind" "cell"
"archetype" "ramp"
"register" "monument"
"register_scope" "all"
"variant" "0"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "10"
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
"name" "west_portal"
"origin" "-112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "east"
"class" "door"
"name" "east_portal"
"origin" "112 0 48"
}
"""
    return map_content

def main():
    out_dir = Path("assets/tiles/authored")
    out_dir.mkdir(parents=True, exist_ok=True)
    file_path = out_dir / "masterpiece_helix_sanctuary.map"
    with open(file_path, "w", encoding="utf-8") as f:
        f.write(generate_masterpiece_map().strip() + "\n")
    print(f"Generated masterpiece tile -> {file_path}")

if __name__ == "__main__":
    main()
