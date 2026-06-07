# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

LiteCord is a privacy-focused, lightweight Discord alternative. This repository is the Rust backend. The frontend lives in a separate repository.

## Commands

```bash
cargo build          # Compile
cargo run            # Run in dev mode
cargo build --release && cargo run --release  # Production
cargo check          # Type-check without building
cargo clippy         # Lint
```

Tests use SurrealDB in-memory (`kv-mem` feature) — no external instance required.

```bash
cargo test                        # Run all tests
cargo test messages_test          # Run a specific integration test file
cargo test --lib                  # Run only unit tests (jwt, hashing)
```

Integration tests live in `tests/`, unit tests are inline `#[cfg(test)]` blocks. The `tests/common/mod.rs` module provides `setup_db()` (spins up an isolated in-memory DB with the schema applied) and `create_test_user()`. Requires a `.env` file for production runs (see `.env.example`).

## Environment Setup

Copy `.env.example` to `.env`. Required variables:

| Variable | Description |
|---|---|
| `ROCKET_DB_URL` | SurrealDB address (e.g. `localhost:8000`) |
| `ROCKET_DB_USER` / `ROCKET_DB_PASSWORD` | SurrealDB credentials |
| `ROCKET_DB_CONFIG_FILE` | Path to schema file (`db.surql`) |
| `ROCKET_JWT_SECRET` | HS384 signing key |
| `ROCKET_AES_KEY` | AES-256-GCM key (exactly 32 bytes) |
| `ROCKET_TOKEN_EXPIRATION_SECONDS` | Access token TTL |
| `ROCKET_REFRESH_TOKEN_EXPIRATION_SECONDS` | Refresh token TTL |

The database schema is applied automatically from `db.surql` on every startup.

## Architecture

**Framework:** Rocket 0.5 (async) — **Database:** SurrealDB 2.4 via WebSocket — **Auth:** JWT (HS384) + Argon2 + AES-256-GCM

### Module Layout

```
src/
  main.rs                    # Rocket launch, route mounting, global state
  lib.rs                     # Module declarations, env config (OnceLock), DB init
  error.rs                   # Shared error type
  jwt.rs                     # Token encode/decode/encrypt
  hashing.rs                 # Argon2 password ops
  models/
    db.rs                    # SurrealDB entity structs (all IDs as Option<Thing>)
    user.rs                  # Request/response models + AuthenticatedUser guard
  routes/
    auth_routes.rs            # POST /auth/signup|login|refresh, GET /auth/me
    channels_routes.rs        # GET /channels/list_dm
    friends_routes.rs         # POST /friends/add_friend|update_friend_request|list_friends
    guilds_routes.rs          # POST|GET /guilds, DELETE /guilds/<id>, POST /guilds/<id>/leave|invites, POST /guilds/join/<code>
    websockets_routes.rs      # GET /ws/
  users/
    auth.rs                  # signup(), signin(), refresh_token(), get_my_info()
  channels.rs                # list_channels_for_user()
  friends.rs                 # add_friend(), update_friend_request(), list_friends()
  guilds.rs                  # create_guild(), delete_guild(), leave_guild(), list_user_guilds(), create_invite(), join_guild()
  chat/
    hub.rs                   # ChatHub — in-memory connection registry + message routing
    types.rs                 # ChatMessage, ServerMessage, AuthMessage, RefreshMessage
```

### Request Lifecycle

HTTP requests hit Rocket route handlers in `src/routes/`, which call business logic in `src/users/`, `src/friends.rs`, or `src/channels.rs`. The SurrealDB connection (`db: State<Db>`) and the chat hub (`hub: State<Arc<Mutex<ChatHub>>>`) are injected as Rocket managed state.

### Authentication Guard

`AuthenticatedUser` in `src/models/user.rs` implements Rocket's `FromRequest`. It extracts and validates the `Authorization: Bearer <token>` header and forwards `user_id` + `token` to handlers. Routes requiring auth accept `user: AuthenticatedUser`.

### WebSocket Lifecycle (`src/routes/websockets_routes.rs`)

1. Client connects → `authenticate()` waits for an `AuthMessage` (JSON with `token`)
2. `register_user()` adds the user to `ChatHub.connections` (a `HashMap<String, Sender<String>>`)
3. `run_message_loop()` runs a `tokio::select!` over:
   - Incoming WS frames → parsed as `ChatMessage`, routed via `ChatHub::send_to()`
   - Broadcast channel → forward to client
   - Auth check timer (every 300 s) → if expired, requests token refresh from client
4. `disconnect_user()` removes entry on close

### ChatHub Routing (`src/chat/hub.rs`)

`send_to()` parses the target string as `<type>:<id>` and dispatches to:
- `send_to_user()` — DM between two users (creates `DMChannel` if needed)
- `send_to_dm_channel()` — broadcast to all channel recipients (with sender authorization check)
- `send_to_channel()` — guild channel (stub, not yet implemented)

`DMChannel` deduplication: recipients are sorted and joined into a `recipients_key`; a UNIQUE index on that field prevents duplicates.

### Database Schema (`db.surql`)

Key tables: `user`, `guild`, `channel`, `DMChannel`, `message`, `refresh_token`, `role`, `guild_invite`, `emoji`, `member_of` (RELATION), `friendship` (RELATION).

- All foreign keys are SurrealDB `Thing` record links.
- `member_of` and `friendship` are edge tables (`RELATION IN user OUT guild/user`).
- `friendship.status` values: `pending`, `accepted`, `rejected`.
- `refresh_token` stores the JWT encrypted with AES-256-GCM (nonce prepended, base64 encoded).

## Development Methodology

### The ONE Thing
Before implementing any feature, identify the single thing that makes everything else easier. Ask: *"What can I do such that by doing it, everything else becomes easier or unnecessary?"* Apply this to prioritize features — foundational horizontal concerns (e.g., message persistence) before vertical features that depend on them.

### TDD
Write tests before or alongside each new function. The test suite uses SurrealDB in-memory so tests are self-contained and fast. For each new business-logic function:
1. Write a failing test that specifies the expected behavior
2. Implement the function until the test passes
3. Ensure all existing tests still pass

New HTTP routes don't need Rocket integration tests — test the underlying business logic function directly (the route is just a thin wrapper).

## Code Conventions

- No inline comments; standard comments only for genuinely complex logic
- Separation of concerns: routes only handle HTTP plumbing, business logic lives in dedicated modules
- SurrealDB entity structs use `Option<Thing>` for IDs (absent on insert, present on read)
- Relation fields use `#[serde(rename = "in"/"out")]`

## Feature Implementation Status

| Feature | Status |
|---|---|
| Signup / Login | Done |
| JWT access + refresh tokens | Done |
| WebSocket real-time messaging | Done |
| Token auto-refresh over WebSocket | Done |
| DM channels (user ↔ user) | Done |
| Friend requests (add / accept / reject / list) | Done |
| Guild system (create / join via invite / leave / delete) | Done |
| Guild channels (text / voice) — create / list / delete + messagerie temps réel | Done |
| Role management (create / delete / assign / remove + `check_permission`) | Done |
| Message persistence + history (`GET /channels/<id>/messages`) | Done |
| Audio channels (type defined, calls not in scope yet) | Schema only |
