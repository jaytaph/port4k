# Port4k — MUD in Rust

This is a minimal, production-grade scaffold for a text MUD server:
- TCP listener (Telnet-friendly) on port 4000
- Pre-login ASCII banner and help
- Commands: `help`, `new <name>`, `login <name>`, `who`, `quit`
- In-memory registry for demo; persistence to follow


## Quick start


```bash
# from repo root
cargo run -p port4k-server
# connect from another terminal
nc 127.0.0.1 4000
# or telnet
# telnet 127.0.0.1 4000
```


Set log level:
```bash
RUST_LOG=port4k_server=debug cargo run -p port4k-server
```


## Next steps
- Swap in persistence (SQLite/Postgres) for accounts
- Add per-room Lua with `mlua`
- Add WebSocket gateway (Axum) while keeping TCP
```
