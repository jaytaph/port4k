# Prison Ship Tutorial - Puzzle Walkthrough

## Story
You wake in a detention cell aboard a transport brig docked at Port4K. No memory of why you're imprisoned. Systems are failing, guards are gone. You must escape the cell, navigate the damaged ship, and reach the airlock to Port4K station.

---

## Room Layout
```
                    [observation_deck]
                            |
                          west
                            |
    [cell_block] → north → [maintenance_corridor] → east → [engineering_bay]
                                    |
                                  north
                                    |
                            [docking_airlock]
                                    |
                                  north
                                    ↓
                              [PORT4K HUB]
                            (tutorial complete)
```

---

## Complete Solution Path

### 1. CELL BLOCK (Starting Room)
**Goal:** Escape the detention cell

**Steps:**
1. `look` - examine your surroundings
2. `examine markings` - find code hint: "4-3-1-2"
3. `take toolkit` or `loot maintenance_kit` - get tools
4. `power console with microcell` - activate guard console
5. `enter 4312 on console` - unlock force field controls
6. `deactivate force field` - open the exit
7. `north` - enter maintenance corridor

**Items Acquired:**
- multi_spanner
- fiber_probe  
- microcell

---

### 2. MAINTENANCE CORRIDOR (Hub)
**Goal:** Restore power and access engineering bay

**Steps:**
1. `look` - note damaged power conduit and access panel
2. `repair conduit` or `use spanner on conduit` - restore corridor power
3. `use fiber probe on panel` or `bypass panel` - unlock east hatch
4. `east` - enter engineering bay

**Can also explore:**
- `west` - observation deck (optional side room)
- `north` - docking airlock (final room, but not ready yet)

---

### 3. ENGINEERING BAY (Optional Resource Room)
**Goal:** Find additional power cells (optional but helpful)

**Steps:**
1. `examine workbench` - notice hidden compartment
2. `open workbench` - reveal hidden panel
3. `loot workbench` - get microcell and 15 credits
4. `prime generator` - prepare aux generator
5. `start generator` - power up charging rack
6. (Optional) `charge microcell` if you have depleted cells

**Items Acquired:**
- microcell (extra)
- 15 credits

**Note:** This room is optional but provides backup power resources.

---

### 4. OBSERVATION DECK (Puzzle Room)
**Goal:** Solve locker puzzle and find rare authorization token

**Steps:**
1. `look` - examine the viewports and navigation chart
2. `read chart` or `examine chart` - get clue: "CREW locker code mirrors CELL override code"
3. `enter 2134 on locker` - unlock crew locker (mirrored from 4312)
   - OR `use spanner on locker` - force it open
4. `open locker` or `loot locker` - get energy cell and 25 credits
5. `examine vent grille` - notice loose screws
6. `open vent grille` or `use fiber probe on grille` - remove grille
7. (Automatically receive aurelite fragment when grille opened)

**Items Acquired:**
- energy_cell (REQUIRED for finale)
- aurelite_fragment (REQUIRED for finale)
- 25 credits

---

### 5. DOCKING AIRLOCK (Finale)
**Goal:** Complete all system checks and open outer hatch

**Required Items:**
- energy_cell (from observation deck)
- fiber_probe (from cell block)
- aurelite_fragment (from observation deck)

**Steps:**
1. `status` or `examine status display` - check what systems need completion
2. `power interface with energy cell` - activate docking interface
3. `connect fiber probe` or `use fiber probe on diagnostic port` - establish station link
4. `sync interface` - complete station handshake
5. `use aurelite fragment on hatch` or `authorize hatch` - grant security clearance
6. `open hatch` - release all locks
7. `north` - ESCAPE TO PORT4K!

**System Checklist:**
- ✓ POWER: energy_cell → docking_interface
- ✓ DIAGNOSTIC LINK: fiber_probe → diagnostic_port  
- ✓ STATION SYNC: sync command on interface
- ✓ AUTHORIZATION: aurelite_fragment → outer_hatch

---

## Teaching Objectives

Each room teaches core MUD mechanics:

**Cell Block:** Basic commands (look, examine, take), item usage, simple puzzle
**Maintenance Corridor:** Sequential actions, tool-based problem solving, state tracking
**Engineering Bay:** Hidden objects, multi-step processes, resource management
**Observation Deck:** Reading clues, code puzzles, alternative solutions
**Docking Airlock:** Synthesis of all skills, checklist completion, multiple dependencies

---

## Item Dependencies

```
maintenance_kit (cell_block)
    ├─ multi_spanner
    │   ├─ repair conduit (maintenance_corridor)
    │   └─ force crew_locker (observation_deck - alt solution)
    ├─ fiber_probe
    │   ├─ bypass access_panel (maintenance_corridor)
    │   ├─ open vent_grille (observation_deck)
    │   └─ connect diagnostic_port (docking_airlock) ⚠️ CONSUMED
    └─ microcell
        └─ power guard_console (cell_block) ⚠️ CONSUMED

workbench cache (engineering_bay)
    └─ microcell (backup power)

crew_locker (observation_deck)
    ├─ energy_cell → power docking_interface (docking_airlock) ⚠️ CONSUMED
    └─ 25 credits

vent_grille (observation_deck)
    └─ aurelite_fragment → authorize outer_hatch (docking_airlock) ⚠️ CONSUMED
```

---

## Alternative Solutions

**Crew Locker:**
- Code solution: `enter 2134 on locker` (puzzle solve)
- Force solution: `use spanner on locker` (brute force)

**Console Code:**
- Clue on wall: examine markings reveals "4-3-1-2"
- Trial and error: system provides hints after failed attempts

---

## Credits & Loot Summary

**Total Credits Available:** 40
- Engineering bay workbench: 15
- Observation deck crew locker: 25

**Critical Items for Completion:**
- energy_cell (observation_deck crew_locker)
- aurelite_fragment (observation_deck vent_grille)
- fiber_probe (cell_block maintenance_kit)

---

## Completion Message

When outer hatch opens:

```
*** ALL SYSTEMS NOMINAL ***

Hydraulics groan. The outer hatch's seals break with a hiss of equalizing pressure.
Warm air rushes in from Port4K. Through the opening, you hear distant voices, machinery, life.

The way north is now open. Your prison has become a doorway.
```

Player exits north → enters Port4K Hub (main game area)

---

## Design Notes

- All rooms are accessible; exploration is encouraged
- Multiple valid solution paths (locker can be forced or code-solved)
- Clear progression gates (can't proceed without completing previous challenges)
- Teaching curve: simple → complex → synthesis
- Narrative reinforcement: prison escape urgency, mystery of abandonment
- Lore hooks: Why imprisoned? Why guards gone? What happened to the ship?
