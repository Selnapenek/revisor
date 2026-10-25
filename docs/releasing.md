# Releasing

A tag like `v1.2.3` starts [the release workflow](../.gitea/workflows/release.yml) on Gitea Actions. It builds the macOS, Fedora, and Arch packages, uploads them to a draft release, and publishes the release when all uploads are done.

## Make a release

```sh
./scripts/release.sh 1.2.3
git push origin HEAD v1.2.3
```

`release.sh` sets the version in both crates, updates `Cargo.lock`, commits, and tags. The workflow stops if the tag and the `revisor-app` version are different. The release notes are the commit subjects since the previous tag.

If a run fails, delete the draft release and the tag in Gitea, fix the cause, then tag again.

## One-time setup

1. Let anonymous users read the releases, because the app checks them without a sign-in:
   - In Gitea `app.ini`, set `REQUIRE_SIGNIN_VIEW = false` in `[service]`. Restart Gitea.
   - Make the `slel/revisor` repository public.
   - Check: `curl https://gitea.slel.pro/api/v1/repos/slel/revisor/releases/latest` must not return 403.
2. Enable Actions for the repository (**Settings > Repository > Actions**).
3. Register two [act_runner](https://docs.gitea.com/usage/actions/act-runner) runners:
   - An Apple Silicon Mac in host mode with the label `macos:host`. Install Rust with rustup and the Xcode command line tools.
   - A Linux runner with Docker and the label `ubuntu-latest`. The default act_runner labels are correct.
4. In **Settings > Actions > Secrets**, add `REVISOR_GITHUB_CLIENT_ID`. It is baked into the packages.
5. Optional: in **Settings > Actions > Variables**, set `REVISOR_SIGNING_IDENTITY` to sign the macOS app with a Developer ID certificate on the Mac runner.

## Release assets

| File | Use |
| --- | --- |
| `Revisor-<version>.dmg` | First install on macOS |
| `Revisor-<version>-macos-arm64.tar.gz` | In-app update on macOS |
| `revisor-<version>-1.fc43.x86_64.rpm` | First install and in-app update on Fedora |
| `revisor-<version>-1-x86_64.pkg.tar.zst` | First install and in-app update on Arch and CachyOS |

The in-app updater finds its package by the end of the file name. Do not rename these files.

## In-app updates

Revisor reads `/api/v1/repos/slel/revisor/releases/latest` when it starts and every 6 hours. **Revisor > Check for Updates…** checks now. A newer version opens a prompt one time and shows **Update to x.y.z** in the title bar. **Update and restart** downloads the package, installs it, and starts the new version:

- macOS: the updater replaces `Revisor.app` where it is installed. The folder must be writable, which is usual for `/Applications`.
- Fedora: `pkexec dnf install` asks for the system password, then upgrades the RPM.
- Arch and CachyOS: `pkexec pacman -U` asks for the system password, then upgrades the package.

Updates work only in the packaged app, not in `cargo run`. Drafts and pre-releases are not offered.
