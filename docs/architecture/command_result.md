# CommandResult Schema

`CommandResult` is the unified output for both Telnet and WebSocket interfaces.

## Structure

```rust
pub struct CommandResult {
    pub text: String,              // Primary narrative text
    pub diffs: Option<RoomDiff>,   // Structured world delta
    pub inventory: Option<InvDiff>,
    pub prompt: Option<String>,    // Prompt to show next
    pub notify: Vec<SystemEvent>,  // Side-channel events
}
```

## Rendering rules

### Telnet
- Render `text` as ANSI.
- Ignore structured diffs.
- Print `prompt` at the bottom.

### WebSocket / HTTP
- Apply `diffs` to client model.
- Show `text` in chat/log view.
- Use `notify` for animations or sounds.

## Example

```
> take coin
You pick up a shiny gold coin.
```

```json
{
  "text": "You pick up a shiny gold coin.",
  "diffs": { "objects_removed": ["coin#123"], "inventory_added": ["coin#123"] },
  "inventory": { "coins": 1 },
  "prompt": "> "
}
```

This unified result allows both clients to share identical server logic while rendering appropriately.
