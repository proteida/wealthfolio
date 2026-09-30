# Runtime env file (`wealthfolio.env`)

Change `WF_*` / `CONNECT_*` runtime configuration without rebuilding the app or
opening a terminal: edit one user-owned file, then restart the app.

## Where the file lives

First existing candidate wins:

| #   | Location                                                                      | Notes                                                                                                                                                                                                                                                                               |
| --- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `$WF_ENV_FILE`                                                                | Explicit override. A missing path is a startup error.                                                                                                                                                                                                                               |
| 2   | `~/Library/Application Support/com.teymz.wealthfolio/wealthfolio.env` (macOS) | **Primary location.** Outside the `.app` bundle, so it **survives app updates**. (`$WF_APP_SUPPORT_DIR` overrides the directory.) Linux: `$XDG_CONFIG_HOME/com.teymz.wealthfolio/wealthfolio.env` (or `~/.config/...`). Windows: `%APPDATA%\com.teymz.wealthfolio\wealthfolio.env`. |
| 3   | `wealthfolio.env` next to the executable                                      | Sidecar fallback for portable/dev layouts. Inside the signed `.app` bundle on macOS — do not rely on it there, updates wipe it.                                                                                                                                                     |

Why not the alternatives: a file next to the macOS binary sits inside the signed
bundle (wiped on update, breaks the signature), and a launchd plist
`EnvironmentVariables` entry only applies to launchd-managed daemons, not GUI
apps started from Finder.

A starting template with no secrets ships as
`packaging/macos/wealthfolio.env.example`. Copy it to location #2 and uncomment
what you need. Keep the real file readable only by you (`chmod 600`); never
commit it.

## Format

`.env`-style, one `KEY=VALUE` per line:

```dotenv
# Full-line comments only. `#` inside a value is literal.
WF_LISTEN_ADDR=127.0.0.1:8088
CONNECT_API_URL=https://api.wealthfolio.app
export WF_REQUEST_TIMEOUT_MS=300000
WF_QUOTED="value with spaces, = and # kept"
WF_SINGLE='literal, no escapes'
WF_EMPTY=
```

Only managed keys are applied: `WF_*`, `CONNECT_*`, and `RUST_LOG`. Anything
else is ignored (key name logged at debug). No variable expansion; double quotes
support `\n \r \t \\ \"`, single quotes are literal. The last occurrence of a
repeated key wins.

## Precedence (highest first)

| Priority | Source                                                                   |
| -------- | ------------------------------------------------------------------------ |
| 1        | Process environment (launchd, shell, Docker `-e`, …) — even when empty   |
| 2        | CWD `.env` (existing `dotenvy` behavior, Docker/dev)                     |
| 3        | `wealthfolio.env` (this file)                                            |
| 4        | Compile-time baked values (`option_env!("CONNECT_*")` in desktop builds) |
| 5        | Built-in defaults                                                        |

Desktop note: `CONNECT_AUTH_URL` / `CONNECT_AUTH_PUBLISHABLE_KEY` /
`CONNECT_API_URL` used to be bake-time-only (`option_env!`); the file (or
process env) now overrides them at runtime, so Connect endpoints can move
without a rebuild.

## Runtime Connect endpoints (no rebuild)

The browser client fetches `GET /api/v1/client-config` at boot. Set any of
these in `wealthfolio.env` (each also honors its bare `CONNECT_*` twin, with
`WF_CONNECT_*` winning when both are set):

| Variable | Effect |
| -------- | ------ |
| `WF_CONNECT_AUTH_URL` | Supabase-compatible auth host for login |
| `WF_CONNECT_AUTH_PUBLISHABLE_KEY` | Auth publishable key (public by design) |
| `WF_CONNECT_API_URL` | Connector/cloud data API base |
| `WF_CONNECT_OAUTH_CALLBACK_URL` | Hosted OAuth bounce page |

Setting auth URL + key switches Connect on even in a build that baked nothing
in (the "Not Configured" screen disappears after the fetch). Anything unset
falls back to the baked-in build value, then to the cloud defaults. The
endpoint is public (the login screen needs it pre-auth) and serves no secrets.
Custom hosts are automatically added to the CSP `connect-src` allowlist.

## Restart requirement

Values are read once at startup. **Edit the file, then fully quit and relaunch
the app** (desktop) or restart the server process — there is no file watching.

## Failure modes

- Missing file: fine, process env and defaults apply.
- Malformed line: startup aborts with `path:line` (values never echoed), e.g.
  `wealthfolio.env:7: malformed line (expected KEY=VALUE)`.
- Resolved sources are logged per key at debug level (keys only, never values).
  On the server, `RUST_LOG=debug` (itself settable via the file) shows lines
  like `runtime-env: 'WF_LISTEN_ADDR' resolved from file`.

Server key reference (ports, secrets, auth, OIDC): `docs/self-host/README.md`.
