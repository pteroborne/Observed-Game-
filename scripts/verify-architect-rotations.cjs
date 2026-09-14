// Cross-check the shipped browser renderer against the actual WASM simulation.
const { chromium } = require("playwright");
const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const base = process.env.ARCHITECT_URL || "http://127.0.0.1:8774";
const evidence = path.resolve("docs/evidence/architect_lab/illustrated");
fs.mkdirSync(evidence, { recursive: true });
(async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    hasTouch: true,
    isMobile: true,
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("response", (r) => {
    if (r.status() >= 400) errors.push(`${r.status()} ${r.url()}`);
  });
  await page.goto(base);
  await page.getByRole("button", { name: "Let me try" }).tap();
  const report = await page.evaluate(async () => {
    const query = new URL(document.querySelector('script[type="module"]').src)
      .search;
    const { default: init, RogueGame } = await import(
      `./architect_lab.js${query}`
    );
    const { createHexLayout } = await import(`./hex-layout.js${query}`);
    const { paintTile, artFiles } = await import(`./tile-art.js${query}`);
    await init();
    const game = new RogueGame(0);
    const contract = JSON.parse(game.render_contract());
    const layout = createHexLayout(contract);
    const check = (condition, message) => {
      if (!condition) throw new Error(message);
    };
    let ports = 0,
      shapes = 0,
      previews = 0;
    const ns = "http://www.w3.org/2000/svg";
    const testSvg = document.createElementNS(ns, "svg");
    testSvg.setAttribute("width", "0");
    testSvg.setAttribute("height", "0");
    document.body.append(testSvg);
    const origin = [4, 4];
    const start = layout.position(origin);
    for (const face of contract.faces) {
      const neighbor = layout.position(origin.map((n, i) => n + face.delta[i]));
      const vector = neighbor.map((n, i) => n - start[i]);
      const direction = layout.direction(face.index);
      const opposite = layout.direction(face.opposite);
      for (let i = 0; i < 2; i++) {
        check(
          Math.abs(direction[i] - vector[i] / Math.hypot(...vector)) < 1e-10,
          `${face.name}: wrong neighbor`,
        );
        check(
          Math.abs(direction[i] + opposite[i]) < 1e-10,
          `${face.name}: wrong opposite`,
        );
      }
      check(
        Math.abs(direction[1] - Math.sin((face.index * Math.PI) / 3)) < 1e-10,
        `${face.name}: screen Y is mirrored`,
      );
      ports++;
    }
    for (const shape of contract.shapes)
      for (let rotation = 0; rotation < 6; rotation++) {
        const expected =
          ((shape.rotations[0] << rotation) |
            (shape.rotations[0] >> (6 - rotation))) &
          63;
        check(
          layout.mask(shape.name, rotation) === expected,
          `${shape.name}/${rotation}: mask`,
        );
        const group = paintTile(testSvg, layout, { doors: expected, rotation });
        for (const face of contract.faces) {
          const edge = group.querySelector(`[data-face="${face.index}"]`);
          check(
            edge.dataset.open === String(!!(expected & (1 << face.index))),
            `${shape.name}/${rotation}: aperture`,
          );
          // Measure the browser's actual SVG transform instead of trusting its label.
          const m = edge.getCTM();
          const direction = layout.direction(face.index);
          check(
            Math.abs(m.a / Math.hypot(m.a, m.b) - direction[0]) < 1e-6,
            "SVG aperture X transform",
          );
          check(
            Math.abs(m.b / Math.hypot(m.a, m.b) - direction[1]) < 1e-6,
            "SVG aperture Y transform",
          );
        }
        const appearance = layout.appearance(expected);
        if (appearance.exact)
          check(
            layout.mask(appearance.name, appearance.rotation) === expected,
            "art classification",
          );
        group.remove();
        shapes++;
      }
    // Every representable WFC horizontal signature has six authoritative edge states.
    for (let mask = 0; mask < 64; mask++) {
      const group = paintTile(testSvg, layout, { doors: mask });
      check(
        group.querySelectorAll('[data-open="true"]').length ===
          mask.toString(2).replaceAll("0", "").length,
        `WFC mask ${mask}`,
      );
      group.remove();
    }
    const initial = JSON.parse(game.snapshot());
    for (let i = 0; i < initial.cards.length; i++) {
      if (initial.cards[i].name === "door") continue;
      for (let rotation = 0; rotation < 6; rotation++) {
        game.reset(0);
        const options = JSON.parse(game.preview(i, 0, 0, 0, rotation));
        const target = options.legal.find((t) =>
          t.rotations.includes(rotation),
        );
        if (!target) continue;
        const preview = JSON.parse(game.preview(i, ...target.cell, rotation));
        check(
          preview.doors === initial.cards[i].rotations[rotation],
          "hand and preview disagree",
        );
        game.play(i, ...target.cell, rotation);
        const actual = JSON.parse(game.snapshot()).cells.find(
          (c) => String(c.cell) === String(target.cell),
        );
        check(
          actual.doors === preview.doors,
          "preview and placed tile disagree",
        );
        previews++;
      }
    }
    const doorFaces = new Set();
    for (const mode of [0, 1, 2])
      for (let rotation = 0; rotation < 6; rotation++) {
        game.reset(mode);
        const state = JSON.parse(game.snapshot());
        const index = state.cards.findIndex((card) => card.name === "door");
        if (index < 0) continue;
        const options = JSON.parse(game.preview(index, 0, 0, 0, rotation));
        const target = options.legal.find((cell) =>
          cell.rotations.includes(rotation),
        );
        if (!target) continue;
        const delta = contract.faces[rotation].delta;
        const neighbor = [
          target.cell[0] + delta[0],
          target.cell[1] + delta[1],
          target.cell[2],
        ];
        game.play(index, ...target.cell, rotation);
        const endpoints = [String(target.cell), String(neighbor)]
          .sort()
          .join("/");
        check(
          JSON.parse(game.snapshot()).doors.some(
            (door) =>
              [String(door.from), String(door.to)].sort().join("/") ===
              endpoints,
          ),
          `door rotation ${rotation}: wrong threshold`,
        );
        doorFaces.add(rotation);
      }
    for (const url of Object.values(artFiles)) {
      const img = new Image();
      img.src = url;
      await img.decode();
      check(
        img.naturalWidth === 512 && img.naturalHeight === 512,
        "missing optimized artwork",
      );
    }
    game.free();
    testSvg.remove();
    return {
      ports,
      shapes,
      signatures: 64,
      previews,
      doorFaces: doorFaces.size,
    };
  });
  assert.equal(report.ports, 6);
  assert.equal(report.doorFaces, 6);
  assert.equal(report.shapes, 30);
  assert.ok(report.previews >= 18, "enough actual legal rotations were played");
  // Six user-visible turns must return both hand and preview to their starting masks.
  await page.locator(".tile.void").first().tap();
  const startMask = await page
    .locator('.card[aria-pressed="true"]')
    .getAttribute("data-mask");
  const startRotation = Number(
    await page
      .locator('.card[aria-pressed="true"]')
      .getAttribute("data-rotation"),
  );
  for (let step = 1; step <= 6; step++) {
    await page.locator("#rotate").tap();
    const card = page.locator('.card[aria-pressed="true"]');
    assert.equal(
      Number(await card.getAttribute("data-rotation")),
      (startRotation + step) % 6,
    );
    if (await page.locator("#play").isEnabled()) {
      const preview = await page
        .locator(".tile.selected")
        .getAttribute("data-preview-mask");
      assert.equal(await card.getAttribute("data-mask"), preview);
    }
  }
  assert.equal(
    await page.locator('.card[aria-pressed="true"]').getAttribute("data-mask"),
    startMask,
  );
  const targetCell = await page
    .locator(".tile.selected")
    .getAttribute("data-cell");
  const previewArt = page.locator(".tile.selected .tile-art");
  const paintedRotation = await previewArt.getAttribute("data-art-rotation");
  const paintedMask = await previewArt.getAttribute("data-mask");
  assert.equal(
    await page
      .locator('.card[aria-pressed="true"] .tile-art')
      .getAttribute("data-art-rotation"),
    paintedRotation,
  );

  await page.screenshot({
    path: path.join(evidence, "phone-preview.png"),
    fullPage: true,
  });
  await page.locator("#play").tap();
  const placedArt = page.locator(`[data-cell="${targetCell}"] .tile-art`);
  assert.equal(await placedArt.getAttribute("data-mask"), paintedMask);
  assert.equal(
    await placedArt.getAttribute("data-art-rotation"),
    paintedRotation,
  );
  await page.locator("#show-tiles").tap();
  for (let rotation = 0; rotation < 6; rotation++) {
    assert.equal(await page.locator("#tile-gallery .tile-art").count(), 5);
    const turns = await page
      .locator("#tile-gallery .tile-art")
      .evaluateAll((nodes) => nodes.map((n) => Number(n.dataset.artRotation)));
    assert.deepEqual(turns, Array(5).fill(rotation));
    if (rotation === 0)
      await page.screenshot({
        path: path.join(evidence, "phone-tiles.png"),
        fullPage: true,
      });
    await page.locator("#gallery-rotate").tap();
  }
  assert.match(await page.locator("#gallery-angle").textContent(), /^0°/);
  await page.setViewportSize({ width: 1440, height: 960 });
  await page.screenshot({
    path: path.join(evidence, "tile-set.png"),
    fullPage: true,
  });
  await page
    .locator("#tiles")
    .screenshot({ path: path.join(evidence, "tile-gallery.png") });
  for (const [width, height] of [
    [320, 740],
    [844, 390],
  ]) {
    await page.setViewportSize({ width, height });
    assert.equal(
      await page.evaluate(() => document.documentElement.scrollWidth),
      width,
    );
    const dialog = page.locator("#tiles");
    assert.ok(
      await dialog.evaluate((n) => n.scrollWidth <= n.clientWidth),
      "gallery has no horizontal overflow",
    );
    await page.getByRole("button", { name: "Back to the hunt" }).tap();
    await page.locator("#show-tiles").tap();
  }
  assert.deepEqual(errors, []);
  console.log(
    "PASS:",
    report,
    "hand/preview/placement agree; all six gallery turns; phone and landscape gallery; five assets decoded.",
  );
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
