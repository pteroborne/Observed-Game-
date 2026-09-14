import init, { RogueGame } from "./architect_lab.js?v=__BUILD__";
import { createHexLayout } from "./hex-layout.js?v=__BUILD__";
import {
  paintTile,
  hexPoints,
  warmArtwork,
  artFiles,
} from "./tile-art.js?v=__BUILD__";
let layout;
const $ = (id) => document.getElementById(id);
const ns = "http://www.w3.org/2000/svg";
let game,
  state,
  forecast,
  selected = 0,
  target = null,
  rotation = 0,
  floor = 0;
let camera = { x: 0, y: 0, scale: 1 },
  baseScale = 1,
  mode = 0,
  seenResult = false;
let knownSignals = new Map(),
  forecastKey = "",
  cardKey = "",
  mapKey = "",
  eventKey = "",
  floorKey = "";
const board = $("board"),
  content = $("map-content");
const actorPositions = new Map();
const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");
const cellKey = (c) => c.join(",");
const same = (a, b) => a && b && cellKey(a) === cellKey(b);
const pos = (cell) => layout.position(cell);
const face = (index) => layout.direction(index);
const points = hexPoints;
function svg(tag, attributes = {}, parent = content) {
  const el = document.createElementNS(ns, tag);
  for (const [k, v] of Object.entries(attributes)) el.setAttribute(k, v);
  parent.append(el);
  return el;
}
function say(text) {
  $("feedback").textContent = text;
}
function refresh(force = false) {
  state = JSON.parse(game.snapshot());
  for (const actor of state.observers)
    knownSignals.set(actor.id, { ...actor, tick: state.tick });
  const key = `${selected}/${target}/${rotation}/${Math.floor(state.tick / 60)}/${state.plays}`;
  if (force || key !== forecastKey) {
    forecast = JSON.parse(
      game.preview(selected, ...(target ?? [0, 0, floor]), rotation),
    );
    forecastKey = key;
  }
  drawCards();
  const nextMapKey = `${key}/${floor}/${state.paused}/${state.caught}`;
  if (force || nextMapKey !== mapKey) {
    drawMap();
    mapKey = nextMapKey;
  }
  drawStatus();
  if (state.outcome === "RogueVictory" && !seenResult) {
    seenResult = true;
    $("result-copy").textContent =
      `Both Observers are in the core. ${state.plays} card${state.plays === 1 ? "" : "s"} played in ${Math.floor(state.tick / 60)} seconds${state.demo ? " by the demo Architect" : ""}.`;
    $("result").showModal();
  }
}
function drawCards() {
  const key = JSON.stringify(state.cards) + selected + "/" + rotation;
  if (key === cardKey) return;
  cardKey = key;
  const focusIndex = document.activeElement?.dataset.card;
  $("hand").replaceChildren();
  state.cards.forEach((card, i) => {
    const button = document.createElement("button");
    button.className = "card";
    button.dataset.card = i;
    button.setAttribute("aria-pressed", String(i === selected));
    button.setAttribute(
      "aria-label",
      `${card.name}, ${card.district ?? "any floor"}. ${card.purpose}`,
    );
    const number = document.createElement("span");
    number.className = "number";
    number.textContent = i + 1;
    const art = svg(
      "svg",
      { viewBox: "-36 -36 72 72", "aria-hidden": "true" },
      button,
    );
    const turn = i === selected ? rotation : 0;
    const doors =
      card.name === "door"
        ? layout.mask("corridor", turn)
        : card.rotations[turn];
    paintTile(art, layout, {
      doors,
      radius: 30,
      ...(card.name === "door" ? { name: "door", rotation: turn } : {}),
    });
    button.dataset.mask = card.rotations[turn];
    button.dataset.rotation = turn;
    const name = document.createElement("span");
    name.className = "name";
    name.textContent = card.name;
    const district = document.createElement("span");
    district.className = "district";
    district.textContent =
      card.district === "Liminal Grid"
        ? "FLOOR 02"
        : card.district
          ? "FLOOR 01"
          : "ANY FLOOR";
    button.append(number, name, district);
    button.onclick = () => {
      selected = i;
      rotation = 0;
      forecastKey = "";
      refresh(true);
      say(
        `${card.name[0].toUpperCase() + card.name.slice(1)} selected. Tap a glowing target.`,
      );
    };
    $("hand").append(button);
  });
  if (focusIndex != null)
    $("hand")
      .querySelector(`[data-card="${focusIndex}"]`)
      ?.focus({ preventScroll: true });
}
function fit() {
  const cells = state.cells.filter(
    (c) => c.cell[2] === floor && (c.solid || c.retracted),
  );
  const p = cells.map((c) => pos(c.cell));
  const minX = Math.min(...p.map((p) => p[0])) - 45,
    maxX = Math.max(...p.map((p) => p[0])) + 45;
  const minY = Math.min(...p.map((p) => p[1])) - 45,
    maxY = Math.max(...p.map((p) => p[1])) + 45;
  const rect = board.getBoundingClientRect();
  baseScale = Math.max(
    0.15,
    Math.min(
      (rect.width - 35) / (maxX - minX),
      (rect.height - (rect.height < 300 ? 85 : 125)) / (maxY - minY),
    ),
  );
  camera = {
    x: rect.width / 2 - ((minX + maxX) / 2) * baseScale,
    y: (rect.height + 25) / 2 - ((minY + maxY) / 2) * baseScale,
    scale: baseScale,
  };
  transform();
}
function transform() {
  content.setAttribute(
    "transform",
    `translate(${camera.x},${camera.y}) scale(${camera.scale})`,
  );
}
function drawMap() {
  const active = document.activeElement?.getAttribute("data-cell");
  content.replaceChildren();
  const legal = new Map(
    (forecast?.legal ?? []).map((p) => [cellKey(p.cell), p.rotations]),
  );
  for (const cell of state.cells.filter((c) => c.cell[2] === floor)) {
    if (!cell.solid && !cell.retracted && !legal.has(cellKey(cell.cell)))
      continue;
    const [x, y] = pos(cell.cell),
      classes = ["tile"];
    if (!cell.solid) classes.push("void");
    if (cell.observed) classes.push("watched");
    if (cell.unstable) classes.push("unstable");
    if (cell.prison) classes.push("prison");
    if (same(cell.cell, target)) classes.push("selected");
    const label = `Tile ${cell.cell[0]}, ${cell.cell[1]}${cell.prison ? ", prison core" : cell.observed ? ", watched" : !cell.solid ? ", empty space" : ""}${legal.has(cellKey(cell.cell)) ? ", legal target" : ""}`;
    const g = svg("g", {
      class: classes.join(" "),
      transform: `translate(${x},${y})`,
      role: "button",
      tabindex: "0",
      "aria-label": label,
      "aria-pressed": String(same(cell.cell, target)),
      "data-cell": cellKey(cell.cell),
    });
    const replacing =
      same(cell.cell, target) &&
      forecast?.ok &&
      state.cards[selected]?.name !== "door";
    if (cell.solid || replacing) {
      paintTile(g, layout, {
        doors: replacing ? forecast.doors : cell.doors,
        ...(cell.prison ? { name: "prison", rotation: 0 } : {}),
      });
      if (replacing) g.dataset.previewMask = forecast.doors;
    }
    g.dataset.mask = cell.doors;
    svg(
      "polygon",
      {
        points: points(),
        class: `hex${cell.solid || replacing ? " illustrated" : ""}`,
      },
      g,
    );
    if (legal.has(cellKey(cell.cell)))
      svg("circle", { cx: 0, cy: 21, r: 3, class: "legal-dot" }, g);
    if (!cell.solid && !replacing)
      svg("text", { x: 0, y: 0, class: "tile-label" }, g).textContent =
        cell.retracted ? "REBUILD" : "BUILD";
    if (cell.observed)
      svg("circle", { cx: 0, cy: 0, r: 22, class: "held-ring" }, g);
    if (cell.unstable) {
      svg("text", { x: 15, y: -10, class: "warning-mark" }, g).textContent =
        "!";
    }
    if (cell.next)
      svg("polygon", { points: points(35), class: "next-ring" }, g);
    if (cell.vertical.length)
      svg("text", { x: 0, y: 17, class: "tile-label" }, g).textContent = "↕";
    g.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        choose(cell.cell);
      }
    });
  }
  const routes = forecast?.ok && target ? [forecast.route ?? []] : state.routes;
  for (const route of routes) {
    const segments = [];
    let segment = [];
    for (const c of route) {
      if (c[2] === floor) segment.push(pos(c));
      else if (segment.length) {
        segments.push(segment);
        segment = [];
      }
    }
    segments.push(segment);
    for (const part of segments)
      if (part.length > 1)
        svg("polyline", {
          points: part.map((p) => p.join(",")).join(" "),
          class: `route ${forecast?.ok && target ? "preview" : ""}`,
        });
  }
  for (const door of state.doors)
    if (door.from[2] === floor) {
      const a = pos(door.from),
        b = pos(door.to),
        dx = b[0] - a[0],
        dy = b[1] - a[1],
        len = Math.hypot(dx, dy);
      if (!door.open) {
        const gate = svg("g", {
          class: "deployed-door",
          "pointer-events": "none",
          transform: `translate(${(a[0] + b[0]) / 2},${(a[1] + b[1]) / 2}) rotate(${(Math.atan2(dy, dx) * 180) / Math.PI})`,
        });
        const crop = svg(
          "svg",
          {
            x: -7,
            y: -16,
            width: 14,
            height: 32,
            viewBox: "-7 -16 14 32",
            overflow: "hidden",
          },
          gate,
        );
        svg(
          "image",
          { href: artFiles.door, x: -24, y: -24, width: 48, height: 48 },
          crop,
        );
      }
      svg("line", {
        x1: (a[0] + b[0]) / 2 - (dy / len) * 12,
        y1: (a[1] + b[1]) / 2 + (dx / len) * 12,
        x2: (a[0] + b[0]) / 2 + (dy / len) * (door.open ? -5 : 12),
        y2: (a[1] + b[1]) / 2 - (dx / len) * (door.open ? -5 : 12),
        stroke: door.open ? "var(--safe)" : "var(--danger)",
        "stroke-width": door.open ? 4 : 1.5,
        opacity: door.open ? 1 : 0.55,
      });
    }
  if (target && target[2] === floor && forecast?.ok) {
    const [x, y] = pos(target),
      card = state.cards[selected];
    if (card?.name !== "door") {
      for (let f = 0; f < 6; f++)
        if (forecast.doors & (1 << f)) {
          const [dx, dy] = face(f);
          svg("line", {
            x1: x + dx * 25,
            y1: y + dy * 25,
            x2: x + dx * 31,
            y2: y + dy * 31,
            class: "preview-arm",
          });
        }
    } else {
      const [dx, dy] = face(rotation);
      svg("line", {
        x1: x + dx * 29 - dy * 10,
        y1: y + dy * 29 + dx * 10,
        x2: x + dx * 29 + dy * 10,
        y2: y + dy * 29 - dx * 10,
        class: "preview-arm",
      });
    }
  }
  for (const signal of knownSignals.values()) {
    if (signal.cell[2] !== floor) continue;
    const live = state.observers.some((a) => a.id === signal.id),
      [x, y] = pos(signal.cell);
    const shared =
      [...knownSignals.values()].filter((other) =>
        same(other.cell, signal.cell),
      ).length > 1;
    const actorX = x + (shared ? signal.id * 17 - 8 : 0);
    const g = svg("g", {
      class: "actor",
      transform: `translate(${actorX},${y})`,
      opacity: live ? 1 : 0.35,
    });
    animateActor(g, `observer-${signal.id}`, actorX, y);
    svg(
      "circle",
      {
        r: 12,
        fill: live ? "var(--watched)" : "none",
        stroke: "var(--watched)",
        "stroke-dasharray": live ? "none" : "3 3",
      },
      g,
    );
    const [dx, dy] = face(signal.facing);
    svg("circle", { cx: dx * 4, cy: dy * 4, r: 5, fill: "var(--screen)" }, g);
    svg("text", { x: 0, y: -17, "text-anchor": "middle" }, g).textContent =
      signal.jailed ? "JAILED" : live ? `O${signal.id + 1}` : "LAST SEEN";
  }
  for (const guardian of state.guardians)
    if (guardian.cell[2] === floor) {
      const [x, y] = pos(guardian.cell);
      const g = svg("g", { class: "actor", transform: `translate(${x},${y})` });
      animateActor(g, `guardian-${guardian.id}`, x, y);
      svg(
        "polygon",
        { points: "0,-16 14,11 -14,11", fill: "var(--guardian)" },
        g,
      );
      svg(
        "path",
        { d: "M0-8V4M0 7V9", stroke: "var(--screen)", "stroke-width": 2 },
        g,
      );
      svg("text", { x: 0, y: -23, "text-anchor": "middle" }, g).textContent =
        guardian.held ? "HELD" : "HUNTER";
    }
  transform();
  if (active)
    content
      .querySelector(`[data-cell="${active}"]`)
      ?.focus({ preventScroll: true });
}
function animateActor(group, key, x, y) {
  const previous = actorPositions.get(key);
  if (
    previous &&
    !reduceMotion.matches &&
    (previous.x !== x || previous.y !== y)
  ) {
    const near = Math.hypot(previous.x - x, previous.y - y) < 100;
    group.animate(
      near
        ? [
            { transform: `translate(${previous.x}px,${previous.y}px)` },
            { transform: `translate(${x}px,${y}px)` },
          ]
        : [{ opacity: 0 }, { opacity: 1 }],
      { duration: 260, easing: "ease-out" },
    );
  }
  actorPositions.set(key, { x, y });
}
function choose(cell) {
  target = cell;
  const legal = forecast?.legal?.find((p) => same(p.cell, cell));
  if (legal && !legal.rotations.includes(rotation))
    rotation = legal.rotations[0];
  refresh(true);
}
function drawStatus() {
  $("caught").textContent = `${state.caught} / ${state.total}`;
  $("elapsed").textContent =
    `${Math.floor(state.tick / 3600)}:${String(Math.floor(state.tick / 60) % 60).padStart(2, "0")}`;
  $("phase").textContent =
    state.outcome === "RogueVictory"
      ? "All Observers captured."
      : state.paused
        ? "Planning · time is held"
        : state.demo
          ? "Demo · Architect is playing"
          : `Hunt live · ${state.detected} detected`;
  $("pause").textContent = state.paused
    ? state.tick
      ? "Resume hunt"
      : "Begin hunt"
    : "Plan";
  $("pause").setAttribute("aria-pressed", String(state.paused));
  $("pause").disabled = state.outcome !== "Running";
  $("demo").textContent = state.demo ? "Take control" : "Watch a demo";
  $("charge").style.width = `${100 - state.cooldown / 3}%`;
  $("charge-label").textContent = state.cooldown
    ? `${(state.cooldown / 60).toFixed(1)}s`
    : "Ready";
  const card = state.cards[selected];
  $("move-title").textContent = card
    ? card.name[0].toUpperCase() + card.name.slice(1)
    : "Choose a card";
  $("target-label").textContent = target
    ? `FLOOR ${target[2] + 1} · TILE ${target[0]}, ${target[1]} · ROTATION ${rotation + 1}`
    : "1. PICK A CARD · 2. TAP A TILE";
  $("effect").textContent = target
    ? (forecast?.reason ?? "Choose a card.")
    : forecast?.legal.length
      ? state.plays
        ? "Where will you change the hunt?"
        : "Reconnect the missing tile."
      : "No targets for this card right now.";
  $("effect").style.color =
    target && !forecast?.ok ? "var(--muted)" : "var(--ink)";
  $("consequence").textContent =
    target && forecast?.ok
      ? forecast.unstable.length
        ? `${forecast.unstable.length} unstable tiles. Exposed tiles retract every 3 seconds.`
        : forecast.repaired
          ? "Repairs the boundary and stops pending collapse."
          : "Connections fit. No new collapse warning."
      : forecast?.legal.length
        ? "Glowing dots mark legal targets. Tap one to preview."
        : "Choose another card, or let the hunt move to release watched tiles.";
  $("play").disabled =
    !target ||
    !forecast?.ok ||
    state.cooldown > 0 ||
    state.demo ||
    state.outcome !== "Running";
  $("play").textContent = state.demo
    ? "Demo in control"
    : state.cooldown
      ? `Recharging · ${(state.cooldown / 60).toFixed(1)}s`
      : target && forecast?.ok
        ? "Play this card"
        : target
          ? "Choose another target"
          : "Choose a tile";
  $("rotate").disabled = !card || state.demo;
  $("collapse").hidden = state.collapse_in === null;
  const next = state.cells.find((c) => c.next);
  $("collapse").textContent = next
    ? `! Tile ${next.cell[0]}, ${next.cell[1]} · floor ${next.cell[2] + 1} retracts in ${(state.collapse_in / 60).toFixed(1)}s`
    : "! Remaining unstable tiles are protected";
  $("event-count").textContent = state.events.length
    ? `· ${state.events.length}`
    : "";
  const nextEventKey = JSON.stringify(state.events);
  if (nextEventKey !== eventKey) {
    eventKey = nextEventKey;
    $("events").replaceChildren(
      ...state.events.map((e) => {
        const li = document.createElement("li");
        li.textContent = `${Math.floor(e.tick / 60)}s — ${e.message}`;
        return li;
      }),
    );
  }
  const nextFloorKey = `${floor}/${state.levels}`;
  if (nextFloorKey !== floorKey) {
    floorKey = nextFloorKey;
    $("floors").replaceChildren(
      ...Array.from({ length: state.levels }, (_, i) => {
        const b = document.createElement("button");
        b.className = "small quiet";
        b.textContent = `${i === 0 ? "01 / Institutional" : "02 / Liminal"}`;
        b.setAttribute("aria-pressed", String(i === floor));
        b.onclick = () => {
          floor = i;
          target = null;
          refresh(true);
          fit();
        };
        return b;
      }),
    );
  }
}
$("play").onclick = () => {
  if (!target) return;
  const name = state.cards[selected]?.name;
  try {
    game.play(selected, ...target, rotation);
    say(
      `${name[0].toUpperCase() + name.slice(1)} played. ${state.paused ? "Press Begin hunt or Resume hunt to watch the consequence." : "Watch the Guardian react."}`,
    );
    target = null;
    rotation = 0;
    refresh(true);
  } catch (error) {
    say(String(error));
    refresh(true);
  }
};
$("rotate").onclick = () => {
  rotation = (rotation + 1) % 6;
  refresh(true);
};
$("pause").onclick = () => {
  game.set_paused(!state.paused);
  refresh(true);
};
$("fit").onclick = fit;
$("help").onclick = () => {
  game.set_paused(true);
  refresh();
  $("guide").showModal();
};
function reset(nextMode = mode) {
  mode = nextMode;
  game.reset(mode);
  selected = 0;
  target = null;
  rotation = 0;
  floor = 0;
  seenResult = false;
  knownSignals.clear();
  actorPositions.clear();
  forecastKey = "";
  cardKey = "";
  refresh(true);
  fit();
  say("Choose a card, then tap a glowing tile. Time is held until you begin.");
}
$("restart").onclick = () => reset();
$("again").onclick = () => {
  $("result").close();
  reset();
};
$("inspect").onclick = () => $("result").close();
$("scenario").onchange = (e) => reset(Number(e.target.value));
$("demo").onclick = () => {
  const demo = !state.demo;
  game.set_demo(demo);
  game.set_paused(!demo);
  refresh(true);
  say(
    demo
      ? "The demo uses the same cards and rules you do."
      : "Your turn. Inspect a card before resuming the hunt.",
  );
};
// Gesture ownership: a drag never becomes a tile tap, and a pinch cancels both taps.
const pointers = new Map();
let gesture = null;
board.addEventListener("pointerdown", (e) => {
  if (e.button !== 0) return;
  board.setPointerCapture(e.pointerId);
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  const tile = e.target.closest("[data-cell]");
  gesture = {
    start: { x: e.clientX, y: e.clientY },
    last: { x: e.clientX, y: e.clientY },
    cell:
      tile?.dataset.cell?.split(",").map(Number) ??
      nearestCell(e.clientX, e.clientY),
    moved: pointers.size > 1,
  };
  if (pointers.size === 2) {
    const [a, b] = [...pointers.values()];
    gesture.distance = Math.hypot(a.x - b.x, a.y - b.y);
  }
});
board.addEventListener("pointermove", (e) => {
  if (!pointers.has(e.pointerId) || !gesture) return;
  const old = pointers.get(e.pointerId);
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  if (pointers.size === 2) {
    const [a, b] = [...pointers.values()],
      distance = Math.hypot(a.x - b.x, a.y - b.y);
    const r = board.getBoundingClientRect();
    zoomAt(
      distance / (gesture.distance || distance),
      (a.x + b.x) / 2 - r.left,
      (a.y + b.y) / 2 - r.top,
    );
    gesture.distance = distance;
    gesture.moved = true;
  } else {
    if (
      Math.hypot(e.clientX - gesture.start.x, e.clientY - gesture.start.y) > 7
    )
      gesture.moved = true;
    if (gesture.moved) {
      camera.x += e.clientX - old.x;
      camera.y += e.clientY - old.y;
      transform();
    }
  }
});
function nearestCell(clientX, clientY) {
  const rect = board.getBoundingClientRect();
  return state.cells
    .filter(
      (c) =>
        c.cell[2] === floor &&
        (c.solid || forecast.legal.some((p) => same(p.cell, c.cell))),
    )
    .map((c) => {
      const [x, y] = pos(c.cell);
      return {
        cell: c.cell,
        d: Math.hypot(
          x * camera.scale + camera.x - (clientX - rect.left),
          y * camera.scale + camera.y - (clientY - rect.top),
        ),
      };
    })
    .filter((c) => c.d < 26)
    .sort((a, b) => a.d - b.d)[0]?.cell;
}
function finishPointer(e, cancelled = false) {
  if (!pointers.has(e.pointerId)) return;
  pointers.delete(e.pointerId);
  if (!cancelled && gesture && !gesture.moved && gesture.cell)
    choose(gesture.cell);
  if (!pointers.size) gesture = null;
  else if (gesture) gesture.moved = true;
}
board.addEventListener("pointerup", (e) => finishPointer(e));
board.addEventListener("pointercancel", (e) => finishPointer(e, true));
function zoomAt(factor, x, y) {
  const next = Math.max(
    baseScale * 0.6,
    Math.min(baseScale * 3, camera.scale * factor),
  );
  const ratio = next / camera.scale;
  camera.x = x - (x - camera.x) * ratio;
  camera.y = y - (y - camera.y) * ratio;
  camera.scale = next;
  transform();
}
board.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    const r = board.getBoundingClientRect();
    zoomAt(Math.exp(-e.deltaY * 0.001), e.clientX - r.left, e.clientY - r.top);
  },
  { passive: false },
);
window.addEventListener("keydown", (e) => {
  if (
    document.querySelector("dialog[open]") ||
    ["INPUT", "SELECT", "BUTTON"].includes(e.target.tagName)
  )
    return;
  if (e.key >= "1" && e.key <= "5") {
    selected = Number(e.key) - 1;
    refresh(true);
  }
  if (e.key.toLowerCase() === "r") $("rotate").click();
  if (e.key === " ") {
    e.preventDefault();
    $("pause").click();
  }
});
document.addEventListener("visibilitychange", () => {
  if (document.hidden && game) {
    game.set_paused(true);
    refresh();
    say("Hunt paused while you were away. Resume when you are ready.");
  }
});
new ResizeObserver(() => {
  if (state) fit();
}).observe(board);
let previous = 0,
  accumulator = 0,
  renderTime = 0;
function frame(now) {
  const elapsed = Math.min((now - previous) / 1000, 0.1);
  previous = now;
  if (game && !document.hidden) {
    accumulator += elapsed * 60;
    const ticks = Math.floor(accumulator);
    accumulator -= ticks;
    game.advance(ticks);
    if (!state.paused && now - renderTime > 100) {
      refresh();
      renderTime = now;
    }
  }
  requestAnimationFrame(frame);
}
let galleryRotation = 0;
function drawGallery() {
  $("tile-gallery").replaceChildren();
  $("gallery-angle").textContent = `${galleryRotation * 60}° clockwise`;
  for (const [name, shape] of [
    ["straight", "corridor"],
    ["bend", "bend"],
    ["junction", "junction"],
    ["door", "corridor"],
    ["prison", "hall"],
  ]) {
    const figure = document.createElement("figure");
    const art = svg(
      "svg",
      {
        viewBox: "-38 -38 76 76",
        role: "img",
        "aria-label": `${name}, ${galleryRotation * 60} degrees`,
      },
      figure,
    );
    const doors = layout.mask(shape, galleryRotation);
    paintTile(art, layout, {
      doors,
      radius: 32,
      name,
      rotation: galleryRotation,
    });
    const caption = document.createElement("figcaption");
    const title = document.createElement("b");
    title.textContent = name;
    const ports = document.createElement("small");
    ports.textContent = layout.contract.faces
      .filter((f) => doors & (1 << f.index))
      .map((f) => ["E", "SE", "SW", "W", "NW", "NE"][f.index])
      .join(" · ");
    caption.append(title, ports);
    figure.append(caption);
    $("tile-gallery").append(figure);
  }
}
$("show-tiles").onclick = () => {
  game.set_paused(true);
  refresh(true);
  drawGallery();
  $("tiles").showModal();
};
$("gallery-rotate").onclick = () => {
  galleryRotation = (galleryRotation + 1) % 6;
  drawGallery();
};
try {
  await init({
    module_or_path: new URL(
      "./architect_lab_bg.wasm?v=__BUILD__",
      import.meta.url,
    ),
  });
  game = new RogueGame(0);
  layout = createHexLayout(JSON.parse(game.render_contract()));
  await warmArtwork();
  for (const [key, value] of Object.entries(JSON.parse(game.theme())))
    document.documentElement.style.setProperty(`--${key}`, value);
  $("loading").hidden = true;
  $("app").hidden = false;
  refresh(true);
  fit();
  $("guide").showModal();
  previous = performance.now();
  requestAnimationFrame(frame);
} catch (error) {
  console.error(error);
  $("loading").replaceChildren();
  const title = document.createElement("h1");
  title.textContent = "The hunt could not load";
  const detail = document.createElement("p");
  detail.textContent =
    "Check your connection and try again. Your browser needs WebAssembly support.";
  const retry = document.createElement("button");
  retry.className = "primary";
  retry.textContent = "Try again";
  retry.onclick = () => location.reload();
  $("loading").append(title, detail, retry);
}
