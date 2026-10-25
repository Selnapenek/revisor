# Releasing

A tag like `v1.2.3` starts [the release workflow](../.github/workflows/release.yml) on GitHub Actions. It builds the macOS, Fedora, and Arch packages. When all builds pass, it publishes a [GitHub release](https://github.com/Selnapenek/revisor/releases) with all packages.

## Make a release

```sh
./scripts/release.sh 1.2.3
git push origin HEAD v1.2.3
```

`release.sh` sets the version in both crates, updates `Cargo.lock`, commits, and tags. The workflow stops if the tag and the `revisor-app` version are different. The release notes are the commit subjects since the previous tag.

If a run fails, delete the tag (`git push origin :v1.2.3` and `git tag -d v1.2.3`), fix the cause, then tag again. The workflow makes the release only at the end, so a failed run leaves no release.

## One-time setup

1. In **Settings > Secrets and variables > Actions > Secrets**, add `REVISOR_GITHUB_CLIENT_ID`. It is baked into the packages.
2. Optional: in **Variables**, set `REVISOR_SIGNING_IDENTITY` to sign the macOS app with a Developer ID certificate. GitHub-hosted runners do not have your certificate, so this needs a step that imports it. Without it, the app has an ad hoc signature.

The macOS build runs on the GitHub-hosted `macos-15` runner (Apple Silicon). The Linux builds run in `fedora:43` and `archlinux:latest` containers on `ubuntu-latest`. Public repositories do not pay for these runners.

## Release assets

| File | Use |
| --- | --- |
| `Revisor-<version>.dmg` | First install on macOS |
| `Revisor-<version>-macos-arm64.tar.gz` | In-app update on macOS |
| `revisor-<version>-1.fc43.x86_64.rpm` | First install and in-app update on Fedora |
| `revisor-<version>-1-x86_64.pkg.tar.zst` | First install and in-app update on Arch and CachyOS |

The in-app updater finds its package by the end of the file name. Do not rename these files.

## In-app updates

Revisor reads `https://api.github.com/repos/Selnapenek/revisor/releases/latest` when it starts and every 6 hours. **Revisor > Check for Updates…** checks now. A newer version opens a prompt one time and shows **Update to x.y.z** in the title bar. **Update and restart** downloads the package, installs it, and starts the new version:

- macOS: the updater replaces `Revisor.app` where it is installed. The folder must be writable, which is usual for `/Applications`.
- Fedora: `pkexec dnf install` asks for the system password, then upgrades the RPM.
- Arch and CachyOS: `pkexec pacman -U` asks for the system password, then upgrades the package.

Updates work only in the packaged app, not in `cargo run`. Drafts and pre-releases are not offered.
