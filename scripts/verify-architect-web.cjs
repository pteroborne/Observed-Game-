const { chromium } = require("playwright");
const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const evidence = path.resolve("docs/evidence/architect_lab/mobile");
fs.mkdirSync(evidence, { recursive: true });
const base = process.env.ARCHITECT_URL || "http://127.0.0.1:8774";
(async () => {
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
  });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto(base);
  await page.getByRole("button", { name: "Let me try" }).tap();
  await page.clock.install();
  assert.match(await page.locator("#phase").textContent(), /Planning/);
  const gap = page.locator(".tile.void").first();
  await gap.tap();
  assert.match(await page.locator("#target-label").textContent(), /TILE/);
  assert.match(
    await page.locator("#effect").textContent(),
    /Opens more ground/,
  );
  assert.equal(await page.locator("#play").isEnabled(), true);
  await page.screenshot({
    path: path.join(evidence, "phone-preview.png"),
    fullPage: true,
  });
  await page.locator("#play").tap();
  assert.match(await page.locator("#charge-label").textContent(), /5.0s/);
  await page.locator("#pause").tap();
  await page.clock.runFor(20000);
  assert.equal(await page.locator("#caught").textContent(), "2 / 2");
  assert.equal(await page.locator("#result").isVisible(), true);
  await page.screenshot({
    path: path.join(evidence, "phone-victory.png"),
    fullPage: true,
  });
  await page.locator("#again").tap();
  assert.equal(await page.locator("#caught").textContent(), "0 / 2");
  assert.match(await page.locator("#phase").textContent(), /Planning/);
  // Real touch drag across a tile must pan without selecting or playing it.
  const before = await page.locator("#map-content").getAttribute("transform");
  const b = await page.locator("#board").boundingBox();
  const client = await ctx.newCDPSession(page);
  await client.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ x: b.x + 130, y: b.y + 130, id: 1 }],
  });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [{ x: b.x + 175, y: b.y + 155, id: 1 }],
  });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchEnd",
    touchPoints: [],
  });
  assert.notEqual(
    await page.locator("#map-content").getAttribute("transform"),
    before,
  );
  assert.match(
    await page.locator("#target-label").textContent(),
    /PICK A CARD/,
  );
  const pan = await page.locator("#map-content").getAttribute("transform");
  await client.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [
      { x: b.x + 110, y: b.y + 130, id: 1 },
      { x: b.x + 190, y: b.y + 130, id: 2 },
    ],
  });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [
      { x: b.x + 90, y: b.y + 130, id: 1 },
      { x: b.x + 210, y: b.y + 130, id: 2 },
    ],
  });
  await client.send("Input.dispatchTouchEvent", {
    type: "touchEnd",
    touchPoints: [],
  });
  assert.notEqual(
    await page.locator("#map-content").getAttribute("transform"),
    pan,
  );
  assert.match(
    await page.locator("#target-label").textContent(),
    /PICK A CARD/,
  );
  // Select both upper-floor modes and exercise floor buttons.
  for (const mode of ["1", "2"]) {
    await page.locator("#scenario").selectOption(mode);
    await page.locator("#floors button").nth(1).tap();
    assert.equal(
      await page.locator("#floors button").nth(1).getAttribute("aria-pressed"),
      "true",
    );
  }
  assert.deepEqual(errors, []);
  await ctx.close();
  for (const [name, width, height] of [
    ["phone", 390, 844],
    ["desktop", 1440, 960],
    ["small", 320, 740],
    ["landscape", 844, 390],
  ]) {
    const surface = await browser.newContext({
      viewport: { width, height },
      hasTouch: true,
      isMobile: width < 850,
      deviceScaleFactor: 1,
    });
    const screen = await surface.newPage();
    screen.on("pageerror", (e) => errors.push(String(e)));
    await screen.goto(base);
    await screen.getByRole("button", { name: "Let me try" }).tap();
    assert.equal(
      await screen.evaluate(() => document.documentElement.scrollWidth),
      width,
      `${name}: no horizontal overflow`,
    );
    assert.equal(await screen.locator(".card").count(), 5);
    const playBounds = await screen.locator("#play").boundingBox();
    assert.ok(playBounds.height >= 44, `${name}: touch-sized primary control`);
    await screen.screenshot({
      path: path.join(evidence, `${name}.png`),
      fullPage: true,
    });
    await surface.close();
  }
  assert.deepEqual(errors, []);
  console.log(
    "PASS: mobile tap → preview → play → capture → result → restart; real touch pan/pinch without accidental selection; both two-floor modes.",
  );
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
