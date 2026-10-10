# Revisor

Fast, offline-capable code review for GitHub repositories.

## Run

Native app built with [gpui-kit](https://gpui-kit.com):

```sh
cargo run -p revisor-app
```

Tests: `cargo test -p revisor-core`.

The old Tauri + SolidJS app (`src/`, `src-tauri/`) still builds with `pnpm tauri dev` until the native app reaches parity.

## GitHub sign-in

Revisor uses GitHub OAuth device flow. See [GitHub OAuth setup](docs/github-oauth.md) to register the OAuth app and set `REVISOR_GITHUB_CLIENT_ID` in `.env`. The client ID is baked in at build time.
