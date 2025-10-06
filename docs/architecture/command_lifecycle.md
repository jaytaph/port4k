# Command Lifecycle

All user commands, regardless of interface (Telnet or WebSocket), follow the same lifecycle.

## 1. Parse
Convert the raw line to an `Intent`:
```rust
Intent { verb, direct_object, modifiers, ... }
```

## 2. Authorize
Ensure the verb is permitted for this connection and account role.

## 3. Resolve
Resolve natural language references:
```
"open the door" → object=door@room
```
Handle pronouns and ambiguous matches.

## 4. Snapshot
Build a consistent `RoomView` from all state layers for this command.

## 5. Validate
Check command preconditions:
- object exists
- not locked or blocked
- enough coins, etc.

## 6. Apply
Apply state changes:
- Live → write via DB transaction  
- Playtest → mutate overlay store

## 7. Hooks
Run Lua event handlers (`on_command`, `on_enter`, etc.) through the Lua worker channel.

## 8. Render
Return a `CommandResult` (UI-neutral).

## 9. Commit
Finalize DB writes and notify other players (broadcast or WS push).

This makes commands deterministic, testable, and safe to run from either interface.
