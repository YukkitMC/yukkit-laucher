# Security Policy

The launcher downloads and runs software on players' computers, so security reports matter a lot to us. Please report problems responsibly.

## Supported versions

| Version | Security fixes |
| :--- | :--- |
| Latest release | Yes |
| Older releases | No, please update |
| `main` branch | Best-effort review |

## Reporting a vulnerability

**Don't post exploit steps, proof-of-concept code or sensitive details in a public issue.**

1. Use GitHub's **Private vulnerability reporting** under this repository's [Security tab](https://github.com/YukkitMC/yukkit-laucher/security), if it's available.
2. If it isn't, contact a YukkitMC maintainer through an **official, private YukkitMC channel** before sharing any details.
3. Include the launcher version or commit, what an attacker could do, steps to reproduce and, if needed, a safe proof of concept.
4. Remove passwords, tokens and unrelated personal data first.

We'll try to confirm the problem, fix it, ship a new release and then coordinate disclosure with you. There's no bug bounty and no guaranteed response time.

## What to report

For example:

- Downloads that skip checksum checks, or files that can be swapped on the way (man-in-the-middle).
- Path traversal: a release, version file or mod that can write outside the install folder.
- Running programs or arguments the player didn't choose.
- Leaking personal data, tokens or files.
- Problems in the build or release pipeline that could ship a tampered `.exe`.

Anti-cheat detections, server bans and feature requests are not security vulnerabilities.

## Official downloads

Only download the launcher from this repository's [Releases page](https://github.com/YukkitMC/yukkit-laucher/releases). Treat copies from anywhere else as untrusted. The launcher itself only installs Yukkit Toolkit from [YukkitMC/yukkit-toolkit releases](https://github.com/YukkitMC/yukkit-toolkit/releases).
