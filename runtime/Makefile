# brief_rt.o — pre-compiled runtime object for Brief LLVM backend
#
# Usage:
#   make                    # builds for detected host triple
#   make TARGET=aarch64-unknown-linux-gnu  # cross-compile
#   make wake               # build with -lrt -lpthread for timerfd/signalfd
#   make clean              # remove build artifacts
#
# Output: build/$(TARGET).o
# Link:   ld program.o build/$(TARGET).o -o program
#         ld program.o build/$(TARGET).o -o program -lrt -lpthread  # if using wake triggers

TARGET ?= $(shell rustc -vV 2>/dev/null | grep host | cut -d' ' -f2 2>/dev/null || uname -m)-unknown-linux-gnu
CC ?= cc
CFLAGS ?= -O2 -g0 -ffreestanding -fno-stack-protector -fno-builtin

SRC := brief_rt.c
BUILD_DIR := build
OUT := $(BUILD_DIR)/$(TARGET).o

.PHONY: all wake clean

all: $(OUT)

wake: CFLAGS += -lrt -lpthread
wake: $(OUT)

$(OUT): $(SRC) | $(BUILD_DIR)
	$(CC) $(CFLAGS) -c -o $@ $<

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

clean:
	rm -rf $(BUILD_DIR)
