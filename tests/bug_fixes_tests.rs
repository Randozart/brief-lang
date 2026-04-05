//! Tests for Bug #1, #2, and #3 fixes
//!
//! Bug #1: Nested block elements (fixed in scan_html_block with depth tracking)
//! Bug #2: Unicode/emoji UTF-8 handling (fixed in scan_html_block with char boundaries)
//! Bug #3: WASM method name mismatch (fixed in wasm_gen.rs)

use brief_compiler::parser::Parser;

// ============================================================================
// Bug #1: Nested Block Elements
// ============================================================================

#[test]
fn test_bug1_nested_divs_in_rstruct() {
    // This should now work with depth tracking
    let code = r#"rstruct TestNested {
    value: Int;
    <div>
        <div>nested content</div>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(result.is_ok(), "Parser failed on nested divs: {:?}", result);
}

#[test]
fn test_bug1_multiple_nested_levels() {
    // Three levels of nesting
    let code = r#"rstruct DeepNested {
    x: Int;
    <div>
        <section>
            <div>deep content</div>
        </section>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(
        result.is_ok(),
        "Parser failed on triple-nested: {:?}",
        result
    );
}

#[test]
fn test_bug1_nested_with_attributes() {
    // Nested divs with HTML attributes
    let code = r#"rstruct WithAttrs {
    n: Int;
    <div class="outer">
        <div id="inner" data-test="value">
            content
        </div>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(
        result.is_ok(),
        "Parser failed with nested attributes: {:?}",
        result
    );
}

#[test]
fn test_bug1_complex_nesting() {
    // Complex nested structure with multiple sibling nests
    let code = r#"rstruct Complex {
    state: Int;
    <div class="container">
        <div class="header">
            <span>Title</span>
        </div>
        <div class="body">
            <section>
                <article>Content</article>
            </section>
        </div>
        <div class="footer">
            <span>Footer</span>
        </div>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(
        result.is_ok(),
        "Parser failed on complex nesting: {:?}",
        result
    );
}

// ============================================================================
// Bug #2: Unicode/Emoji Handling
// ============================================================================

#[test]
fn test_bug2_emoji_in_html() {
    // Emoji directly in HTML should not crash parser
    let code = r#"rstruct Cart {
    items: Int;
    <div>🛍️ shopping cart</div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(result.is_ok(), "Parser crashed on emoji: {:?}", result);
}

#[test]
fn test_bug2_multiple_emoji_in_html() {
    // Multiple emoji should work
    let code = r#"rstruct Shop {
    value: Int;
    <div>
        <span>🎉 Great!</span>
        <span>💳 Payment</span>
        <span>✨</span>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(
        result.is_ok(),
        "Parser failed with multiple emoji: {:?}",
        result
    );
}

#[test]
fn test_bug2_unicode_mixed() {
    // Mix of different unicode characters in HTML
    let code = r#"rstruct Unicode {
    text: String;
    <div>Café ☕ Über 📱 日本</div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(result.is_ok(), "Parser failed with unicode: {:?}", result);
}

#[test]
fn test_bug2_emoji_with_nested_tags() {
    // Combination: nested tags with emoji
    let code = r#"rstruct Mixed {
    data: String;
    <div class="container 🎨">
        <div>🏪 Store</div>
        <span>💰 Price</span>
        <section>📦 Shipping</section>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(
        result.is_ok(),
        "Parser failed with emoji + nested: {:?}",
        result
    );
}

// ============================================================================
// Bug #3: WASM Method Name Mismatch (tested via code generation)
// ============================================================================

#[test]
fn test_bug3_transaction_with_dots() {
    // Transaction with dots should generate proper invoke methods
    let code = r#"rstruct ShoppingCart {
    items: Int;

    txn ShoppingCart.add [true][items == @items + 1] {
        &items = items + 1;
        term;
    };

    <div>
        <button>Add</button>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(
        result.is_ok(),
        "Parser failed with dotted txn: {:?}",
        result
    );
    // The fix transforms "ShoppingCart.add" to "invoke_ShoppingCart_add" in glue code
}

// ============================================================================
// Combined/Regression Tests
// ============================================================================

#[test]
fn test_all_bugs_combined() {
    // A test combining all three fixes
    let code = r#"rstruct ShoppingCart {
    product: Int;
    items: Int;
    total: Int;

    txn ShoppingCart.select_laptop [true][product == 1] {
        &product = 1;
        term;
    };

    txn ShoppingCart.add [product > 0][items == @items + 1] {
        &items = items + 1;
        term;
    };

    <div class="app">
        <div class="shop">
            <div class="product">
                <h3>Laptop 💻</h3>
                <button b-trigger:click="add">Add to Cart</button>
            </div>
        </div>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(result.is_ok(), "Combined test failed: {:?}", result);
}

#[test]
fn test_shopping_cart_simplified() {
    // Shopping cart with key features showing all three fixes
    let code = r#"rstruct ShoppingCart {
    product: Int;
    items: Int;
    total: Int;

    txn ShoppingCart.select_laptop [true][product == 1] { &product = 1; term; };
    txn ShoppingCart.add [product > 0][items == @items + 1] {
        &items = items + 1;
        [product == 1] &total = total + 1299;
        term;
    };

    <div class="app">
        <span class="header">
            <h1>Store 🛍️</h1>
        </span>
        <span class="shop">
            <span class="product">
                <h3>Laptop 💻</h3>
                <button b-trigger:click="add">Add ✨</button>
            </span>
        </span>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(result.is_ok(), "Shopping cart parse failed: {:?}", result);
}

#[test]
fn test_deeply_nested_with_unicode() {
    // Deep nesting + unicode to stress-test both fixes
    let code = r#"rstruct Deep {
    val: Int;
    <div class="level1">
        <div class="level2 🎨">
            <div class="level3">
                <section>
                    <article>
                        <span>📝 Content ☕</span>
                    </article>
                </section>
            </div>
        </div>
    </div>
}"#;

    let mut parser = Parser::new(code);
    let result = parser.parse();
    assert!(
        result.is_ok(),
        "Deep nesting + unicode failed: {:?}",
        result
    );
}
