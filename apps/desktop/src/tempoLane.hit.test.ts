/**
 * @vitest-environment node
 */
import { existsSync } from "node:fs";
import { createServer } from "vite";
import { chromium, type Browser, type Page } from "playwright-core";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { fileURLToPath } from "node:url";

const chrome =
  process.env.CHROME_PATH ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const hasChrome = existsSync(chrome);

describe.skipIf(!hasChrome)("tempo lane hit target", () => {
  let browser: Browser;
  let page: Page;
  let closeServer: () => Promise<void>;

  beforeAll(async () => {
    const root = fileURLToPath(new URL("..", import.meta.url));
    const server = await createServer({
      configFile: fileURLToPath(new URL("../vite.config.ts", import.meta.url)),
      root,
      server: { port: 0, strictPort: false, host: "127.0.0.1" },
      logLevel: "error",
    });
    await server.listen();
    const addr = server.httpServer?.address();
    const port = typeof addr === "object" && addr ? addr.port : 0;
    closeServer = () => server.close();
    browser = await chromium.launch({ executablePath: chrome, headless: true });
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
    page = await context.newPage();
    await page.goto(`http://127.0.0.1:${port}/src/dev/tempo-hit.html`);
    await page.getByRole("button", { name: "Bar 1.1 · 120.00 BPM" }).waitFor();
  }, 60_000);

  afterAll(async () => {
    await browser?.close();
    await closeServer?.();
  });

  it("hits the bar 1.1 tempo point and a 30 px upward drag adds 3 BPM", async () => {
    const dot = page.getByRole("button", { name: "Bar 1.1 · 120.00 BPM" });
    const box = await dot.boundingBox();
    expect(box).toBeTruthy();
    const x = box!.x + box!.width / 2;
    const y = box!.y + box!.height / 2;
    const hit = await page.evaluate(({ x, y }) => {
      const el = document.elementFromPoint(x, y);
      return el?.getAttribute("aria-label") ?? el?.getAttribute("title") ?? el?.tagName ?? "";
    }, { x, y });
    expect(hit).toBe("Bar 1.1 · 120.00 BPM");

    await page.mouse.click(x, y);
    expect(await page.getByTestId("beat").getAttribute("data-beat")).toBe("2.000");

    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y - 30, { steps: 8 });
    await page.mouse.up();

    await page.getByRole("button", { name: "Bar 1.1 · 123.00 BPM" }).waitFor();
    expect(await page.getByTestId("beat").getAttribute("data-beat")).toBe("2.000");
  }, 30_000);
});
