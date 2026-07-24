#!/usr/bin/env python3
"""
Masterpiece 1 Generator: "The Brutalist Lightwell Silo"
Translates brutalist lightwell silo intent into integer TrenchBroom .map brushes using exact CCW plane winding:
- Base Hex Floor Slab (Z: 0 to 8)
- Central Lightwell Obelisk Core (Z: 8 to 128)
- Sloped Inner-Wall Ramp Flight (Z: 8 to 68)
- Mezzanine Balcony Deck (Z: 68 to 76)
- Vaulted Dome Ceiling Header (Z: 120 to 128)
- Typed Spatial Ports (East Door, West Door, Zenith Up Shaft)
"""

from pathlib import Path

def generate_silo_map():
    map_content = """// Observed 2 Masterpiece 1: "The Brutalist Lightwell Silo"
// Archetype: ramp | Kind: Cell | Register: monument | Rotation: SixFold
// Monumental lightwell silo combining central obelisk, sloped ramp helix, mezzanine balcony, & vaulted ceiling canopy.

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
// 2. Central Lightwell Obelisk Core (Z: 8 to 128, CCW loop: (36,-36)->(36,36)->(-36,36)->(-36,-36)->(36,-36))
( 36 -36 8 ) ( 36 -36 128 ) ( 36 36 8 ) __TB_empty 0 0 0 1 1
( 36 36 8 ) ( 36 36 128 ) ( -36 36 8 ) __TB_empty 0 0 0 1 1
( -36 36 8 ) ( -36 36 128 ) ( -36 -36 8 ) __TB_empty 0 0 0 1 1
( -36 -36 8 ) ( -36 -36 128 ) ( 36 -36 8 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 32 0 8 ) ( 0 32 8 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 32 128 ) ( 32 0 128 ) __TB_empty 0 0 0 1 1
}
{
// 3. Sloped Inner-Wall Ramp Flight (4.0m Rise along West face: Z: 8 to Z: 68)
( 40 64 0 ) ( 40 -64 0 ) ( 40 64 128 ) __TB_empty 0 0 0 1 1
( -104 -64 0 ) ( -104 64 0 ) ( -104 -64 8 ) __TB_empty 0 0 0 1 1
( -104 64 0 ) ( 40 64 0 ) ( -104 64 8 ) __TB_empty 0 0 0 1 1
( 40 -64 0 ) ( -104 -64 0 ) ( 40 -64 68 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( -104 -64 8 ) ( -104 64 8 ) ( 40 -64 68 ) __TB_empty 0 0 0 1 1
}
{
// 4. Mezzanine Balcony Deck (Z: 68 to 76, CCW loop: (104,-36)->(104,36)->(36,36)->(36,-36)->(104,-36))
( 104 -36 68 ) ( 104 -36 76 ) ( 104 36 68 ) __TB_empty 0 0 0 1 1
( 104 36 68 ) ( 104 36 76 ) ( 36 36 68 ) __TB_empty 0 0 0 1 1
( 36 36 68 ) ( 36 36 76 ) ( 36 -36 68 ) __TB_empty 0 0 0 1 1
( 36 -36 68 ) ( 36 -36 76 ) ( 104 -36 68 ) __TB_empty 0 0 0 1 1
( 0 0 68 ) ( 32 0 68 ) ( 0 32 68 ) __TB_empty 0 0 0 1 1
( 0 0 76 ) ( 0 32 76 ) ( 32 0 76 ) __TB_empty 0 0 0 1 1
}
{
// 5. Vaulted Dome Ceiling Header (Z: 120 to 128)
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
"id" "authored/masterpiece_lightwell_silo"
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
"name" "west_silo_portal"
"origin" "-112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "east"
"class" "door"
"name" "east_silo_portal"
"origin" "112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "up"
"class" "shaft_open"
"name" "zenith_silo_shaft"
"origin" "0 0 128"
}
"""
    return map_content

def main():
    out_dir = Path("assets/tiles/authored")
    out_dir.mkdir(parents=True, exist_ok=True)
    file_path = out_dir / "masterpiece_lightwell_silo.map"
    with open(file_path, "w", encoding="utf-8") as f:
        f.write(generate_silo_map().strip() + "\n")
    print(f"Generated Masterpiece 1 -> {file_path}")

if __name__ == "__main__":
    main()
