// Render each screen of the companion window with a stand-in for the Rust
// commands, and fail on script errors or sideways scrolling.
//
//   node tests/ui-screens.mjs <output folder>
//
// Needs Playwright (npm install -g playwright). Set CHROMIUM to a browser path
// if Playwright's own browser is not installed.
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const { chromium } = await import("playwright").catch(() => import("/opt/node22/lib/node_modules/playwright/index.mjs"));
const UI = pathToFileURL(join(dirname(fileURLToPath(import.meta.url)), "..", "app", "ui", "index.html")).href;
const OUT = process.argv[2] || "companion-shots";
mkdirSync(OUT, { recursive: true });
const now = Math.floor(Date.now() / 1000);

const base = {
  version: "0.2.0", setupDone: false, connected: false, autoUpload: false, autostart: false,
  gameDir: null, savesFound: 0, addonVersion: null, bundledAddonVersion: "0.1.13-probe", wowRunning: false,
  working: false, lastError: null, observations: 0, pending: 0, lastUploadAt: null, lastUploadCount: null, thisWeek: [],
};
const home = { ...base, setupDone: true, connected: true, autoUpload: true, autostart: true,
  gameDir: "C:\\Program Files (x86)\\World of Warcraft\\_classic_beta_", savesFound: 1, addonVersion: "0.1.13-probe",
  observations: 331, pending: 0, lastUploadAt: now - 7200, lastUploadCount: 48,
  thisWeek: [{ label: "Quests", count: 142 }, { label: "Creatures and NPCs", count: 96 }, { label: "Merchants and trainers", count: 21 }, { label: "Items and loot", count: 60 }, { label: "Travel", count: 3 }] };
const records = [
  { label: "Quest objectives and rewards", observedAt: now - 3600, uploaded: true, sent: JSON.stringify({ kind: "quest_objectives", seq: 331, data: { questID: 426, rewardChoices: [3447, 3834], xp: 875, money: 450 } }, null, 2) },
  { label: "Merchant stock", observedAt: now - 5400, uploaded: false, sent: "{ }" },
  { label: "Creature or character seen", observedAt: now - 86400 * 2, uploaded: true, sent: "{ }" },
];

function mock(state, recs) {
  return `window.__TAURI__ = {
    core: { invoke: async (cmd, args) => {
      const s = window.__state;
      if (cmd === "get_state") return s;
      if (cmd === "detect_game") return null;
      if (cmd === "recent_records") return ${JSON.stringify(recs)};
      return null;
    } },
    event: { listen: async () => () => {} },
    dialog: { open: async () => null },
  };
  window.__state = ${JSON.stringify(state)};`;
}

const shots = [
  ["01-setup-game-missing", base, null, 900, 660],
  ["02-setup-game-found", { ...base, gameDir: home.gameDir }, null, 900, 660],
  ["03-setup-addon", { ...base, gameDir: home.gameDir, wowRunning: true }, "addon", 900, 660],
  ["04-setup-account", { ...base, gameDir: home.gameDir, addonVersion: "0.1.13-probe" }, "account", 900, 660],
  ["05-setup-uploads", { ...base, gameDir: home.gameDir, addonVersion: "0.1.13-probe", connected: true }, "uploads", 900, 760],
  ["06-setup-done", { ...base, gameDir: home.gameDir, addonVersion: "0.1.13-probe", connected: true }, "done", 900, 660],
  ["07-home", home, "home", 900, 660],
  ["08-home-waiting-error", { ...home, pending: 12, lastError: "Could not reach RestedRealm" }, "home", 900, 660],
  ["09-data", home, "data", 900, 660],
  ["10-settings", home, "settings", 900, 760],
  ["11-home-narrow", home, "home", 720, 540],
  ["12-setup-uploads-narrow", { ...base, gameDir: home.gameDir, addonVersion: "0.1.13-probe", connected: true }, "uploads", 720, 540],
];

const browser = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
const errors = [];
for (const [name, state, target, width, height] of shots) {
  const page = await browser.newPage({ viewport: { width, height } });
  page.on("pageerror", (e) => errors.push(`${name}: ${e.message}`));
  page.on("console", (m) => { if (m.type() === "error") errors.push(`${name}: ${m.text()}`); });
  await page.addInitScript(mock(state, records));
  await page.goto(UI);
  await page.waitForTimeout(250);
  if (target === "addon" || target === "account" || target === "uploads" || target === "done") {
    await page.evaluate((t) => goStep(t), target);
  } else if (target) {
    await page.evaluate((t) => show(t), target);
  }
  if (target === "data") await page.click(".records summary");
  await page.waitForTimeout(150);
  await page.screenshot({ path: `${OUT}/${name}.png`, fullPage: false });
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  if (overflow) errors.push(`${name}: horizontal overflow`);
  await page.close();
}
await browser.close();
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(`${shots.length} screens rendered to ${OUT} without errors or sideways scrolling`);
