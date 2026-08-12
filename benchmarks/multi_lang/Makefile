# Makefile for Multi-Language Bridge Benchmarks
# 2026-07-24
#
# Targets:
#   all       — build Briev .so + protocol shim
#   run       — build and run all benchmarks
#   clean     — remove build artifacts

PROJECT_ROOT ?= $(abspath $(dir $(firstword $(MAKEFILE_LIST)))/../..)
BUILD_DIR ?= $(PROJECT_ROOT)/target/multi_lang
BRIEVC ?= $(PROJECT_ROOT)/target/release/briev-compiler
CC ?= gcc

.PHONY: all run clean

all: $(BUILD_DIR)/export_add.so $(BUILD_DIR)/proto_shim

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# ── Briev .so ───────────────────────────────────────────────────────────
# Build the shared library with export defn add/mul

$(BUILD_DIR)/export_add.so: $(PROJECT_ROOT)/benchmarks/multi_lang/export_add.bv $(BRIEVC)
	@mkdir -p $(BUILD_DIR)
	cd $(PROJECT_ROOT) && \
	  $(BRIEVC) build benchmarks/multi_lang/export_add.bv --shared --out $(BUILD_DIR) 2>&1

# ── Protocol shim ───────────────────────────────────────────────────────
# Standalone executable that dlopens the .so and communicates via stdin/stdout

$(BUILD_DIR)/proto_shim: $(PROJECT_ROOT)/benchmarks/multi_lang/proto_shim.c $(BUILD_DIR)/export_add.so
	$(CC) -O2 -o $@ $< -ldl
	@echo "  proto shim: $@"

# ── Run all benchmarks ──────────────────────────────────────────────────

run: all
	@echo ""
	python3 $(PROJECT_ROOT)/benchmarks/multi_lang/bench_multi_lang.py
	@echo ""
	node $(PROJECT_ROOT)/benchmarks/multi_lang/bench_node.mjs
	@echo ""
	bash $(PROJECT_ROOT)/benchmarks/multi_lang/bench_shell.sh

# ── Clean ───────────────────────────────────────────────────────────────

clean:
	rm -rf $(BUILD_DIR)
	rm -rf $(PROJECT_ROOT)/benchmarks/multi_lang/node_modules
	rm -f $(PROJECT_ROOT)/benchmarks/multi_lang/package.json
	rm -f $(PROJECT_ROOT)/benchmarks/multi_lang/package-lock.json
