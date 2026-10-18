# GitHub OAuth setup

Revisor uses GitHub OAuth device flow by default. Users can also choose to enter a personal access token. Revisor keeps each credential in the operating system keyring. The OAuth client ID is public. Do not add a client secret to Revisor.

## Register the OAuth app

1. In GitHub, create an OAuth app under **Settings → Developer settings → OAuth Apps**.
2. Set the application name and homepage URL for Revisor.
3. Enable **Device Flow** in the app settings. A callback URL is not used by this flow.
4. Configure the app to issue non-expiring access tokens. Revisor does not yet support refresh tokens.
5. Copy the app's client ID.

Revisor requests the `repo` scope. This gives access to public and private repositories and is needed for the current repository and review features.

## Configure, run, or build Revisor

Copy `.env.example` to `.env`, then replace the example value with your client ID:

```sh
cp .env.example .env
# Edit .env and set REVISOR_GITHUB_CLIENT_ID to your client ID.
cargo run -p revisor-app
```

Use `cargo build --release -p revisor-app` to build the native app. The core crate's build script reads `.env` at compile time, and Cargo rebuilds when it changes. A shell value for `REVISOR_GITHUB_CLIENT_ID` takes precedence. The `.env` file is local and is not part of Git. Do not add a client secret to it.

The client ID is not a secret. Do not set a client secret. Users sign in by selecting **Sign in with GitHub**, then entering the code shown by Revisor at GitHub's device page. Revisor completes sign-in after approval.

## Use a personal access token instead

On the sign-in screen, select **Use a personal access token instead**. Create a GitHub personal access token that has the `repo` scope, then paste it into Revisor. Revisor validates the token and stores it in the operating system keyring. Do not put a personal token in `.env`.

If device flow is disabled, the client ID is wrong, or the app returns expiring credentials, Revisor displays an error. Enable device flow and configure non-expiring tokens in the registered OAuth app.
