// Announcements data: fetch the Blogger feed, turn entries into posts, cache them, track what was seen.
//
// Post: { id, title, published (ISO), html (raw, sanitize before showing), cover (url | null),
//         url (original post | null), tags: string[], excerpt }
//
// In the desktop app the feed is fetched by Rust (fetch_blog_feed), so CORS never applies.
// In the browser preview it tries fetch() and falls back to Blogger's JSONP format.

const AnnouncementsService = (() => {
  const C = ANNOUNCEMENTS_CONFIG;
  const listeners = new Set();
  let state = { posts: null, fetchedAt: 0, loading: false, error: null, fromCache: false };
  let inflight = null;

  /* ---------- Storage (every access guarded: storage can be unavailable) ---------- */
  const read = (key) => { try { return localStorage.getItem(key); } catch { return null; } };
  const write = (key, value) => { try { localStorage.setItem(key, value); } catch {} };

  function readCache() {
    try {
      const c = JSON.parse(read(C.cacheKey) || "null");
      return c && Array.isArray(c.posts) && typeof c.at === "number" ? c : null;
    } catch { return null; }
  }

  // Saved by an older version (may hold posts whose HTML was never decoded).
  C.oldCacheKeys.forEach((k) => { try { localStorage.removeItem(k); } catch {} });

  /* ---------- Feed → posts ---------- */
  const parseHtml = (html) => new DOMParser().parseFromString(html || "", "text/html"); // inert: no scripts, no loads

  /**
   * When HTML is pasted into Blogger's visual editor, the feed carries it escaped
   * ("<p>&lt;p&gt;&lt;img …"), and it would show as text. Decode it then — but only when the
   * post has escaped tags and no real markup besides plain wrappers, so a normal post that
   * shows code (inside <pre>/<code>) stays as it is.
   */
  function decodeIfEscaped(html) {
    if (!/&lt;\/?[a-z][a-z0-9]*(\s|&gt;|\/)/i.test(html)) return html;
    const doc = parseHtml(html);
    const realTags = doc.body.querySelectorAll("*:not(p):not(br):not(div):not(span)").length;
    return realTags ? html : doc.body.textContent;
  }

  function excerptOf(doc) {
    // textContent glues blocks together ("new" + "Right" -> "newRight"); separate them first.
    doc.body.querySelectorAll("p, div, br, li, h1, h2, h3, h4, h5, h6, pre, blockquote, tr, figcaption").forEach((el) => el.after(" "));
    doc.body.querySelectorAll("script, style, iframe").forEach((el) => el.remove());
    const text = (doc.body.textContent || "").replace(/\s+/g, " ").trim();
    if (text.length <= C.excerptLength) return text;
    const cut = text.slice(0, C.excerptLength);
    return cut.slice(0, Math.max(cut.lastIndexOf(" "), C.excerptLength - 20)).replace(/[,.;:!?-]+$/, "") + "…";
  }

  // Blogger thumbnails are 72 px (".../s72-c/..." or "...=s72-c"); ask for the 1200 px version.
  const bigImage = (url) => url.replace(/\/s72(-c)?\//, "/s1200/").replace(/=s72(-c)?$/, "=s1200");

  function toPost(entry) {
    const html = decodeIfEscaped((entry.content && entry.content.$t) || (entry.summary && entry.summary.$t) || "");
    const doc = parseHtml(html);
    const thumb = entry.media$thumbnail && entry.media$thumbnail.url;
    const firstImg = doc.querySelector("img[src]");
    const alt = (entry.link || []).find((l) => l.rel === "alternate");
    return {
      id: entry.id.$t,
      title: (entry.title && entry.title.$t) || "",
      published: entry.published && entry.published.$t,
      html,
      // No thumbnail in the feed: the first picture in the (decoded) post becomes the cover.
      cover: thumb ? bigImage(thumb) : firstImg ? firstImg.getAttribute("src") : null,
      url: alt ? alt.href : null,
      tags: (entry.category || []).map((c) => c.term).filter(Boolean),
      excerpt: excerptOf(doc),
    };
  }

  function toPosts(json) {
    const entries = (json && json.feed && json.feed.entry) || []; // no "entry" = no posts yet
    return entries.map(toPost).sort((a, b) => String(b.published).localeCompare(String(a.published)));
  }

  /* ---------- Fetching ---------- */
  function jsonp(url) {
    return new Promise((resolve, reject) => {
      const cb = `__yukkitFeed${Date.now()}`;
      const script = document.createElement("script");
      const done = () => { delete window[cb]; script.remove(); clearTimeout(timer); };
      const timer = setTimeout(() => { done(); reject(new Error("The blog didn't answer in time.")); }, 15000);
      window[cb] = (data) => { done(); resolve(data); };
      script.onerror = () => { done(); reject(new Error("The blog couldn't be reached.")); };
      script.src = url.replace("alt=json", "alt=json-in-script") + `&callback=${cb}`;
      document.head.append(script);
    });
  }

  async function fetchFeed() {
    const tauri = window.__TAURI__;
    if (tauri) return tauri.core.invoke("fetch_blog_feed", { url: C.feedUrl });
    try {
      const res = await fetch(C.feedUrl);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return await res.json();
    } catch {
      return jsonp(C.feedUrl); // CORS blocked in a plain browser: Blogger's JSONP works there
    }
  }

  /* ---------- Public API ---------- */
  function emit() { listeners.forEach((fn) => fn(state)); }
  function set(patch) { state = { ...state, ...patch }; emit(); }

  /**
   * Shows the cache right away, then refreshes in the background when the cache is older than
   * the refresh interval (or force is set). Never hits the blog more than once at a time.
   */
  function load({ force = false } = {}) {
    if (!state.posts) {
      const cache = readCache();
      if (cache) set({ posts: cache.posts, fetchedAt: cache.at, fromCache: true });
    }
    const fresh = state.fetchedAt && Date.now() - state.fetchedAt < C.refreshEveryMs;
    if ((fresh && !force) || inflight) return inflight || Promise.resolve(state);
    set({ loading: true, error: null });
    inflight = fetchFeed()
      .then((json) => {
        const posts = toPosts(json);
        const at = Date.now();
        write(C.cacheKey, JSON.stringify({ at, posts }));
        set({ posts, fetchedAt: at, loading: false, error: null, fromCache: false });
      })
      .catch((err) => set({ loading: false, error: err || new Error("Unknown error") }))
      .finally(() => { inflight = null; });
    return inflight;
  }

  /** How many posts are newer than the last one the player opened the page on. */
  function unreadCount() {
    const posts = state.posts || [];
    if (!posts.length) return 0;
    const seen = read(C.seenKey);
    if (!seen) return posts.length;
    const i = posts.findIndex((p) => p.id === seen);
    return i === -1 ? posts.length : i;
  }

  function markAllSeen() {
    const posts = state.posts || [];
    // Only notify when something changed; listeners call this, so an unconditional emit would loop.
    if (!posts.length || read(C.seenKey) === posts[0].id) return;
    write(C.seenKey, posts[0].id);
    emit();
  }

  return {
    load, unreadCount, markAllSeen,
    subscribe(fn) { listeners.add(fn); fn(state); return () => listeners.delete(fn); },
    get state() { return state; },
    find: (id) => (state.posts || []).find((p) => p.id === id) || null,
  };
})();
