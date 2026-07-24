#!/usr/bin/env python3
"""Tile Forge - programmatic authoring of Observed 2 strict (v2) hex tiles.

Why this exists: the strict authoring contract (exact quantized-hexagon
footprint, mitered walls, seam-aligned door apertures, convex brushes with
correct plane winding) is plane-equation arithmetic that is easy to get wrong
when writing `.map` text by hand. This module ports the proven brush math from
`crates/observed_authoring/src/tile_source/geometry.rs` one-to-one, so a tile
is composed from primitives (walls, door walls, slabs, prisms, sloped flights)
and emitted as valid TrenchBroom `.map` text.

Coordinates are TrenchBroom units: Z-up, 16 units per meter, tile-local origin
at the cell center. The importer converts to world Y-up meters.

Authoring loop (run from the repo root):
    python tools/tileforge.py                        # regenerate registered tiles
    cargo run -p observed_authoring --bin tilec -- validate assets/tiles/authored/<name>.map
    cargo run -p observed_authoring --bin tilec -- render-cad assets/tiles/authored/<name>.map docs/evidence/<name>_cad.svg
    cargo run -p observed_authoring --bin tilec -- build
    # then preview in the lab (see docs/tile_authoring.md)

Contract cheat sheet (enforced by `tilec validate`):
- Footprint: every vertex inside the canonical hex prism. Corners (TB units):
  (112,64) (112,-64) (0,-128) (-112,-64) (-112,64) (0,128); height = levels*128.
- Faces (edge i = corner i -> i+1): 0 east, 1 south_east, 2 south_west,
  3 west, 4 north_west, 5 north_east.
- Door ports: origin must be the exact face-edge midpoint at z = level*128+48.
- Cell hull budget 32 (room 128). Floor must cover center + 6 corner samples.
- Headroom above the cell-center floor must be >= 2.2 m.
- Interior geometry that will be rotated must stay inside radius 104 units.
"""

import math
import os

UPM = 16.0
LEVEL = 128.0            # one level in TB units (8 m)
WALL = 8.0               # wall thickness (0.5 m)
FLOOR_TOP = 8.0          # floor slab thickness (0.5 m)
DOOR_HALF_WIDTH = 36.0   # door aperture is 72 units (4.5 m) wide
DOOR_TOP = 72.0          # lintel underside (4 m clearance above the floor)
SAFE_INTERIOR_RADIUS = 104.0

# World-plan corners (meters, x east / z south), CCW; face i edge = C[i]->C[i+1].
CORNERS_PLAN = [(7, -4), (7, 4), (0, 8), (-7, 4), (-7, -4), (0, -8)]
CORNERS = [(x * UPM, -z * UPM) for (x, z) in CORNERS_PLAN]
FACE_NAMES = ["east", "south_east", "south_west", "west", "north_west", "north_east"]


# --- vector helpers ---------------------------------------------------------

def lerp(a, b, t):
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)


def edge(face):
    """The two TB corners bounding lateral face `face` (A, B order)."""
    return CORNERS[face], CORNERS[(face + 1) % 6]


def face_mid(face):
    a, b = edge(face)
    return ((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5)


def offset_inward(a, b, t):
    """Move edge (a, b) inward (toward the hexagon center) by t units."""
    d = (b[0] - a[0], b[1] - a[1])
    length = math.hypot(*d)
    outward = (-d[1] / length, d[0] / length)
    sign = -1.0 if outward[0] * a[0] + outward[1] * a[1] > 0.0 else 1.0
    m = (outward[0] * sign * t, outward[1] * sign * t)
    return (a[0] + m[0], a[1] + m[1]), (b[0] + m[0], b[1] + m[1])


# --- plane and brush emission (exact port of geometry.rs) -------------------

def _fmt(v):
    if float(v) == int(v):
        return str(int(v))
    return f"{v:.4f}"


def plane_line(a, b, c):
    """One brush plane through 3D points a, b, c; outward normal is
    cross(c - a, b - a)."""
    pts = (a, b, c)
    coords = " ".join("( " + " ".join(_fmt(p[i]) for i in range(3)) + " )" for p in pts)
    return coords + " __TB_empty 0 0 0 1 1\n"


def side_plane(a2, b2, z0, interior_hint):
    """Vertical plane through 2D segment (a2, b2), wound so the outward normal
    points away from `interior_hint` (a 2D point on the solid side)."""
    a = (a2[0], a2[1], z0)
    b = (b2[0], b2[1], z0)
    c = (a2[0], a2[1], z0 + 64.0)
    normal = (-(b2[1] - a2[1]), b2[0] - a2[0])
    toward = (interior_hint[0] - a2[0], interior_hint[1] - a2[1])
    if normal[0] * toward[0] + normal[1] * toward[1] > 0.0:
        return plane_line(a, c, b)
    return plane_line(a, b, c)


def flat_plane(z, top):
    a = (0.0, 0.0, z)
    b = (64.0, 0.0, z)
    c = (0.0, 64.0, z)
    return plane_line(a, c, b) if top else plane_line(a, b, c)


def custom_plane(p0, p1, p2, want_up):
    """Plane through three 3D points, wound so the outward normal points up
    (want_up) or down."""
    n_z = ((p1[0] - p0[0]) * (p2[1] - p0[1])) - ((p1[1] - p0[1]) * (p2[0] - p0[0]))
    # normal = cross(p2 - p0, p1 - p0); its z component is -n_z
    up = -n_z > 0.0
    if up != want_up:
        p1, p2 = p2, p1
    return plane_line(p0, p1, p2)


def plane3(p0, p1, p2, interior):
    """Plane through three 3D points, wound so the outward normal points away
    from `interior` (a 3D point inside the brush). The general-orientation
    helper for chamfer/bevel planes."""
    u = (p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2])
    v = (p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2])
    # emission convention: outward normal = cross(C - A, B - A)
    n = (
        v[1] * u[2] - v[2] * u[1],
        v[2] * u[0] - v[0] * u[2],
        v[0] * u[1] - v[1] * u[0],
    )
    toward = (interior[0] - p0[0], interior[1] - p0[1], interior[2] - p0[2])
    if n[0] * toward[0] + n[1] * toward[1] + n[2] * toward[2] > 0.0:
        p1, p2 = p2, p1
    return plane_line(p0, p1, p2)


def rim_chamfers(corners, z_edge, z_face, chamfer, interior):
    """Chamfer planes around a convex plan polygon's horizontal rim: each
    plane runs from the side faces at `z_edge` to the top/bottom face inset by
    `chamfer`. Extra planes on a convex brush cost nothing — this is how
    tiles get smooth transitions instead of hard assembled-block edges.
    Inward is judged against the polygon's own centroid, so off-center decks
    and parapets chamfer correctly."""
    out = ""
    ref = centroid(corners)
    n = len(corners)
    for i in range(n):
        a, b = corners[i], corners[(i + 1) % n]
        d = (b[0] - a[0], b[1] - a[1])
        length = math.hypot(*d)
        normal = (-d[1] / length, d[0] / length)
        to_ref = (ref[0] - a[0], ref[1] - a[1])
        sign = 1.0 if normal[0] * to_ref[0] + normal[1] * to_ref[1] > 0.0 else -1.0
        inward = (normal[0] * sign * chamfer, normal[1] * sign * chamfer)
        mid = (
            (a[0] + b[0]) * 0.5 + inward[0],
            (a[1] + b[1]) * 0.5 + inward[1],
        )
        out += plane3(
            (a[0], a[1], z_edge),
            (b[0], b[1], z_edge),
            (mid[0], mid[1], z_face),
            interior,
        )
    return out


def centroid(corners):
    return (
        sum(p[0] for p in corners) / len(corners),
        sum(p[1] for p in corners) / len(corners),
    )


def prism(corners, z0, z1, hint=None, chamfer_top=0.0, chamfer_bottom=0.0):
    """A convex vertical prism from a plan polygon (perimeter order).
    Optional rim chamfers soften the top/bottom edges (extra planes, same
    single convex brush)."""
    hint = hint or centroid(corners)
    interior = (hint[0], hint[1], (z0 + z1) * 0.5)
    out = "{\n"
    n = len(corners)
    for i in range(n):
        out += side_plane(corners[i], corners[(i + 1) % n], z0, hint)
    out += flat_plane(z0, False)
    out += flat_plane(z1, True)
    if chamfer_top > 0.0:
        out += rim_chamfers(corners, z1 - chamfer_top, z1, chamfer_top, interior)
    if chamfer_bottom > 0.0:
        out += rim_chamfers(corners, z0 + chamfer_bottom, z0, chamfer_bottom, interior)
    out += "}\n"
    return out


def sloped_prism(corners, z0, top_points, hint=None):
    """A convex prism with a flat bottom at z0 and a slanted top plane through
    three 3D `top_points`."""
    hint = hint or centroid(corners)
    out = "{\n"
    n = len(corners)
    for i in range(n):
        out += side_plane(corners[i], corners[(i + 1) % n], z0, hint)
    out += flat_plane(z0, False)
    out += custom_plane(*top_points, want_up=True)
    out += "}\n"
    return out


def box(min3, max3):
    corners = [
        (min3[0], min3[1]),
        (max3[0], min3[1]),
        (max3[0], max3[1]),
        (min3[0], max3[1]),
    ]
    return prism(corners, min3[2], max3[2])


def hex_slab(z0, z1, chamfer_top=0.0, chamfer_bottom=0.0):
    """A full-footprint hexagonal slab (floor / ceiling). Rim chamfers give
    floors a threshold bevel at door openings and ceilings a soft cornice."""
    interior = (0.0, 0.0, (z0 + z1) * 0.5)
    out = "{\n"
    for face in range(6):
        a, b = edge(face)
        out += side_plane(a, b, z0, (0.0, 0.0))
    out += flat_plane(z0, False)
    out += flat_plane(z1, True)
    if chamfer_top > 0.0:
        out += rim_chamfers(CORNERS, z1 - chamfer_top, z1, chamfer_top, interior)
    if chamfer_bottom > 0.0:
        out += rim_chamfers(CORNERS, z0 + chamfer_bottom, z0, chamfer_bottom, interior)
    out += "}\n"
    return out


def band(face, t0, t1, z0, z1):
    """A band hugging `face` between inward offsets t0..t1, mitered against
    the neighbor faces so bands at matching offsets meet cleanly."""
    a, b = edge(face)
    oa, ob = offset_inward(a, b, t0)
    ia, ib = offset_inward(a, b, t1)
    pa0, pb0 = edge((face + 5) % 6)
    na0, nb0 = edge((face + 1) % 6)
    pa, pb = offset_inward(pa0, pb0, t0)
    na, nb = offset_inward(na0, nb0, t0)
    mid = (
        (oa[0] + ob[0] + ia[0] + ib[0]) * 0.25,
        (oa[1] + ob[1] + ia[1] + ib[1]) * 0.25,
    )
    out = "{\n"
    out += side_plane(oa, ob, z0, (0.0, 0.0))
    out += side_plane(ia, ib, z0, ((oa[0] + ob[0]) * 0.5, (oa[1] + ob[1]) * 0.5))
    out += side_plane(pa, pb, z0, mid)
    out += side_plane(na, nb, z0, mid)
    out += flat_plane(z0, False)
    out += flat_plane(z1, True)
    out += "}\n"
    return out


def wall(face, z0, z1):
    """A full sealed wall on `face`, mitered against its neighbors."""
    return band(face, 0.0, WALL, z0, z1)


def door_wall(face, z0, z1, sill=FLOOR_TOP, top=DOOR_TOP, splay=10.0, lintel_bevel=8.0):
    """A doorway wall on `face`: two jambs beside the canonical 4.5 m opening,
    a lintel wall above `top`, and a sill band below `sill`. The aperture at
    the seam plane stays exactly canonical (72 wide, sill..top) so tiles keep
    matching across seams; `splay` widens only the *interior* reveal so the
    opening reads as a shaped portal instead of a punched hole, and
    `lintel_bevel` chamfers the lintel soffit's inner edge."""
    a, b = edge(face)
    ia, ib = offset_inward(a, b, WALL)
    inward = (ia[0] - a[0], ia[1] - a[1])
    d = (b[0] - a[0], b[1] - a[1])
    length = math.hypot(*d)
    u = (d[0] / length, d[1] / length)
    mid = ((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5)
    da = (mid[0] - u[0] * DOOR_HALF_WIDTH, mid[1] - u[1] * DOOR_HALF_WIDTH)
    db = (mid[0] + u[0] * DOOR_HALF_WIDTH, mid[1] + u[1] * DOOR_HALF_WIDTH)
    ida, idb = offset_inward(da, db, WALL)
    # Splayed reveal: interior aperture corners slide away from the door
    # center, angling the reveal faces.
    ida = (ida[0] - u[0] * splay, ida[1] - u[1] * splay)
    idb = (idb[0] + u[0] * splay, idb[1] + u[1] * splay)
    out = ""
    left_hint = (
        (a[0] + da[0]) * 0.5 + inward[0] * 0.5,
        (a[1] + da[1]) * 0.5 + inward[1] * 0.5,
    )
    out += prism([a, da, ida, ia], z0, top, left_hint)
    right_hint = (
        (db[0] + b[0]) * 0.5 + inward[0] * 0.5,
        (db[1] + b[1]) * 0.5 + inward[1] * 0.5,
    )
    out += prism([db, b, ib, idb], z0, top, right_hint)
    wall_hint = (mid[0] + inward[0] * 0.5, mid[1] + inward[1] * 0.5)
    if top < z1:
        # Lintel wall with a bevelled soffit edge on the interior side.
        interior = (wall_hint[0], wall_hint[1], (top + z1) * 0.5)
        out_lintel = "{\n"
        for seg in [(a, b), (ib, ia)]:
            out_lintel += side_plane(seg[0], seg[1], top, wall_hint)
        out_lintel += side_plane(a, ia, top, wall_hint)
        out_lintel += side_plane(b, ib, top, wall_hint)
        out_lintel += flat_plane(top, False)
        out_lintel += flat_plane(z1, True)
        if lintel_bevel > 0.0:
            inset = offset_inward(a, b, WALL - lintel_bevel * 0.5)
            bevel_mid = (
                (inset[0][0] + inset[1][0]) * 0.5,
                (inset[0][1] + inset[1][1]) * 0.5,
            )
            out_lintel += plane3(
                (ia[0], ia[1], top + lintel_bevel),
                (ib[0], ib[1], top + lintel_bevel),
                (bevel_mid[0], bevel_mid[1], top),
                interior,
            )
        out_lintel += "}\n"
        out += out_lintel
    if sill > z0:
        out += prism([a, b, ib, ia], z0, sill, wall_hint)
    return out


def pylon(radius, z0, z1, phase_deg=0.0, chamfer_top=0.0, chamfer_bottom=0.0):
    """A regular hexagonal pylon centered at the origin. Chamfers turn hard
    plinth/capital edges into bevelled transitions."""
    corners = []
    for i in range(6):
        ang = math.radians(phase_deg + i * 60.0)
        corners.append((radius * math.cos(ang), radius * math.sin(ang)))
    return prism(
        corners,
        z0,
        z1,
        (0.0, 0.0),
        chamfer_top=chamfer_top,
        chamfer_bottom=chamfer_bottom,
    )


# --- entities ---------------------------------------------------------------

def point_entity(props):
    out = "{\n"
    for key, value in props:
        out += f'"{key}" "{value}"\n'
    out += "}\n"
    return out


def worldspawn(brushes):
    return '{\n"classname" "worldspawn"\n' + brushes + "}\n"


def tile_meta(tile_id, archetype, variant, levels, weight, register="generic",
              register_scope="all", rotation_policy="sixfold", kind="cell"):
    return point_entity([
        ("classname", "tile_meta"),
        ("authoring_version", "2"),
        ("id", tile_id),
        ("kind", kind),
        ("archetype", archetype),
        ("register", register),
        ("register_scope", register_scope),
        ("variant", str(variant)),
        ("levels", str(levels)),
        ("rotation_policy", rotation_policy),
        ("weight", str(weight)),
    ])


def tile_cell(q=0, r=0, level=0, levels=1, floor="solid"):
    return point_entity([
        ("classname", "tile_cell"),
        ("q", str(q)),
        ("r", str(r)),
        ("level", str(level)),
        ("levels", str(levels)),
        ("floor", floor),
    ])


def vertical_port(face_name, klass, name, level=0):
    """An up/down port at the exact cell-center origin the validator demands."""
    z = (level + 1) * LEVEL if face_name == "up" else level * LEVEL
    return point_entity([
        ("classname", "tile_port"),
        ("q", "0"),
        ("r", "0"),
        ("level", str(level)),
        ("face", face_name),
        ("class", klass),
        ("name", name),
        ("origin", f"0 0 {_fmt(z)}"),
    ])


def lateral_port(face, klass, name, level=0):
    """A lateral port with the exact origin the validator demands."""
    mid = face_mid(face)
    origin = f"{_fmt(mid[0])} {_fmt(mid[1])} {_fmt(level * LEVEL + 48.0)}"
    return point_entity([
        ("classname", "tile_port"),
        ("q", "0"),
        ("r", "0"),
        ("level", str(level)),
        ("face", FACE_NAMES[face]),
        ("class", klass),
        ("name", name),
        ("origin", origin),
    ])


def tile_light(x, y, z):
    """A semantic practical source. Colour and energy stay presentation-owned."""
    return point_entity([
        ("classname", "tile_light"),
        ("kind", "practical"),
        ("origin", f"{_fmt(x)} {_fmt(y)} {_fmt(z)}"),
    ])


def ceiling_fixture(x=0.0, y=0.0, ceiling=LEVEL, half_x=18.0, half_y=10.0):
    """A recessed housing physically attached to a ceiling plus its source."""
    brush = box(
        (x - half_x, y - half_y, ceiling - 8.0),
        (x + half_x, y + half_y, ceiling),
    )
    return brush, tile_light(x, y, ceiling - 12.0)


def wall_fixture(face, along=0.5, z=88.0, width=20.0):
    """A shallow sconce attached to a wall and a source just inside it."""
    a, b = edge(face)
    length = math.hypot(b[0] - a[0], b[1] - a[1])
    half_t = width / (2.0 * length)
    oa, ob = offset_inward(a, b, WALL)
    ia, ib = offset_inward(a, b, WALL + 6.0)
    p0, p1 = lerp(oa, ob, along - half_t), lerp(oa, ob, along + half_t)
    q0, q1 = lerp(ia, ib, along - half_t), lerp(ia, ib, along + half_t)
    brush = prism([p0, p1, q1, q0], z - 8.0, z + 8.0, chamfer_top=2.0)
    la, lb = offset_inward(a, b, WALL + 14.0)
    source = lerp(la, lb, along)
    return brush, tile_light(source[0], source[1], z)


# --- the hall family: proper walls for the wall-less corpus ----------------
#
# These regenerate the eight committed corpus tiles (same stable IDs, same
# archetypes/variants/ports, so every seam signature is unchanged) with real
# envelopes: bevelled floor/ceiling slabs, sealed walls with a trim band, and
# canonical splayed-reveal doorways, plus a small identity element per type.

PORT_SHORT = ["east", "se", "sw", "west", "nw", "ne"]


def _square(center, half):
    return [
        (center[0] - half, center[1] - half),
        (center[0] + half, center[1] - half),
        (center[0] + half, center[1] + half),
        (center[0] - half, center[1] + half),
    ]


def hall_shell(door_faces):
    h = LEVEL
    brushes = "// Floor and ceiling slabs (bevelled rims)\n"
    brushes += hex_slab(0.0, FLOOR_TOP, chamfer_top=3.0)
    brushes += hex_slab(h - FLOOR_TOP, h, chamfer_bottom=3.0)
    for face in range(6):
        if face in door_faces:
            brushes += f"// Door wall: {FACE_NAMES[face]}\n"
            brushes += door_wall(face, 0.0, h)
        else:
            brushes += f"// Sealed wall + trim: {FACE_NAMES[face]}\n"
            brushes += wall(face, 0.0, h)
            brushes += band(face, WALL, WALL + 8.0, FLOOR_TOP, FLOOR_TOP + 12.0)
    return brushes


def hall_meta_and_ports(name, archetype, door_faces, extra_ports=""):
    out = tile_meta(f"authored/{name}", archetype, 0, 1, 10)
    out += tile_cell()
    for face in door_faces:
        short = PORT_SHORT[face] if face not in (0, 3) else FACE_NAMES[face]
        out += lateral_port(face, "door", f"{short}_port")
    out += extra_ports
    return out


def hall_straight():
    brushes = hall_shell([0, 3])
    brushes += "// Colonnade: two pillar pairs flanking the walk axis\n"
    for x in (-44.0, 44.0):
        for y in (-34.0, 34.0):
            brushes += prism(_square((x, y), 6.0), FLOOR_TOP, LEVEL - FLOOR_TOP, chamfer_top=3.0)
    lights = ""
    for x in (-48.0, 48.0):
        fixture, source = ceiling_fixture(x=x)
        brushes += fixture
        lights += source
    out = "// Straight hall, doors east/west, colonnade interior.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += hall_meta_and_ports("hall_straight", "hall_straight", [0, 3])
    out += lights
    return out


def hall_cap():
    brushes = hall_shell([0])
    brushes += "// Back-wall alcove: plinth and stele opposite the door\n"
    brushes += prism(
        [(-98.0, -34.0), (-72.0, -34.0), (-72.0, 34.0), (-98.0, 34.0)],
        FLOOR_TOP,
        24.0,
        chamfer_top=3.0,
    )
    brushes += prism(
        [(-96.0, -10.0), (-84.0, -10.0), (-84.0, 10.0), (-96.0, 10.0)],
        24.0,
        104.0,
        chamfer_top=4.0,
    )
    fixture, lights = ceiling_fixture(x=-48.0)
    brushes += fixture
    out = "// Dead-end cap, door east; alcove stele marks the sealed back.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += hall_meta_and_ports("hall_cap", "hall_cap", [0])
    out += lights
    return out


def _hall_turn(name, archetype, second_face):
    brushes = hall_shell([0, second_face])
    brushes += "// Guide pillars opposite the elbow\n"
    m0, m1 = face_mid(0), face_mid(second_face)
    bis = (m0[0] + m1[0], m0[1] + m1[1])
    length = math.hypot(*bis)
    d = (bis[0] / length, bis[1] / length)
    perp = (-d[1], d[0])
    for side in (-1.0, 1.0):
        center = (
            -d[0] * 40.0 + perp[0] * side * 40.0,
            -d[1] * 40.0 + perp[1] * side * 40.0,
        )
        brushes += prism(_square(center, 6.5), FLOOR_TOP, LEVEL - FLOOR_TOP, chamfer_top=3.0)
    fixture, lights = ceiling_fixture()
    brushes += fixture
    out = f"// Corner hall, doors east/{FACE_NAMES[second_face]}.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += hall_meta_and_ports(name, archetype, [0, second_face])
    out += lights
    return out


def hall_turn_60():
    return _hall_turn("hall_turn_60", "hall_turn_60", 5)


def hall_turn_120():
    return _hall_turn("hall_turn_120", "hall_turn_120", 4)


def _hall_junction(name, archetype, door_faces):
    brushes = hall_shell(door_faces)
    brushes += "// Waypoint pylon with base collar\n"
    brushes += pylon(14.0, FLOOR_TOP, LEVEL - FLOOR_TOP, chamfer_top=5.0)
    brushes += pylon(24.0, FLOOR_TOP, 22.0, chamfer_top=5.0)
    lights = ""
    for x in (-44.0, 44.0):
        fixture, source = ceiling_fixture(x=x, half_x=14.0, half_y=8.0)
        brushes += fixture
        lights += source
    out = f"// Junction hall, doors {', '.join(FACE_NAMES[f] for f in door_faces)}.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += hall_meta_and_ports(name, archetype, door_faces)
    out += lights
    return out


def hall_junction_3way():
    return _hall_junction("hall_junction_3way", "hall_junction_3way", [0, 3, 5])


def hall_junction_4way():
    return _hall_junction("hall_junction_4way", "hall_junction_4way", [0, 2, 3, 5])


def hall_straight_buttressed():
    brushes = hall_shell([0, 3])
    brushes += "// Grounded side buttresses keep the long axis open\n"
    for face in (1, 2, 4, 5):
        brushes += band(face, WALL, WALL + 18.0, FLOOR_TOP, 72.0)
    lights = ""
    for x in (-48.0, 48.0):
        fixture, source = ceiling_fixture(x=x, half_x=16.0, half_y=9.0)
        brushes += fixture
        lights += source
    out = "// Straight hall variant: structural side buttresses, clear E/W route.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += tile_meta("authored/hall_straight_buttressed", "hall_straight", 1, 1, 7)
    out += tile_cell()
    out += lateral_port(0, "door", "east_port")
    out += lateral_port(3, "door", "west_port")
    out += lights
    return out


def hall_turn_60_buttressed():
    brushes = hall_shell([0, 5])
    brushes += "// Grounded outer-corner masses frame the bend\n"
    for face in (2, 3):
        brushes += band(face, WALL, WALL + 22.0, FLOOR_TOP, 88.0)
    brushes += pylon(10.0, FLOOR_TOP, LEVEL - FLOOR_TOP, phase_deg=30.0, chamfer_top=3.0)
    fixture, lights = ceiling_fixture(x=26.0, y=28.0, half_x=15.0, half_y=9.0)
    brushes += fixture
    out = "// 60-degree turn variant: supported cove around a full-height pier.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += tile_meta("authored/hall_turn_60_buttressed", "hall_turn_60", 1, 1, 7)
    out += tile_cell()
    out += lateral_port(0, "door", "east_port")
    out += lateral_port(5, "door", "ne_port")
    out += lights
    return out


def room_grounded_hub():
    brushes = "// Fully enclosed decision room with six physical thresholds\n"
    brushes += hex_slab(0.0, FLOOR_TOP, chamfer_top=3.0)
    brushes += hex_slab(LEVEL - FLOOR_TOP, LEVEL, chamfer_bottom=3.0)
    for face in range(6):
        brushes += door_wall(face, 0.0, LEVEL)
    brushes += "// Full-height central service pier: grounded structure, no mezzanine\n"
    brushes += pylon(14.0, FLOOR_TOP, LEVEL - FLOOR_TOP, phase_deg=30.0, chamfer_top=4.0)
    lights = ""
    for x, y in ((-48.0, -28.0), (48.0, -28.0), (0.0, 54.0)):
        fixture, source = ceiling_fixture(x=x, y=y, half_x=13.0, half_y=8.0)
        brushes += fixture
        lights += source
    out = "// Grounded sanctuary hub: six decisions around a supported service pier.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += tile_meta("authored/room_grounded_hub", "sanctuary", 0, 1, 10)
    out += tile_cell()
    for face in range(6):
        out += lateral_port(face, "door", f"{PORT_SHORT[face]}_portal")
    out += lights
    return out


def hall_ramp():
    top = 2.0 * LEVEL
    brushes = "// Ground slab below the supported full-level ramp\n"
    brushes += hex_slab(0.0, FLOOR_TOP, chamfer_top=2.0)
    brushes += "// One solid ramp mass: west sill 0.5 m, east sill 8.5 m\n"
    brushes += sloped_prism(
        list(CORNERS),
        0.0,
        ((-112.0, -64.0, FLOOR_TOP), (-112.0, 64.0, FLOOR_TOP), (112.0, -64.0, LEVEL + FLOOR_TOP)),
    )
    brushes += hex_slab(top - FLOOR_TOP, top, chamfer_bottom=3.0)
    for face in range(6):
        if face == 3:
            brushes += door_wall(face, 0.0, top)
        elif face == 0:
            brushes += door_wall(face, 0.0, top, sill=LEVEL + FLOOR_TOP, top=LEVEL + DOOR_TOP)
        else:
            brushes += wall(face, 0.0, top)
    lights = ""
    for face, along, z in ((2, 0.72, 88.0), (5, 0.28, 184.0)):
        fixture, source = wall_fixture(face, along=along, z=z)
        brushes += fixture
        lights += source
    out = "// Ground-supported two-level ramp: enter west, exit east one level up.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += tile_meta("authored/hall_ramp", "hall_ramp", 0, 2, 10)
    out += tile_cell(levels=2, floor="ramp")
    out += lateral_port(3, "door", "west_entry")
    out += vertical_port("up", "ramp_open", "upper_ramp")
    out += lights
    return out


# --- the silo wellshaft family ---------------------------------------------
#
# A 7-hex composition: solid center core, six ring tiles carrying a
# continuous helical ramp around it (each tile rises LEVEL/6, so one full
# loop climbs exactly one level), and a bridge variant whose outer face
# opens onto a landing. The ring tile is authored once in the E-position
# frame — core at local west, enter low at south_west, exit high at
# north_west, sealed outer arc (east / south_east / north_east) — and the
# composition places rotated+raised copies, so every ramp seam matches by
# construction. Walk surfaces exceed the 1.5 m floor-probe band, hence
# `floor="open"`.

RING_RISE = LEVEL / 6.0  # 21.333 units (1.333 m) per ring tile


def _silo_meta(name, archetype, ports=""):
    out = tile_meta(
        f"authored/{name}", archetype, 0, 1, 1, rotation_policy="none"
    )
    out += tile_cell(floor="open")
    out += ports
    return out


def _ring_ramp():
    """Discrete helicoid: five triangular facets fanning between the core-side
    west edge and the outer rim. Height is CONSTANT along the entry (SW) and
    exit (NW) seam edges — the seam profile is flat, so rotated+raised copies
    meet exactly no matter how the neighbor is oriented. Heights rise by
    RING_RISE thirds around the outer rim and halve along the core edge."""
    lo = FLOOR_TOP
    hi = FLOOR_TOP + RING_RISE
    third = RING_RISE / 3.0
    # TB-unit vertices with helicoid heights (entry seam at lo, exit at hi).
    c2 = (0.0, -128.0, lo)          # entry edge, outer corner
    c3 = (-112.0, -64.0, lo)        # entry edge, core corner
    c1 = (112.0, -64.0, lo + third) # outer rim
    c0 = (112.0, 64.0, lo + 2.0 * third)
    k1 = (-112.0, 0.0, lo + RING_RISE * 0.5)  # core edge midpoint
    c4 = (-112.0, 64.0, hi)         # exit edge, core corner
    c5 = (0.0, 128.0, hi)           # exit edge, outer corner
    triangles = [
        (c2, c1, c3),
        (c3, c1, k1),
        (k1, c1, c0),
        (k1, c0, c4),
        (c4, c0, c5),
    ]
    out = ""
    for tri in triangles:
        plan = [(p[0], p[1]) for p in tri]
        out += sloped_prism(plan, 0.0, tri)
    return out


def silo_core():
    brushes = "// Solid full-height core mass\n"
    brushes += prism(list(CORNERS), 0.0, LEVEL, (0.0, 0.0))
    out = "// Silo core: 100% solid center column of the 7-hex wellshaft.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += _silo_meta("silo_core", "silo_core")
    return out


def silo_ring():
    brushes = "// Helical ramp facet (rises RING_RISE from SW edge to NW edge)\n"
    brushes += _ring_ramp()
    brushes += "// Sealed outer arc: east / south_east / north_east\n"
    for face in (0, 1, 5):
        brushes += wall(face, 0.0, LEVEL)
    fixture, lights = wall_fixture(0, z=88.0)
    brushes += fixture
    out = "// Silo ring segment: one sixth of the helical wellshaft ramp.\n"
    out += "// Core sits beyond the west face; SW/NW faces continue the ramp.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += _silo_meta("silo_ring", "silo_ring")
    out += lights
    return out


def silo_ring_bridge():
    brushes = "// Helical ramp facet\n"
    brushes += _ring_ramp()
    brushes += "// Sealed outer faces flanking the bridge door\n"
    for face in (1, 5):
        brushes += wall(face, 0.0, LEVEL)
    brushes += "// Bridge landing pad in front of the east door\n"
    brushes += prism(
        [(56.0, -36.0), (104.0, -36.0), (104.0, 36.0), (56.0, 36.0)],
        0.0,
        24.0,
        chamfer_top=2.0,
    )
    brushes += "// East door raised to the landing height\n"
    brushes += door_wall(0, 0.0, LEVEL, sill=24.0, top=96.0)
    fixture, lights = wall_fixture(1, along=0.58, z=96.0)
    brushes += fixture
    out = "// Silo ring segment with the per-level bridge landing and door.\n"
    out += "// Generated by tools/tileforge.py - edit that script, not this file.\n"
    out += worldspawn(brushes)
    out += _silo_meta(
        "silo_ring_bridge",
        "silo_ring_bridge",
        ports=lateral_port(0, "door", "bridge_door"),
    )
    out += lights
    return out


TILES = {
    "assets/tiles/authored/silo_core.map": silo_core,
    "assets/tiles/authored/silo_ring.map": silo_ring,
    "assets/tiles/authored/silo_ring_bridge.map": silo_ring_bridge,
    "assets/tiles/authored/hall_straight.map": hall_straight,
    "assets/tiles/authored/hall_straight_buttressed.map": hall_straight_buttressed,
    "assets/tiles/authored/hall_cap.map": hall_cap,
    "assets/tiles/authored/hall_turn_60.map": hall_turn_60,
    "assets/tiles/authored/hall_turn_60_buttressed.map": hall_turn_60_buttressed,
    "assets/tiles/authored/hall_turn_120.map": hall_turn_120,
    "assets/tiles/authored/hall_junction_3way.map": hall_junction_3way,
    "assets/tiles/authored/hall_junction_4way.map": hall_junction_4way,
    "assets/tiles/authored/hall_ramp.map": hall_ramp,
    "assets/tiles/authored/room_grounded_hub.map": room_grounded_hub,
}


def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    for rel_path, builder in TILES.items():
        path = os.path.join(root, rel_path)
        text = builder()
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
        print(f"wrote {rel_path} ({text.count('{') - text.count('classname')} entities+brushes)")
    print("next: cargo run -p observed_authoring --bin tilec -- validate <map>")


if __name__ == "__main__":
    main()
