# RoomView Merge Contract

`RoomView` represents the *current visible state* of a room for a specific session.  
It merges static blueprint data with dynamic zone or overlay state.

## Merge algorithm

| Field | Merge rule | Notes |
|-------|-------------|-------|
| title | blueprint only | never overridden |
| body / short | blueprint + conditional fragments | may depend on room state |
| objects | (blueprint ∪ spawned) − removed | obey `visible_when_locked` |
| coins / counters | blueprint − picked_up | clamped ≥ 0 |
| exits | blueprint exits − locked (unless `visible_when_locked`) | |
| scripts | blueprint only | read-only |
| states | overlay → zone → blueprint defaults | |

## Example

Blueprint room:
```yaml
objects: ["coin", "torch"]
coins: 10
```

Zone state:
```json
{ "coins_picked_up": 2 }
```

RoomView result:
```json
{ "objects": ["coin", "torch"], "coins": 8 }
```

## Caching

A `RoomView` is immutable during command execution and may be cached per `(zone_id, room_id, version)`.  
Invalidated when any write occurs.
