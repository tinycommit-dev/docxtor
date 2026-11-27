// Draws the app icons (web/public/icons/*.png) from web/public/favicon.svg with the browser Playwright already has.
// Run it again only when the favicon changes: node scripts/make-icons.mjs
import { chromium } from "@playwright/test";
import { mkdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const svg = readFileSync(resolve(root, "web/public/favicon.svg"), "utf8");
const out = resolve(root, "web/public/icons");
mkdirSync(out, { recursive: true });

// A maskable icon keeps its mark inside the middle 80% and fills the whole square, so any mask shape leaves it whole.
const page = (size, maskable) => `<!doctype html><html><body style="margin:0;background:${maskable ? "#201515" : "transparent"}">
<div style="width:${size}px;height:${size}px;display:grid;place-items:center">
<div style="width:${maskable ? size * 0.7 : size}px;height:${maskable ? size * 0.7 : size}px">${svg.replace("<svg ", '<svg width="100%" height="100%" ')}</div></div></body></html>`;

const browser = await chromium.launch();
const tab = await browser.newPage();
for (const [name, size, maskable] of [["icon-192.png", 192, false], ["icon-512.png", 512, false], ["icon-maskable-512.png", 512, true]]) {
  await tab.setViewportSize({ width: size, height: size });
  await tab.setContent(page(size, maskable));
  await tab.screenshot({ path: resolve(out, name), omitBackground: !maskable });
  console.log("wrote", name);
}
await browser.close();
