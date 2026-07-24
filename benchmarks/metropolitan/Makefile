# Makefile for Metropolitan FFI Benchmarks
# 2026-07-24: Measure per-call latency for each language generator.

OUT_DIR = $(abspath $(dir $(firstword $(MAKEFILE_LIST)))/out)
BRIEFC ?= $(abspath $(dir $(firstword $(MAKEFILE_LIST)))/../../target/release/brief-compiler)
CC ?= gcc

.PHONY: all build-c build-python build-shim run

all: build-c build-python build-shim

# ── C benchmark (Tier 1) ────────────────────────────────────────────────
build-c: $(OUT_DIR)/bench_c
$(OUT_DIR)/bench_c: bench_c.c $(OUT_DIR)/bench_add.so
	$(CC) -O2 -o $@ bench_c.c -ldl

# ── Python C extension (Tier 2) ─────────────────────────────────────────
build-python: $(OUT_DIR)/brief_bridge.so
$(OUT_DIR)/brief_bridge.so: $(OUT_DIR)/bridge.c $(OUT_DIR)/bench_add.so
	$(CC) -O2 -fPIC -shared -o $@ $(OUT_DIR)/bridge.c -ldl \
	  $(shell python3-config --includes --ldflags --embed 2>/dev/null || python3-config --includes --ldflags 2>/dev/null || echo '-I/usr/include/python3.* -lpython3.*')

# ── Protocol shim (Tier 2) ──────────────────────────────────────────────
build-shim: $(OUT_DIR)/proto_shim
$(OUT_DIR)/proto_shim: $(OUT_DIR)/proto_shim.c $(OUT_DIR)/bench_add.so
	$(CC) -O2 -o $@ $(OUT_DIR)/proto_shim.c -ldl

# ── Run all benchmarks ──────────────────────────────────────────────────
run: all
	@echo ""
	$(OUT_DIR)/bench_c $(OUT_DIR)/bench_add.so
	@echo ""
	@echo "--- Python ---"
	python3 bench_python.py
	@echo ""
	@echo "--- Node.js ---"
	node bench_node.mjs
	@echo ""
	@echo "--- Protocol ---"
	bash bench_protocol.sh
	@echo ""
	@echo "--- Done ---"

# ── Clean ───────────────────────────────────────────────────────────────
clean:
	rm -f $(OUT_DIR)/bench_c $(OUT_DIR)/brief_bridge.so $(OUT_DIR)/proto_shim
