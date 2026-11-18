// Announcements page settings. Change the blog here; nothing else needs to know about it.
const ANNOUNCEMENTS_CONFIG = Object.freeze({
  /** Public Blogger feed (JSON). max-results caps how many posts are loaded. */
  feedUrl: "https://yukkitmc.blogspot.com/feeds/posts/default?alt=json&max-results=20",
  /** How often the blog is checked for new posts (and the shortest gap between two checks). */
  refreshEveryMs: 3 * 60 * 1000,
  /** Length of the plain-text summary on cards. */
  excerptLength: 160,
  /** Hosts allowed inside <iframe> in posts (YouTube embeds). */
  iframeHosts: ["www.youtube.com", "youtube.com", "www.youtube-nocookie.com", "youtube-nocookie.com"],
  /** localStorage keys. v2 drops caches saved before escaped posts were decoded. */
  cacheKey: "announcements_cache_v2",
  oldCacheKeys: ["yukkit-announcements-cache"],
  seenKey: "yukkit-announcements-seen",
});
