// Announcements page settings. Change the blog here; nothing else needs to know about it.
const ANNOUNCEMENTS_CONFIG = Object.freeze({
  /** Public Blogger feed (JSON). max-results caps how many posts are loaded. */
  feedUrl: "https://yukkitmc.blogspot.com/feeds/posts/default?alt=json&max-results=20",
  /** Don't ask the blog again sooner than this. */
  refreshEveryMs: 10 * 60 * 1000,
  /** Length of the plain-text summary on cards. */
  excerptLength: 160,
  /** Hosts allowed inside <iframe> in posts (YouTube embeds). */
  iframeHosts: ["www.youtube.com", "youtube.com", "www.youtube-nocookie.com", "youtube-nocookie.com"],
  /** localStorage keys. */
  cacheKey: "yukkit-announcements-cache",
  seenKey: "yukkit-announcements-seen",
});
