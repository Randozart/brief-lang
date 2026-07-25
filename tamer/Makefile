CC ?= gcc
CFLAGS ?= -Wall -Wextra -Wpedantic -std=c99 -O0 -g
LDFLAGS ?=

all: test_simple test_add test_from_briefc

test_simple: tests/test_simple.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_simple.c interp.c $(LDFLAGS)

test_add: tests/test_add.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_add.c interp.c $(LDFLAGS)

test_from_briefc: tests/test_from_briefc.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_from_briefc.c interp.c $(LDFLAGS)

check: all
	./test_simple
	./test_add
	./test_from_briefc /tmp/test_vm.lair

clean:
	rm -f test_simple test_add test_from_briefc

.PHONY: all check clean
