// Runs before the stylesheets: applies the saved theme so a dark/OLED choice never flashes light.
// A file rather than an inline <script> so the Content Security Policy can stay script-src 'self'.
try {
  const t = localStorage.getItem("yukkit-theme");
  if (t === "dark" || t === "oled") document.documentElement.dataset.theme = t;
} catch {}
