"""Assemble the production sector brushes for a full-room evidence CAD snapshot."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).with_name("rain-plan.map")


def entities(text):
    start = depth = 0
    for i, ch in enumerate(text):
        if ch == "{":
            if depth == 0:
                start = i
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                yield text[start:i + 1]


def translate(text, x, y):
    def point(match):
        a, b, c = map(float, match.groups())
        return f"( {a + x:.4f} {b + y:.4f} {c:.4f} )"
    text = re.sub(r"\(\s*([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s*\)", point, text)
    def origin(match):
        a, b, c = map(float, match.groups())
        return f'"origin" "{a + x:.4f} {b + y:.4f} {c:.4f}"'
    return re.sub(r'"origin" "([-\d.]+) ([-\d.]+) ([-\d.]+)"', origin, text)


brushes, lights = [], []
for stem, x, y in [("rain_court", 0, 0), ("rain_court_sw", 224, 0), ("rain_court_nw", 112, -192)]:
    source = list(entities((ROOT / "assets/tiles/authored" / f"{stem}.map").read_text()))
    world = translate(source[0], x, y)
    brushes.append(world[world.index('\n{'):world.rfind("}")])
    lights.extend(translate(e, x, y) for e in source if '"classname" "tile_light"' in e)

# The evidence room's exterior contract matches the three-cell production triad.
contract = '''{
"classname" "tile_meta"
"id" "evidence/rain-court"
"kind" "room"
"room_role" "decision"
"archetype" "rain_court"
"register" "generic"
"register_scope" "shadow_screen"
"variant" "0"
"levels" "1"
"rotation_policy" "none"
"weight" "1"
}
'''
for q, r, face, x, y in [(0, 0, "north_west", -56, 96), (1, 0, "east", 336, 0), (0, 1, "south_west", 56, -288)]:
    contract += f'''{{
"classname" "tile_cell"
"q" "{q}"
"r" "{r}"
"level" "0"
"levels" "1"
"floor" "solid"
}}
{{
"classname" "tile_port"
"q" "{q}"
"r" "{r}"
"level" "0"
"face" "{face}"
"class" "door"
"name" "threshold_{q}_{r}"
"origin" "{x} {y} 48"
}}
'''
OUT.write_text('{\n"classname" "worldspawn"' + ''.join(brushes) + '}\n' + contract + '\n'.join(lights) + '\n')
print(OUT)
