#!/usr/bin/env python3
"""
7-Tile Masterpiece Suite Generator: "The Monumental Obsidian Cathedral Cluster"
Guarantees 100% watertight base floor slabs (Z: 0 to 8) for all 7 member tiles.
"""

from pathlib import Path

def generate_7tile_suite():
    tiles = {}

    base_slab = """// Base Floor Slab (Z: 0 to 8)
( 112 64 0 ) ( 112 -64 0 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 0 ) ( 0 -128 0 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 0 ) ( -112 -64 0 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 0 ) ( -112 64 0 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 0 ) ( 0 128 0 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 0 ) ( 112 64 0 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 0 64 8 ) ( 64 0 8 ) __TB_empty 0 0 0 1 1"""

    # 1. Center Cell (0,0): Solid Obsidian Core (Z: 0 to 128)
    tiles["masterpiece_7hex_core_silo.map"] = f"""// Observed 2 Masterpiece 7-Tile Suite: Center Cell (0,0) - Solid Obsidian Core
{{
"classname" "worldspawn"
{{
{base_slab}
}}
{{
// Solid Core Obelisk (Z: 8 to 128)
( 112 64 8 ) ( 112 -64 8 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 8 ) ( 0 -128 8 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 8 ) ( -112 -64 8 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 8 ) ( -112 64 8 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 8 ) ( 0 128 8 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 8 ) ( 112 64 8 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 64 0 8 ) ( 0 64 8 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}}
}}
{{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/masterpiece_7hex_core_silo"
"kind" "cell"
"archetype" "tower_solid_core"
"register" "monument"
"register_scope" "all"
"variant" "0"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "10"
}}
{{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}}
"""

    # 2. East Cell (1,0): Ground Entrance Deck (Z = 0m)
    tiles["masterpiece_7hex_deck_east.map"] = f"""// Observed 2 Masterpiece 7-Tile Suite: East Cell (1,0) - Ground Entrance Deck
{{
"classname" "worldspawn"
{{
{base_slab}
}}
{{
// Ceiling Header Vault (Z: 120 to 128)
( 112 64 120 ) ( 112 -64 120 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 120 ) ( 0 -128 120 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 120 ) ( -112 -64 120 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 120 ) ( -112 64 120 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 120 ) ( 0 128 120 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 120 ) ( 112 64 120 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 120 ) ( 64 0 120 ) ( 0 64 120 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}}
}}
{{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/masterpiece_7hex_deck_east"
"kind" "cell"
"archetype" "tower_flat_deck"
"register" "monument"
"register_scope" "all"
"variant" "0"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "5"
}}
{{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}}
{{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "east"
"class" "door"
"name" "east_entrance"
"origin" "112 0 48"
}}
"""

    # 3. SE Cell (1,-1): Lower Inner Ramp Flight (0m -> 4m)
    tiles["masterpiece_7hex_ramp_se.map"] = f"""// Observed 2 Masterpiece 7-Tile Suite: SE Cell (1,-1) - Lower Inner Ramp Flight
{{
"classname" "worldspawn"
{{
{base_slab}
}}
{{
// Sloped Inner-Wall Ramp Deck (4.0m Rise along West face: Z: 8 to Z: 72)
( 40 64 0 ) ( 40 -64 0 ) ( 40 64 128 ) __TB_empty 0 0 0 1 1
( -104 -64 0 ) ( -104 64 0 ) ( -104 -64 8 ) __TB_empty 0 0 0 1 1
( -104 64 0 ) ( 40 64 0 ) ( -104 64 8 ) __TB_empty 0 0 0 1 1
( 40 -64 0 ) ( -104 -64 0 ) ( 40 -64 72 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( -104 -64 8 ) ( -104 64 8 ) ( 40 -64 72 ) __TB_empty 0 0 0 1 1
}}
{{
// Ceiling Header Vault (Z: 120 to 128)
( 112 64 120 ) ( 112 -64 120 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 120 ) ( 0 -128 120 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 120 ) ( -112 -64 120 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 120 ) ( -112 64 120 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 120 ) ( 0 128 120 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 120 ) ( 112 64 120 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 120 ) ( 64 0 120 ) ( 0 64 120 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}}
}}
{{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/masterpiece_7hex_ramp_se"
"kind" "cell"
"archetype" "tower_inner_ramp"
"register" "monument"
"register_scope" "all"
"variant" "0"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "5"
}}
{{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}}
"""

    # 4. SW Cell (0,-1): Mid Mezzanine Landing Deck (Z = 4m)
    tiles["masterpiece_7hex_deck_sw.map"] = f"""// Observed 2 Masterpiece 7-Tile Suite: SW Cell (0,-1) - Mid Mezzanine Landing Deck
{{
"classname" "worldspawn"
{{
{base_slab}
}}
{{
// Mezzanine Platform Deck (Z: 8 to 72)
( 112 64 8 ) ( 112 -64 8 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 8 ) ( 0 -128 8 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 8 ) ( -112 -64 8 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 8 ) ( -112 64 8 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 8 ) ( 0 128 8 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 8 ) ( 112 64 8 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 64 0 8 ) ( 0 64 8 ) __TB_empty 0 0 0 1 1
( 0 0 72 ) ( 0 64 72 ) ( 64 0 72 ) __TB_empty 0 0 0 1 1
}}
{{
// Ceiling Header Vault (Z: 120 to 128)
( 112 64 120 ) ( 112 -64 120 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 120 ) ( 0 -128 120 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 120 ) ( -112 -64 120 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 120 ) ( -112 64 120 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 120 ) ( 0 128 120 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 120 ) ( 112 64 120 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 120 ) ( 64 0 120 ) ( 0 64 120 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}}
}}
{{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/masterpiece_7hex_deck_sw"
"kind" "cell"
"archetype" "tower_flat_deck"
"register" "monument"
"register_scope" "all"
"variant" "1"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "5"
}}
{{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}}
"""

    # 5. West Cell (-1,0): Cantilevered Gantry Overpass (Z = 4m)
    tiles["masterpiece_7hex_gantry_west.map"] = f"""// Observed 2 Masterpiece 7-Tile Suite: West Cell (-1,0) - Cantilevered Gantry Overpass
{{
"classname" "worldspawn"
{{
{base_slab}
}}
{{
// Gantry Walkway Bridge (Z: 64 to 72)
( 112 -24 64 ) ( 112 -24 72 ) ( 112 24 64 ) __TB_empty 0 0 0 1 1
( 112 24 64 ) ( 112 24 72 ) ( -112 24 64 ) __TB_empty 0 0 0 1 1
( -112 24 64 ) ( -112 24 72 ) ( -112 -24 64 ) __TB_empty 0 0 0 1 1
( -112 -24 64 ) ( -112 -24 72 ) ( 112 -24 64 ) __TB_empty 0 0 0 1 1
( 0 0 64 ) ( 32 0 64 ) ( 0 32 64 ) __TB_empty 0 0 0 1 1
( 0 0 72 ) ( 0 32 72 ) ( 32 0 72 ) __TB_empty 0 0 0 1 1
}}
{{
// Ceiling Header Vault (Z: 120 to 128)
( 112 64 120 ) ( 112 -64 120 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 120 ) ( 0 -128 120 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 120 ) ( -112 -64 120 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 120 ) ( -112 64 120 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 120 ) ( 0 128 120 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 120 ) ( 112 64 120 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 120 ) ( 64 0 120 ) ( 0 64 120 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}}
}}
{{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/masterpiece_7hex_gantry_west"
"kind" "cell"
"archetype" "ramp"
"register" "monument"
"register_scope" "all"
"variant" "0"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "5"
}}
{{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}}
{{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "west"
"class" "door"
"name" "west_gantry"
"origin" "-112 0 48"
}}
"""

    # 6. NW Cell (-1,1): Upper Inner Ramp Flight (4m -> 8m, Z: 8 to 128)
    tiles["masterpiece_7hex_ramp_nw.map"] = f"""// Observed 2 Masterpiece 7-Tile Suite: NW Cell (-1,1) - Upper Inner Ramp Flight
{{
"classname" "worldspawn"
{{
{base_slab}
}}
{{
// Sloped Ramp Deck (4.0m Rise along West face: Z: 8 to Z: 72)
( 40 64 0 ) ( 40 -64 0 ) ( 40 64 128 ) __TB_empty 0 0 0 1 1
( -104 -64 0 ) ( -104 64 0 ) ( -104 -64 8 ) __TB_empty 0 0 0 1 1
( -104 64 0 ) ( 40 64 0 ) ( -104 64 8 ) __TB_empty 0 0 0 1 1
( 40 -64 0 ) ( -104 -64 0 ) ( 40 -64 72 ) __TB_empty 0 0 0 1 1
( 0 0 0 ) ( 64 0 0 ) ( 0 64 0 ) __TB_empty 0 0 0 1 1
( -104 -64 8 ) ( -104 64 8 ) ( 40 -64 72 ) __TB_empty 0 0 0 1 1
}}
{{
// Ceiling Header Vault (Z: 120 to 128)
( 112 64 120 ) ( 112 -64 120 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 120 ) ( 0 -128 120 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 120 ) ( -112 -64 120 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 120 ) ( -112 64 120 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 120 ) ( 0 128 120 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 120 ) ( 112 64 120 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 120 ) ( 64 0 120 ) ( 0 64 120 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}}
}}
{{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/masterpiece_7hex_ramp_nw"
"kind" "cell"
"archetype" "tower_inner_ramp"
"register" "monument"
"register_scope" "all"
"variant" "1"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "5"
}}
{{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}}
"""

    # 7. NE Cell (0,1): Upper Observation Deck (Z = 8m) & Skylight Port
    tiles["masterpiece_7hex_deck_ne.map"] = f"""// Observed 2 Masterpiece 7-Tile Suite: NE Cell (0,1) - Upper Observation Deck
{{
"classname" "worldspawn"
{{
{base_slab}
}}
{{
// High Observation Deck (Z: 8 to 128)
( 112 64 8 ) ( 112 -64 8 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 8 ) ( 0 -128 8 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 8 ) ( -112 -64 8 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 8 ) ( -112 64 8 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 8 ) ( 0 128 8 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 8 ) ( 112 64 8 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 8 ) ( 64 0 8 ) ( 0 64 8 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}}
{{
// Ceiling Header Vault (Z: 120 to 128)
( 112 64 120 ) ( 112 -64 120 ) ( 112 64 128 ) __TB_empty 0 0 0 1 1
( 112 -64 120 ) ( 0 -128 120 ) ( 112 -64 128 ) __TB_empty 0 0 0 1 1
( 0 -128 120 ) ( -112 -64 120 ) ( 0 -128 128 ) __TB_empty 0 0 0 1 1
( -112 -64 120 ) ( -112 64 120 ) ( -112 -64 128 ) __TB_empty 0 0 0 1 1
( -112 64 120 ) ( 0 128 120 ) ( -112 64 128 ) __TB_empty 0 0 0 1 1
( 0 128 120 ) ( 112 64 120 ) ( 0 128 128 ) __TB_empty 0 0 0 1 1
( 0 0 120 ) ( 64 0 120 ) ( 0 64 120 ) __TB_empty 0 0 0 1 1
( 0 0 128 ) ( 0 64 128 ) ( 64 0 128 ) __TB_empty 0 0 0 1 1
}}
}}
{{
"classname" "tile_meta"
"authoring_version" "2"
"id" "authored/masterpiece_7hex_deck_ne"
"kind" "cell"
"archetype" "tower_flat_deck"
"register" "monument"
"register_scope" "all"
"variant" "2"
"levels" "1"
"rotation_policy" "sixfold"
"weight" "5"
}}
{{
"classname" "tile_cell"
"q" "0"
"r" "0"
"level" "0"
"levels" "1"
"floor" "solid"
}}
{{
"classname" "tile_port"
"q" "0"
"r" "0"
"level" "0"
"face" "up"
"class" "shaft_open"
"name" "zenith_skylight"
"origin" "0 0 128"
}}
"""

    out_dir = Path("assets/tiles/authored")
    out_dir.mkdir(parents=True, exist_ok=True)

    for filename, content in tiles.items():
        file_path = out_dir / filename
        with open(file_path, "w", encoding="utf-8") as f:
            f.write(content.strip() + "\n")
        print(f"Generated 7-Tile Suite Member -> {file_path}")

if __name__ == "__main__":
    generate_7tile_suite()
