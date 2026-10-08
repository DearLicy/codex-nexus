# Codex Nexus architecture

## Goals

Codex Nexus is a native desktop companion with a local-first boundary. The Slint window presents state and user actions, the Rust core owns privileged local capabilities, and integrations are explicit and replaceable. A feature should remain useful with no network connection unless it specifically needs a remote service.

## Runtime layers

```text
┌─────────────────────────────────────────────┐
│ Slint UI (`native/ui/`)                     │
│ native window, view state, user actions     │
└──────────────────────┬──────────────────────┘
                       │ typed Rust callbacks
┌──────────────────────▼──────────────────────┐
│ Native host (`native/src/`)                  │
│ Slint lifecycle, callbacks, platform window  │
└───────────────┬──────────────────┬───────────┘
                │                  │
┌───────────────▼──────────────┐ ┌─▼───────────┐
│ Local services and storage   │ │ Integrations│
│ filesystem, keychain, state │ │ optional net│
└──────────────────────────────┘ └──────────────┘
```

The optional Node sidecars in `packages/` sit behind this boundary. The
loopback gateway aggregates configured provider routes, while the image MCP
service exposes image operations over MCP stdio and stores returned artifacts
in the configured local directory. They receive a generated local manifest or
loopback URL; provider credentials are not hard-coded in either package.

### Slint UI

`native/ui/` owns rendering, interaction state, and user-facing validation. It should not access native files, process APIs, or credentials directly. Calls into the desktop layer use narrow typed Slint callbacks. Loading, success, and error states should be explicit so a disconnected or partially configured integration is understandable.

### Native host and Rust core

`native/src/` is the desktop host and `src-tauri/` is the reusable Rust core. Callbacks should accept validated input and return typed results or safe, user-actionable errors. Keep the host layer small; put filesystem, process, persistence, and integration work in testable Rust modules. Do not log tokens, cookies, full request bodies, or private file contents.

### State and persistence

User preferences and cached metadata should be stored locally using a versioned schema. Reads should tolerate an absent or older store; writes should be atomic where practical. If a migration is needed, make it idempotent and preserve a backup or recoverable path before replacing user data. Secrets belong in the operating system keychain or an equivalent secure store, not in the ordinary application state file.

### Integrations

Network integrations are optional adapters behind a small interface. They must declare what data leaves the device, use bounded timeouts, surface authentication failures clearly, and avoid sending local content by default. Integration code should be replaceable without changing the UI contract; mock adapters make offline tests deterministic.

## Data flow

1. The Slint UI validates the user action and invokes one narrow callback.
2. The native host validates again at the trust boundary and selects a local service or integration adapter.
3. The service reads local state or performs an explicitly requested request.
4. The Rust function returns a typed result or a categorized error.
5. The Slint UI updates its view state and presents the outcome without exposing internal paths or secrets.

Events are for state changes that may occur outside a single request (for example, a background refresh). They should be scoped, cancellable where possible, and de-duplicated before reaching the UI.

## Security boundaries

- Treat Slint UI input, imported files, and remote responses as untrusted.
- Keep native callbacks allowlisted and minimize filesystem scope.
- Store credentials in the system keychain; redact values before logging.
- Use HTTPS for remote integrations and fail closed on certificate or origin errors.
- Do not silently upload local workspace content.

## Build and release

The native application is built with the Rust manifests in `native/` and `src-tauri/`; Slint compiles the UI into the desktop binary. CI runs the Rust core and native host checks independently so a failure identifies its layer. Release automation should pin toolchain versions, generate reproducible artifacts where supported, and publish checksums alongside installers.

## Current core modules

The Rust core currently keeps these responsibilities separate:

- `models.rs` defines provider accounts, route policy, job state, and artifact records. Credential fields are references to a secret, never the secret material itself.
- `routing.rs` implements single, round-robin, weighted, priority, random, quota-aware and fallback selection, sticky-session bindings, cooldowns, and bounded retry decisions without blocking the UI thread.
- `artifacts.rs` scans job artifacts within a configured root, never follows symlinks, hashes safe files, marks unusual files for review, and only moves them into the recoverable quarantine after an explicit user action.
- `image_pool.rs` keeps image concurrency independent from text-session affinity and assigns queued jobs to the member with the fewest in-flight requests.
- `store.rs` provides versioned SQLite metadata tables; `secrets.rs` encrypts values with an envelope key supplied by the platform keychain boundary.

## Gateway and image fallback

`packages/gateway/server.mjs` is a loopback-only aggregation surface. It
strictly validates `namespace/model`, resolves an account using pool policy and
session affinity, forwards Responses, Chat Completions and image requests, and
emits routing metadata without request bodies. The Rust control plane should
generate its manifest and API key at runtime.

`packages/image-mcp/server.mjs` is the fallback for runtimes that cannot expose
native image generation. It implements the MCP initialize/tools handshake and
four image tools, saving base64 results to the managed image directory while
returning MCP image content for inline display. Account qualification and
provider capability flags remain hard requirements; the fallback does not
grant image access to an account that lacks it.

The native host exposes these modules through typed Slint callbacks without coupling their domain logic to a WebView or browser framework.
