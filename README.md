# Modkist

Modkist is a desktop mod manager for Zeepkist. It installs mods from [mod.io](https://mod.io), manages BepInEx, and supports local sideload files.

The app uses Tauri 2, Nuxt 4 (Vue), and Rust.

## Features

- Browse, install, and update mods from mod.io.
- Install and manage BepInEx in the Zeepkist game directory.
- Switch mod profiles.
- Sideload local `.dll`, `.zeeplevel`, and `.zip` files.
- Detect Zeepkist from Steam or from a Wine prefix.
- Launch Zeepkist from the app.

## Requirements

You need:

- Node.js 20 or later
- A stable Rust toolchain
- A mod.io game API key and game ID for Zeepkist
- Zeepkist installed (Steam app ID `1440670`)

On Linux, install the system packages that Tauri needs for your distribution. See the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## Get started

1. Clone this repository.
2. Copy `.env.example` to `.env`.
3. Set `MODIO_API_KEY` and `MODIO_GAME_ID` in `.env`.
4. Run `npm install`.
5. Run `npm run tauri dev`.

The app window opens. If the setup page asks for a path, select your Zeepkist directory. If you need subscriptions or account features, sign in with mod.io.

## Configuration

Modkist reads these values from `.env` or from the environment:

| Variable | Required | Purpose |
| --- | --- | --- |
| `MODIO_API_KEY` | Yes | Game API key from the mod.io dashboard |
| `MODIO_GAME_ID` | Yes | Numeric game ID from the mod.io dashboard |
| `MODIO_API_HOST` | No | Custom API host from the game dashboard |
| `MODIO_USE_TEST_ENV` | No | Set to `true` to use `api.test.mod.io` |
| `RUST_LOG` | No | Rust log level for `tauri dev` |
| `SENTRY_DSN` | No | Sentry DSN for error reports |

Use the game API key from your game dashboard on mod.io. Do not use the personal read-only key from https://mod.io/me/access.

Release builds embed `MODIO_API_KEY`, `MODIO_GAME_ID`, and `SENTRY_DSN` at compile time. Dev mode reads `.env` at runtime.

## Scripts

| Command | Action |
| --- | --- |
| `npm run tauri dev` | Run the desktop app in development mode |
| `npm run tauri build` | Build installers for the current platform |
| `npm run typecheck` | Run the TypeScript type check |

## CI builds

GitHub Actions has two workflows:

- **Build** — manual builds for macOS, Windows, and Linux. Bundles appear as workflow artifacts.
- **Release** — version bump, git tag, GitHub release, and installer upload.

Both workflows need the repository secrets `MODIO_API_KEY` and `MODIO_GAME_ID`. `SENTRY_DSN` is optional.
