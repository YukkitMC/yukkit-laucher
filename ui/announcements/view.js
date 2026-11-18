// Announcements UI: list (featured + cards), reader, loading / empty / error states, nav badge.
// Post HTML always goes through DOMPurify; iframes are kept only for YouTube embeds.

const AnnouncementsView = (() => {
  const C = ANNOUNCEMENTS_CONFIG;
  const S = AnnouncementsService;
  const $id = (id) => document.getElementById(id);
  const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  const httpUrl = (u) => (typeof u === "string" && /^https?:\/\//i.test(u) ? u : null);
  let openId = null;

  /* ---------- Sanitizing ---------- */
  function iframeAllowed(src) {
    try {
      const u = new URL(src, "https://www.youtube.com");
      return C.iframeHosts.includes(u.hostname) && u.pathname.startsWith("/embed/");
    } catch { return false; }
  }
  DOMPurify.addHook("uponSanitizeElement", (node, data) => {
    if (data.tagName === "iframe" && !iframeAllowed(node.getAttribute("src") || "")) node.remove();
  });
  DOMPurify.addHook("afterSanitizeAttributes", (node) => {
    if (node.tagName === "A") { node.removeAttribute("target"); node.setAttribute("rel", "noopener noreferrer"); }
    if (node.tagName === "IMG") { node.setAttribute("loading", "lazy"); node.setAttribute("decoding", "async"); }
    if (node.tagName === "IFRAME") {
      const u = new URL(node.getAttribute("src"), "https://www.youtube.com");
      u.protocol = "https:";
      node.setAttribute("src", u.href);
      node.setAttribute("sandbox", "allow-scripts allow-same-origin allow-presentation allow-popups");
      node.setAttribute("referrerpolicy", "strict-origin-when-cross-origin");
      node.setAttribute("allow", "encrypted-media; picture-in-picture; fullscreen");
      node.setAttribute("loading", "lazy");
    }
  });
  const sanitize = (html) => DOMPurify.sanitize(html, {
    ADD_TAGS: ["iframe"],
    ADD_ATTR: ["allow", "allowfullscreen", "frameborder"],
    // Blogger's inline styles and font colors fight the app's light/dark themes.
    FORBID_TAGS: ["style", "form", "input", "button", "textarea", "select", "font"],
    FORBID_ATTR: ["style", "color", "bgcolor", "face", "align"],
  });

  /* ---------- Pieces ---------- */
  const formatDate = (iso) => {
    const d = new Date(iso);
    return isNaN(d) ? "" : d.toLocaleDateString(I18n.locale(), { day: "numeric", month: "short", year: "numeric" });
  };

  function coverHtml(post, cls) {
    const src = httpUrl(post.cover);
    return src
      ? `<div class="ann-cover ${cls}"><img src="${esc(src)}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" /></div>`
      : `<div class="ann-cover ann-cover-empty ${cls}"><img src="img/yukkit.svg" alt="" /></div>`;
  }

  const tagsHtml = (tags) => tags.length
    ? `<span class="ann-tags">${tags.slice(0, 3).map((t) => `<span class="badge">${esc(t)}</span>`).join("")}</span>` : "";

  function cardHtml(post, featured) {
    return `
      <button class="ann-card${featured ? " ann-featured" : ""}" data-post="${esc(post.id)}">
        ${coverHtml(post, featured ? "ann-cover-featured" : "")}
        <span class="ann-card-body">
          ${featured ? `<span class="ann-latest">${esc(t("Latest"))}</span>` : ""}
          <span class="ann-card-title">${esc(post.title || t("Untitled post"))}</span>
          <span class="ann-meta"><time datetime="${esc(post.published)}">${esc(formatDate(post.published))}</time>${tagsHtml(post.tags)}</span>
          ${post.excerpt ? `<span class="ann-excerpt">${esc(post.excerpt)}</span>` : ""}
        </span>
      </button>`;
  }

  // A broken cover becomes the logo placeholder instead of a broken-image icon.
  function guardCovers(root) {
    root.querySelectorAll(".ann-cover img").forEach((img) => {
      img.addEventListener("error", () => {
        const box = img.parentElement;
        box.classList.add("ann-cover-empty");
        img.src = "img/yukkit.svg";
      }, { once: true });
    });
  }

  /* ---------- States ---------- */
  const skeleton = () => `
    <div class="ann-skeleton ann-featured" aria-hidden="true"><span class="sk sk-cover"></span><span class="sk-lines"><span class="sk sk-title"></span><span class="sk sk-line"></span><span class="sk sk-line short"></span></span></div>
    <div class="ann-grid" aria-hidden="true">${'<div class="ann-skeleton"><span class="sk sk-cover"></span><span class="sk sk-title"></span><span class="sk sk-line"></span></div>'.repeat(3)}</div>`;

  const message = (icon, title, text, retry) => `
    <div class="ann-message">
      <svg class="icon lg" aria-hidden="true"><use href="#${icon}"/></svg>
      <p class="ann-message-title">${esc(title)}</p>
      <p class="ann-message-text">${esc(text)}</p>
      ${retry ? `<button class="btn btn-small" data-ann-retry><svg class="icon" aria-hidden="true"><use href="#i-retry"/></svg>${esc(t("Try again"))}</button>` : ""}
    </div>`;

  /* ---------- List ---------- */
  function renderList(st) {
    const list = $id("ann-list");
    const status = $id("ann-status");
    list.setAttribute("aria-busy", String(st.loading && !st.posts));
    status.innerHTML = "";

    if (!st.posts) {
      list.innerHTML = st.error
        ? message("i-alert", t("Couldn't load announcements"), t("Check your internet connection and try again."), true)
        : skeleton();
      return;
    }
    if (st.error) {
      // Offline (or blog down) but we have a saved copy: keep it, say so quietly.
      const when = st.fetchedAt ? new Date(st.fetchedAt).toLocaleString(I18n.locale(), { dateStyle: "short", timeStyle: "short" }) : "";
      status.innerHTML = `<p class="ann-offline"><svg class="icon" aria-hidden="true"><use href="#i-info"/></svg>
        <span>${esc(t("You're offline. Showing what was saved on {when}.", { when }))}</span>
        <button class="link" data-ann-retry>${esc(t("Try again"))}</button></p>`;
    }
    if (!st.posts.length) {
      list.innerHTML = message("i-megaphone", t("No announcements yet"), t("When the Yukkit team posts news, it shows up here."), false);
      return;
    }
    const [first, ...rest] = st.posts;
    list.innerHTML = cardHtml(first, true) + (rest.length ? `<div class="ann-grid">${rest.map((p) => cardHtml(p, false)).join("")}</div>` : "");
    guardCovers(list);
  }

  /* ---------- Reader ---------- */
  function renderReader(post) {
    const reader = $id("ann-reader");
    const url = httpUrl(post.url);
    reader.innerHTML = `
      <button class="btn btn-small btn-quiet ann-back" data-ann-back><svg class="icon" aria-hidden="true"><use href="#i-chevron-left"/></svg>${esc(t("All announcements"))}</button>
      <article class="ann-article" aria-labelledby="ann-article-title">
        ${coverHtml(post, "ann-cover-hero")}
        <header class="ann-article-head">
          <h1 class="ann-article-title" id="ann-article-title" tabindex="-1">${esc(post.title || t("Untitled post"))}</h1>
          <p class="ann-meta"><time datetime="${esc(post.published)}">${esc(formatDate(post.published))}</time>${tagsHtml(post.tags)}</p>
        </header>
        <div class="ann-content">${sanitize(post.html)}</div>
        ${url ? `<p class="ann-source"><button class="link" data-ann-link="${esc(url)}">${esc(t("Open on Blogger"))}<svg class="icon" aria-hidden="true"><use href="#i-arrow-up-right"/></svg></button></p>` : ""}
      </article>`;
    // Responsive 16:9 frame for videos, scroll box for wide tables.
    const title = reader.querySelector("#ann-article-title");
    reader.querySelectorAll(".ann-content iframe").forEach((f) => {
      const wrap = document.createElement("div");
      wrap.className = "ann-embed";
      f.replaceWith(wrap);
      wrap.append(f);
      // The YouTube player grabs focus once it loads, which strands keyboard users (and Esc) inside
      // it and scrolls the page down to the video. Undo both unless the player was actually used.
      const scroller = document.querySelector(".content");
      let used = false, lastScroll = scroller.scrollTop, checks = 0;
      wrap.addEventListener("pointerenter", () => { used = true; }, { once: true });
      const guard = setInterval(() => {
        if (!f.isConnected || used || ++checks > 25) return clearInterval(guard);
        if (document.activeElement === f) {
          title.focus({ preventScroll: true });
          scroller.scrollTop = lastScroll;
        } else {
          lastScroll = scroller.scrollTop;
        }
      }, 120);
    });
    reader.querySelectorAll(".ann-content table").forEach((tb) => {
      const wrap = document.createElement("div");
      wrap.className = "ann-table";
      tb.replaceWith(wrap);
      wrap.append(tb);
    });
    guardCovers(reader);
  }

  function open(id) {
    const post = S.find(id);
    if (!post) return;
    openId = id;
    renderReader(post);
    $id("ann-list-view").hidden = true;
    $id("ann-reader").hidden = false;
    document.querySelector(".content").scrollTop = 0;
    $id("ann-article-title").focus({ preventScroll: true });
  }

  function close() {
    if (!openId) return;
    const id = openId;
    openId = null;
    $id("ann-reader").hidden = true;
    $id("ann-reader").innerHTML = "";
    $id("ann-list-view").hidden = false;
    const card = [...document.querySelectorAll("#ann-list [data-post]")].find((c) => c.dataset.post === id);
    if (card) { card.focus({ preventScroll: true }); card.scrollIntoView({ block: "nearest" }); }
  }

  /* ---------- Nav badge ---------- */
  function renderBadge() {
    const n = S.unreadCount();
    const badge = $id("nav-ann-badge");
    badge.hidden = n === 0;
    badge.textContent = n > 9 ? "9+" : String(n);
    $id("nav-ann-new").textContent = n ? t(n === 1 ? "1 new" : "{n} new", { n }) : "";
  }

  /* ---------- Wiring ---------- */
  const onPage = () => $id("page-announcements").classList.contains("active");

  function init(openExternal) {
    const page = $id("page-announcements");
    page.addEventListener("click", (e) => {
      const card = e.target.closest("[data-post]");
      if (card) return open(card.dataset.post);
      if (e.target.closest("[data-ann-back]")) return close();
      if (e.target.closest("[data-ann-retry]")) return S.load({ force: true });
      const ext = e.target.closest("[data-ann-link]");
      if (ext) return openExternal(ext.dataset.annLink);
      // Links inside a post open in the system browser, never inside the launcher.
      const a = e.target.closest(".ann-content a[href]");
      if (a) {
        e.preventDefault();
        const href = a.getAttribute("href");
        if (href.startsWith("#")) return;
        openExternal(new URL(href, "https://yukkitmc.blogspot.com/").href);
      }
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && openId && onPage() && !document.querySelector("dialog[open]")) { e.preventDefault(); close(); }
    });
    S.subscribe((st) => {
      renderBadge();
      if (!openId) renderList(st);
      if (onPage() && st.posts) S.markAllSeen();
    });
    document.addEventListener("yukkit:language", () => {
      renderBadge();
      if (openId) { const p = S.find(openId); if (p) renderReader(p); } else renderList(S.state);
    });
    S.load();
    // Keep the badge current while the launcher stays open; the service throttles to 10 minutes.
    setInterval(() => S.load(), C.refreshEveryMs);
  }

  /** Called when the page becomes visible. */
  function show() {
    if (openId) close();
    S.load();
    if (S.state.posts) S.markAllSeen();
  }

  return { init, show };
})();
