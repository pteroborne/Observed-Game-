#!/usr/bin/env python3
"""
Masterpiece 2 Generator: "The Cantilevered Gantry Vault"
Translates industrial brutalist gantry vault intent into integer TrenchBroom .map brushes using exact CCW plane winding:
- Base Hex Floor Slab (Z: 0 to 8)
- West Concrete Support Pylon (Z: 8 to 68)
- East Concrete Support Pylon (Z: 8 to 68)
- Cantilevered Gantry Walkway Bridge (Z: 68 to 76)
- Vaulted Ceiling Header Canopy (Z: 120 to 128)
- Typed Spatial Ports (East Door, West Door, Zenith Up Shaft)
"""

from pathlib import Path

def generate_gantry_vault_map():
    map_content = """// Observed 2 Masterpiece 2: "The Cantilevered Gantry Vault"
// Archetype: ramp | Kind: Cell | Register: megastructure | Rotation: SixFold
// Industrial brutalist gantry vault featuring cantilevered 3D bridge spanning central abyss between twin support pylons.

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
// 2. West Concrete Support Pylon (Z: 8 to 68, CCW loop: (-36,-24)->(-36,24)->(-60,24)->(-60,-24)->(-36,-24))
( -36 -24 8 ) ( -36 -24 68 ) ( -36 24 8 ) __TB_empty 0 0 0 1 1
( -36 24 8 ) ( -36 24 68 ) ( -60 24 8 ) __TB_empty 0 0 0 1 1
( -60 24 8 ) ( -60 24 68 ) ( -60 -24 8 ) __TB_empty 0 0 0 1 1
( -60 -24 8 ) ( -60 -24 68 ) ( -36 -24 8 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 32 0 8 ) ( 0 32 8 ) __TB_empty 0 0 0 1 1
( 0 0 68 ) ( 0 32 68 ) ( 32 0 68 ) __TB_empty 0 0 0 1 1
}
{
// 3. East Concrete Support Pylon (Z: 8 to 68, CCW loop: (60,-24)->(60,24)->(36,24)->(36,-24)->(60,-24))
( 60 -24 8 ) ( 60 -24 68 ) ( 60 24 8 ) __TB_empty 0 0 0 1 1
( 60 24 8 ) ( 60 24 68 ) ( 36 24 8 ) __TB_empty 0 0 0 1 1
( 36 24 8 ) ( 36 24 68 ) ( 36 -24 8 ) __TB_empty 0 0 0 1 1
( 36 -24 8 ) ( 36 -24 68 ) ( 60 -24 8 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 32 0 8 ) ( 0 32 8 ) __TB_empty 0 0 0 1 1
( 0 0 68 ) ( 0 32 68 ) ( 32 0 68 ) __TB_empty 0 0 0 1 1
}
{
// 4. Cantilevered Gantry Walkway Bridge (Z: 68 to 76, CCW loop: (112,-24)->(112,24)->(-112,24)->(-112,-24)->(112,-24))
( 112 -24 68 ) ( 112 -24 76 ) ( 112 24 68 ) __TB_empty 0 0 0 1 1
( 112 24 68 ) ( 112 24 76 ) ( -112 24 68 ) __TB_empty 0 0 0 1 1
( -112 24 68 ) ( -112 24 76 ) ( -112 -24 68 ) __TB_empty 0 0 0 1 1
( -112 -24 68 ) ( -112 -24 76 ) ( 112 -24 68 ) __TB_empty 0 0 0 1 1
( 0 0 68 ) ( 32 0 68 ) ( 0 32 68 ) __TB_empty 0 0 0 1 1
( 0 0 76 ) ( 0 32 76 ) ( 32 0 76 ) __TB_empty 0 0 0 1 1
}
{
// 5. Vaulted Ceiling Header Canopy (Z: 120 to 128)
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
"id" "authored/masterpiece_gantry_vault"
"kind" "cell"
"archetype" "ramp"
"register" "megastructure"
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
"name" "west_gantry_portal"
"origin" "-112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "east"
"class" "door"
"name" "east_gantry_portal"
"origin" "112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "up"
"class" "shaft_open"
"name" "zenith_gantry_shaft"
"origin" "0 0 128"
}
"""
    return map_content

def main():
    out_dir = Path("assets/tiles/authored")
    out_dir.mkdir(parents=True, exist_ok=True)
    file_path = out_dir / "masterpiece_gantry_vault.map"
    with open(file_path, "w", encoding="utf-8") as f:
        f.write(generate_gantry_vault_map().strip() + "\n")
    print(f"Generated Masterpiece 2 -> {file_path}")

if __name__ == "__main__":
    main()
