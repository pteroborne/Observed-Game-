#!/usr/bin/env python3
"""
Practice Scenario C / Masterpiece Generator: "The Masterpiece Monumental Sanctuary v2"
Translates monumental multi-tiered cathedral hub intent into integer TrenchBroom .map brushes using exact CCW plane winding:
- Base Hex Floor Slab (Z: 0 to 8)
- Central Lightwell Obelisk Core (Z: 8 to 128)
- Sloped Inner-Wall Ramp Flight (Z: 8 to 68)
- High Cantilevered Gantry Overpass (Z: 68 to 76)
- Vaulted Ceiling Canopy (Z: 120 to 128)
- East & West Door Ports + Zenith Up Light Shaft Port
"""

from pathlib import Path

def generate_masterpiece_v2_map():
    map_content = """// Observed 2 Practice Scenario C: "The Masterpiece Monumental Sanctuary v2"
// Archetype: ramp | Kind: Cell | Register: monument | Rotation: SixFold
// Monumental multi-tiered cathedral hub combining central lightwell, high gantry bridge, & sloped ramp helix.

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
// 2. Central Lightwell Obelisk Pillar (Z: 8 to 128, CCW loop: (36,-36)->(36,36)->(-36,36)->(-36,-36)->(36,-36))
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
// 4. High Cantilevered Gantry Overpass Bridge (Z: 68 to 76, CCW loop: (112,-24)->(112,24)->(-112,24)->(-112,-24)->(112,-24))
( 112 -24 68 ) ( 112 -24 76 ) ( 112 24 68 ) __TB_empty 0 0 0 1 1
( 112 24 68 ) ( 112 24 76 ) ( -112 24 68 ) __TB_empty 0 0 0 1 1
( -112 24 68 ) ( -112 24 76 ) ( -112 -24 68 ) __TB_empty 0 0 0 1 1
( -112 -24 68 ) ( -112 -24 76 ) ( 112 -24 68 ) __TB_empty 0 0 0 1 1
( 0 0 68 ) ( 32 0 68 ) ( 0 32 68 ) __TB_empty 0 0 0 1 1
( 0 0 76 ) ( 0 32 76 ) ( 32 0 76 ) __TB_empty 0 0 0 1 1
}
{
// 5. Vaulted Ceiling Canopy (Z: 120 to 128)
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
"id" "authored/masterpiece_sanctuary_v2"
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
"name" "west_sanctuary_portal"
"origin" "-112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "east"
"class" "door"
"name" "east_sanctuary_portal"
"origin" "112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "up"
"class" "shaft_open"
"name" "zenith_sanctuary_shaft"
"origin" "0 0 128"
}
"""
    return map_content

def main():
    out_dir = Path("assets/tiles/authored")
    out_dir.mkdir(parents=True, exist_ok=True)
    file_path = out_dir / "masterpiece_sanctuary_v2.map"
    with open(file_path, "w", encoding="utf-8") as f:
        f.write(generate_masterpiece_v2_map().strip() + "\n")
    print(f"Generated Practice Scenario C / Masterpiece v2 -> {file_path}")

if __name__ == "__main__":
    main()
