# Makefile for GLUE vs C FFI bridge benchmarks
# 2026-07-22
#
# Targets:
#   all        — build C + Briv .so files
#   c          — build C reference .so only
#   briv      — build Briv bridge .so only
#   run        — build all and run benchmark
#   clean      — remove build artifacts

PROJECT_ROOT ?= $(abspath $(dir $(firstword $(MAKEFILE_LIST)))/../..)
BRIDGE_DIR ?= $(PROJECT_ROOT)/target/bridge_bench

CC ?= gcc
CFLAGS ?= -shared -fPIC -O2
BRIEFC ?= $(PROJECT_ROOT)/target/debug/briv-compiler

.PHONY: all c briv run clean

all: c briv

$(BRIDGE_DIR):
	mkdir -p $(BRIDGE_DIR)

# ── C reference .so ────────────────────────────────────────────────────

c: $(BRIDGE_DIR) $(BRIDGE_DIR)/libstr_prepend_c.so

$(BRIDGE_DIR)/libstr_prepend_c.so: str_prepend.c
	$(CC) $(CFLAGS) -o $@ $<
	@echo "  C .so: $@"

# ── Briv bridge .so ────────────────────────────────────────────────────
# Uses the full pipeline: briv build --llvm → .ll → llc → .o → cc → .so
# This matches the verified path from tests/pp_roundtrip_tests.rs

briv: $(BRIDGE_DIR)/libpp_types.so

$(BRIDGE_DIR)/libpp_types.so: $(PROJECT_ROOT)/pp-types.bv $(PROJECT_ROOT)/lib/pp/types.bv $(PROJECT_ROOT)/lib/pp/exprs.bv $(BRIEFC)
	@mkdir -p $(BRIDGE_DIR)
	cd $(PROJECT_ROOT) && \
	  $(BRIEFC) build pp-types.bv --llvm --out $(BRIDGE_DIR) 2>&1 && \
	  llc -filetype=obj -relocation-model=pic \
	    -o $(BRIDGE_DIR)/pp-types.o $(BRIDGE_DIR)/pp-types.ll 2>&1 && \
	  $(CC) -c -fPIC -o $(BRIDGE_DIR)/briv_rt.o lib/runtime/briv_rt.c 2>&1 && \
	  $(CC) $(CFLAGS) -o $(BRIDGE_DIR)/libpp_types.so $(BRIDGE_DIR)/pp-types.o $(BRIDGE_DIR)/briv_rt.o && \
	  rm -f $(BRIDGE_DIR)/pp-types.o $(BRIDGE_DIR)/pp-types.ll $(BRIDGE_DIR)/briv_rt.o
	@echo "  Briv .so: $@"
	@ls -la $(BRIDGE_DIR)/libpp_types.so

# ── Run benchmark ──────────────────────────────────────────────────────

run: all
	python3 bench_glue_cross.py

# ── Clean ──────────────────────────────────────────────────────────────

clean:
	rm -rf $(BRIDGE_DIR)

# ── Native-speed benchmark (rank.bv: real compute, Briv vs C) ──────────
# 2026-08-03: build rank.so via the real --library pipeline and the C
# reference, then run bench_glue_speed.py.

speed: $(BRIDGE_DIR) $(BRIDGE_DIR)/librank.so $(BRIDGE_DIR)/librank_ref.so
	python3 bench_glue_speed.py

$(BRIDGE_DIR)/librank.so: $(PROJECT_ROOT)/examples/glue-host/rank.bv $(BRIEFC)
	@mkdir -p $(BRIDGE_DIR)
	cd $(PROJECT_ROOT) && \
	  $(BRIEFC) build examples/glue-host/rank.bv --library --out $(BRIDGE_DIR) && \
	  cp $(BRIDGE_DIR)/rank.so $(BRIDGE_DIR)/librank.so && \
	  rm -f $(BRIDGE_DIR)/rank.ll $(BRIDGE_DIR)/rank.o $(BRIDGE_DIR)/rank.briv_rt.o
	@echo "  Briv .so: $@"

$(BRIDGE_DIR)/librank_ref.so: $(PROJECT_ROOT)/benchmarks/bridge/rank_ref.c
	$(CC) -shared -fPIC -O3 -o $@ $<
	@echo "  C ref .so: $@"
