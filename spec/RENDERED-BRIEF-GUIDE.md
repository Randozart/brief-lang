# Rendered Brief Guide

**Version:** 6.2  
**Purpose:** Building reactive web interfaces with Brief  

---

## Overview

Rendered Brief (`.rbv` files) extends Brief with HTML templating. It compiles to HTML + WASM + a thin JavaScript layer that handles updates between the state machine and the DOM.

### What it Adds

- HTML template syntax embedded in Brief
- Component-based architecture
- Reactive DOM updates
- Event handling
- CSS styling

### Compilation Target

`.rbv` files compile to:
- **HTML**: The template structure
- **WASM**: The Brief state machine
- **JavaScript**: Thin layer handling DOM updates and event bridging

### Basic Structure

```html
<script>
rstruct Counter {
    count: Int;
    
    txn increment [true][count == @count + 1] {
        &count = count + 1;
        term;
    };
    
    <div class="counter">
        <span b-text="count">0</span>
        <button b-trigger:click="increment">+</button>
    </div>
}
</script>

<view>
    <Counter />
</view>

<style>
    .counter { font-size: 24px; }
</style>
```

---

## The Script Block

The Brief code goes in a `<script>` block. For `.rbv` files, the script language is implicit (Brief):

```html
<script>
let message: String = "Hello";
let count: Int = 0;

txn update [true][true] {
    &count = count + 1;
    term;
};

<view class="app">
    <span b-text="message">Default</span>
    <span b-text="count">0</span>
</view>
</script>
```

Note: `<script type="brief">` is also valid, but for `.rbv` files, plain `<script>` is sufficient.

### rstruct (Render Struct)

The view is declared inside an `rstruct`, which combines state with HTML:

```brief
rstruct Counter {
    count: Int;
    
    txn increment [true][count == @count + 1] {
        &count = count + 1;
        term;
    };
    
    <div>
        <span b-text="count">0</span>
    </div>
}
```

---

## b-text Directive

Updates element text content when the expression changes:

```html
<span b-text="count">0</span>
<span b-text="message">Default</span>
<span b-text="count + 1">1</span>
<span b-text="user.name">Name</span>
```

---

## b-show Directive

Shows/hides element based on condition:

```html
<span b-show="is_logged_in">Welcome!</span>
<span b-show="count > 0">Items: <span b-text="count">0</span></span>
<span b-show="step == 1">Step 1 content</span>
```

---

## b-trigger Directive

Binds events to transactions:

```html
<button b-trigger:click="increment">+</button>
<input b-trigger:input="update_text" />
<select b-trigger:change="select_option">
```

### Available Events

| Event | Description |
|-------|-------------|
| `click` | Mouse click |
| `input` | Input field change |
| `change` | Select/input change |
| `submit` | Form submission |
| `keydown` | Keyboard press |
| `mouseover` | Mouse hover |

---

## b-each Directive

Iterates over lists:

```html
<div b-each:item="items" class="item">
    <span b-text="item">Item</span>
</div>
```

---

## Component Composition

Components can contain other components:

```brief
rstruct Header {
    title: String;
    
    <header>
        <h1 b-text="title">Title</h1>
    </header>
}

rstruct Page {
    title: String;
    
    <div>
        <Header title={title} />
        <p>Content here</p>
    </div>
}
```

---

## rstruct Methods

Transactions inside rstructs are methods that can be triggered:

```brief
rstruct Counter {
    count: Int;
    
    txn increment [true][count == @count + 1] {
        &count = count + 1;
        term;
    };
    
    txn decrement [count > 0][count == @count - 1] {
        &count = count - 1;
        term;
    };
    
    txn reset [true][count == 0] {
        &count = 0;
        term;
    };
    
    <div class="counter">
        <span b-text="count">0</span>
        <button b-trigger:click="increment">+</button>
        <button b-trigger:click="decrement">-</button>
        <button b-trigger:click="reset">Reset</button>
    </div>
}
```

---

## State Management

### Local State

Variables declared inside `<script>` are component-local:

```html
<script>
let count: Int = 0;

txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
};
</script>
```

### Shared State

Modules can export state for sharing:

```brief
// shared_state.bv
let global_count: Int = 0;
```

---

## CSS Styling

Styles can be defined inline or imported from external files.

### Inline Styles

Styles are written in a `<style>` block:

```html
<style>
    * {
        box-sizing: border-box;
        margin: 0;
    }
    
    .container {
        max-width: 600px;
        margin: 0 auto;
    }
    
    button {
        padding: 10px 20px;
        background: #667eea;
        color: white;
    }
    
    button:hover {
        background: #764ba2;
    }
</style>
```

### Importing CSS Files

Import external CSS files using the `import` statement:

```brief
import "./styles/main.css";
import "./styles/theme.css";
import "./styles/components.css";
```

```html
<script>
import "./styles/main.css";

rstruct MyPage {
    // ...
}
</script>
```

Multiple CSS files can be imported and are concatenated in order.

### Component Styles

Components can have scoped styles:

```brief
rstruct Button {
    <style>
        .btn {
            padding: 10px 20px;
            border-radius: 4px;
        }
        .btn-primary {
            background: #667eea;
        }
    </style>
    
    <button class="btn btn-primary">Click</button>
}
```

---

## SVG Support

Rendered Brief supports inline SVG for icons, graphics, and vector artwork.

### Inline SVG

Embed SVG directly in components:

```brief
rstruct Icon {
    name: String;
    
    <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"/>
        <path d="M12 8v8M8 12h8"/>
    </svg>
}
```

### SVG with State-Driven Attributes

Bind SVG attributes to Brief state:

```brief
rstruct ProgressIcon {
    progress: Int;
    
    <svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
        <circle b-attr:cx="50" b-attr:cy="50" b-attr:r="45" fill="none" stroke="#eee" stroke-width="10"/>
        <circle b-attr:cx="50" b-attr:cy="50" b-attr:r="45" fill="none" stroke="#667eea" stroke-width="10"
                b-attr:stroke-dasharray="{progress * 2.83} 283"
                transform="rotate(-90 50 50)"/>
    </svg>
}
```

### Importing SVG Files

Import SVG files as components:

```brief
import "./icons/logo.svg" as Logo;
import "./icons/arrow.svg" as Arrow;

rstruct Header {
    <header>
        <Logo width="100" height="50"/>
        <Arrow direction="right"/>
    </header>
}
```

---

## Complete Example

```html
<script>
let items: List<String> = ["Task 1", "Task 2"];
let new_item: String = "";

txn add_item [true][items.len() == @items.len() + 1] {
    &items = items + [new_item];
    &new_item = "";
    term;
};

txn remove_item(i: Int) [i >= 0 && i < items.len()][items.len() == @items.len() - 1] {
    &items = items[0..i] + items[i+1..items.len()];
    term;
};

rstruct TodoApp {
    <div class="app">
        <h1>Todo List</h1>
        
        <div class="input-row">
            <input b-trigger:input="update_text" />
            <button b-trigger:click="add_item">Add</button>
        </div>
        
        <div b-each:item="items" class="item">
            <span b-text="item">Item</span>
            <button b-trigger:click="remove_item">X</button>
        </div>
        
        <span b-show="items.len() == 0">No items</span>
    </div>
}
</script>

<view>
    <TodoApp />
</view>

<style>
    .app { max-width: 600px; margin: 20px auto; }
    .item { padding: 10px; border-bottom: 1px solid #eee; }
    .input-row { display: flex; gap: 10px; }
</style>
```

---

## File Extension

- `.bv` - Pure Brief (no HTML, compiles to backend/CLI)
- `.rbv` - Rendered Brief (with HTML, compiles to HTML + WASM + JS)

### Script Tag Syntax

For `.rbv` files, the Brief language is implicit:

```html
<!-- These are equivalent for .rbv files: -->
<script> ... </script>
<script type="brief"> ... </script>
```

Use `<script type="brief">` in `.bv` files with embedded HTML, or when you want to be explicit.

---

## Compilation

```bash
# Compile to JavaScript
brief compile counter.rbv

# Serve locally
brief serve counter.rbv
```

---

## Key Concepts

1. **State drives the DOM**: Brief state changes trigger DOM updates
2. **Transactions are methods**: `b-trigger:click="method"` calls transactions
3. **Reactive expressions**: `b-text="count * 2"` updates when count changes
4. **Conditional rendering**: `b-show="condition"` toggles visibility
5. **Component composition**: Components can contain other components
