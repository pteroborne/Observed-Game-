# Asset plan — what to add next

> **Complete and integrated (June 20, 2026).** Items 1–15 are present and used by
> `observed_game`. Item 16 (the HDR environment) is present but **not rendered** —
> Bevy image-based lighting needs a `.ktx2` cubemap, not an equirectangular `.hdr`;
> see [`SOURCES.md`](SOURCES.md). Exact source files and CC0 provenance are recorded
> there.

A prioritized shopping list of **CC0** assets to flesh out the first-person match
(currently: textured walls/floor, an open ceiling, wireframe-free but sparse rooms,
no characters, no audio). Each row gives the drop-in path, format, a **specific**
recommendation, and a CC0 source. Drop a file at the listed path and I wire it in —
items marked **(slot exists)** already load; **(new)** needs a small code change I'll
make when the file lands.

**Formats Bevy loads here:** textures `PNG`/`JPG` (sRGB albedo); models `glTF`/`GLB`
(prefer `.glb`); sounds `OGG`/`WAV`. Convert FBX/OBJ → glTF in Blender. MP3 is off.

**Already present:** `textures/wall.png`, `textures/floor.png`, `models/prop.glb`.

**Aesthetic to aim for:** an industrial / sci-fi *megastructure* — metal panels,
grime, emergency lighting, modular facility kit. Keep models **low-poly** so a full
maze of them stays cheap.

---

## Priority 1 — biggest payoff (structure, readability, life)

### 1. Ceiling texture — `textures/ceiling.png` · PNG · (new)
Caps the maze (right now there's a black void above the walls). A tileable metal /
panelled ceiling.
- **Recommended:** ambientCG `MetalPlates006` or `Tiles101`; or Poly Haven
  `industrial_ceiling` style. Darker than the floor so it reads as overhead.
- **Source:** https://ambientcg.com/ · https://polyhaven.com/textures

### 2. Ceiling light fixture — `models/light_fixture.glb` · GLB · (new)
A small lamp/strip mesh I place at intervals along corridors/rooms; I anchor a real
point light to each so the facility is lit *by fixtures*, not a flat sky light.
- **Recommended:** Kenney **Sci-Fi RTS** / **Space Kit** ceiling lamp, or Quaternius
  **Sci-Fi** light. A simple emissive panel or caged bulb.
- **Source:** https://kenney.nl/assets · https://quaternius.com/

### 3. Exit gate / portal — `models/exit_gate.glb` · GLB · (new)
The exit room's landmark — a large gate, blast door, or elevator the escapers run
to. Replaces the generic prop as the goal marker.
- **Recommended:** Kenney **Space Kit** hangar door / gate, or Quaternius **Sci-Fi**
  door/elevator. Something tall and obviously a "way out".
- **Source:** https://kenney.nl/assets · https://quaternius.com/

### 4. Exit emissive panel — `textures/exit_panel.png` · PNG · (new)
A bright green glowing panel/sign for the exit room walls/floor ring (paired with an
emissive material so it reads from across the maze). The exit is currently a thin
green gizmo line.
- **Recommended:** a simple high-contrast green "EXIT" / chevron panel — Kenney
  **UI**/sign packs, or any green emissive decal on ambientCG/OpenGameArt.
- **Source:** https://kenney.nl/assets · https://opengameart.org/ (filter CC0)

### 5. Player model — `models/player.glb` · GLB (rigged ideal, static OK) · (new)
Your runner. Used for the local team's teammate(s) and (optionally) a third-person
toggle. Low-poly humanoid or robot.
- **Recommended:** Quaternius **Ultimate Modular Sci-Fi/Characters** or **Robot**
  pack; or Kenney **Mini Characters** / **Blocky Characters**. Pick one silhouette
  for "you".
- **Source:** https://quaternius.com/ · https://kenney.nl/assets

### 6. Bot / rival model — `models/bot.glb` · GLB · (new)
A visually distinct opponent for the three AI teams (recolored per team). Drones or a
different robot read well as "not you".
- **Recommended:** Quaternius **Robots** pack (a different bot than the player), or
  Kenney **Space Kit** drone. Must differ clearly from the player silhouette.
- **Source:** https://quaternius.com/ · https://kenney.nl/assets

---

## Priority 2 — connective tissue & gameplay objects

### 7. Doorway frame — `models/doorway.glb` · GLB · (new)
An arch/frame I place at each room↔corridor opening, so the graph's *connections*
read as real doorways (and visually sell the "passages reroute when unobserved").
- **Recommended:** Kenney **Mini Dungeon** / **Sci-Fi RTS** doorway, or Quaternius
  **Modular Dungeon** arch. Should match the wall height (~3.4 units).
- **Source:** https://kenney.nl/assets · https://quaternius.com/

### 8. Equipment / deployable — `models/equipment.glb` · GLB · (new)
A carryable device for the route/equipment systems — a cable spool, beacon, or relay.
Ties the proven `equipment_lab` / `route_lab` ideas into the visible world.
- **Recommended:** Kenney **Tools** (cable reel / generator), or a beacon from Poly
  Pizza. Hand-prop scale.
- **Source:** https://kenney.nl/assets · https://poly.pizza/ (filter CC0)

### 9. Decor: crate / container — `models/decor_crate.glb` · GLB · (new)
Scatter in rooms to break up empty space and give a sense of scale. Crates/barrels.
- **Recommended:** Kenney **Industrial Kit** or **Survival Kit** crates & barrels.
- **Source:** https://kenney.nl/assets

### 10. Decor: console / pipes — `models/decor_console.glb` · GLB · (new)
Wall terminals, pipes, vents — facility flavor along corridors and room edges.
- **Recommended:** Kenney **Industrial Kit** (pipes, control panels) or Quaternius
  **Sci-Fi Interior** props.
- **Source:** https://kenney.nl/assets · https://quaternius.com/

### 11. Hazard / collapse beacon — `models/hazard.glb` · GLB · (new)
A warning beacon or striped barrier to mark the **collapsing rooms** the director is
purging (currently a thin red gizmo line). A rotating amber light sells the threat.
- **Recommended:** Kenney **Tower Defense** / construction barrier or warning light.
- **Source:** https://kenney.nl/assets

---

## Priority 3 — audio & atmosphere (cheap, big immersion gain)

### 12. Footstep — `sounds/footstep.ogg` · OGG/WAV · (new)
Per-step movement feedback (I trigger it from the controller's stride).
- **Recommended:** Kenney **Footsteps**/**RPG Audio**, or Freesound CC0 "footstep
  metal/concrete".
- **Source:** https://kenney.nl/assets?q=audio · https://freesound.org/ (filter CC0)

### 13. Reroute / decohere — `sounds/reroute.ogg` · OGG/WAV · (new)
A mechanical shift/clunk when corridors atomically reroute off-camera — audio cue for
the core "changes when unobserved" mechanic.
- **Recommended:** Freesound CC0 "mechanism / servo / stone slide"; Kenney **Sci-Fi
  Sounds**.
- **Source:** https://freesound.org/ · https://kenney.nl/assets?q=audio

### 14. Escape success — `sounds/escape.ogg` · OGG/WAV · (new)
A success sting when a team reaches the exit (and a darker variant could mark
absorption).
- **Recommended:** Kenney **UI Audio** / **Interface Sounds** positive sting.
- **Source:** https://kenney.nl/assets?q=audio

### 15. Facility ambience loop — `sounds/ambience.ogg` · OGG/WAV · (new)
A low looping hum/drone under the match for dread and presence.
- **Recommended:** Freesound CC0 "industrial room tone / sci-fi ambience loop".
- **Source:** https://freesound.org/ (filter CC0)

### 16. (Optional, advanced) Skybox / environment — `textures/environment.hdr` · HDRI · (new)
A dark interior HDRI for ambient reflections + a backdrop beyond openings. **Note:**
Bevy wants `.ktx2` (or an `.hdr`) for image-based lighting, which may need extra
features — treat as a stretch goal.
- **Recommended:** Poly Haven CC0 HDRIs (an industrial/interior one), converted to
  `.ktx2`.
- **Source:** https://polyhaven.com/hdris

---

## Suggested order to source

If you grab a handful first, these give the most visible jump: **#1 ceiling**,
**#2 light fixtures**, **#3 exit gate**, **#5 player** + **#6 bot**. That turns the
sparse box into a lit, enclosed facility with characters. Then #7–#11 add detail, and
#12–#15 add the audio layer.

When files land at these paths I'll register them in the drop-in manifest and wire
them into the match (ceiling mesh, fixture-anchored point lights, doorway placement at
graph connections, characters at team/player positions, decor scatter, hazard markers
on collapse rooms, and the sound triggers). Anything absent keeps its current flat /
gizmo fallback — nothing breaks if you only add some.
