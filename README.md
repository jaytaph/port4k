# Port4k

*A modern, scriptable MUD engine written in Rust.*

> Build a shared world where creators can script rooms and puzzles in a safe sandbox—playtest instantly, then go live for everyone.

---

## Table of Contents

* [Highlights](#highlights)
* [Architecture](#architecture)
* [Repository Layout](#repository-layout)
* [Quick Start](#quick-start)

    * [1) Prerequisites](#1-prerequisites)
    * [2) Configure](#2-configure)
    * [3) Database & Migrations](#3-database--migrations)
    * [4) Run the Server](#4-run-the-server)
    * [5) Web & Telnet Clients](#5-web--telnet-clients)
    * [6) Docker Compose (optional)](#6-docker-compose-optional)
* [Gameplay & Commands](#gameplay--commands)

    * [Playtest vs Live](#playtest-vs-live)
    * [Movement & Interaction](#movement--interaction)
    * [Builder Commands (bp)](#builder-commands-bp)
* [Blueprints & Lua Scripts](#blueprints--lua-scripts)

    * [Sandboxing](#sandboxing)
    * [Example Lua Snippet](#example-lua-snippet)
* [Data Model (Rooms, Exits, KV)](#data-model-rooms-exits-kv)
* [Configuration](#configuration)
* [Development](#development)
* [Roadmap](#roadmap)
* [Contributing](#contributing)
* [License](#license)

---

## Highlights

* **Rust server** with async I/O (Tokio) and clear module boundaries (`commands`, `lua`, `db`, `telnet`, `ws`, etc.).
* **Two interfaces** out of the box:

    * **Telnet** (with proper CRLF, NAWS, echo control, and ANSI colors/themes).
    * **Web client** via **WebSocket** (e.g., proxied by Caddy).
* **Blueprint (bp) system**: creators add rooms, objects, exits, and attach **Lua** scripts for behavior.
* **Safe Lua sandbox** for `on_enter` / `on_command` hooks.
* **Playtest mode**: jump into a blueprint instantly without affecting persistent world state; toggle back to **Live** at any time.
* **PostgreSQL** persistence: rooms, exits (incl. locked states), per-room KV, players, inventory, coins/XP (WIP).

## Architecture

```mermaid
flowchart LR
  subgraph Clients
    A[Telnet Client]
    B[Web Client]
  end

  subgraph Server[port4k-server]
    C[Session Manager]
    D[Commands]
    E[Lua Sandbox]
    F[Registry / Services]
    G[(DB Layer)]
  end

  subgraph Postgres
    H[(rooms / exits / kv / players)]
  end

  A -->|telnet| C
  B -->|websocket /ws| C
  C --> D
  D --> E
  D --> F
  F --> G --> H
  E --> F
```

* **Session Manager** handles telnet/WS connections and per-user session state.
* **Commands** parse and route user input (e.g., `look`, `go`, `take`, `playtest`, `@bp ...`).
* **Lua Sandbox** runs blueprint scripts under strict restrictions.
* **Registry/Services** provide DB access, configuration, and helpers.

## Repository Layout

```
port4k/
├─ port4k-core/       # Shared core types, models, utilities
├─ port4k-server/     # Server binary (telnet+ws, commands, lua sandbox, db access)
├─ site/              # Static landing page and minimal web client
├─ migrations/        # SQL migrations and seeds
├─ docker/            # Docker & Caddy examples
└─ README.md
```

> Folder names/components may evolve—see commit history and module docs for the most current structure.

## Quick Start

### 1) Prerequisites

* **Rust** (stable)
* **PostgreSQL** 13+
* **Caddy** (optional, for static site + WS reverse proxy)

### 2) Configure

Create a `.env` (or export env vars) for the server:

```bash
cp .env.example .env
# edit as needed
```

Common variables:

```env
# Postgres connection
DATABASE_URL=postgres://postgres:postgres@localhost:5432/port4k

# Server binds
PORT4K_BIND=127.0.0.1:4001      # WebSocket / HTTP endpoint
PORT4K_TELNET=0.0.0.0:4000      # Telnet endpoint

# Logging
RUST_LOG=info,port4k=debug
```

### 3) Database & Migrations

Create the database and apply SQL migrations:

```bash
createdb port4k || true
psql "$DATABASE_URL" -f migrations/0001_init.sql
# (add more migration files as they appear in the repo)
```

> Some tables you'll see include `bp_rooms`, `bp_exits`, `bp_room_kv`, plus player/account tables.

### 4) Run the Server

```bash
# from repo root
cargo run -p port4k-server
```

### 5) Web & Telnet Clients

* **Web**: If using the included Caddy config, visit: `http://localhost:4040`.
* **Telnet**: Connect to the telnet endpoint (default `4000`):

```bash
telnet localhost 4000
```

### 6) Docker Compose (optional)

A minimal stack (Postgres + server + static site via Caddy):

```yaml
version: "3.9"
services:
  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: port4k
    ports:
      - "5432:5432"
    volumes:
      - pgdata:/var/lib/postgresql/data

  server:
    build: .
    environment:
      DATABASE_URL: postgres://postgres:postgres@db:5432/port4k
      PORT4K_BIND: 0.0.0.0:4001
      PORT4K_TELNET: 0.0.0.0:4000
      RUST_LOG: info,port4k=debug
    depends_on:
      - db
    ports:
      - "4000:4000"  # telnet
      - "4001:4001"  # ws/http

  caddy:
    image: caddy:2
    volumes:
      - ./docker/Caddyfile:/etc/caddy/Caddyfile:ro
      - ./site:/port4k-site:ro
    ports:
      - "4040:4040"
    depends_on:
      - server

volumes:
  pgdata: {}
```

Example **Caddyfile** (no admin port):

```caddy
localhost:4040 {
    root * /port4k-site
    file_server

    # Proxy all websocket traffic to Rust backend
    handle_path /ws* {
        reverse_proxy 127.0.0.1:4001
    }

    header {
        @assets {
            path /assets/* *.css *.js *.png *.jpg *.svg
        }
        header @assets Cache-Control "public, max-age=31536000, immutable"
    }
}
```

## Gameplay & Commands

### Playtest vs Live

* **Playtest** lets builders jump into a blueprint without altering persistent state.
* Toggle back to **Live** and continue where you left off.

Example flow in chat/console:

```
playtest <bp-key>
...you are now inside the blueprint entry room...
playtest            # exits playtest, returns to previous live room
```

### Movement & Interaction

> (Exact commands may evolve; check in-game `help`.)

```
look
look <object>
go <north|south|east|west>
open <door>
close it
say "hello"
inventory
```

### Builder Commands (bp)

Use `@bp` to manage blueprints from inside the world (builder permissions required):

```
@bp room add <bp>:<key> "Title" "Long description..."
@bp exit add <bp>:<from_key> <dir> <bp>:<to_key> [locked]
@bp kv set <bp>:<room_key> <key> <json_value>
```

> `locked` exits are hidden/unavailable until unlocked by script or action.

## Blueprints & Lua Scripts

Blueprints define rooms, exits, objects, and attach **Lua** scripts for room events.
Common hooks:

* `on_enter(ctx)` — run when a player enters the room.
* `on_command(ctx, verb, args)` — intercept or augment player commands.

### Sandboxing

Lua runs in a restricted environment. Potentially dangerous functions (e.g., `require`, `io`, `os`, `dofile`, `loadfile`) are blocked. Scripts interact with the world through a controlled `ctx` API (e.g., `ctx:get_object_state`, `ctx:set_object_state`, `ctx:say`, etc.).

### Example Lua Snippet

```lua
-- room:<bp>:<room_key>:on_command
function(ctx, verb, args)
  if verb == "open" and (args[1] == "door" or args[1] == "the" and args[2] == "door") then
    local locked = ctx:get_object_state("blast_door", "locked")
    if locked then
      if ctx:has_item("vault_key") then
        ctx:set_object_state("blast_door", "locked", false)
        return "You unlock and open the blast door."  -- returning text echoes to player
      else
        return "The blast door is locked. You need a key."
      end
    else
      return "The blast door is already open."
    end
  end
end
```

## Data Model (Rooms, Exits, KV)

A simplified view of tables used by the blueprint system:

* `bp_rooms(bp_key, key, title, body, lockdown, short, hints, objects, scripts)`
* `bp_exits(from_room_id, dir, to_room_id, locked, description, visible_when_locked)`
* `bp_room_kv(room_id, key, value)`

> Migrations may evolve the schema. See `migrations/` for authoritative DDL.

## Configuration

Key settings (env vars):

* `DATABASE_URL` — Postgres connection string.
* `PORT4K_BIND` — HTTP/WS bind address, e.g., `0.0.0.0:4001`.
* `PORT4K_TELNET` — Telnet bind address, e.g., `0.0.0.0:4000`.
* `RUST_LOG` — logging (e.g., `info,port4k=debug`).

ANSI color/theme and other toggles live in code for now; see `port4k-server/src/`.

## Development

* **Build & Run**

  ```bash
  cargo build
  cargo run -p port4k-server
  ```
* **Tests**

  ```bash
  cargo test --workspace
  ```
* **Linting**

  ```bash
  cargo clippy --all-targets -- -D warnings
  cargo fmt --all
  ```
* **Notes**

    * Avoid holding `RwLockWriteGuard`/`MutexGuard` across `await`. Extract the data you need, drop the guard (scope), then call async functions.
    * Telnet handling uses CRLF and option negotiation (character-at-a-time, SGA, echo, NAWS).
    * The commands module owns player-facing verbs; unknown verbs in **playtest** may be forwarded to Lua (subject to design).

## Roadmap

* ✅ Playtest vs Live session flow
* ✅ Locked exits & room KV storage
* ⏳ Expanded object system (use/examine, containers, equipment)
* ⏳ Web UI for builders (visual room graph, script editor)
* ⏳ Richer ANSI theming and accessibility options
* ⏳ Fine-grained permissions and moderation
* ⏳ Save/Load player progress, coins/XP skills
* ⏳ Better docs & examples (starter blueprints)

> Check the issue tracker for the most current plan.

## Contributing

Contributions are welcome! Please:

1. Fork and create a feature branch.
2. Add tests where practical.
3. Run `cargo fmt` and `cargo clippy` before pushing.
4. Open a PR with a clear description and motivation.

If you plan a large change, open an issue to discuss design and scope first.

## License

Licensed under **MIT**.

```
SPDX-License-Identifier: MIT
```

See [`LICENSE`](./LICENSE).
