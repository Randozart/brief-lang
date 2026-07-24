# Makefile for Metropolitan FFI Benchmarks
# 2026-07-24: Measure per-call latency for each language generator.
# Bridge files are generated once via the metropolitan plugin, then cached.
# Run 'make generate' to regenerate bridge files.

PROJECT_ROOT ?= $(abspath $(dir $(firstword $(MAKEFILE_LIST)))/../..)
OUT_DIR ?= $(abspath $(dir $(firstword $(MAKEFILE_LIST)))/out)
BRIEFC ?= $(PROJECT_ROOT)/target/release/brief-compiler
CC ?= gcc
LLC ?= llc
LD ?= wasm-ld
BV_SRC = bench_add.bv

.PHONY: all clean run generate

all: $(OUT_DIR)/bench_c $(OUT_DIR)/proto_shim $(OUT_DIR)/brief_bridge.so $(OUT_DIR)/bench_add.wasm

$(OUT_DIR):
	mkdir -p $@

# ── Generate: Build .so + all bridge files via metropolitan plugin ─────
generate: $(OUT_DIR)/bench_add.so
	cd $(PROJECT_ROOT) && echo 'import "lib/ffi/metropolitan";' > $(OUT_DIR)/_metro.bv && cat $(BV_SRC) >> $(OUT_DIR)/_metro.bv
	cd $(PROJECT_ROOT) && $(BRIEFC) build $(OUT_DIR)/_metro.bv --llvm --out $(OUT_DIR) --allow-write --allow-run 2>&1
	cd $(PROJECT_ROOT) && $(BRIEFC) build $(BV_SRC) --llvm --out $(OUT_DIR) 2>&1

$(OUT_DIR)/bench_add.so: $(BV_SRC) $(BRIEFC) | $(OUT_DIR)
	cd $(PROJECT_ROOT) && $(BRIEFC) build $(BV_SRC) --shared --out $(OUT_DIR) 2>&1

# ── Tier 1: C (dlopen dlsym) ───────────────────────────────────────────
$(OUT_DIR)/bench_c: bench_c.c $(OUT_DIR)/bench_add.so
	$(CC) -O2 -o $@ bench_c.c -ldl

# ── Tier 2: Python C extension ─────────────────────────────────────────
$(OUT_DIR)/brief_bridge.so: $(OUT_DIR)/bridge.c
	$(CC) -O2 -fPIC -shared -o $@ $(OUT_DIR)/bridge.c -ldl \
	  $(shell python3-config --includes --ldflags --embed 2>/dev/null || python3-config --includes --ldflags 2>/dev/null || echo '-I/usr/include/python3.* -lpython3.*')

# ── Tier 2: Protocol shim ──────────────────────────────────────────────
$(OUT_DIR)/proto_shim: $(OUT_DIR)/proto_shim.c
	$(CC) -O2 -o $@ $(OUT_DIR)/proto_shim.c -ldl

# ── Tier 2: WASM ───────────────────────────────────────────────────────
$(OUT_DIR)/bench_add.wasm: $(OUT_DIR)/bench_add.ll
	$(LLC) -march=wasm32 -filetype=obj $< -o $(OUT_DIR)/bench_add.o
	$(LD) --no-entry --export-all $(OUT_DIR)/bench_add.o -o $@

$(OUT_DIR)/bench_add.ll: $(BV_SRC) $(BRIEFC) | $(OUT_DIR)
	cd $(PROJECT_ROOT) && $(BRIEFC) build $(BV_SRC) --llvm --out $(OUT_DIR) 2>&1

# ── Run all benchmarks ─────────────────────────────────────────────────
run: all
	@echo ""
	$(OUT_DIR)/bench_c $(OUT_DIR)/bench_add.so
	@echo ""
	@echo "--- Python ---"
	cd $(PROJECT_ROOT)/benchmarks/metropolitan && python3 bench_python.py
	@echo ""
	@echo "--- Node.js ---"
	cd $(PROJECT_ROOT)/benchmarks/metropolitan && node bench_node.mjs
	@echo ""
	@echo "--- WASM ---"
	cd $(PROJECT_ROOT)/benchmarks/metropolitan && node bench_wasm.mjs
	@echo ""
	@echo "--- Protocol ---"
	cd $(PROJECT_ROOT)/benchmarks/metropolitan && bash bench_protocol.sh
	@echo ""
	@echo "--- Done ---"

# ── Clean ───────────────────────────────────────────────────────────────
clean:
	rm -rf $(OUT_DIR)
