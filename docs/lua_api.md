# Port4K Lua Scripting API Documentation

## Overview

Port4K uses Lua as its scripting language to create dynamic, interactive game experiences. Scripts can be attached to rooms and objects to respond to player actions, control game state, and create custom behaviors.

---

## Script Hooks

Scripts are triggered by specific events in the game. Each hook receives a Lua environment with relevant context objects.

### Room Hooks

#### `on_first_enter`

Called the first time a player enters a room.

```lua
function on_first_enter(ctx)
    send("Welcome! You've never been here before.")
    send("A mysterious door appears before you.")
end
```

#### `on_enter`

Called every time a player enters a room.

```lua
function on_enter(ctx)
    send("You step into the familiar chamber.")

    -- Check room state
    if room.state.door_unlocked then
        send("The door to the north is open.")
    end
end
```

#### `on_leave`

Called when a player leaves a room.

```lua
function on_leave(ctx)
    send("You hear the door lock behind you.")
    set_exit_locked("north", true)
end
```

#### `on_command`

Called when a player issues a command that isn't handled by the standard game commands.

```lua
function on_command(ctx)
    if intent.verb == "pray" then
        send("You kneel and pray to the ancient gods.")
        send("A soft light fills the room.")
        return true  -- Command handled
    end

    -- Return false or nil to let the game handle it normally
    return false
end
```

### Object Hooks

#### `on_use`

Called when a player interacts with an object (e.g., "use key", "examine painting").

```lua
function on_use(ctx)
  send("You examine the " .. obj.name .. " closely.")

  if intent.verb == "use" and intent.preposition == "on" then
    -- Player used object on something else
    send("You use the " .. obj.name .. " on the door.")
    set_exit_locked("north", false)
    return true  -- Action handled
  end

  return false  -- Let game handle it
end
```

---

## Global Context Objects

These objects are available in all script hooks:

### `account`

Information about the current player.

```lua
account.id          -- UUID string
account.username    -- Player's username
account.email       -- Player's email
account.role        -- Player's role (e.g., "player", "admin")
account.created_at  -- ISO 8601 timestamp
account.last_login  -- ISO 8601 timestamp or empty string
```

**Example:**

```lua
send("Welcome back, " .. account.username .. "!")
```

### `room`

Information about the current room.

```lua
room.id          -- Room UUID
room.key         -- Room key (e.g., "entrance")
room.title       -- Room title
room.description -- Room description
room.short       -- Short description
room.hints       -- Array of hint objects
room.objects     -- Table of objects (keyed by object key)
room.exits       -- Table of exits (keyed by direction)
room.state       -- Key-value state storage
```

**Example:**

```lua
-- Check if an object exists
if room.objects.key then
  send("You notice a rusty key on the ground.")
end

-- Check exits
if room.exits.north then
  if room.exits.north.locked then
    send("The north exit is locked.")
  end
end

-- Access room state
local visit_count = tonumber(room.state.visits or "0")
send("You've visited this room " .. visit_count .. " times.")
```

### `intent` (available in `on_command` and `on_use`)

Information about the player's command.

```lua
intent.verb         -- Main verb (e.g., "look", "use", "take")
intent.original     -- Original command string
intent.raw_verb     -- Raw verb before normalization (or nil)
intent.args         -- Array of command arguments
intent.direction    -- Direction if movement command (or nil)
intent.preposition  -- Preposition (e.g., "on", "with", "in")
intent.quantifier   -- Quantifier (e.g., "all", "some")
```

**Example:**

```lua
if intent.verb == "pull" and intent.args[1] == "lever" then
  send("You pull the lever with all your might!")
  return true  -- Command handled
end
```

### `obj` (available in `on_use`)

Information about the object being interacted with.

```lua
obj.key         -- Object key
obj.name        -- Object name
obj.short       -- Short description
obj.body        -- Full description
obj.visible     -- Is object visible?
obj.takeable    -- Can be taken?
obj.hidden      -- Is hidden?
obj.revealed    -- Has been revealed?
obj.locked      -- Is locked?
obj.stackable   -- Can stack?
```

**Example:**

```lua
if obj.locked then
  send("The " .. obj.name .. " is locked tight.")
  return false  -- Can't use it, let game handle it
end
```

---

## API Functions

### Output Functions

#### `send(text, newline)`

Send a message to the current player.

```lua
send("You hear a distant rumble.")
send("HP: 100", false)  -- No newline
```

#### `broadcast_room(text, newline)`

Send a message to all players in the room.

```lua
broadcast_room("The door slams shut with a thunderous boom!")
```

#### `say(message)`

Add text to the output buffer (legacy function, prefer `send`).

```lua
say("The ancient mechanism clicks into place.")
```

### Room Query Functions

#### `get_object(key)`

Get an object from the current room by its key.

```lua
local key_obj = get_object("rusty_key")
if key_obj then
  send("The " .. key_obj.name .. " glints in the light.")
else
  send("You don't see that here.")
end
```

### Room Manipulation Functions

#### `set_exit_locked(direction, locked)`

Lock or unlock an exit in the current room.

```lua
-- Lock the north exit
set_exit_locked("north", true)

-- Unlock the south exit
set_exit_locked("south", false)

-- Toggle default (locks if second param omitted)
set_exit_locked("east")
```

**Valid directions:** `"north"`, `"south"`, `"east"`, `"west"`, `"up"`, `"down"`, `"northeast"`, `"northwest"`, `"southeast"`, `"southwest"`

---

## Return Values

Scripts can return a boolean to indicate whether they handled the action:

### Handled (true)

Command was handled by the script, don't run default game behavior.

```lua
if intent.verb == "dance" then
  send("You do a little jig!")
  return true
end
```

### Not Handled (false or nil)

Script didn't handle this command, let the game's default handler try.

```lua
-- Explicit false
if intent.verb == "something_weird" then
  return false
end

-- Implicit nil (no return statement)
-- The game will try to handle the command
```

**Note:** For room hooks like `on_enter`, `on_leave`, and `on_first_enter`, the return value is ignored since these are just side effects.

---

## Complete Examples

### Example 1: Puzzle Room

```lua
-- on_enter hook
function on_enter(ctx)
  local lever_pulled = room.state.lever_pulled

  if lever_pulled == "true" then
    send("The room is bathed in an eerie green light.")
  else
    send("The room is pitch black except for a faint outline of a lever.")
  end
end

-- on_command hook
function on_command(ctx)
  if intent.verb == "pull" and intent.args[1] == "lever" then
    send("You pull the lever down with a satisfying *CLUNK*.")
    send("Torches burst into life along the walls!")

    -- Save state (note: this would need state-setting API)
    -- room.state.lever_pulled = "true"

    -- Unlock the north exit
    set_exit_locked("north", false)
    broadcast_room(account.username .. " has solved the lever puzzle!")

    return true  -- Command handled
  end

  return false  -- Not handled
end
```

### Example 2: Interactive Object

```lua
-- Object: "ancient_scroll"
-- on_use hook
function on_use(ctx)
  if intent.verb == "read" then
    send("You unfurl the ancient scroll and read the faded text:")
    send("")
    send("  'Only those who speak the word of power'")
    send("  'May pass through the sealed door.'")
    send("  'The word is whispered by the wind: AZATHOTH'")
    send("")

    -- Mark as read in player state (would need state API)
    -- obj.state.read = "true"

    return true  -- Read action handled
  end

  if intent.verb == "take" then
    if obj.takeable then
      return false  -- Let default handling take it
    else
      send("The scroll crumbles to dust as you touch it.")
      return true  -- Prevent taking it
    end
  end

  return false  -- Let game handle other actions
end
```

### Example 3: First-Time Greeting

```lua
-- on_first_enter hook
function on_first_enter(ctx)
  send("╔════════════════════════════════════════╗")
  send("║  Welcome to the Chamber of Mysteries  ║")
  send("╚════════════════════════════════════════╝")
  send("")
  send("You step into a vast chamber filled with arcane symbols.")
  send("An old wizard turns to greet you...")
  send("")
  send("'Ah, " .. account.username .. "! I've been expecting you.'")

  -- Unlock a secret passage for first-time visitors
  set_exit_locked("down", false)
end
```

### Example 4: Time-Based Events

```lua
-- on_enter hook
function on_enter(ctx)
  -- Get current hour (would need time API)
  -- local hour = os.date("%H")

  -- For demonstration, check state
  local time_of_day = room.state.time or "day"

  if time_of_day == "night" then
    send("Moonlight streams through the broken windows.")
    send("Shadows dance across the walls.")

    -- At night, the ghost appears
    if room.objects.ghost and room.objects.ghost.visible then
      send("A spectral figure materializes before you!")
    end
  else
    send("Sunlight fills the room with warmth.")
    send("Dust motes float lazily in the air.")
  end
end
```

---

## Best Practices

### 1. **Always check for nil values**

```lua
if room.objects.key then
  -- Safe to access
  send("You see a " .. room.objects.key.name)
end
```

### 2. **Provide helpful feedback**

```lua
-- Good
send("You try to open the door, but it's locked tight.")
send("Perhaps you need a key?")

-- Less helpful
send("Can't do that.")
```

### 3. **Use return values appropriately**

```lua
-- Return true when you handle the action completely
if intent.verb == "jump" then
  send("You jump up and down. Wheee!")
  return true  -- Prevents "I don't know how to jump"
end

-- Return false or nothing to let the game handle it
return false
```

### 4. **Keep scripts focused**

Each hook should handle one specific aspect of behavior. Don't try to handle everything in one script.

### 5. **Test edge cases**

```lua
-- Check for required objects
local key = get_object("golden_key")
if not key then
  send("You need the golden key to unlock this door.")
  return false  -- Let game handle it normally
end

-- Check object state
if key.locked then
  send("The key is sealed with magic and won't work.")
  return false
end

-- Success - handle the action
send("You use the golden key. *Click!*")
set_exit_locked("north", false)
return true
```

---

## Debugging

### Print debugging with `send()`

```lua
send("DEBUG: intent.verb = " .. tostring(intent.verb))
send("DEBUG: room.state.count = " .. tostring(room.state.count))
```

### Inspect tables with `pairs()`

```lua
-- List all objects in the room
for key, obj in pairs(room.objects) do
    send("Object: " .. key .. " = " .. obj.name)
end

-- List all exits
for dir, exit in pairs(room.exits) do
    send("Exit: " .. dir .. " -> " .. exit.to_room_key)
end
```

### Check environment variables

```lua
-- See what's available in REPL
for k, v in pairs(_ENV) do
    print(k, type(v))
end
```

---

## Limitations & Notes

1. **Read-only context objects** - `account`, `room`, `obj`, `intent` are read-only. You cannot modify them directly.

2. **State persistence** - Currently, room state is read-only from Lua. State-setting APIs are coming soon.

3. **Sandboxed environment** - Lua scripts run in a sandboxed environment with limited standard library access for security.

4. **Timeout** - Scripts have a 5-second timeout to prevent infinite loops.

5. **No file I/O** - Scripts cannot access the filesystem for security reasons.

6. **Async operations** - Functions like `set_exit_locked()` are fire-and-forget and execute asynchronously.


---

## REPL Usage

You can test Lua code interactively in the game using the REPL:

```lua
lua> account.username
"alice"

lua> room.title
"The Grand Hall"

lua> send("Hello, world!")
Hello, world!

lua> for k, v in pairs(room.objects) do print(k) end
torch
chest
key

lua> x = 10
lua> x + 5
15
```

The REPL has access to all the same APIs as regular scripts, making it perfect for testing and debugging.

---

## Getting Help

- Check the examples above for common patterns
- Use the REPL to explore available objects and functions
- Test small pieces of code before integrating into larger scripts
- Remember: all context objects are read-only tables

Happy scripting! 🎮✨