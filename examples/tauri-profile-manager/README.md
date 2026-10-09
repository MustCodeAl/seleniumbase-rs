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

### Authentication

Every endpoint requires this run's API token as a bearer token:

```
Authorization: Bearer <token>
```

The token is 256 bits of OS randomness, minted fresh on each start and never
written to disk. The app window fetches it over Tauri IPC (`get_api_token`),
which is reachable only from the app's own frontend.

This matters more than it first looks. Binding to `127.0.0.1` keeps the API off
the network, but it does **not** keep it away from the browser: any page the
user has open can also reach loopback. Without a token, a visited web page could
call `GET /api/v1/profiles` and read every saved profile — cookies and proxy
credentials included — or call `GET /api/v1/profiles/{id}/start?url=...` to
drive a logged-in browser session to a URL of its choosing. Because that one is
a plain GET, an `<img>` tag would be enough.

Two further limits back the token up:

- **CORS is restricted to the app window's own origins.** A request carrying
  any other `Origin` header is refused by the server before it reaches a
  handler, and a foreign page's preflight is refused too, so it cannot even
  attach the `Authorization` header. Tools with no `Origin` header, such as
  `curl`, are unaffected.
- **The `Host` header must name a loopback address.** This blocks DNS
  rebinding, where an attacker points a hostname they control at `127.0.0.1` to
  reach the API from a page they serve.

Requests failing either check are refused with `403 FORBIDDEN_HOST`, and a
missing or wrong token with `401 UNAUTHORIZED`.

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
# Copy the token from the app window, or read it from the running process.
TOKEN='<token from get_api_token>'

curl http://127.0.0.1:45001/api/v1/profiles \
  -H "authorization: Bearer $TOKEN"

curl -X POST http://127.0.0.1:45001/api/v1/profiles \
  -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"name":"EU Proxy","container_url":"http://localhost:4444","proxy":"http://proxy:8080"}'
```

Without the header the same request returns `401`:

```bash
curl -i http://127.0.0.1:45001/api/v1/profiles
# HTTP/1.1 401 Unauthorized
```

## Known gaps

This is an example, not a finished product. Before relying on it:

- **Profiles are stored in clear text.** `profiles.json` in the OS app-data
  directory holds cookies and any credentials embedded in proxy URLs. Encrypt
  it at rest, or keep secrets in the platform keychain.
- **The `2fa` and `user` endpoints are stubs.** They return fixed placeholder
  values and authenticate nobody.
- **The window runs with `"csp": null`.** Setting a real Content-Security-Policy
  in `tauri.conf.json` limits what injected content could do.

## Adding anti-detect hardening

To move closer to commercial-grade anti-detection:

1. Replace `selenium/standalone-chrome` with a custom Dockerfile that patches
   `cdc_` markers and injects anti-fingerprint extensions.
2. Use `DriverMode::Uc` in profiles to enable undetected-chrome args.
3. Add proxy assignment per profile and route containers through it.
4. Store cookies/cache per profile in `data/<profile>` and mount them into the
   container.

