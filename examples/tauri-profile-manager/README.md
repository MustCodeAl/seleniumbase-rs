# Tauri Profile Manager

A desktop multi-profile browser manager built with [Tauri](https://tauri.app) and `seleniumbase-rs`. Each profile connects to a dedicated Docker browser container so sessions stay isolated.

## Architecture

- **Tauri desktop app** — Rust backend + vanilla HTML/JS frontend.
- **Profile store** — JSON file saved in the OS app-data directory.
- **Browser containers** — three `selenium/standalone-chrome` containers, each with its own WebDriver port and persistent profile directory.
- **Automation engine** — `seleniumbase-rs::BaseCase` connects to each container with per-profile `user_agent`, `proxy`, `locale`, and `headless` settings.

## Run the browser grid

```bash
docker compose up -d
```

This exposes:

| Container | WebDriver | VNC (noVNC) |
|-----------|-----------|-------------|
| browser-a | http://localhost:4444 | http://localhost:7900 |
| browser-b | http://localhost:4445 | http://localhost:7901 |
| browser-c | http://localhost:4446 | http://localhost:7902 |

VNC password is `secret` by default.

## Start the Tauri app

```bash
cd src-tauri
cargo tauri dev
```

## Build a release bundle

```bash
cd src-tauri
cargo tauri build
```

## Features

- Create / delete isolated browser profiles.
- Launch a profile against any WebDriver container.
- Navigate, screenshot, and close sessions from the UI.
- Per-profile fingerprint hints: user agent, locale, proxy, headless.
- Per-profile geolocation override via CDP `Emulation.setGeolocationOverride`.
- Tags and folders for organizing profiles.
- A local profile-compatible REST API (`http://127.0.0.1:45001/api/v1`) with CORS enabled.
- Ready for stealth/CDP/UC mode via `DriverMode` in `BrowserConfig`.
- UI tools for cloning, exporting/importing, proxy validation, and cookie management.

## Profile-compatible REST API

The Tauri backend starts an Actix-web server on `http://127.0.0.1:45001`. The UI uses it for tags/folders and profile tools, and external tools can call it directly.

Real endpoints:

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/version` | Launcher version |
| GET | `/api/v1/status` | Active sessions |
| GET | `/api/v1/profiles` | List profiles |
| POST | `/api/v1/profiles` | Create profile |
| GET | `/api/v1/profiles/:id` | Get profile |
| POST | `/api/v1/profiles/:id` | Update profile |
| DELETE | `/api/v1/profiles/:id` | Delete profile |
| GET | `/api/v1/profiles/:id/start?url=...` | Launch profile |
| GET | `/api/v1/profiles/:id/stop` | Stop profile session |
| POST | `/api/v1/profiles/:id/clone` | Clone profile |
| GET | `/api/v1/profiles/:id/export` | Export profile JSON |
| POST | `/api/v1/profiles/import` | Import profile JSON |
| POST | `/api/v1/cookie_import` | Import cookies into profile/session |
| POST | `/api/v1/cookie_export` | Export stored cookies |
| POST | `/api/v1/proxy/validate` | Validate proxy via ipinfo.io |
| GET/POST | `/api/v1/tags` | List / create tags |
| POST/DELETE | `/api/v1/tags/:id` | Update / delete tag |
| GET/POST | `/api/v1/folders` | List / create folders |
| POST/DELETE | `/api/v1/folders/:id` | Update / delete folder |
| GET | `/api/v1/stop_all` | Stop every active session |

Stub endpoints (return placeholder data):

- `/api/v1/browser_cores`, `/api/v1/load_browser_core`, `/api/v1/delete_browser_core`
- `/api/v1/workspaces`
- `/api/v1/user/signin`, `/api/v1/user/refresh_token`
- `/api/v1/bookmarks/export`, `/api/v1/bookmarks/import`
- `/api/v1/2fa/setup`, `/api/v1/2fa/enable`

Example:

```bash
curl http://127.0.0.1:45001/api/v1/profiles
curl -X POST http://127.0.0.1:45001/api/v1/profiles \
  -H 'content-type: application/json' \
  -d '{"name":"EU Proxy","container_url":"http://localhost:4444","proxy":"http://proxy:8080"}'
```

## Adding anti-detect hardening

To move closer to commercial-grade anti-detection:

1. Replace `selenium/standalone-chrome` with a custom Dockerfile that patches
   `cdc_` markers and injects anti-fingerprint extensions.
2. Use `DriverMode::Uc` in profiles to enable undetected-chrome args.
3. Add proxy assignment per profile and route containers through it.
4. Store cookies/cache per profile in `data/<profile>` and mount them into the
   container.

