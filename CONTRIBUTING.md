# Contributing to the Yukkit Launcher

Thanks for helping make the launcher better. A clear bug report or a small, focused pull request is worth more than a big one nobody asked for.

## Before you start

- Read the [README](README.md), the [Code of Conduct](CODE_OF_CONDUCT.md) and the [MIT License](LICENSE).
- Check the existing [issues](https://github.com/YukkitMC/yukkit-laucher/issues) and [pull requests](https://github.com/YukkitMC/yukkit-laucher/pulls) so you don't duplicate work.
- For bigger changes, open an issue first and describe what you want to do.
- Never post a security problem in a public issue. Follow [SECURITY.md](SECURITY.md).

Problems *inside the game* (a module that doesn't work, a crash caused by the mod) belong in [Yukkit Toolkit's issues](https://github.com/YukkitMC/yukkit-toolkit/issues). Problems with installing, updating, starting or the launcher window belong here.

## Reporting a bug

Please include:

1. What happened, and what you expected to happen.
2. Steps that reproduce it.
3. Launcher version (bottom left of the window), Windows version, and the game version you picked.
4. The **Logs** page contents, or the text from **Details → Copy** on the error. Remove your username or file paths if you'd rather keep them private.

## Suggesting a feature

Explain the problem it solves for players and how you'd expect it to work. A clear use case matters more than a long feature list.

## Adding a Minecraft version

New versions only need a change to [`versions.json`](versions.json). See *Adding a Minecraft version* in the README. Open a pull request once a Yukkit Toolkit release for that version exists.

## Working on the code

```bash
npm install
npm run dev
```

- **Interface:** `ui/` is plain HTML, CSS and JavaScript. Opening `ui/index.html` in a browser shows a preview with sample data.
- **Backend:** `src-tauri/src/`. Run `cargo fmt` and `cargo clippy` in `src-tauri/` before opening a pull request.
- **Wording:** write messages for players, not developers. Say what happened and what to do next in plain English. Put technical details in the error's `details`, not its message.
- **Safety:** keep downloads checksum-verified wherever the source gives a hash. Never add code that sends account data, files or telemetry anywhere without the player choosing it.

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Keep each pull request about one thing.
3. Describe what changed and how you tested it. Screenshots help for UI changes.
4. Make sure the **Build** workflow passes.

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
