# State Layers & Write Rules

All world data in Port4k is expressed as layered state.  
Each layer overrides or augments the ones beneath it.

```
┌────────────┐  Layer 3 – Session (ephemeral)
│ pronouns,  │  UI state such as "it", pagination, last target
│ view state │
├────────────┤
│ Playtest   │  Layer 2 – Ephemeral overlay (per session)
│ overlay    │  Simulates mutations without persisting
├────────────┤
│ Zone data  │  Layer 1 – Persistent world state
│ (database) │  Stores actual world changes
├────────────┤
│ Blueprint  │  Layer 0 – Static design template
│ (read-only)│
└────────────┘
```

## Read rule
Data is read *top-down* until a layer provides a value.

Example:
> “How many coins are here?”  
→ check playtest overlay → else zone → else blueprint.

## Write rule
Writes always go to the highest applicable layer for the current `ZoneKind`.

| ZoneKind | Write target | Persistence |
|-----------|---------------|-------------|
| `Live` | Zone layer (DB) | Persistent |
| `Draft` | Zone layer (sandbox DB) | Persistent (isolated) |
| `Playtest` | Playtest overlay (in-memory) | Ephemeral |
| `Test { owner }` | Playtest overlay | Ephemeral/private |

When a playtest ends, its overlay layer is dropped. Persistent layers remain intact.

## Inventory scoping
Player inventories follow the same layering:
- Live → DB
- Playtest → overlay `(session_id, zone_id)`
- On exit, overlay inventories are discarded.
