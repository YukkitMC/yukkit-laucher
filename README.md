# Yukkit Launcher

A desktop launcher for **LiquidBounce** built with Rust and Tauri 2. Open it, see the current LiquidBounce build, press one button, and play.

- **Home**: the latest LiquidBounce version, its Minecraft version, the changelog, and one button that installs, updates, repairs or plays, depending on what you need.
- **Clear progress**: six named steps (updates → Minecraft → Java → game files → sounds & textures → LiquidBounce) with a progress bar and file counts or MB.
- **Plain-English problems**: no internet, drive full, folder not writable, Java missing or a game crash each get a clear explanation and a *Try again* button. Technical details stay behind *Show details*.
- **Works offline**: if the servers can't be reached, the launcher still starts the version you already installed.
- **Settings**: player name, install location, memory, automatic or custom Java, extra Java arguments, nightly builds, and whether the launcher stays open while you play.
- **Logs**: launcher and game output, ready to copy when you ask for help.

## How it installs LiquidBounce

The launcher follows the same steps as the official [LiquidLauncher](https://github.com/CCBlueX/LiquidLauncher):

1. It asks the LiquidBounce API (`api.liquidbounce.net`) for the newest `nextgen` build and its launch manifest.
2. It loads the Fabric profile and the vanilla Minecraft profile from Mojang, then merges them.
3. It downloads Eclipse Temurin Java (the version the build asks for) from Adoptium into `runtimes/`.
4. It downloads the client jar, libraries and assets, checking each file's SHA-1. Up to 16 downloads run at once, and failed downloads are retried.
5. It installs the mods from the manifest. The LiquidBounce jar itself comes through LiquidBounce's official download page, which opens in a window, just like in the official launcher. That page keeps the client free. After the first time, the jar is reused from `mod_cache/`.
6. It starts the game with Java, using an offline account and the player name from Settings.

Everything lives in one folder (by default `%APPDATA%\YukkitLauncher`), and settings live in `%APPDATA%\YukkitLauncher\settings.json`.

## Run it locally

You need:
- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) 18 or newer (only for the Tauri CLI)
- On Windows: WebView2, which is already included with Windows 10/11. On Linux, install the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/).

```bash
npm install          # installs the Tauri CLI
npm run dev          # opens the launcher in development mode
```

The UI is plain HTML/CSS/JS in `ui/`, so there's no bundler and nothing to compile on the frontend. If you open `ui/index.html` directly in a browser, it shows a preview with sample data.

## Build

```bash
npm run build        # → src-tauri/target/release/bundle/nsis/*-setup.exe
```

GitHub Actions (`.github/workflows/build.yml`) builds the Windows installer and a portable `.exe` on every push. To publish a new GitHub Release, change `version` in `src-tauri/tauri.conf.json` (and `Cargo.toml`) and push to `main`. The workflow creates the release `v<version>` if it doesn't exist yet. Pushing a `v*` tag works too.

## Project layout

```
ui/                     launcher UI (index.html, styles.css, app.js, img/)
src-tauri/src/
  main.rs               Tauri commands and app state
  api.rs                LiquidBounce API (builds, launch manifest, changelog)
  installer.rs          install/update pipeline and saved launch plan
  minecraft.rs          Mojang/Fabric profiles, rules, libraries
  java.rs               Java detection and download
  download_page.rs      official LiquidBounce download page window
  launch.rs             starts the game and streams its logs
  net.rs                downloads with retries and checksum checks
  errors.rs             turns errors into plain-English messages
  settings.rs           settings file
```

## Notes

- This is a **community launcher** and is not affiliated with CCBlueX. LiquidBounce, its logo and its artwork belong to CCBlueX. The icon and background are taken from the GPL-3.0 licensed LiquidLauncher repository.
- Version 0.1 uses **offline accounts**. Microsoft sign-in is the natural next step if you want to play on servers that require a premium account.
