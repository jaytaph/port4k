# Lua Execution & Fallback Policy

Lua powers dynamic room behavior but operates under strict sandboxing.

## Philosophy
Lua is for *behavior*, not for persistence or I/O.

## Execution
- Runs on a dedicated thread via the Lua channel.
- Each script has a time limit (e.g. 500 ms).
- Only the provided `ctx` object is accessible.
- Forbidden: `os`, `io`, `require`, `dofile`, etc.

## Invocation policy

| Mode       | Unknown verbs                    | Hooks                                      | Write target |
|------------|----------------------------------|--------------------------------------------|--------------|
| Playtest   | Allowed – forwarded to Lua       | `on_command_playtest`, `on_enter_playtest` | Ephemeral    |
| Live       | Disallowed (→ “Unknown command”) | `on_command`, `on_enter`                   | Persistent   |
| Draft/Test | Allowed                          | Both                                       | Ephemeral    |

## Error handling
- **Playtest:** return the error to the user.
- **Live:** log error, continue with default output.

This prevents sandbox escapes while still giving creators full flexibility in Playtest.
