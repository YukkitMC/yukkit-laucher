// Yukkit Launcher UI. Talks to the Rust side through Tauri commands and events.
const tauri = window.__TAURI__;
const invoke = tauri ? tauri.core.invoke : mockInvoke;
const listen = tauri ? tauri.event.listen : async () => {};

const $ = (id) => document.getElementById(id);
const state = {
  overview: null,
  notes: null,
  busy: false,
  running: false,
  settings: null,
  defaultInstallDir: "",
  log: [],
  lastError: null,
};

/* ---------- Navigation ---------- */
const PAGES = ["play", "versions", "customize", "notes", "settings", "logs"];
function showPage(name) {
  document.querySelectorAll(".nav-item").forEach((b) => {
    const on = b.dataset.page === name;
    b.classList.toggle("active", on);
    if (on) b.setAttribute("aria-current", "page"); else b.removeAttribute("aria-current");
  });
  document.querySelectorAll(".page").forEach((p) => p.classList.toggle("active", p.id === `page-${name}`));
  if (name === "logs") renderLog();
  if (name === "notes") loadNotes();
  if (name === "customize") loadCustomize();
  $("notice").classList.toggle("on-other-page", name !== "play");
}
document.querySelectorAll(".nav-item").forEach((b) => b.addEventListener("click", () => showPage(b.dataset.page)));
document.querySelectorAll("[data-goto]").forEach((b) => b.addEventListener("click", () => showPage(b.dataset.goto)));
$("player-chip").onclick = () => { showPage("settings"); $("s-username").focus(); };
$("change-name").onclick = $("player-chip").onclick;
document.querySelectorAll("[data-href]").forEach((b) => (b.onclick = () => openLink(b.dataset.href)));
function openLink(url) { invoke("open_link", { url }).catch(showError); }

/* ---------- Keyboard shortcuts ----------
 * Ctrl+1…5 sections, Ctrl+, Settings, F5 re-checks status (instead of reloading the page). */
document.addEventListener("keydown", (e) => {
  if (e.ctrlKey && !e.altKey && !e.shiftKey) {
    const n = Number(e.key);
    if (n >= 1 && n <= PAGES.length) { e.preventDefault(); showPage(PAGES[n - 1]); return; }
    if (e.key === ",") { e.preventDefault(); showPage("settings"); return; }
    if (e.key.toLowerCase() === "f" && $("page-notes").classList.contains("active")) { e.preventDefault(); $("notes-search").focus(); $("notes-search").select(); return; }
  }
  if (e.key === "F5") { e.preventDefault(); refresh(); }
});

/* ---------- Window material and theme ----------
 * The window uses Mica on Windows 11 (tauri.conf.json). Windows 10 has no Mica, so the page keeps
 * its opaque background there. Windows 11 reports platformVersion 13 or higher.
 * Mica follows the Windows appearance, so it is only shown with the Automatic theme. */
let micaAvailable = false;
let currentTheme = document.documentElement.dataset.theme || "system";
(async () => {
  try {
    if (!tauri || !navigator.userAgentData) return;
    const { platform, platformVersion } = await navigator.userAgentData.getHighEntropyValues(["platformVersion"]);
    micaAvailable = platform === "Windows" && parseInt(platformVersion, 10) >= 13;
    applyTheme(currentTheme);
  } catch {}
})();

function applyTheme(theme) {
  currentTheme = theme;
  const root = document.documentElement;
  if (theme === "system") delete root.dataset.theme; else root.dataset.theme = theme;
  root.classList.toggle("mica", micaAvailable && theme === "system");
  try { localStorage.setItem("yukkit-theme", theme); } catch {}
  // Title bar and window frame follow the chosen theme too.
  try { if (tauri) tauri.window.getCurrentWindow().setTheme(theme === "system" ? null : "dark").catch(() => {}); } catch {}
}

/* ---------- Confirmation sheet ----------
 * Replaces window.confirm(): named destructive action, Cancel is the default and Esc cancels. */
function confirmSheet({ title, text, action }) {
  const d = $("confirm-dialog");
  $("confirm-title").textContent = title;
  $("confirm-text").textContent = text;
  $("confirm-ok").textContent = action;
  return new Promise((resolve) => {
    const done = (value) => { d.close(); resolve(value); };
    $("confirm-ok").onclick = () => done(true);
    $("confirm-cancel").onclick = () => done(false);
    d.oncancel = (e) => { e.preventDefault(); done(false); };
    d.showModal();
    $("confirm-cancel").focus();
  });
}

/* ---------- Toast ---------- */
let toastTimer;
function toast(text) {
  $("toast-text").textContent = text;
  $("toast").classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $("toast").classList.add("hidden"), 2400);
}

/* ---------- Notices (plain-English errors) ---------- */
function normalizeError(err) {
  if (err && typeof err === "object" && err.title) return err;
  return { title: t("Something went wrong"), message: String(err), details: String(err) };
}
function showError(err, retry, { info = false } = {}) {
  const e = normalizeError(err);
  state.lastError = e;
  const n = $("notice");
  n.classList.remove("hidden");
  n.classList.toggle("info", info);
  n.setAttribute("role", info ? "status" : "alert");
  n.querySelector(".icon use").setAttribute("href", info ? "#i-info" : "#i-alert");
  $("notice-title").textContent = e.title;
  $("notice-text").textContent = e.message;
  $("notice-details").textContent = e.details || "";
  $("notice-details").classList.add("hidden");
  $("notice-toggle").setAttribute("aria-expanded", "false");
  $("notice-toggle").classList.toggle("hidden", !e.details);
  $("notice-copy").classList.toggle("hidden", !e.details);
  $("notice-retry").classList.toggle("hidden", !retry);
  $("notice-retry").onclick = () => { hideError(); retry && retry(); };
  if (!info) appendLog(`[problem] ${e.title}: ${e.message}${e.details ? "\n          " + e.details : ""}`);
}
function hideError() { $("notice").classList.add("hidden"); }
$("notice-close").onclick = hideError;
$("notice-toggle").onclick = () => {
  const open = $("notice-details").classList.toggle("hidden") === false;
  $("notice-toggle").setAttribute("aria-expanded", String(open));
};
$("notice-copy").onclick = async () => {
  const e = state.lastError;
  if (!e) return;
  await navigator.clipboard.writeText(`${e.title}\n${e.message}\n\n${e.details}`).catch(() => {});
  toast(t("Copied"));
};

/* ---------- Play page ---------- */
function selectedVersion() {
  const o = state.overview;
  return o && o.versions.find((v) => v.id === o.selected);
}

function setRow(id, ready, text) {
  const row = $(id);
  row.querySelector(".row-text").textContent = text;
  const icon = row.querySelector(".row-icon");
  icon.classList.toggle("ok", ready);
  icon.classList.toggle("caution", !ready);
  icon.querySelector("use").setAttribute("href", ready ? "#i-check-circle" : "#i-down-circle");
  const s = row.querySelector(".row-state");
  s.className = "status row-state " + (ready ? "ok" : "caution");
  s.textContent = ready ? t("Ready") : t("Will be installed");
}
const statusLabel = (kind, text) =>
  `<span class="status ${kind}"><svg class="icon" aria-hidden="true"><use href="#i-${kind === "ok" ? "check-circle" : "down-circle"}"/></svg>${text}</span>`;

function renderPlay() {
  const o = state.overview;
  if (!o) return;
  const v = selectedVersion();
  $("player-name").textContent = o.username;
  $("playing-as-name").textContent = o.username;
  $("player-avatar").textContent = (o.username || "P")[0].toUpperCase();

  // Version picker
  const sel = $("version-select");
  sel.innerHTML = o.versions
    .map((x) => `<option value="${esc(x.id)}">${esc(x.minecraft)}${x.channel ? " · " + esc(x.channel) : ""}</option>`)
    .join("");
  sel.value = o.selected;
  sel.disabled = state.busy;

  if (!v) {
    $("play-eyebrow").textContent = t("No versions available");
    $("play-lede").textContent = t("The launcher couldn't find any game versions to offer.");
    renderButton();
    return;
  }

  const ready = v.installed && v.checks.java && v.checks.minecraft && v.checks.toolkit;
  const status = !v.installed
    ? statusLabel("caution", t("Not installed yet"))
    : !ready
      ? statusLabel("caution", t("Needs repair"))
      : o.updateAvailable
        ? statusLabel("caution", t("Update available"))
        : statusLabel("ok", t("Ready to play"));
  $("play-eyebrow").innerHTML = `<span>Minecraft ${esc(v.minecraft)}</span><span aria-hidden="true">·</span>${status}`;

  const tag = v.installed ? v.installed.toolkitTag : o.latest ? o.latest.tag : "";
  $("play-lede").textContent = `${tag ? tag + " · " : ""}${v.description || t("Yukkit Toolkit on Fabric.")}`;

  const upd = $("update-line");
  if (o.updateAvailable && o.latest) {
    upd.classList.remove("hidden");
    $("update-text").textContent = t("{tag} is out. It installs when you press Play.", { tag: o.latest.tag });
  } else {
    upd.classList.add("hidden");
  }

  setRow("row-java", v.checks.java, v.checks.java ? t("Installed and ready.") : t("Downloaded for you the first time you play."));
  setRow("row-minecraft", v.checks.minecraft,
    v.checks.minecraft ? t("Minecraft {v} with the Fabric mod loader.", { v: v.minecraft }) : t("Minecraft {v} and Fabric, about 600 MB the first time.", { v: v.minecraft }));
  setRow("row-toolkit", v.checks.toolkit,
    v.checks.toolkit ? t("{tag} with Fabric API {api}.", { tag: v.installed.toolkitTag, api: v.installed.fabricApi }) : o.latest ? t("{tag} from GitHub.", { tag: o.latest.tag }) : t("The newest build from GitHub."));

  renderButton();
}

function renderButton() {
  const o = state.overview;
  const v = selectedVersion();
  const btn = $("play-btn");
  btn.classList.remove("running");
  btn.querySelector(".icon use").setAttribute("href", "#i-play");
  if (state.running) {
    btn.disabled = true;
    btn.classList.add("running");
    btn.querySelector(".icon use").setAttribute("href", "#i-check-circle");
    $("play-label").textContent = t("Playing");
    $("play-sub").textContent = t("Close the game to play again");
    return;
  }
  if (state.busy) {
    btn.disabled = true;
    $("play-label").textContent = t("Preparing…");
    $("play-sub").textContent = t("This can take a few minutes");
    return;
  }
  btn.disabled = !v;
  if (!v) { $("play-label").textContent = t("Play"); $("play-sub").textContent = t("Nothing to play yet"); return; }
  const ready = v.installed && v.checks.java && v.checks.minecraft && v.checks.toolkit;
  $("play-label").textContent = t("Play {v}", { v: v.minecraft });
  $("play-sub").textContent = !v.installed
    ? t("Installs first, about 600 MB")
    : !ready ? t("Repairs missing files first")
    : o.updateAvailable ? t("Updates first") : t("Ready to launch");
}

function renderNews() {
  const o = state.overview;
  const box = $("news");
  const list = (state.notes || []).slice(0, 5); // 3 shown in a narrow window, 5 when wide (play.css)
  if (!list.length) {
    box.innerHTML = `<div class="news-empty">${o && o.error ? t("News shows up here when you're online.") : t("Loading news…")}</div>`;
    return;
  }
  box.innerHTML = list
    .map((r, i) => `
      <button class="group-row news-row" data-i="${i}">
        <div class="row-main">
          <div class="row-title">${esc(shortTitle(r.title))}</div>
          <div class="row-subtitle">${esc(firstLine(r.notes))}</div>
          <div class="news-meta">${esc(formatDate(r.date))}${r.prerelease ? " · " + t("Preview") : ""}</div>
        </div>
        <svg class="icon row-chevron" aria-hidden="true"><use href="#i-chevron-right"/></svg>
      </button>`)
    .join("");
  box.querySelectorAll(".news-row").forEach((c) => (c.onclick = () => showPage("notes")));
}

async function refresh() {
  try {
    state.overview = await invoke("get_overview");
    state.running = state.overview.gameRunning;
    renderPlay();
    renderVersions();
    const o = state.overview;
    const v = selectedVersion();
    if (o.error) {
      showError(
        v && v.installed ? { ...o.error, message: o.error.message + " You can still play the version you have." } : o.error,
        refresh,
        { info: !!(v && v.installed) }
      );
    }
    loadNotes(true);
  } catch (e) {
    showError(e, refresh);
  }
}

$("version-select").onchange = async (e) => {
  const id = e.target.value;
  try {
    await invoke("select_version", { versionId: id });
    state.notes = null;
    await refresh();
  } catch (err) { showError(err); }
};

async function play() {
  const v = selectedVersion();
  if (!v) return;
  state.busy = true;
  hideError();
  setProgress({ step: 1, steps: 6, title: t("Getting ready"), detail: "", fraction: null });
  renderPlay();
  try {
    await invoke("play", { versionId: v.id });
    toast(t("Starting Minecraft. The game window opens in a moment."));
  } catch (e) {
    showError(e, play);
  } finally {
    $("progress").classList.add("hidden");
    state.busy = false;
    refresh();
  }
}
$("play-btn").onclick = play;

function setProgress(p) {
  $("progress").classList.remove("hidden");
  $("progress-title").textContent = p.title;
  $("progress-step").textContent = t("Step {n} of {m}", { n: p.step, m: p.steps });
  $("progress-detail").textContent = p.detail || "";
  const fill = $("progress-fill");
  const bar = $("progress-bar");
  if (p.fraction === null || p.fraction === undefined) {
    fill.classList.add("indeterminate");
    bar.removeAttribute("aria-valuenow");
    bar.setAttribute("aria-valuetext", t("Step {n} of {m}", { n: p.step, m: p.steps }));
  } else {
    fill.classList.remove("indeterminate");
    const overall = (p.step - 1 + Math.min(1, Math.max(0, p.fraction))) / p.steps;
    const pct = Math.round(overall * 100);
    fill.style.width = `${pct}%`;
    bar.setAttribute("aria-valuenow", String(pct));
    bar.setAttribute("aria-valuetext", `${pct}%, ` + t("Step {n} of {m}", { n: p.step, m: p.steps }));
  }
}

/* ---------- Versions page ---------- */
function renderVersions() {
  const o = state.overview;
  const list = $("version-list");
  if (!o || !o.versions.length) { list.innerHTML = `<p class="list-empty">${t("No versions yet.")}</p>`; return; }
  list.innerHTML = o.versions
    .map((v) => {
      const inUse = v.id === o.selected;
      const badges = [
        v.channel ? `<span class="badge caution">${esc(v.channel)}</span>` : "",
        v.installed ? `<span class="badge">${t("Installed")}</span>` : "",
      ].join("");
      const meta = [
        `Minecraft ${esc(v.minecraft)}`,
        "Fabric",
        v.installed ? `Toolkit ${esc(v.installed.toolkitTag)}` : t("Not installed"),
      ].join(" · ");
      const actions = [
        inUse
          ? `<span class="in-use"><svg class="icon" aria-hidden="true"><use href="#i-check-circle"/></svg>${t("In use")}</span>`
          : `<button class="btn btn-small" data-use="${esc(v.id)}">${t("Use this version")}</button>`,
        v.installed
          ? `<button class="btn btn-small btn-quiet btn-danger" data-remove="${esc(v.id)}">Remove</button>`
          : `<button class="btn btn-small btn-primary" data-install="${esc(v.id)}">Install</button>`,
      ].join("");
      return `
        <div class="group-row version-row">
          <div class="row-main">
            <div class="row-title">${esc(v.name)} ${badges}</div>
            <div class="row-subtitle">${meta}</div>
            ${v.description ? `<div class="version-desc">${esc(v.description)}</div>` : ""}
          </div>
          <div class="version-actions">${actions}</div>
        </div>`;
    })
    .join("");

  list.querySelectorAll("[data-use]").forEach((b) => (b.onclick = async () => {
    await invoke("select_version", { versionId: b.dataset.use }).catch(showError);
    state.notes = null;
    await refresh();
    toast(t("Switched to {v}", { v: b.dataset.use }));
  }));
  list.querySelectorAll("[data-install]").forEach((b) => (b.onclick = () => installVersion(b.dataset.install)));
  list.querySelectorAll("[data-remove]").forEach((b) => (b.onclick = async () => {
    const ok = await confirmSheet({
      title: t("Remove {v}?", { v: b.dataset.remove }),
      text: t("The game files for this version are deleted. Your worlds and settings for it are kept."),
      action: t("Remove"),
    });
    if (!ok) return;
    try {
      await invoke("remove_version", { versionId: b.dataset.remove });
      toast(t("Removed. Your worlds are still there."));
      refresh();
    } catch (e) { showError(e); }
  }));
}

async function installVersion(id) {
  if (state.busy) return;
  await invoke("select_version", { versionId: id }).catch(() => {});
  showPage("play");
  state.busy = true;
  hideError();
  await refresh();
  setProgress({ step: 1, steps: 6, title: t("Getting ready"), detail: "", fraction: null });
  renderButton();
  try {
    await invoke("install", { versionId: id });
    toast(t("Installed. Press Play when you're ready."));
  } catch (e) {
    showError(e, () => installVersion(id));
  } finally {
    $("progress").classList.add("hidden");
    state.busy = false;
    refresh();
  }
}

/* ---------- Patch notes ---------- */
async function loadNotes(quiet) {
  if (!state.notes) {
    try {
      state.notes = await invoke("get_patch_notes");
    } catch (e) {
      if (!quiet) {
        $("notes").innerHTML = `<p class="notes-empty">${t("Patch notes need an internet connection.")} ${esc(normalizeError(e).message)}</p>`;
        $("notes-pager").classList.add("hidden");
        $("notes-count").textContent = "";
      }
      renderNews();
      return;
    }
  }
  renderNews();
  renderNotes();
}

/* Search and pages for patch notes. Filtering happens on the releases already loaded. */
const NOTES_PER_PAGE = 3;
state.notesQuery = "";
state.notesPage = 1;

function filteredNotes() {
  const q = state.notesQuery.trim().toLowerCase();
  const list = (state.notes || []).map((r, i) => ({ r, i }));
  if (!q) return list;
  return list.filter(({ r }) => [r.title, r.tag, r.notes].some((t) => String(t || "").toLowerCase().includes(q)));
}

function renderNotes() {
  const box = $("notes");
  const pager = $("notes-pager");
  const count = $("notes-count");
  if (!state.notes) return;
  if (!state.notes.length) {
    box.innerHTML = `<p class="notes-empty">${t("No releases yet.")}</p>`;
    pager.classList.add("hidden");
    count.textContent = "";
    return;
  }
  const hits = filteredNotes();
  const pages = Math.max(1, Math.ceil(hits.length / NOTES_PER_PAGE));
  state.notesPage = Math.min(Math.max(1, state.notesPage), pages);
  const start = (state.notesPage - 1) * NOTES_PER_PAGE;
  const shown = hits.slice(start, start + NOTES_PER_PAGE);

  const q = state.notesQuery.trim();
  count.textContent = q
    ? t(hits.length === 1 ? "{n} release matches \"{q}\"" : "{n} releases match \"{q}\"", { n: hits.length, q })
    : t(hits.length === 1 ? "{n} release" : "{n} releases", { n: hits.length });

  if (!hits.length) {
    box.innerHTML = `
      <div class="notes-empty-state">
        <p class="notes-empty-title">${esc(t("No release mentions \"{q}\"", { q }))}</p>
        <p class="notes-empty">${t("Try a module name, a version such as alpha.3, or a shorter word.")}</p>
        <button class="btn btn-small" id="notes-clear">${t("Clear search")}</button>
      </div>`;
    $("notes-clear").onclick = () => { setNotesQuery(""); $("notes-search").focus(); };
    pager.classList.add("hidden");
    return;
  }

  box.innerHTML = shown
    .map(({ r, i }) => `
      <article class="release">
        <div class="release-head">
          <h2 class="release-title">${esc(shortTitle(r.title))}</h2>
          <span class="release-meta">${esc(r.tag)} · ${esc(formatDate(r.date))}</span>
        </div>
        <div class="md">${markdown(r.notes)}</div>
        <button class="link" data-release="${i}">${t("Open on GitHub")}<svg class="icon" aria-hidden="true"><use href="#i-arrow-up-right"/></svg></button>
      </article>`)
    .join("");
  box.querySelectorAll("[data-release]").forEach((b) => (b.onclick = () => openLink(state.notes[b.dataset.release].url)));
  if (q) highlight(box.querySelectorAll(".release-title, .md"), q);
  renderPager(pages);
}

function renderPager(pages) {
  const pager = $("notes-pager");
  pager.classList.toggle("hidden", pages <= 1);
  if (pages <= 1) { pager.innerHTML = ""; return; }
  const cur = state.notesPage;
  // First, last, and the pages next to the current one; gaps become an ellipsis.
  const nums = [...new Set([1, cur - 1, cur, cur + 1, pages])].filter((n) => n >= 1 && n <= pages).sort((a, b) => a - b);
  const items = [];
  nums.forEach((n, k) => {
    if (k && n - nums[k - 1] > 1) items.push(`<span class="pager-gap" aria-hidden="true">…</span>`);
    items.push(`<button class="pager-num" data-page-n="${n}" ${n === cur ? 'aria-current="page"' : ""} aria-label="${t("Page {n} of {m}", { n, m: pages })}">${n}</button>`);
  });
  pager.innerHTML = `
    <button class="btn btn-small btn-quiet" data-page-n="${cur - 1}" ${cur === 1 ? "disabled" : ""}><svg class="icon" aria-hidden="true"><use href="#i-chevron-left"/></svg>${t("Newer")}</button>
    <div class="pager-nums">${items.join("")}</div>
    <button class="btn btn-small btn-quiet" data-page-n="${cur + 1}" ${cur === pages ? "disabled" : ""}>${t("Older")}<svg class="icon" aria-hidden="true"><use href="#i-chevron-right"/></svg></button>`;
  pager.querySelectorAll("[data-page-n]").forEach((b) => (b.onclick = () => goToNotesPage(Number(b.dataset.pageN))));
}

function goToNotesPage(n) {
  state.notesPage = n;
  renderNotes();
  $("notes-title").scrollIntoView({ block: "start" });
  const first = $("notes").querySelector(".release-title");
  if (first) { first.tabIndex = -1; first.focus({ preventScroll: true }); }
}

function setNotesQuery(q) {
  state.notesQuery = q;
  state.notesPage = 1;
  $("notes-search").value = q;
  renderNotes();
}

/* Wraps each match in <mark> by walking text nodes, so the escaped Markdown HTML stays intact. */
function highlight(roots, q) {
  const needle = q.toLowerCase();
  roots.forEach((root) => {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const nodes = [];
    while (walker.nextNode()) if (walker.currentNode.nodeValue.toLowerCase().includes(needle)) nodes.push(walker.currentNode);
    nodes.forEach((node) => {
      const text = node.nodeValue;
      const lower = text.toLowerCase();
      const frag = document.createDocumentFragment();
      let at = 0, hit;
      while ((hit = lower.indexOf(needle, at)) !== -1) {
        frag.append(text.slice(at, hit));
        const m = document.createElement("mark");
        m.textContent = text.slice(hit, hit + needle.length);
        frag.append(m);
        at = hit + needle.length;
      }
      frag.append(text.slice(at));
      node.replaceWith(frag);
    });
  });
}

let notesSearchTimer;
$("notes-search").addEventListener("input", (e) => {
  clearTimeout(notesSearchTimer);
  notesSearchTimer = setTimeout(() => setNotesQuery(e.target.value), 120);
});
$("notes-search").addEventListener("keydown", (e) => {
  if (e.key === "Escape" && e.target.value) { e.preventDefault(); e.stopPropagation(); setNotesQuery(""); }
});

/* ---------- Settings (auto-save) ---------- */
function setSeg(id, value) {
  $(id).querySelectorAll("button").forEach((b) => {
    const on = b.dataset.value === value;
    b.classList.toggle("active", on);
    b.setAttribute("aria-checked", String(on));
    b.tabIndex = on ? 0 : -1; // roving tabindex: one stop per group, arrows move inside it
  });
}
// Radio-group keyboard behavior for segmented controls: arrow keys choose the neighbor.
document.querySelectorAll("[role=radiogroup]").forEach((group) => {
  group.addEventListener("keydown", (e) => {
    const keys = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };
    if (!(e.key in keys)) return;
    e.preventDefault();
    const items = [...group.querySelectorAll("button")];
    const i = items.indexOf(document.activeElement);
    const next = items[(i + keys[e.key] + items.length) % items.length];
    next.focus();
    // Arrows choose; they never open a file picker (the Upload tile waits for Enter or a click).
    if (!next.classList.contains("tile-upload")) next.click();
  });
});
function segValue(id) {
  const active = $(id).querySelector("button.active");
  return active ? active.dataset.value : null;
}
function memLabel() {
  const mb = Number($("s-memory").value);
  const text = `${(mb / 1024).toFixed(mb % 1024 ? 1 : 0)} GB`;
  const el = $("s-memory");
  $("s-memory-label").textContent = text;
  el.setAttribute("aria-valuetext", text);
  // Filled part of the track follows the thumb.
  el.style.setProperty("--pct", `${((mb - el.min) / (el.max - el.min)) * 100}%`);
}

async function loadSettings() {
  const view = await invoke("get_settings");
  const s = view.settings;
  state.settings = s;
  state.defaultInstallDir = view.defaultInstallDir;
  state.launcherVersion = view.version;
  $("launcher-version").textContent = t("Launcher {v}", { v: view.version });
  $("s-language").value = s.language || "";
  let cached = "";
  try { cached = localStorage.getItem("yukkit-language") || ""; } catch {}
  if ((s.language || "") !== cached) I18n.setLanguage(s.language || "");
  $("s-username").value = s.username;
  $("cz-nametag").textContent = s.username;
  nameFeedback();
  $("s-auto").checked = s.autoUpdate;
  setSeg("s-channel", s.previews ? "preview" : "stable");
  $("s-memory").value = s.memoryMb;
  memLabel();
  setSeg("s-java-mode", s.javaPath ? "custom" : "auto");
  $("s-java").value = s.javaPath;
  $("s-java-row").classList.toggle("hidden", !s.javaPath);
  $("s-install").textContent = view.installDir;
  $("s-jvm").value = s.jvmArgs;
  $("s-game").value = s.gameArgs;
  setSeg("s-keep", s.keepOpen ? "open" : "min");
  setSeg("s-theme", s.theme || "system");
  applyTheme(s.theme || "system");
}

async function saveSettings(patch, quietToast) {
  $("s-error").textContent = "";
  const next = { ...state.settings, ...patch };
  try {
    await invoke("save_settings", { settings: next });
    state.settings = next;
    if (!quietToast) toast(t("Saved"));
    if ("username" in patch || "previews" in patch || "installDir" in patch || "javaPath" in patch) refresh();
    return true;
  } catch (e) {
    const err = normalizeError(e);
    $("s-error").textContent = `${err.title}. ${err.message}`;
    return false;
  }
}

/* Player name: live feedback while typing; saves only a name the game accepts. */
const NAME_RULE = /^[A-Za-z0-9_]{3,16}$/;
function nameFeedback() {
  const v = $("s-username").value.trim();
  const valid = NAME_RULE.test(v);
  const bad = v.length > 0 && /[^A-Za-z0-9_]/.test(v);
  $("s-username-count").textContent = `${v.length}/16`;
  $("s-name-avatar").textContent = (v || "P")[0].toUpperCase();
  if (valid) $("cz-nametag").textContent = v;
  let kind = "info", msg = t("What other players see. 3–16 letters, numbers or underscores.");
  if (bad) { kind = "error"; msg = t("Only letters, numbers and underscores. No spaces or symbols."); }
  else if (v.length && v.length < 3) { kind = "error"; msg = t(3 - v.length === 1 ? "Add 1 more character. Names need at least 3." : "Add {n} more characters. Names need at least 3.", { n: 3 - v.length }); }
  else if (valid) { kind = "ok"; msg = t("Looks good. Other players will see this name."); }
  const box = $("s-name-box");
  box.classList.toggle("invalid", kind === "error");
  box.classList.toggle("valid", kind === "ok");
  $("s-username").setAttribute("aria-invalid", String(kind === "error"));
  const hint = $("s-username-hint");
  hint.className = `name-hint ${kind}`;
  hint.querySelector("use").setAttribute("href", kind === "ok" ? "#i-check-circle" : kind === "error" ? "#i-alert" : "#i-info");
  $("s-username-hint-text").textContent = msg;
  return valid;
}
$("s-username").oninput = nameFeedback;
$("s-username").onkeydown = (e) => { if (e.key === "Enter") e.target.blur(); };
$("s-username").onchange = (e) => { if (nameFeedback()) saveSettings({ username: e.target.value.trim() }); };
$("s-auto").onchange = (e) => saveSettings({ autoUpdate: e.target.checked });
$("s-memory").oninput = memLabel;
$("s-memory").onchange = (e) => saveSettings({ memoryMb: Number(e.target.value) });
$("s-jvm").onchange = (e) => saveSettings({ jvmArgs: e.target.value.trim() });
$("s-game").onchange = (e) => saveSettings({ gameArgs: e.target.value.trim() });
$("s-java").onchange = (e) => saveSettings({ javaPath: e.target.value.trim() });

$("s-channel").querySelectorAll("button").forEach((b) => (b.onclick = () => {
  setSeg("s-channel", b.dataset.value);
  state.notes = null;
  saveSettings({ previews: b.dataset.value === "preview" });
}));
$("s-language").onchange = async (e) => {
  if (await saveSettings({ language: e.target.value }, true)) {
    I18n.setLanguage(e.target.value);
    toast(t("Saved"));
  }
};
// Text that app.js writes is rebuilt in the new language; static text is handled by I18n.
document.addEventListener("yukkit:language", () => {
  renderPlay();
  renderVersions();
  renderNotes();
  renderNews();
  if (cz.loaded) renderPickers();
  if (state.settings) { nameFeedback(); memLabel(); }
  if (state.launcherVersion) $("launcher-version").textContent = t("Launcher {v}", { v: state.launcherVersion });
  if ($("page-logs").classList.contains("active")) renderLog();
});
$("s-theme").querySelectorAll("button").forEach((b) => (b.onclick = () => {
  setSeg("s-theme", b.dataset.value);
  applyTheme(b.dataset.value);
  saveSettings({ theme: b.dataset.value });
}));
$("s-keep").querySelectorAll("button").forEach((b) => (b.onclick = () => {
  setSeg("s-keep", b.dataset.value);
  saveSettings({ keepOpen: b.dataset.value === "open" });
}));
$("s-java-mode").querySelectorAll("button").forEach((b) => (b.onclick = () => {
  setSeg("s-java-mode", b.dataset.value);
  const custom = b.dataset.value === "custom";
  $("s-java-row").classList.toggle("hidden", !custom);
  if (!custom) saveSettings({ javaPath: "" });
  else $("s-java").focus();
}));
$("s-java-browse").onclick = async () => {
  const file = await invoke("pick_java").catch(showError);
  if (file) { $("s-java").value = file; saveSettings({ javaPath: file }); }
};
$("s-install-browse").onclick = async () => {
  const dir = await invoke("pick_folder").catch(showError);
  if (dir && (await saveSettings({ installDir: dir }))) $("s-install").textContent = dir;
};
$("s-install-reset").onclick = async () => {
  if (await saveSettings({ installDir: "" })) $("s-install").textContent = state.defaultInstallDir;
};
$("s-install-open").onclick = () => invoke("open_install_folder").catch(showError);

/* ---------- Customize: skin and cape ---------- */
const cz = { loaded: false, skin: new Map(), cape: new Map(), customSkin: null, customCape: null };

// Picture for an id: canvas for presets, image for custom, null for default skin / no cape.
function czImage(kind, id) {
  if (kind === "skin" && id === "default") return null;
  if (kind === "cape" && id === "none") return null;
  if (id === "custom") return kind === "skin" ? cz.customSkin : cz.customCape;
  return cz[kind].get(id) || null;
}

function czModel() { return (state.settings && state.settings.skinModel) || "classic"; }

function renderPreview() {
  if (!state.settings) return;
  const skin = czImage("skin", state.settings.skin || "default");
  const cape = czImage("cape", state.settings.cape || "none");
  const silhouette = getComputedStyle(document.documentElement).getPropertyValue("--fill-track").trim() || "#8e8e96";
  Cosmetics.render($("cz-front"), { skin, cape, model: czModel(), view: "front", silhouette });
  Cosmetics.render($("cz-back"), { skin, cape, model: czModel(), view: "back", silhouette });
}

function tile(kind, id, name, img, selected) {
  const b = document.createElement("button");
  b.className = "tile";
  b.setAttribute("role", "radio");
  b.dataset.value = id;
  const art = document.createElement("canvas");
  art.className = "tile-art";
  art.setAttribute("aria-hidden", "true");
  if (kind === "skin") {
    art.width = 64; art.height = 128;
    Cosmetics.render(art, { skin: img, cape: null, model: czModel(), view: "front", silhouette: getComputedStyle(document.documentElement).getPropertyValue("--fill-track").trim() });
  } else {
    art.width = 60; art.height = 96;
    if (img) {
      const ctx = art.getContext("2d");
      ctx.imageSmoothingEnabled = false;
      const k = img.width / 64;
      ctx.drawImage(img, 1 * k, 1 * k, 10 * k, 16 * k, 0, 0, 60, 96);
    }
  }
  const label = document.createElement("span");
  label.className = "tile-label";
  label.textContent = name;
  b.append(art, label);
  if (!img && (id === "default" || id === "none")) b.classList.add("tile-empty");
  if (id === "custom" && !img) {
    b.classList.add("tile-upload");
    art.remove();
    b.insertAdjacentHTML("afterbegin", `<svg class="icon lg" aria-hidden="true"><use href="#i-upload"/></svg>`);
  }
  b.onclick = () => (id === "custom" && !img ? uploadCosmetic(kind) : chooseCosmetic(kind, id));
  return b;
}

function renderPickers() {
  const s = state.settings;
  const skins = $("cz-skins");
  skins.replaceChildren(
    tile("skin", "default", t("Default"), null),
    ...Cosmetics.SKINS.map((p) => tile("skin", p.id, t(p.name), cz.skin.get(p.id))),
    tile("skin", "custom", cz.customSkin ? t("Your skin") : t("Upload…"), cz.customSkin),
  );
  const capes = $("cz-capes");
  capes.replaceChildren(
    tile("cape", "none", t("No cape"), null),
    ...Cosmetics.CAPES.map((p) => tile("cape", p.id, t(p.name), cz.cape.get(p.id))),
    tile("cape", "custom", cz.customCape ? t("Your cape") : t("Upload…"), cz.customCape),
  );
  setSeg("cz-skins", s.skin || "default");
  setSeg("cz-capes", s.cape || "none");
  setSeg("s-model", czModel());
}

async function loadCustomize() {
  if (!state.settings) return;
  if (!cz.loaded) {
    // Presets are painted once per arm model; custom pictures come from the launcher's folder.
    Cosmetics.SKINS.forEach((p) => cz.skin.set(p.id, Cosmetics.paintSkin(p, czModel())));
    Cosmetics.CAPES.forEach((p) => cz.cape.set(p.id, Cosmetics.paintCape(p)));
    try {
      const view = await invoke("get_cosmetics");
      if (view && view.customSkin) cz.customSkin = await Cosmetics.fromBytes(view.customSkin);
      if (view && view.customCape) cz.customCape = await Cosmetics.fromBytes(view.customCape);
    } catch (e) { showError(e); }
    cz.loaded = true;
  }
  renderPickers();
  renderPreview();
}

async function activePng(kind, id) {
  const img = czImage(kind, id);
  if (!img) return null;
  if (img instanceof HTMLCanvasElement) return Cosmetics.toPng(img);
  const c = document.createElement("canvas");
  c.width = img.width; c.height = img.height;
  c.getContext("2d").drawImage(img, 0, 0);
  return Cosmetics.toPng(c);
}

async function chooseCosmetic(kind, id) {
  const key = kind === "skin" ? "skin" : "cape";
  setSeg(kind === "skin" ? "cz-skins" : "cz-capes", id);
  try {
    await invoke("set_cosmetic", { kind, png: await activePng(kind, id) });
    if (await saveSettings({ [key]: id }, true)) {
      renderPreview();
      toast(kind === "skin" ? t("Skin saved") : id === "none" ? t("Cape removed") : t("Cape saved"));
    }
  } catch (e) { showError(e); }
}

async function uploadCosmetic(kind) {
  try {
    const bytes = await invoke("pick_cosmetic", { kind });
    if (!bytes) return;
    const img = await Cosmetics.fromBytes(bytes);
    if (kind === "skin") cz.customSkin = img; else cz.customCape = img;
    renderPickers();
    await chooseCosmetic(kind, "custom");
  } catch (e) { showError(e); }
}

$("cz-skin-upload").onclick = () => uploadCosmetic("skin");
$("cz-cape-upload").onclick = () => uploadCosmetic("cape");
$("s-model").querySelectorAll("button").forEach((b) => (b.onclick = async () => {
  setSeg("s-model", b.dataset.value);
  if (!(await saveSettings({ skinModel: b.dataset.value }, true))) return;
  // Preset skins are repainted for the new arm width.
  Cosmetics.SKINS.forEach((p) => cz.skin.set(p.id, Cosmetics.paintSkin(p, czModel())));
  renderPickers();
  renderPreview();
  const id = state.settings.skin || "default";
  if (id !== "default" && id !== "custom") invoke("set_cosmetic", { kind: "skin", png: await activePng("skin", id) }).catch(showError);
}));

/* ---------- Logs ---------- */
function appendLog(line) {
  state.log.push(line);
  if (state.log.length > 5000) state.log.splice(0, state.log.length - 5000);
  if ($("page-logs").classList.contains("active")) {
    const el = $("log");
    const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 30;
    el.textContent += line + "\n";
    if (atBottom) el.scrollTop = el.scrollHeight;
  }
}
function renderLog() {
  const el = $("log");
  el.textContent = state.log.length ? state.log.join("\n") + "\n" : t("Nothing here yet. Press Play and the launcher's steps and the game's output show up here.");
  el.scrollTop = el.scrollHeight;
}
$("log-copy").onclick = async () => { await navigator.clipboard.writeText(state.log.join("\n")).catch(() => {}); toast(t("Log copied")); };
$("log-clear").onclick = () => { state.log = []; renderLog(); };

/* ---------- Events from Rust ---------- */
listen("progress", (e) => setProgress(e.payload));
listen("launcher-log", (e) => appendLog(`[launcher] ${e.payload}`));
listen("game-log", (e) => appendLog(e.payload));
listen("game-state", (e) => {
  state.running = e.payload.running;
  renderButton();
  const code = e.payload.code;
  if (!e.payload.running) {
    refresh();
    if (code !== null && code !== undefined && code !== 0) {
      showError({
        title: t("Minecraft closed unexpectedly"),
        message: t("The game stopped with an error (code {code}). Try changing the memory in Settings, or look at the Logs page to see what happened right before it closed.", { code }),
        details: state.log.slice(-40).join("\n"),
      }, play);
      showPage("play");
    }
  }
});

/* ---------- Helpers ---------- */
function esc(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
function formatDate(iso) {
  if (!iso) return "";
  const d = new Date(iso + "T12:00:00");
  return isNaN(d) ? iso : d.toLocaleDateString(I18n.locale(), { year: "numeric", month: "short", day: "numeric" });
}
function shortTitle(t) { return String(t || "").replace(/\s*·\s*Minecraft.*$/, ""); }
function firstLine(md) {
  const lines = String(md || "").split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("#"));
  const bullet = lines.find((l) => /^[-*] /.test(l)) || lines[0] || "";
  return bullet.replace(/^[-*] /, "").replace(/\*\*|`/g, "");
}
/** Small, safe Markdown subset for release notes: headings, bullets, paragraphs, bold, code. */
function markdown(src) {
  const inline = (t) => esc(t)
    .replace(/\*\*(.+?)\*\*/g, "<b>$1</b>")
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");
  const out = [];
  let list = false;
  for (const raw of String(src || "").split(/\r?\n/)) {
    const line = raw.trim();
    const bullet = line.match(/^[-*] (.*)/);
    if (bullet) {
      if (!list) { out.push("<ul>"); list = true; }
      out.push(`<li>${inline(bullet[1])}</li>`);
      continue;
    }
    if (list) { out.push("</ul>"); list = false; }
    const h = line.match(/^(#{1,4}) (.*)/);
    if (h) out.push(`<h${h[1].length === 1 ? 1 : h[1].length === 4 ? 4 : 2}>${inline(h[2])}</h${h[1].length === 1 ? 1 : h[1].length === 4 ? 4 : 2}>`);
    else if (line) out.push(`<p>${inline(line)}</p>`);
  }
  if (list) out.push("</ul>");
  return out.join("");
}

/* Browser preview without Tauri, for working on the design. */
async function mockInvoke(cmd) {
  const notes = [
    { tag: "v0.2.0-alpha.4", title: "Yukkit Toolkit v0.2.0-alpha.4 · Minecraft 26.3", date: "2026-11-13", prerelease: false, url: "#",
      notes: "# Yukkit Toolkit v0.2.0-alpha.4\n\n**Alpha build, not fully tested in-game.** Back up your worlds.\n\n## What's new\n- Black and blue theme for the module window.\n- Fix: typed search text was invisible.\n- 139 modules across Combat, Movement, Player, World and more." },
    { tag: "v0.2.0-alpha.3", title: "Yukkit Toolkit v0.2.0-alpha.3 · Minecraft 26.3", date: "2026-11-13", prerelease: false, url: "#",
      notes: "## What's new\n- Module window restyled and enlarged; search moved to the title bar.\n- Fix: legacy toggles could not be disabled." },
    { tag: "v0.2.0-alpha.2", title: "Yukkit Toolkit v0.2.0-alpha.2 · Minecraft 26.3", date: "2026-11-13", prerelease: false, url: "#",
      notes: "## What's new\n- New Right Shift window with search and a Teams tab.\n- PlayerESP module with team colours." },
  ];
  switch (cmd) {
    case "get_overview":
      return {
        versions: [{ id: "26.3", name: "Yukkit Toolkit for 26.3", minecraft: "26.3", channel: "alpha",
          description: "Fabric build with 139 modules, the Right Shift module window, minimap, Freecam and team tools.",
          installed: null, checks: { java: false, minecraft: false, toolkit: false } }],
        selected: "26.3", latest: notes[0], updateAvailable: false, error: null, gameRunning: false, username: "Steve",
      };
    case "get_patch_notes": return notes;
    case "get_settings":
      return { settings: { selectedVersion: "26.3", installDir: "", username: "Steve", memoryMb: 4096, javaPath: "", jvmArgs: "", gameArgs: "", previews: false, autoUpdate: true, keepOpen: true },
        installDir: "C:\\Users\\you\\AppData\\Roaming\\YukkitLauncher", defaultInstallDir: "C:\\Users\\you\\AppData\\Roaming\\YukkitLauncher", version: "0.2.0" };
    default: return null;
  }
}

I18n.translatePage();
loadSettings().catch(showError);
refresh();
