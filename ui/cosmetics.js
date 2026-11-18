// Skins and capes: preset textures painted in code, and a front/back preview renderer.
// Textures follow Minecraft's layout: skins 64×64 (or legacy 64×32, or HD multiples), capes 64×32.
// The presets are original pixel art made for Yukkit; none of Mojang's skins are bundled.

const Cosmetics = (() => {
  /* ---------- Painting helpers ---------- */
  const shade = (hex, amount) => {
    const n = parseInt(hex.slice(1), 16);
    const f = (c) => Math.max(0, Math.min(255, Math.round(c * (1 + amount))));
    return `rgb(${f(n >> 16)}, ${f((n >> 8) & 255)}, ${f(n & 255)})`;
  };
  const rect = (ctx, color, x, y, w, h) => { ctx.fillStyle = color; ctx.fillRect(x, y, w, h); };
  const px = (ctx, color, x, y) => rect(ctx, color, x, y, 1, 1);

  // Faces of a texture box at (u, v) with size w × h × d.
  const faces = (u, v, w, h, d) => ({
    top: [u + d, v, w, d], bottom: [u + d + w, v, w, d],
    right: [u, v + d, d, h], front: [u + d, v + d, w, h],
    left: [u + d + w, v + d, d, h], back: [u + 2 * d + w, v + d, w, h],
  });
  // Paints every face; sides slightly darker so the model reads as a solid.
  const box = (ctx, f, color) => {
    rect(ctx, shade(color, 0.08), ...f.top);
    rect(ctx, shade(color, -0.18), ...f.bottom);
    rect(ctx, shade(color, -0.1), ...f.right);
    rect(ctx, color, ...f.front);
    rect(ctx, shade(color, -0.1), ...f.left);
    rect(ctx, shade(color, -0.06), ...f.back);
  };

  const ARM_W = { classic: 4, slim: 3 };
  const parts = (model) => {
    const a = ARM_W[model] || 4;
    return {
      head: faces(0, 0, 8, 8, 8),
      body: faces(16, 16, 8, 12, 4),
      rArm: faces(40, 16, a, 12, 4),
      lArm: faces(32, 48, a, 12, 4),
      rLeg: faces(0, 16, 4, 12, 4),
      lLeg: faces(16, 48, 4, 12, 4),
    };
  };

  /* ---------- Skin presets ---------- */
  const SKINS = [
    { id: "yukkit", name: "Yukkit", skin: "#e0ac88", hair: "#3b2a20", eyes: "#2b6cff", shirt: "#0066e0", accent: "#ffffff", pants: "#23262e", shoes: "#111214", mark: "y" },
    { id: "night", name: "Night", skin: "#c99572", hair: "#121214", eyes: "#5aa5ff", shirt: "#1b1b1f", accent: "#5aa5ff", pants: "#2d2d33", shoes: "#0a0a0a", mark: "stripe" },
    { id: "ember", name: "Ember", skin: "#f1c3a0", hair: "#a5341c", eyes: "#3d7a2e", shirt: "#c62f2f", accent: "#ffc93c", pants: "#3a2b25", shoes: "#1c1410", mark: "zip" },
    { id: "moss", name: "Moss", skin: "#8d5a3b", hair: "#1d140f", eyes: "#3b2a20", shirt: "#3f7d3a", accent: "#d9c58a", pants: "#6b4a2e", shoes: "#2a1c12", mark: "belt" },
    { id: "frost", name: "Frost", skin: "#f5d9c5", hair: "#e8e8ee", eyes: "#4aa3d8", shirt: "#dfeefa", accent: "#4aa3d8", pants: "#8aa4b8", shoes: "#4a5a68", mark: "stripe" },
  ];

  function paintSkin(p, model) {
    const c = document.createElement("canvas");
    c.width = 64; c.height = 64;
    const ctx = c.getContext("2d");
    const P = parts(model);
    const aw = ARM_W[model] || 4;

    // Head: skin, hair on top and around, face on the front.
    box(ctx, P.head, p.skin);
    rect(ctx, p.hair, ...P.head.top);
    const [fx, fy] = P.head.front;
    rect(ctx, p.hair, fx, fy, 8, 2);
    px(ctx, p.hair, fx, fy + 2); px(ctx, p.hair, fx + 7, fy + 2);
    rect(ctx, p.hair, P.head.right[0], P.head.right[1], 8, 4);
    rect(ctx, p.hair, P.head.left[0], P.head.left[1], 8, 4);
    rect(ctx, p.hair, P.head.back[0], P.head.back[1], 8, 7);
    px(ctx, "#ffffff", fx + 1, fy + 4); px(ctx, p.eyes, fx + 2, fy + 4);
    px(ctx, p.eyes, fx + 5, fy + 4); px(ctx, "#ffffff", fx + 6, fy + 4);
    rect(ctx, shade(p.skin, -0.22), fx + 3, fy + 6, 2, 1);

    // Body and sleeves.
    box(ctx, P.body, p.shirt);
    for (const arm of [P.rArm, P.lArm]) {
      box(ctx, arm, p.shirt);
      // Hands: last 3 rows of each side face are skin.
      for (const f of ["front", "back", "left", "right"]) {
        const [x, y, w, h] = arm[f];
        rect(ctx, f === "front" ? p.skin : shade(p.skin, -0.1), x, y + h - 3, w, 3);
      }
      rect(ctx, shade(p.skin, -0.18), ...arm.bottom);
    }
    const [bx, by] = P.body.front;
    if (p.mark === "y") {
      // The Yukkit "Y" on the chest.
      for (const [x, y] of [[1, 2], [6, 2], [2, 3], [5, 3], [3, 4], [4, 4], [3, 5], [4, 5], [3, 6], [4, 6], [3, 7], [4, 7]]) px(ctx, p.accent, bx + x, by + y);
    } else if (p.mark === "stripe") {
      rect(ctx, p.accent, bx, by + 4, 8, 1);
      for (const arm of [P.rArm, P.lArm]) rect(ctx, p.accent, arm.front[0], arm.front[1] + 4, aw, 1);
    } else if (p.mark === "zip") {
      rect(ctx, p.accent, bx + 3, by, 2, 12);
      rect(ctx, shade(p.shirt, -0.25), bx + 3, by + 11, 2, 1);
    } else if (p.mark === "belt") {
      rect(ctx, p.accent, bx, by + 10, 8, 1);
      px(ctx, "#c9a227", bx + 3, by + 10); px(ctx, "#c9a227", bx + 4, by + 10);
    }
    rect(ctx, shade(p.skin, -0.05), bx + 2, by, 4, 1); // neckline

    // Legs with shoes.
    for (const leg of [P.rLeg, P.lLeg]) {
      box(ctx, leg, p.pants);
      for (const f of ["front", "back", "left", "right"]) {
        const [x, y, w, h] = leg[f];
        rect(ctx, f === "front" ? p.shoes : shade(p.shoes, -0.1), x, y + h - 2, w, 2);
      }
      rect(ctx, shade(p.shoes, -0.2), ...leg.bottom);
    }
    return c;
  }

  /* ---------- Cape presets ---------- */
  const CAPES = [
    { id: "yukkit", name: "Yukkit", base: "#0066e0", detail: "#ffffff", style: "logo" },
    { id: "crimson", name: "Crimson", base: "#c62f2f", detail: "#7d1515", style: "band" },
    { id: "void", name: "Void", base: "#000000", detail: "#5aa5ff", style: "edge" },
    { id: "gold", name: "Gold", base: "#c9952a", detail: "#f2c94c", style: "diagonal" },
    { id: "mint", name: "Mint", base: "#2fb894", detail: "#e9fff8", style: "stripe" },
  ];

  function paintCape(p) {
    const c = document.createElement("canvas");
    c.width = 64; c.height = 32;
    const ctx = c.getContext("2d");
    const f = faces(0, 0, 10, 16, 1);
    box(ctx, f, p.base);
    const [ox, oy] = f.front; // the outer face, seen from behind the player
    if (p.style === "logo") {
      rect(ctx, shade(p.base, 0.15), ox, oy, 10, 1);
      for (const [x, y] of [[2, 4], [7, 4], [3, 5], [6, 5], [4, 6], [5, 6], [4, 7], [5, 7], [4, 8], [5, 8], [4, 9], [5, 9], [4, 10], [5, 10]]) px(ctx, p.detail, ox + x, oy + y);
    } else if (p.style === "band") {
      rect(ctx, p.detail, ox, oy + 12, 10, 4);
      rect(ctx, shade(p.base, 0.2), ox, oy + 11, 10, 1);
    } else if (p.style === "edge") {
      rect(ctx, p.detail, ox, oy, 10, 1);
      rect(ctx, p.detail, ox, oy, 1, 16);
      rect(ctx, p.detail, ox + 9, oy, 1, 16);
      rect(ctx, p.detail, ox, oy + 15, 10, 1);
    } else if (p.style === "diagonal") {
      for (let i = 0; i < 16; i++) { const x = Math.floor(i * 10 / 16); px(ctx, p.detail, ox + x, oy + i); if (x + 1 < 10) px(ctx, p.detail, ox + x + 1, oy + i); }
    } else if (p.style === "stripe") {
      rect(ctx, p.detail, ox + 4, oy, 2, 16);
    }
    // Inner face slightly darker, as fabric seen from inside.
    rect(ctx, shade(p.base, -0.25), ...f.back);
    return c;
  }

  /* ---------- Preview ---------- */
  // Front/back view of the player, 16 × 32 units with a 2 unit margin for the cape.
  const UNIT_W = 16, UNIT_H = 32;

  function drawPart(ctx, src, k, sx, sy, sw, sh, dx, dy, s, mirror) {
    if (!src) return;
    ctx.save();
    if (mirror) {
      ctx.translate((dx + sw) * s, dy * s);
      ctx.scale(-1, 1);
      ctx.drawImage(src, sx * k, sy * k, sw * k, sh * k, 0, 0, sw * s, sh * s);
    } else {
      ctx.drawImage(src, sx * k, sy * k, sw * k, sh * k, dx * s, dy * s, sw * s, sh * s);
    }
    ctx.restore();
  }

  function drawSilhouette(ctx, s, color) {
    ctx.fillStyle = color;
    for (const [x, y, w, h] of [[4, 0, 8, 8], [4, 8, 8, 12], [0, 8, 4, 12], [12, 8, 4, 12], [4, 20, 4, 12], [8, 20, 4, 12]]) {
      ctx.fillRect(x * s, y * s, w * s, h * s);
    }
  }

  /**
   * Draws the character into `canvas`.
   * skin: canvas/image or null (null = the game's default, shown as a silhouette)
   * cape: canvas/image or null; view: "front" | "back"
   */
  function render(canvas, { skin, cape, model = "classic", view = "front", silhouette = "#8e8e96" }) {
    const ctx = canvas.getContext("2d");
    const s = Math.floor(Math.min(canvas.width / UNIT_W, canvas.height / UNIT_H));
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.imageSmoothingEnabled = false;
    ctx.save();
    ctx.translate(Math.floor((canvas.width - UNIT_W * s) / 2), Math.floor((canvas.height - UNIT_H * s) / 2));

    const capeK = cape ? cape.width / 64 : 1;
    // From the back, the cape hangs behind everything except the head.
    if (!skin) drawSilhouette(ctx, s, silhouette);
    else {
      const k = skin.width / 64;
      const legacy = skin.height * 2 === skin.width; // 64×32: no separate left limbs, no overlays below the head
      const a = ARM_W[model] || 4;
      const front = view === "front";
      // [source x, y, w, h] for front and back faces of each part, base layer then overlay.
      const P = {
        head: front ? [8, 8, 8, 8] : [24, 8, 8, 8],
        hat: front ? [40, 8, 8, 8] : [56, 8, 8, 8],
        body: front ? [20, 20, 8, 12] : [32, 20, 8, 12],
        jacket: front ? [20, 36, 8, 12] : [32, 36, 8, 12],
        rArm: front ? [44, 20, a, 12] : [48 + a, 20, a, 12],
        rSleeve: front ? [44, 36, a, 12] : [48 + a, 36, a, 12],
        lArm: front ? [36, 52, a, 12] : [40 + a, 52, a, 12],
        lSleeve: front ? [52, 52, a, 12] : [56 + a, 52, a, 12],
        rLeg: front ? [4, 20, 4, 12] : [12, 20, 4, 12],
        rPant: front ? [4, 36, 4, 12] : [12, 36, 4, 12],
        lLeg: front ? [20, 52, 4, 12] : [28, 52, 4, 12],
        lPant: front ? [4, 52, 4, 12] : [12, 52, 4, 12],
      };
      // Screen positions: from the front the player's right side is on the viewer's left.
      const rArmX = front ? 4 - a : 12, lArmX = front ? 12 : 4 - a;
      const rLegX = front ? 4 : 8, lLegX = front ? 8 : 4;
      const layer = (key, x, y, overlay) => {
        if (overlay && legacy && key !== "hat") return;
        const [sx, sy, sw, sh] = P[key];
        drawPart(ctx, skin, k, sx, sy, sw, sh, x, y, s, false);
      };
      const limb = (key, legacyKey, x, y) => {
        if (legacy) { const [sx, sy, sw, sh] = P[legacyKey]; drawPart(ctx, skin, k, sx, sy, sw, sh, x, y, s, true); }
        else layer(key, x, y, false);
      };
      layer("head", 4, 0); layer("body", 4, 8);
      layer("rArm", rArmX, 8); limb("lArm", "rArm", lArmX, 8);
      layer("rLeg", rLegX, 20); limb("lLeg", "rLeg", lLegX, 20);
      layer("jacket", 4, 8, true); layer("rSleeve", rArmX, 8, true); layer("lSleeve", lArmX, 8, true);
      layer("rPant", rLegX, 20, true); layer("lPant", lLegX, 20, true);
      layer("hat", 4, 0, true);
    }
    if (cape && view === "back") {
      // Outer face of the cape, hanging from the shoulders.
      ctx.drawImage(cape, 1 * capeK, 1 * capeK, 10 * capeK, 16 * capeK, 3 * s, 8 * s, 10 * s, 16 * s);
    }
    ctx.restore();
  }

  /* ---------- Bytes ---------- */
  const toPng = (canvas) => new Promise((resolve) =>
    canvas.toBlob(async (blob) => resolve(Array.from(new Uint8Array(await blob.arrayBuffer()))), "image/png"));

  const fromBytes = (bytes) => new Promise((resolve, reject) => {
    const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "image/png" }));
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = url;
  });

  return { SKINS, CAPES, paintSkin, paintCape, render, toPng, fromBytes };
})();
