# Research: Brief + DBrief Text Adventure Engine

## Concept: "The Brief Adventures"

This document explores how to build a Zork-style text adventure using Brief + DBrief. This is **research**, not an implementation plan.

---

## 1. Architecture Overview

| Component | Language | Purpose |
|-----------|----------|---------|
| World data | `.dbvl` | Rooms, items, NPC dialogue |
| Game state | `.bv` | Transaction logic |
| Views | `.rbv` | Text output to terminal/web |

---

## 2. Data Layer (DBrief)

### 2.1 World Database

```dbvl
// world.dbvl - The game world

@rooms {
    "start": {
        name: "The Cellar",
        desc: "You are in a dark, dusty cellar. Cobwebs hang from the ancient beams.",
        exits: { north: "kitchen" },
        items: ["rusty_key"],
        visited: false
    },
    "kitchen": {
        name: "The Kitchen", 
        desc: "A rotting kitchen with a cast-iron stove. The smell is... memorable.",
        exits: { south: "start", east: "pantry" },
        items: ["candle"],
        visited: false
    },
    "pantry": {
        name: "The Pantry",
        desc: "Shelves lined with dusty jars. Something scuttles in the corner.",
        exits: { west: "kitchen" },
        items: ["cheese"],
        visited: false
    }
}

@items {
    "rusty_key": {
        name: "Rusty Key",
        desc: "An old iron key, covered in rust.",
        takeable: true,
        location: "start"
    },
    "candle": {
        name: "Wax Candle",
        desc: "A stubby candle. Could be useful in the dark.",
        takeable: true,
        location: "kitchen"
    },
    "cheese": {
        name: "Moldy Cheese",
        desc: "It's definitely seen better days. The mouse loves it though.",
        takeable: true,
        location: "pantry"
    }
}

@npcs {
    "rat_king": {
        name: "The Rat King",
        desc: "A massive rat wearing a crown. He squeaks threateningly.",
        dialogue: [
            { trigger: "hello", response: "Squeak! The throne is mine!" },
            { trigger: "cheese", response: "You have cheese? I could be... convinced." }
        ],
        location: "pantry",
        hostile: false
    }
}
```

### 2.2 Room Lookup Rules (Inference)

```brief
// rules.dbvs - Logical rules for the game world

// Can we go this direction?
RULE can_go(Room, Dir) :- 
    @rooms{ name: Room, exits: Exits },
    Exits.contains(Dir)

// What items are in a room?
RULE item_in_room(Item, Room) :-
    @items{ location: Room }

// Is an item takeable here?
RULE can_take(Item, Room) :-
    @items{ name: Item, location: Room, takeable: true },
    @rooms{ name: Room }
```

---

## 3. Game Logic Layer (Brief)

### 3.1 Core Transactions

```brief
// game.bv - Game logic

STATE current_room: String = "start"
STATE inventory: Vector[String] = []
STATE turn_count: UInt[16] = 0
STATE message: String = ""

node look [true][message == current_desc] {
    message = @rooms[current_room].desc
    
    // List visible items
    [inventory .#Size > 0] {
        message += "\n\nYou are carrying: "
        message += inventory.join(", ")
    }
    
    // List visible items in room
    let visible_items = @items->FILTER location == current_room
    [visible_items .#Size > 0] {
        message += "\n\nYou see: "
        message += visible_items.map(name).join(", ")
    }
}

node go(direction) [
    @rooms[current_room].exits.contains(direction)
][
    current_room == @rooms[current_room].exits.get(direction)
] {
    current_room = @rooms[current_room].exits.get(direction)
    message = @rooms[current_room].desc
}

node go(direction) [
    !@rooms[current_room].exits.contains(direction)
][
    message == "You can't go that way."
] {
    message = "You can't go that way."
}

node take(item) [
    @items[item].location == current_room
][
    inventory.contains(item)
] {
    @items[item].location = "inventory"
    inventory.push(item)
    message = "You take the " + @items[item].name + "."
}

node take(item) [
    @items[item].location != current_room
][
    message == "You don't see that here."
] {
    message = "You don't see that here."
}

node inventory [
    true
][
    message == inventory_msg
] {
    message = "You are carrying: " + inventory.join(", ")
}
```

---

## 4. View Layer (RBV)

### 4.1 Terminal Output

```brief
// game.rbv - Terminal interface

RENDER terminal {
    CLEAR
    
    HEADER "The Brief Adventures"
    DIV banner {
        "═══════════════════════════════════"
    }
    
    DIV output {
        b-text="message"
    }
    
    DIV input {
        INPUT cmd AS command
        BUTTON "Go" ONCLICK "submit(cmd)"
    }
    
    FOOTER "Commands: look, go [direction], take [item], inventory, quit"
}
```

### 4.2 Enhanced Web View

```brief
// game_web.rbv - Rich web interface

RENDER game {
    SIDEBAR inventory {
        DIV header "Inventory"
        FOR item IN inventory {
            DIV item { @items[item].name }
        }
    }
    
    MAIN room {
        H2 @rooms[current_room].name
        P @rooms[current_room].desc
        
        DIV exits {
            FOR dir IN @rooms[current_room].exits.keys() {
                BUTTON dir ONCLICK "go(dir)"
            }
        }
        
        DIV items {
            FOR item IN @items->FILTER location == current_room {
                DIV item { @items[item].name }
                BUTTON "Take" ONCLICK "take(item)"
            }
        }
    }
}
```

---

## 5. Puzzle System

### 5.1 Quest Rules

```brief
// quests.dbvs - Quest logic

RULE quest_unlocked(quest) :-
    @quests[quest].requires_done

RULE can_unlock_rat_king [
    inventory.contains("cheese")
] :- true

RULE reward_rat_king [
    inventory.contains("crown")
] :- true
```

### 5.2 Conditional Descriptions

```brief
node look [true][message == enhanced_desc] {
    // Base description
    message = @rooms[current_room].desc
    
    // Add secret revealed only if you have the key
    [inventory.contains("rusty_key")] {
        message += "\n\nThe key glows faintly in the darkness..."
        message += "\nYou notice a hidden passage behind the wine racks!"
    }
}
```

---

## 6. Save/Load System

### 6.1 Serialization

DBrief naturally supports save/load:

```brief
// Save game
TXN save_game [
    true
][saved == true] {
    @savefile->write(
        current_room,
        inventory,
        turn_count,
        @rooms,
        @items
    )
    saved = true
}

// Load game  
TXN load_game [
    @savefile->exists()
][loaded == true] {
    current_room = @savefile->read(current_room)
    inventory = @savefile->read(inventory)
    turn_count = @savefile->read(turn_count)
    loaded = true
}
```

---

## 7. Key Game Features

| Feature | DBrief | Brief |
|---------|--------|-------|
| **Room definitions** | `@rooms` vector | - |
| **Item tracking** | `@items` vector | - |
| **NPC dialogue** | `@npcs` + rules | - |
| **Navigation** | - | `go` txn |
| **Inventory** | - | `take`/`drop` txn |
| **Puzzles** | Rules engine | `rct` guards |
| **Save/Load** | Serialize vector | Transaction |

---

## 8. Why This Works

1. **Separation of concerns**: Data (world) vs Logic (transactions)
2. **Declarative rules**: "Can I go there?" is a query, not nested ifs
3. **Natural save/load**: DBrief data is inherently serializable
4. **Reactive UI**: Brief + RBV updates views automatically
5. **Extensible**: Add rooms/items without changing game logic

---

## 9. Future Enhancements

- [ ] Combat system with NPC AI
- [ ] Time-based events (day/night cycle)
- [ ] Random encounters
- [ ] Sound effects via FFI
- [ ] Graphics view (SVG from DBrief geometry data)

---

## 10. Conclusion

Brief + DBrief provides a clean separation for game development:
- **DBrief** handles the declarative world state
- **Brief** handles the imperative logic
- **RBV** handles the presentation

This mirrors classic adventure engines but with modern verification:
- Compiler checks for undefined rooms
- Contract analysis catches impossible states  
- Type safety prevents invalid item operations

*The text adventure was the perfect testing ground for languages. Brief + DBrief could be too.*