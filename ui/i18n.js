// Interface language: English (source), Portuguese (Brazil) and Spanish.
//
// Keys are the English text itself, so English needs no dictionary and a missing translation
// falls back to readable English. Static text in index.html is translated by walking the page;
// text built in app.js goes through t(). Elements marked data-no-i18n (user data, text that
// app.js writes) are skipped; their title / aria-label / placeholder are still translated.
// Patch notes and messages coming from the Rust side stay in English.

const I18n = (() => {
  const LOCALES = { en: "en-US", pt: "pt-BR", es: "es-ES" };
  const DICT = { pt: {}, es: {} }; // filled in i18n-strings.js
  const ATTRS = ["title", "aria-label", "placeholder"];
  const SVG = "http://www.w3.org/2000/svg";
  const srcText = new WeakMap(); // text node -> English source
  const srcAttr = new WeakMap(); // element -> { attribute: English source }
  let lang = "en";

  const norm = (s) => s.replace(/\s+/g, " ").trim();

  /** "" or unknown = follow the system language (pt*, es*, otherwise English). */
  function resolve(pref) {
    if (pref && LOCALES[pref]) return pref;
    const sys = (navigator.language || "en").slice(0, 2).toLowerCase();
    return LOCALES[sys] ? sys : "en";
  }

  function t(key, vars) {
    let s = (lang !== "en" && DICT[lang][key]) || key;
    if (vars) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? vars[k] : m));
    return s;
  }

  function translateTree(node) {
    if (node.nodeType === Node.TEXT_NODE) {
      if (!node.nodeValue.trim()) return;
      if (!srcText.has(node)) srcText.set(node, node.nodeValue);
      const src = srcText.get(node);
      const lead = /^\s/.test(src) ? " " : "", trail = /\s$/.test(src) ? " " : "";
      node.nodeValue = lead + t(norm(src)) + trail;
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE || node.namespaceURI === SVG) return;
    if (node.tagName === "SCRIPT" || node.tagName === "STYLE") return;
    let saved = srcAttr.get(node);
    for (const a of ATTRS) {
      if (!node.hasAttribute(a)) continue;
      if (!saved) srcAttr.set(node, (saved = {}));
      if (!(a in saved)) saved[a] = node.getAttribute(a);
      node.setAttribute(a, t(saved[a]));
    }
    if (node.hasAttribute("data-no-i18n")) return;
    for (const child of node.childNodes) translateTree(child);
  }

  /** Every English string the static page would ask for (used to keep dictionaries complete). */
  function staticKeys(root = document.body) {
    const keys = new Set();
    const visit = (node) => {
      if (node.nodeType === Node.TEXT_NODE) { if (node.nodeValue.trim()) keys.add(norm(srcText.get(node) || node.nodeValue)); return; }
      if (node.nodeType !== Node.ELEMENT_NODE || node.namespaceURI === SVG || node.tagName === "SCRIPT") return;
      const saved = srcAttr.get(node) || {};
      for (const a of ATTRS) if (node.hasAttribute(a)) keys.add(saved[a] || node.getAttribute(a));
      if (!node.hasAttribute("data-no-i18n")) node.childNodes.forEach(visit);
    };
    visit(root);
    return [...keys];
  }

  function setLanguage(pref) {
    lang = resolve(pref);
    document.documentElement.lang = LOCALES[lang].slice(0, 2);
    try { localStorage.setItem("yukkit-language", pref || ""); } catch {}
    if (document.body) translateTree(document.body);
    document.dispatchEvent(new CustomEvent("yukkit:language", { detail: lang }));
  }

  // Use the last choice right away so the first render is already in the right language.
  try { lang = resolve(localStorage.getItem("yukkit-language") || ""); } catch { lang = resolve(""); }

  return {
    t, setLanguage, staticKeys, DICT,
    get lang() { return lang; },
    locale: () => LOCALES[lang],
    translatePage: () => translateTree(document.body),
  };
})();
const t = I18n.t;
