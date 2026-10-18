# Revisor

Fast, offline-capable code review for GitHub repositories.

## Roadmap

See the [product roadmap](docs/roadmap.md) for current priorities, progress, release criteria, and the next task.

## Run

Native app built with [gpui-kit](https://gpui-kit.com):

```sh
cargo run -p revisor-app
```

Tests: `cargo test -p revisor-core`.

## GitHub sign-in

Revisor uses GitHub OAuth device flow. See [GitHub OAuth setup](docs/github-oauth.md) to register the OAuth app and set `REVISOR_GITHUB_CLIENT_ID` in `.env`. The client ID is baked in at build time.
