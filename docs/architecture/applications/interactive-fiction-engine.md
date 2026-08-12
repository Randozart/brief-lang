# Interactive Fiction Engine — World Model + Rule Dispatch

**Date:** 2026-07-26
**Status:** Aspirational / Concept

---

## 1. Motivation

Interactive fiction (parser games like *Zork*, *Spider and Web*, games written in
*Dialog* or *Inform 7*) has a natural two-part architecture that maps directly
onto Briev's strengths:

1. **A relational world model** — rooms, items, containment, player state.
2. **A reactive rule engine** — parsing player input, checking preconditions,
   mutating state, printing descriptions.

Data Briev (`.dbv`) is a natural fit for the world model. Briev's `node` and
`txn` constructs are a natural fit for the rule engine. The combination is a
zero-runtime, compile-time-proven interactive fiction platform.

---

## 2. World Model in `.dbv`

The world is a set of flat, positional schemas. Rooms, items, and player state
are stored as Data Briev entries.

### 2.1 Room Schema

```briev
schema Room (name) {
    name: String;
    desc: String;
    north: String;
    south: String;
    east: String;
    west: String;
};
```

Entries are pure data — no markup, no quotes, no nested JSON:

```briev
as Room {
    kitchen: Kitchen; You are in a dusty kitchen. A heavy door leads south.; ; cellar; ; ;
    cellar: Cellar; Damp and dark. Steps lead north.; kitchen; ; ; ; ;
    garden: Garden; Overgrown with ivy. A gate to the east.; ; ; ; kitchen; ;
};
```

Positional fields map directly to schema order. Empty strings represent
"no exit in this direction." The compiler packs this into a tight binary struct
array — zero parsing overhead at runtime.

### 2.2 Item Schema

```briev
schema Item (name) {
    name: String;
    desc: String;
    location: String;
    fixed: Bool;
};

as Item {
    key: brass key; An old intricately carved brass key.; kitchen; false;
    table: heavy table; A sturdy wooden table bolted to the floor.; kitchen; true;
    lantern: oil lantern; Casts a warm glow.; cellar; false;
};
```

`location` references a Room name (or `"player"` for inventory). `fixed`
prevents `take` on immovable objects. The schema enforces field count and
type at parse time — a missing `fixed` field is a compile error, not a
runtime crash.

### 2.3 Player State

```briev
schema Player {
    location: String;
    inventory: String[];
};

as Player {
    @ kitchen; { key; lantern; };
};
```

Player state is a singleton positional entry. The inventory is a nested block
of item key strings.

---

## 3. Rule Dispatch with `node`

Each verb (action) is a reactive `node` with a precondition that matches the
parsed command. This avoids a central `if/else` chain — each verb's rule is
an independent, provable contract.

### 3.1 Core Types

```briev
type Command {
    verb: String;
    noun: String;
};
```

The parser produces a `Command` from player input. Each `node` pattern-matches
on `cmd.verb` and `cmd.noun`.

### 3.2 Take Action

```briev
txn handle_take(cmd: Command, player_location: String)
    [cmd.verb == take][cmd.verb == take && handled == true] -> Bool
{
    let item = find_item(cmd.noun);
    when item == 0 as Ptr<Item> {
        print(I do not see that here.\n);
        term false;
    };
    when item.location != player_location {
        print(You cannot reach the + cmd.noun + .\n);
        term false;
    };
    when item.fixed {
        print(The + cmd.noun + is too heavy to lift.\n);
        term false;
    };
    item.location = player;
    print(Taken.\n);
    term true;
};
```

Each guard checks one precondition. The chain reads declaratively — "if not
found, if not here, if fixed, otherwise take." The `[pre][post]` contract
guarantees convergence: the action terminates with a boolean result.

### 3.3 Move Action

```briev
txn handle_move(cmd: Command, player_location: String)
    [cmd.verb == go || cmd.verb == north || cmd.verb == south][handled == true] -> Bool
{
    let room = find_room(player_location);
    let exit = match cmd.verb {
        north: room.north;
        south: room.south;
        east:  room.east;
        west:  room.west;
        _:     ;
    };
    when exit == || exit is None {
        print(You cannot go that way.\n);
        term false;
    };
    player_location = exit;
    look_at(exit);
    term true;
};
```

The exit lookup is a flat match on verb string. No branching state machine.
The `[pre][post]` contract proves that any invalid direction is caught and
produces a descriptive message.

---

## 4. Distribution Model

### 4.1 Native Binary

The Briev compiler compiles the world model + rule nodes into a single native
binary. The `.dbv` world data is packed into a memory-mapped `.beastdb` binary
blob in `.rodata`. At runtime, the game is a ~10KB ELF with zero startup time,
zero heap allocation, and zero runtime dependencies.

### 4.2 `.beastpack` + `.lair` VM

The same source can compile to a platform-independent `.beastpack` that runs
inside the `.lair` VM. This produces safe, sandboxed `.bounty` files that:
- Execute on any platform without recompilation
- Have no filesystem or network access by default
- Are trivially distributable (single file, no runtime install)

### 4.3 Embedded Target

Because the binary is tiny and allocation-free, the game can run on:
- Classic 8-bit microcontrollers (6502, Z80 with a serial terminal)
- Modern MCUs (STM32, ESP32 with UART debug console)
- Any POSIX terminal with `echo` and `read`

---

## 5. Comparison to Existing IF Platforms

| Aspect | Inform 7 | Dialog | Briev + .dbv |
|--------|----------|--------|--------------|
| World model | Custom I7 syntax | Custom Dialog syntax | Standard `.dbv` — any editor |
| Rule language | Natural language | Lisp-like | Briev contracts |
| Type safety | Runtime | Runtime | Compile-time proven |
| Distribution | Glulx blorb | Interpreter + story file | Native binary or `.beastpack` |
| Memory model | VM heap | VM heap | Zero-heap (`.beastdb` mmap) |
| Install size | ~2MB interpreter | ~1MB interpreter | ~10KB standalone |

---

## 6. Open Questions

1. **Parser grammar**: Briev `node`s need a string-input command parser.
   Should this be a built-in `parse_command(string)` intrinsic, or should
   the author write a `txn` that splits on word boundaries?

2. **Room graph cycle detection**: The `.dbv` room entries reference each
   other by key string. A compile-time pass should verify that all room
   references resolve and that there are no unreachable rooms.

3. **Verb extensibility**: Can an author add custom verbs without modifying
   the dispatch layer? (e.g., `pray`, `whistle`, `climb`). A trait/protocol
   for `VerbHandler` could let each verb be its own `node` file.

4. **Save/restore**: Serializing player state (location, inventory, item
   flags) back to `.dbvl` for save files. The format's line-oriented nature
   makes incremental saves natural — append a delta, compact later.
