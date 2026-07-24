#!/usr/bin/env python3
"""
Practice Scenario B Generator: "The Claustrophobic Choke Vault"
Translates low-ceiling, high-density choke point intent into integer TrenchBroom .map brushes using exact counterclockwise CCW plane winding.
"""

from pathlib import Path

def generate_choke_map():
    map_content = """// Observed 2 Practice Scenario B: "The Claustrophobic Choke Vault"
// Archetype: ramp | Kind: Cell | Register: institutional | Rotation: SixFold
// Low-ceiling, high-density structural junction with heavy side-flanges & constricted passage.

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
// 2. North Heavy Wall Flange (Z: 8 to 128, CCW loop: (112,32)->(112,64)->(0,128)->(-112,64)->(-112,32)->(112,32))
( 112 32 8 ) ( 112 32 128 ) ( 112 64 8 ) __TB_empty 0 0 0 1 1
( 112 64 8 ) ( 112 64 128 ) ( 0 128 8 ) __TB_empty 0 0 0 1 1
( 0 128 8 ) ( 0 128 128 ) ( -112 64 8 ) __TB_empty 0 0 0 1 1
( -112 64 8 ) ( -112 64 128 ) ( -112 32 8 ) __TB_empty 0 0 0 1 1
( -112 32 8 ) ( -112 32 128 ) ( 112 32 8 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 32 0 8 ) ( 0 32 8 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 32 128 ) ( 32 0 128 ) __TB_empty 0 0 0 1 1
}
{
// 3. South Heavy Wall Flange (Z: 8 to 128, CCW loop: (-112,-32)->(-112,-64)->(0,-128)->(112,-64)->(112,-32)->(-112,-32))
( -112 -32 8 ) ( -112 -32 128 ) ( -112 -64 8 ) __TB_empty 0 0 0 1 1
( -112 -64 8 ) ( -112 -64 128 ) ( 0 -128 8 ) __TB_empty 0 0 0 1 1
( 0 -128 8 ) ( 0 -128 128 ) ( 112 -64 8 ) __TB_empty 0 0 0 1 1
( 112 -64 8 ) ( 112 -64 128 ) ( 112 -32 8 ) __TB_empty 0 0 0 1 1
( 112 -32 8 ) ( 112 -32 128 ) ( -112 -32 8 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 32 0 8 ) ( 0 32 8 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 32 128 ) ( 32 0 128 ) __TB_empty 0 0 0 1 1
}
{
// 4. Low Ceiling Lintel Vault (Z: 48 to 128)
( 112 32 48 ) ( 112 32 128 ) ( -112 32 48 ) __TB_empty 0 0 0 1 1
( -112 32 48 ) ( -112 32 128 ) ( -112 -32 48 ) __TB_empty 0 0 0 1 1
( -112 -32 48 ) ( -112 -32 128 ) ( 112 -32 48 ) __TB_empty 0 0 0 1 1
( 112 -32 48 ) ( 112 -32 128 ) ( 112 32 48 ) __TB_empty 0 0 0 1 1
( 0 0 48 ) ( 32 0 48 ) ( 0 32 48 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 32 128 ) ( 32 0 128 ) __TB_empty 0 0 0 1 1
}
}
{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/practice_choke_vault"
"kind" "cell"
"archetype" "ramp"
"register" "institutional"
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
"name" "west_choke_portal"
"origin" "-112 0 48"
}
{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "east"
"class" "door"
"name" "east_choke_portal"
"origin" "112 0 48"
}
"""
    return map_content

def main():
    out_dir = Path("assets/tiles/authored")
    out_dir.mkdir(parents=True, exist_ok=True)
    file_path = out_dir / "practice_choke_vault.map"
    with open(file_path, "w", encoding="utf-8") as f:
        f.write(generate_choke_map().strip() + "\n")
    print(f"Generated Practice Scenario B -> {file_path}")

if __name__ == "__main__":
    main()
