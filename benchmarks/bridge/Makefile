# Makefile for GLUE vs C FFI bridge benchmarks
# 2026-07-22
#
# Targets:
#   all        — build C + Brief .so files
#   c          — build C reference .so only
#   brief      — build Brief bridge .so only
#   run        — build all and run benchmark
#   clean      — remove build artifacts

PROJECT_ROOT ?= $(abspath $(dir $(firstword $(MAKEFILE_LIST)))/../..)
BRIDGE_DIR ?= $(PROJECT_ROOT)/target/bridge_bench

CC ?= gcc
CFLAGS ?= -shared -fPIC -O2
BRIEFC ?= $(PROJECT_ROOT)/target/debug/brief-compiler

.PHONY: all c brief run clean

all: c brief

$(BRIDGE_DIR):
	mkdir -p $(BRIDGE_DIR)

# ── C reference .so ────────────────────────────────────────────────────

c: $(BRIDGE_DIR) $(BRIDGE_DIR)/libstr_prepend_c.so

$(BRIDGE_DIR)/libstr_prepend_c.so: str_prepend.c
	$(CC) $(CFLAGS) -o $@ $<
	@echo "  C .so: $@"

# ── Brief bridge .so ────────────────────────────────────────────────────
# Uses the full pipeline: brief build --llvm → .ll → llc → .o → cc → .so
# This matches the verified path from tests/pp_roundtrip_tests.rs

brief: $(BRIDGE_DIR)/libpp_types.so

$(BRIDGE_DIR)/libpp_types.so: $(PROJECT_ROOT)/pp-types.bv $(PROJECT_ROOT)/lib/pp/types.bv $(PROJECT_ROOT)/lib/pp/exprs.bv $(BRIEFC)
	@mkdir -p $(BRIDGE_DIR)
	cd $(PROJECT_ROOT) && \
	  $(BRIEFC) build pp-types.bv --llvm --out $(BRIDGE_DIR) 2>&1 && \
	  llc -filetype=obj -relocation-model=pic \
	    -o $(BRIDGE_DIR)/pp-types.o $(BRIDGE_DIR)/pp-types.ll 2>&1 && \
	  $(CC) -c -fPIC -o $(BRIDGE_DIR)/brief_rt.o lib/runtime/brief_rt.c 2>&1 && \
	  $(CC) $(CFLAGS) -o $(BRIDGE_DIR)/libpp_types.so $(BRIDGE_DIR)/pp-types.o $(BRIDGE_DIR)/brief_rt.o && \
	  rm -f $(BRIDGE_DIR)/pp-types.o $(BRIDGE_DIR)/pp-types.ll $(BRIDGE_DIR)/brief_rt.o
	@echo "  Brief .so: $@"
	@ls -la $(BRIDGE_DIR)/libpp_types.so

# ── Run benchmark ──────────────────────────────────────────────────────

run: all
	python3 bench_glue_cross.py

# ── Clean ──────────────────────────────────────────────────────────────

clean:
	rm -rf $(BRIDGE_DIR)
