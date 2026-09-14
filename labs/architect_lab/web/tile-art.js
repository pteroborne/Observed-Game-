// Illustration is decoration. Ports and walls always come from the Rust mask.
const ns = "http://www.w3.org/2000/svg";
let serial = 0;
export const artFiles = Object.fromEntries(
  ["straight", "bend", "junction", "door", "prison"].map((name) => [
    name,
    new URL(`./art/${name}.webp?v=__BUILD__`, import.meta.url).href,
  ]),
);
const add = (parent, tag, attrs) => {
  const node = document.createElementNS(ns, tag);
  for (const [key, value] of Object.entries(attrs))
    node.setAttribute(key, value);
  parent.append(node);
  return node;
};
export const hexPoints = (radius = 31) =>
  Array.from({ length: 6 }, (_, i) => {
    const angle = ((i * 60 + 30) * Math.PI) / 180;
    return `${Math.cos(angle) * radius},${Math.sin(angle) * radius}`;
  }).join(" ");

export function paintTile(
  parent,
  layout,
  { doors, radius = 31, name, rotation } = {},
) {
  const appearance = layout.appearance(doors);
  const art =
    name ?? (appearance.name === "corridor" ? "straight" : appearance.name);
  const turn = rotation ?? appearance.rotation;
  const id = `tile-clip-${serial++}`;
  const group = add(parent, "g", {
    class: "tile-art",
    "data-mask": doors,
    "data-art": art,
    "data-art-rotation": turn,
    "pointer-events": "none",
  });
  const defs = add(group, "defs", {});
  add(add(defs, "clipPath", { id }), "polygon", { points: hexPoints(radius) });
  const clipped = add(group, "g", { "clip-path": `url(#${id})` });
  add(clipped, "polygon", { points: hexPoints(radius), fill: "var(--stone)" });
  // Unrepresented WFC signatures use only the central flagstone material.
  // This prevents a decorative branch from implying an additional passage.
  const fallback = !name && !appearance.exact;
  const extent = fallback ? radius * 3.8 : radius;
  add(clipped, "image", {
    href: artFiles[art],
    x: -extent,
    y: -extent,
    width: extent * 2,
    height: extent * 2,
    transform: `rotate(${layout.angle(turn)})`,
    preserveAspectRatio: "xMidYMid slice",
  });
  const scale = radius / 31;
  const edges = add(clipped, "g", { transform: `scale(${scale})` });
  const apothem = 31 * Math.cos(Math.PI / 6);
  for (const entry of layout.contract.faces) {
    const f = entry.index;
    const open = !!(doors & (1 << f));
    const edge = add(edges, "g", {
      transform: `rotate(${layout.angle(f)})`,
      "data-face": f,
      "data-open": open,
    });
    // Cover the painted edge before cutting its exact aperture.
    add(edge, "path", {
      d: `M${apothem - 2},-16 V16`,
      stroke: "var(--tile-ink)",
      "stroke-width": 7,
      fill: "none",
    });
    add(edge, "path", {
      d: `M${apothem - 2},-15 V15`,
      stroke: "var(--wall-stone)",
      "stroke-width": 3,
      fill: "none",
    });
    if (open) {
      add(edge, "path", {
        d: `M${apothem - 9},-8 H${apothem + 2} V8 H${apothem - 9}`,
        fill: "var(--stone)",
        stroke: "none",
      });
      add(edge, "path", {
        d: `M${apothem - 8},-9 H${apothem + 1} M${apothem - 8},9 H${apothem + 1}`,
        stroke: "var(--tile-ink)",
        "stroke-width": 1.7,
        fill: "none",
      });
    } else {
      add(edge, "path", {
        d: `M${apothem - 4},-7 H${apothem} M${apothem - 4},6 H${apothem}`,
        stroke: "var(--tile-ink)",
        "stroke-width": 1,
        fill: "none",
      });
    }
  }
  return group;
}

export async function warmArtwork() {
  await Promise.all(
    Object.values(artFiles).map((url) => {
      const image = new Image();
      image.src = url;
      return image.decode().catch(() => {}); // Exact walls remain readable offline.
    }),
  );
}
