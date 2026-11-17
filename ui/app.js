// Yukkit Launcher UI. Talks to the Rust side through Tauri commands and events.
const tauri = window.__TAURI__;
const invoke = tauri ? tauri.core.invoke : mockInvoke;
const listen = tauri ? tauri.event.listen : async () => {};

const $ = (id) => document.getElementById(id);
const state = {
  overview: null,
  busy: false,
  running: false,
  lastError: null,
  lastAction: null,
  settings: null,
  defaultInstallDir: "",
  log: [],
};

/* ---------- Navigation ---------- */
function showPage(name) {
  document.querySelectorAll(".nav-item").forEach((b) => b.classList.toggle("active", b.dataset.page === name));
  document.querySelectorAll(".page").forEach((p) => p.classList.toggle("active", p.id === `page-${name}`));
  if (name === "logs") renderLog();
}
document.querySelectorAll(".nav-item").forEach((b) => b.addEventListener("click", () => showPage(b.dataset.page)));
$("player-chip").addEventListener("click", () => showPage("settings"));
document.querySelectorAll("[data-href]").forEach((b) =>
  b.addEventListener("click", () => invoke("open_link", { url: b.dataset.href }).catch(showError))
);

/* ---------- Toast ---------- */
let toastTimer;
function toast(text) {
  const t = $("toast");
  t.textContent = text;
  t.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), 2600);
}

/* ---------- Errors ---------- */
function normalizeError(err) {
  if (err && typeof err === "object" && err.title) return err;
  return { title: "Something went wrong", message: String(err), details: String(err) };
}
function showError(err, retry) {
  const e = normalizeError(err);
  state.lastError = e;
  $("error-card").classList.remove("hidden", "info");
  $("error-title").textContent = e.title;
  $("error-message").textContent = e.message;
  $("error-details").textContent = e.details || "No extra details.";
  $("error-details").classList.add("hidden");
  $("error-toggle").textContent = "Show details";
  $("error-toggle").classList.toggle("hidden", !e.details);
  $("error-copy").classList.toggle("hidden", !e.details);
  $("error-retry").classList.toggle("hidden", !retry);
  $("error-retry").onclick = () => { hideError(); retry && retry(); };
  appendLog(`[error] ${e.title}: ${e.message}${e.details ? "\n        " + e.details : ""}`);
  showPage("home");
}
function hideError() { $("error-card").classList.add("hidden"); }
$("error-dismiss").onclick = hideError;
$("error-toggle").onclick = () => {
  const d = $("error-details");
  d.classList.toggle("hidden");
  $("error-toggle").textContent = d.classList.contains("hidden") ? "Show details" : "Hide details";
};
$("error-copy").onclick = async () => {
  const e = state.lastError;
  if (!e) return;
  await navigator.clipboard.writeText(`${e.title}\n${e.message}\n\n${e.details}`).catch(() => {});
  toast("Details copied");
};

/* ---------- Home ---------- */
function setCheck(id, ok, installed) {
  const li = $(id);
  li.classList.toggle("ok", ok);
  li.classList.toggle("missing", !ok);
  li.querySelector("em").textContent = ok ? "Ready" : installed ? "Needs repair" : "Will be installed";
}

function renderOverview() {
  const o = state.overview;
  if (!o) return;
  const build = o.latest || o.installed;

  $("player-name").textContent = o.username;
  $("player-avatar").textContent = (o.username || "P")[0].toUpperCase();

  if (build) {
    $("hero-title").textContent = `LiquidBounce ${build.lbVersion || ""}`.trim();
    const chips = [
      `<span class="chip accent">Minecraft ${escapeHtml(build.mcVersion)}</span>`,
      `<span class="chip">${build.release ? "Release" : "Nightly"}</span>`,
    ];
    if (build.date) chips.push(`<span class="chip">${escapeHtml(formatDate(build.date))}</span>`);
    if (o.updateAvailable) chips.push(`<span class="chip warn">Update available</span>`);
    if (!o.latest && o.installed) chips.push(`<span class="chip warn">Offline</span>`);
    $("hero-chips").innerHTML = chips.join("");
    $("hero-message").textContent = build.message || "";
  } else {
    $("hero-title").textContent = "LiquidBounce";
    $("hero-chips").innerHTML = "";
    $("hero-message").textContent = "Free and open-source Minecraft client.";
  }

  const installed = !!o.installed;
  setCheck("check-java", o.checks.java, installed);
  setCheck("check-minecraft", o.checks.minecraft, installed);
  setCheck("check-client", o.checks.client, installed);
  const allOk = o.checks.java && o.checks.minecraft && o.checks.client;
  $("setup-note").textContent = allOk
    ? "Everything is installed. Updates are downloaded automatically when you press Play."
    : "Anything missing is downloaded automatically when you press the button above.";

  renderAction();

  if (o.apiError && !o.installed) {
    showError(o.apiError, refresh);
  } else if (o.apiError) {
    const card = $("error-card");
    showError({ ...o.apiError, message: o.apiError.message + " You can still play the version you already have." }, refresh);
    card.classList.add("info");
  }
}

function renderAction() {
  const o = state.overview;
  const btn = $("main-action");
  const label = $("main-action-label");
  const hint = $("action-hint");
  btn.classList.remove("playing");

  if (state.running) {
    btn.disabled = true;
    btn.classList.add("playing");
    label.textContent = "Playing";
    hint.textContent = "LiquidBounce is running. Close the game to play again.";
    return;
  }
  if (state.busy) {
    btn.disabled = true;
    label.textContent = state.lastAction === "install" ? "Installing…" : "Starting…";
    hint.textContent = "You can keep using your computer while this runs.";
    return;
  }
  btn.disabled = !o;
  if (!o) { label.textContent = "Please wait"; hint.textContent = ""; return; }

  const complete = o.installed && o.checks.java && o.checks.minecraft && o.checks.client;
  if (!o.installed) {
    label.textContent = "Install & Play";
    hint.textContent = o.latest ? "First install downloads about 600 MB. Only needed once." : "Connect to the internet to install.";
  } else if (o.updateAvailable) {
    label.textContent = "Update & Play";
    hint.textContent = `A new LiquidBounce build is ready (you have ${o.installed.lbVersion || "an older build"}).`;
  } else if (!complete) {
    label.textContent = "Repair & Play";
    hint.textContent = "Some files are missing. They'll be downloaded again.";
  } else {
    label.textContent = "Play";
    hint.textContent = `Signed in offline as ${o.username}.`;
  }
}

async function refresh() {
  hideError();
  try {
    state.overview = await invoke("get_overview");
    state.running = state.overview.gameRunning;
    renderOverview();
    const build = state.overview.latest;
    if (build) loadChangelog(build.buildId);
    else $("changelog").innerHTML = '<span class="muted">The changelog will appear when you\'re online.</span>';
  } catch (e) {
    showError(e, refresh);
  }
}

async function loadChangelog(id) {
  try {
    const text = await invoke("get_changelog", { buildId: id });
    $("changelog").textContent = (text || "").trim() || "No changelog for this build.";
  } catch {
    $("changelog").innerHTML = '<span class="muted">Changelog unavailable right now.</span>';
  }
}

async function runMain() {
  state.lastAction = "play";
  state.busy = true;
  hideError();
  setProgress({ step: 1, steps: 6, title: "Getting ready", detail: "", fraction: null });
  renderAction();
  try {
    await invoke("play");
    toast("Starting Minecraft — the game window will open shortly");
    $("progress").classList.add("hidden");
  } catch (e) {
    $("progress").classList.add("hidden");
    showError(e, runMain);
  } finally {
    state.busy = false;
    renderAction();
    refresh();
  }
}
$("main-action").addEventListener("click", runMain);

function setProgress(p) {
  $("progress").classList.remove("hidden");
  $("progress-title").textContent = p.title;
  $("progress-step").textContent = `Step ${p.step} of ${p.steps}`;
  $("progress-detail").textContent = p.detail || "";
  const fill = $("progress-fill");
  if (p.fraction === null || p.fraction === undefined) {
    fill.classList.add("indeterminate");
  } else {
    fill.classList.remove("indeterminate");
    const overall = ((p.step - 1) + Math.min(1, Math.max(0, p.fraction))) / p.steps;
    fill.style.width = `${Math.round(overall * 100)}%`;
  }
}

/* ---------- Settings ---------- */
let javaMode = "auto";
function setJavaMode(mode) {
  javaMode = mode;
  document.querySelectorAll("[data-java]").forEach((b) => b.classList.toggle("active", b.dataset.java === mode));
  $("s-java-row").classList.toggle("hidden", mode !== "custom");
}
document.querySelectorAll("[data-java]").forEach((b) => b.addEventListener("click", () => setJavaMode(b.dataset.java)));

function memLabel() {
  const mb = Number($("s-memory").value);
  $("s-memory-label").textContent = `${(mb / 1024).toFixed(mb % 1024 ? 1 : 0)} GB`;
}
$("s-memory").addEventListener("input", memLabel);

async function loadSettings() {
  const view = await invoke("get_settings");
  const s = view.settings;
  state.settings = s;
  state.defaultInstallDir = view.defaultInstallDir;
  $("s-username").value = s.username;
  $("s-install").value = view.installDir;
  $("s-memory").value = s.memoryMb;
  memLabel();
  $("s-java").value = s.javaPath;
  setJavaMode(s.javaPath ? "custom" : "auto");
  $("s-jvm").value = s.jvmArgs;
  $("s-nightly").checked = s.nightly;
  $("s-keep").checked = s.keepOpen;
  $("about-line").textContent = `Yukkit Launcher ${view.version} · A community launcher for LiquidBounce. LiquidBounce is made by CCBlueX.`;
}

$("s-install-browse").onclick = async () => {
  const dir = await invoke("pick_folder").catch(showError);
  if (dir) $("s-install").value = dir;
};
$("s-install-reset").onclick = () => { $("s-install").value = state.defaultInstallDir; };
$("s-install-open").onclick = () => invoke("open_install_folder").catch(showError);
$("s-java-browse").onclick = async () => {
  const file = await invoke("pick_java").catch(showError);
  if (file) $("s-java").value = file;
};

$("s-save").onclick = async () => {
  $("s-error").textContent = "";
  const install = $("s-install").value.trim();
  const settings = {
    ...state.settings,
    username: $("s-username").value.trim(),
    installDir: install === state.defaultInstallDir ? "" : install,
    memoryMb: Number($("s-memory").value),
    javaPath: javaMode === "custom" ? $("s-java").value.trim() : "",
    jvmArgs: $("s-jvm").value.trim(),
    nightly: $("s-nightly").checked,
    keepOpen: $("s-keep").checked,
  };
  try {
    await invoke("save_settings", { settings });
    state.settings = settings;
    $("s-saved").classList.remove("hidden");
    setTimeout(() => $("s-saved").classList.add("hidden"), 2000);
    refresh();
  } catch (e) {
    const err = normalizeError(e);
    $("s-error").textContent = `${err.title}. ${err.message}`;
  }
};

/* ---------- Logs ---------- */
function appendLog(line) {
  state.log.push(line);
  if (state.log.length > 4000) state.log.splice(0, state.log.length - 4000);
  if ($("page-logs").classList.contains("active")) {
    const el = $("log");
    const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 30;
    el.textContent += line + "\n";
    if (atBottom) el.scrollTop = el.scrollHeight;
  }
}
function renderLog() {
  const el = $("log");
  el.textContent = state.log.join("\n") + (state.log.length ? "\n" : "");
  el.scrollTop = el.scrollHeight;
}
$("log-copy").onclick = async () => {
  await navigator.clipboard.writeText(state.log.join("\n")).catch(() => {});
  toast("Log copied");
};
$("log-clear").onclick = () => { state.log = []; renderLog(); };

/* ---------- Events from Rust ---------- */
listen("progress", (e) => setProgress(e.payload));
listen("launcher-log", (e) => appendLog(`[launcher] ${e.payload}`));
listen("game-log", (e) => appendLog(e.payload));
listen("game-state", (e) => {
  state.running = e.payload.running;
  renderAction();
  const code = e.payload.code;
  if (!e.payload.running && code !== null && code !== undefined && code !== 0) {
    showError({
      title: "Minecraft closed unexpectedly",
      message: `The game stopped with an error (code ${code}). Try lowering or raising the memory in Settings, or check the Logs page for what happened right before it closed.`,
      details: state.log.slice(-30).join("\n"),
    }, runMain);
  }
});

/* ---------- Helpers ---------- */
function escapeHtml(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
function formatDate(iso) {
  const d = new Date(iso);
  return isNaN(d) ? iso : d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

/* Browser preview without Tauri (for designing the UI). */
async function mockInvoke(cmd) {
  const build = { buildId: 1, lbVersion: "0.30.0", mcVersion: "1.21.4", release: true, date: "2026-11-01", message: "Preview mode — open the app with Tauri to see real data.", commit: "abc1234" };
  switch (cmd) {
    case "get_overview": return { latest: build, apiError: null, installed: null, checks: { java: false, minecraft: false, client: false }, updateAvailable: false, gameRunning: false, username: "Player" };
    case "get_changelog": return "• New modules\n• Bug fixes\n• Performance improvements";
    case "get_settings": return { settings: { installDir: "", username: "Player", memoryMb: 4096, javaPath: "", jvmArgs: "", nightly: false, keepOpen: true, sessionToken: "" }, installDir: "C:\\Users\\you\\AppData\\Roaming\\YukkitLauncher", defaultInstallDir: "C:\\Users\\you\\AppData\\Roaming\\YukkitLauncher", version: "preview" };
    default: return null;
  }
}

loadSettings().catch(showError);
refresh();
