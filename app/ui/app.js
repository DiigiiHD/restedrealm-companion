// RestedRealm Companion window. All work happens in Rust; this file only shows
// state and sends the player's choices.
"use strict";

const tauri = window.__TAURI__;
const invoke = (command, args) => tauri.core.invoke(command, args);
const $ = (id) => document.getElementById(id);

const STEPS = ["game", "addon", "account", "uploads", "done"];
let state = null;
let view = null;
let step = null;

// ---------- helpers ----------

const ago = new Intl.RelativeTimeFormat("en", { numeric: "auto" });
function relative(seconds) {
  const diff = seconds - Date.now() / 1000;
  const units = [["year", 31536000], ["month", 2592000], ["week", 604800], ["day", 86400], ["hour", 3600], ["minute", 60]];
  for (const [unit, size] of units) {
    if (Math.abs(diff) >= size) return ago.format(Math.round(diff / size), unit);
  }
  return "just now";
}
const count = (n, one, many) => `${n.toLocaleString("en")} ${n === 1 ? one : many}`;
const message = (error) => String(error && error.message ? error.message : error);

function setSwitch(button, on) {
  button.setAttribute("aria-checked", on ? "true" : "false");
}
const isOn = (button) => button.getAttribute("aria-checked") === "true";

async function busy(button, work) {
  button.disabled = true;
  try {
    return await work();
  } finally {
    button.disabled = false;
  }
}

async function refresh() {
  state = await invoke("get_state");
  render();
}

// ---------- navigation ----------

function show(next) {
  view = next;
  for (const id of ["setup", "home", "data", "settings"]) $(id).hidden = id !== next;
  document.querySelector(".tabs").hidden = next === "setup";
  for (const tab of document.querySelectorAll(".tabs button")) {
    if (tab.dataset.view === next) tab.setAttribute("aria-current", "page");
    else tab.removeAttribute("aria-current");
  }
  if (next === "data") loadRecords();
  render();
}

function firstStep() {
  if (!state.gameDir) return "game";
  if (!state.addonVersion) return "addon";
  if (!state.connected) return "account";
  return "uploads";
}

function goStep(next) {
  step = next;
  const index = STEPS.indexOf(next);
  for (const li of document.querySelectorAll(".steps li")) {
    const i = STEPS.indexOf(li.dataset.step);
    li.classList.toggle("done", i < index);
    li.classList.toggle("current", i === index);
  }
  for (const panel of document.querySelectorAll(".step")) panel.hidden = panel.dataset.step !== next;
  render();
  const heading = document.querySelector(`.step[data-step="${next}"] h1`);
  if (heading) heading.focus?.();
}

// ---------- rendering ----------

function render() {
  if (!state) return;
  if (view === "setup") renderSetup();
  if (view === "home") renderHome();
  if (view === "settings") renderSettings();
}

function renderSetup() {
  // Game
  const found = $("game-found");
  if (state.gameDir) {
    found.className = "found ok";
    found.textContent = `Found: ${state.gameDir}`;
  } else {
    found.className = "found missing";
    found.textContent = "World of Warcraft: Forever was not found in the usual places. Choose its folder.";
  }
  $("game-next").disabled = !state.gameDir;

  // Addon
  const addon = $("addon-found");
  const current = state.addonVersion && state.addonVersion === state.bundledAddonVersion;
  if (current) {
    addon.className = "found ok";
    addon.textContent = `The addon is installed (version ${state.addonVersion}).`;
  } else if (state.addonVersion) {
    addon.className = "found missing";
    addon.textContent = `An older addon is installed (version ${state.addonVersion}). Update it to version ${state.bundledAddonVersion}.`;
  } else {
    addon.className = "found missing";
    addon.textContent = "The addon is not installed yet.";
  }
  $("addon-install").textContent = state.addonVersion ? "Update the addon" : "Install the addon";
  $("addon-install").hidden = !!current;
  $("addon-next").hidden = !current;
  $("addon-running").hidden = !state.wowRunning;

  // Account
  $("account-form").hidden = state.connected;
  $("account-done").hidden = !state.connected;
}

function renderHome() {
  const dot = $("state-dot");
  const title = $("state-title");
  const detail = $("state-detail");
  const last = state.lastUploadAt
    ? `Last upload ${relative(state.lastUploadAt)}${state.lastUploadCount ? `, ${count(state.lastUploadCount, "record", "records")}` : ""}.`
    : "Nothing uploaded yet.";
  if (state.working) {
    dot.className = "dot working";
    title.textContent = "Checking your saves";
    detail.textContent = last;
  } else if (!state.connected) {
    dot.className = "dot problem";
    title.textContent = "Not connected";
    detail.textContent = "Connect this PC to your RestedRealm account in Settings.";
  } else if (state.lastError) {
    dot.className = "dot problem";
    title.textContent = "Needs attention";
    detail.textContent = state.pending > 0 ? `${count(state.pending, "record", "records")} waiting. ${last}` : last;
  } else if (state.pending > 0) {
    dot.className = "dot waiting";
    title.textContent = `${count(state.pending, "record", "records")} waiting`;
    detail.textContent = state.autoUpload ? `Uploading shortly. ${last}` : `Automatic upload is off. ${last}`;
  } else {
    dot.className = "dot ok";
    title.textContent = "Up to date";
    detail.textContent = last;
  }
  $("home-error").hidden = !state.lastError;
  $("home-error").textContent = state.lastError || "";
  $("sync").disabled = state.working || !state.connected;

  const week = $("week");
  week.replaceChildren(...state.thisWeek.map((group) => {
    const tile = document.createElement("div");
    tile.className = "tile";
    const number = document.createElement("strong");
    number.textContent = group.count.toLocaleString("en");
    const label = document.createElement("span");
    label.textContent = group.label;
    tile.append(number, label);
    return tile;
  }));
  $("week-empty").hidden = state.thisWeek.length > 0;
  $("totals").textContent = `${count(state.observations, "record", "records")} kept on this PC.` +
    (state.wowRunning ? " World of Warcraft is open; new records arrive after you log out or type /reload." : "");
}

function renderSettings() {
  setSwitch($("set-auto"), state.autoUpload);
  setSwitch($("set-start"), state.autostart);
  $("set-game").textContent = state.gameDir || "Not chosen";
  $("set-addon").textContent = state.addonVersion
    ? `Version ${state.addonVersion} installed${state.bundledAddonVersion && state.bundledAddonVersion !== state.addonVersion ? `; version ${state.bundledAddonVersion} is available` : ""}.`
    : "Not installed.";
  $("set-addon-install").textContent = state.addonVersion ? "Reinstall" : "Install";
  $("set-addon-install").disabled = !state.gameDir;
  $("set-account").textContent = state.connected ? "This PC is connected." : "Not connected.";
  $("set-account-action").textContent = state.connected ? "Disconnect" : "Connect";
  $("version").textContent = state.version;
}

async function loadRecords() {
  const records = await invoke("recent_records", { limit: 100 });
  const list = $("records");
  list.replaceChildren(...records.map((record) => {
    const item = document.createElement("li");
    const details = document.createElement("details");
    const summary = document.createElement("summary");
    const what = document.createElement("span");
    what.className = "what";
    what.textContent = record.label;
    const when = document.createElement("span");
    when.className = "when";
    when.textContent = record.observedAt ? relative(record.observedAt) : "";
    const chip = document.createElement("span");
    chip.className = record.uploaded ? "chip sent" : "chip";
    chip.textContent = record.uploaded ? "Uploaded" : "Waiting";
    summary.append(what, when, chip);
    const pre = document.createElement("pre");
    pre.textContent = record.sent;
    details.append(summary, pre);
    item.append(details);
    return item;
  }));
  $("records-empty").hidden = records.length > 0;
}

// ---------- actions ----------

async function chooseGame(errorBox) {
  errorBox.textContent = "";
  const picked = await tauri.dialog.open({ directory: true, title: "Choose the World of Warcraft folder" });
  if (!picked) return false;
  try {
    await invoke("set_game_dir", { path: picked });
    await refresh();
    return true;
  } catch (error) {
    errorBox.textContent = message(error);
    return false;
  }
}

function wire() {
  // Setup
  $("game-browse").addEventListener("click", () => chooseGame($("game-error")));
  $("game-next").addEventListener("click", () => goStep("addon"));
  $("addon-install").addEventListener("click", (e) => busy(e.currentTarget, async () => {
    $("addon-error").textContent = "";
    try {
      await invoke("install_addon");
      await refresh();
    } catch (error) {
      $("addon-error").textContent = message(error);
    }
  }));
  $("addon-next").addEventListener("click", () => goStep("account"));

  const code = $("code");
  code.addEventListener("input", () => {
    code.value = code.value.replace(/[^A-Za-z0-9_-]/g, "");
    $("code-connect").disabled = code.value.length !== 16;
  });
  const connect = (button) => busy(button, async () => {
    $("account-error").textContent = "";
    try {
      await invoke("pair", { code: code.value });
      code.value = "";
      await refresh();
    } catch (error) {
      $("account-error").textContent = message(error);
    }
  });
  $("code-connect").addEventListener("click", (e) => connect(e.currentTarget));
  code.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !$("code-connect").disabled) connect($("code-connect"));
  });

  for (const button of document.querySelectorAll(".step .switch")) {
    button.addEventListener("click", () => setSwitch(button, !isOn(button)));
  }
  $("uploads-next").addEventListener("click", (e) => busy(e.currentTarget, async () => {
    await invoke("set_auto_upload", { on: isOn($("setup-auto")) });
    try {
      await invoke("set_autostart", { on: isOn($("setup-start")) });
    } catch {
      // Starting with Windows can be switched on later in Settings.
    }
    goStep("done");
  }));
  $("setup-finish").addEventListener("click", async () => {
    await invoke("finish_setup");
    await refresh();
    show("home");
  });
  for (const back of document.querySelectorAll("[data-back]")) {
    back.addEventListener("click", () => goStep(STEPS[Math.max(0, STEPS.indexOf(step) - 1)]));
  }
  for (const next of document.querySelectorAll("[data-next]")) {
    next.addEventListener("click", () => goStep(STEPS[STEPS.indexOf(step) + 1]));
  }

  // Pages on the website
  for (const link of document.querySelectorAll("[data-open]")) {
    link.addEventListener("click", () => invoke("open_page", { page: link.dataset.open }));
  }

  // Tabs
  for (const tab of document.querySelectorAll(".tabs button")) {
    tab.addEventListener("click", () => show(tab.dataset.view));
  }

  // Home
  $("sync").addEventListener("click", () => invoke("sync_now"));

  // Settings
  const settingsError = $("settings-error");
  const guard = async (work) => {
    settingsError.textContent = "";
    try {
      await work();
    } catch (error) {
      settingsError.textContent = message(error);
    }
    await refresh();
  };
  $("set-auto").addEventListener("click", () => guard(() => invoke("set_auto_upload", { on: !state.autoUpload })));
  $("set-start").addEventListener("click", () => guard(() => invoke("set_autostart", { on: !state.autostart })));
  $("set-game-change").addEventListener("click", () => chooseGame(settingsError));
  $("set-addon-install").addEventListener("click", (e) => busy(e.currentTarget, () => guard(() => invoke("install_addon"))));
  $("set-account-action").addEventListener("click", () => {
    if (state.connected) {
      guard(() => invoke("disconnect"));
    } else {
      show("setup");
      goStep("account");
    }
  });
  // Deleting asks twice: the button changes to a confirmation for a few seconds.
  let confirmTimer = null;
  $("set-forget").addEventListener("click", (e) => {
    const button = e.currentTarget;
    if (!confirmTimer) {
      button.textContent = "Click again to delete";
      confirmTimer = setTimeout(() => {
        button.textContent = "Delete local data";
        confirmTimer = null;
      }, 4000);
      return;
    }
    clearTimeout(confirmTimer);
    confirmTimer = null;
    button.textContent = "Delete local data";
    guard(() => invoke("forget_local_data"));
  });
}

// ---------- start ----------

async function start() {
  wire();
  await refresh();
  if (!state.setupDone) {
    if (!state.gameDir) {
      const detected = await invoke("detect_game");
      if (detected) {
        try {
          await invoke("set_game_dir", { path: detected });
        } catch {
          // Leave it for the player to choose.
        }
        await refresh();
      }
    }
    show("setup");
    goStep(firstStep());
  } else {
    show("home");
  }
  await tauri.event.listen("state-changed", () => refresh());
  // Keep relative times fresh while the window is open.
  setInterval(() => render(), 30000);
}

start().catch((error) => {
  document.body.textContent = `RestedRealm Companion could not start its window: ${message(error)}`;
});
