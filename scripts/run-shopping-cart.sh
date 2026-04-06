#!/bin/bash
# Easy command to compile and run the shopping cart example

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# Default output directory
OUTPUT_DIR="${1:-.shopif_cart_build}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Brief Shopping Cart - Compilation${NC}"
echo "=================================="
echo ""
echo "Step 1: Building compiler (release mode)..."
cargo build --release -q 2>/dev/null || echo "(compiler already built)"

echo -e "Step 2: Compiling shopping cart to $OUTPUT_DIR..."
./target/release/brief-compiler rbv examples/shopping_cart.rbv --out "$OUTPUT_DIR"

echo ""
echo -e "${GREEN}✓ Compilation complete!${NC}"
echo ""
echo "Next steps:"
echo "1. Navigate to the output directory:"
echo "   cd $OUTPUT_DIR"
echo ""
echo "2. Build the WASM:"
echo "   cd src && wasm-pack build --target web --dev"
echo ""
echo "3. Serve the HTML file:"
echo "   python3 -m http.server 8000"
echo ""
echo "4. Open browser to:"
echo "   http://localhost:8000/shopping_cart.html"
echo ""
echo "Files generated:"
echo "  - shopping_cart.html (the web app)"
echo "  - shopping_cart.css (styles)"
echo "  - shopping_cart_glue.js (glue code)"
echo "  - src/lib.rs (Rust WASM)"
echo "  - Cargo.toml (dependencies)"
